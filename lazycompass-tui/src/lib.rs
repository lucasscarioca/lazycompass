use anyhow::{Context, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use lazycompass_core::{
    Config, ConnectionSpec, SavedAggregation, SavedQuery, SavedScope, WriteGuard,
    redact_sensitive_text,
};
use lazycompass_mongo::{
    Bson, Document, DocumentDeleteSpec, DocumentInsertSpec, DocumentListSpec, DocumentReplaceSpec,
    MongoExecutor, parse_json_document,
};
use lazycompass_storage::{
    ConfigPaths, StorageSnapshot, append_connection_to_global_config,
    append_connection_to_repo_config, load_storage_with_config, saved_aggregation_path,
    saved_query_path, write_saved_aggregation, write_saved_query,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use std::collections::VecDeque;
use std::fs;
use std::io::{Stdout, Write, stdout};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;

const PAGE_SIZE: u64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyAction {
    Quit,
    MoveDown,
    MoveUp,
    Back,
    Forward,
    GoTop,
    GoBottom,
    NextPage,
    PreviousPage,
    Insert,
    Edit,
    Delete,
    SaveQuery,
    SaveAggregation,
    RunSavedQuery,
    RunSavedAggregation,
    ClearApplied,
    ToggleHelp,
    AddConnection,
}

#[derive(Debug, Clone, Copy)]
struct KeyBinding {
    action: KeyAction,
    code: KeyCode,
    modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy)]
struct HintGroup {
    actions: &'static [KeyAction],
    label: &'static str,
}

const KEY_BINDINGS: &[KeyBinding] = &[
    KeyBinding {
        action: KeyAction::Quit,
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::MoveDown,
        code: KeyCode::Char('j'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::MoveUp,
        code: KeyCode::Char('k'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::Back,
        code: KeyCode::Char('h'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::Forward,
        code: KeyCode::Char('l'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::Forward,
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::GoBottom,
        code: KeyCode::Char('G'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::GoBottom,
        code: KeyCode::Char('G'),
        modifiers: KeyModifiers::SHIFT,
    },
    KeyBinding {
        action: KeyAction::NextPage,
        code: KeyCode::PageDown,
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::PreviousPage,
        code: KeyCode::PageUp,
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::Insert,
        code: KeyCode::Char('i'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::Edit,
        code: KeyCode::Char('e'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::Delete,
        code: KeyCode::Char('d'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::SaveQuery,
        code: KeyCode::Char('Q'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::SaveQuery,
        code: KeyCode::Char('Q'),
        modifiers: KeyModifiers::SHIFT,
    },
    KeyBinding {
        action: KeyAction::SaveAggregation,
        code: KeyCode::Char('A'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::SaveAggregation,
        code: KeyCode::Char('A'),
        modifiers: KeyModifiers::SHIFT,
    },
    KeyBinding {
        action: KeyAction::RunSavedQuery,
        code: KeyCode::Char('r'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::RunSavedAggregation,
        code: KeyCode::Char('a'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::ClearApplied,
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::ToggleHelp,
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::ToggleHelp,
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::SHIFT,
    },
    KeyBinding {
        action: KeyAction::AddConnection,
        code: KeyCode::Char('n'),
        modifiers: KeyModifiers::NONE,
    },
];

const HINT_MOVE: &[KeyAction] = &[KeyAction::MoveDown, KeyAction::MoveUp];
const HINT_SCROLL: &[KeyAction] = &[KeyAction::MoveDown, KeyAction::MoveUp];
const HINT_FORWARD: &[KeyAction] = &[KeyAction::Forward];
const HINT_BACK: &[KeyAction] = &[KeyAction::Back];
const HINT_TOP_BOTTOM: &[KeyAction] = &[KeyAction::GoTop, KeyAction::GoBottom];
const HINT_PAGE: &[KeyAction] = &[KeyAction::PreviousPage, KeyAction::NextPage];
const HINT_EDITING: &[KeyAction] = &[KeyAction::Insert, KeyAction::Edit, KeyAction::Delete];
const HINT_EDIT_DELETE: &[KeyAction] = &[KeyAction::Edit, KeyAction::Delete];
const HINT_SAVE: &[KeyAction] = &[KeyAction::SaveQuery, KeyAction::SaveAggregation];
const HINT_RUN: &[KeyAction] = &[KeyAction::RunSavedQuery, KeyAction::RunSavedAggregation];
const HINT_HELP: &[KeyAction] = &[KeyAction::ToggleHelp];
const HINT_QUIT: &[KeyAction] = &[KeyAction::Quit];

const CONNECTION_HINTS: &[HintGroup] = &[
    HintGroup {
        actions: HINT_MOVE,
        label: "move",
    },
    HintGroup {
        actions: HINT_FORWARD,
        label: "enter",
    },
    HintGroup {
        actions: HINT_TOP_BOTTOM,
        label: "top/bottom",
    },
    HintGroup {
        actions: &[KeyAction::AddConnection],
        label: "new connection",
    },
    HintGroup {
        actions: HINT_HELP,
        label: "help",
    },
    HintGroup {
        actions: HINT_QUIT,
        label: "quit",
    },
];

const DATABASE_HINTS: &[HintGroup] = &[
    HintGroup {
        actions: HINT_MOVE,
        label: "move",
    },
    HintGroup {
        actions: HINT_FORWARD,
        label: "enter",
    },
    HintGroup {
        actions: HINT_BACK,
        label: "back",
    },
    HintGroup {
        actions: HINT_TOP_BOTTOM,
        label: "top/bottom",
    },
    HintGroup {
        actions: HINT_HELP,
        label: "help",
    },
    HintGroup {
        actions: HINT_QUIT,
        label: "quit",
    },
];

const COLLECTION_HINTS: &[HintGroup] = DATABASE_HINTS;

const DOCUMENT_HINTS: &[HintGroup] = &[
    HintGroup {
        actions: HINT_MOVE,
        label: "move",
    },
    HintGroup {
        actions: HINT_FORWARD,
        label: "view",
    },
    HintGroup {
        actions: HINT_BACK,
        label: "back",
    },
    HintGroup {
        actions: HINT_EDITING,
        label: "insert/edit/delete",
    },
    HintGroup {
        actions: HINT_SAVE,
        label: "save query/agg",
    },
    HintGroup {
        actions: HINT_RUN,
        label: "run saved",
    },
    HintGroup {
        actions: HINT_PAGE,
        label: "page",
    },
    HintGroup {
        actions: HINT_TOP_BOTTOM,
        label: "top/bottom",
    },
    HintGroup {
        actions: HINT_HELP,
        label: "help",
    },
    HintGroup {
        actions: HINT_QUIT,
        label: "quit",
    },
];

const SAVED_QUERY_HINTS: &[HintGroup] = &[
    HintGroup {
        actions: HINT_MOVE,
        label: "move",
    },
    HintGroup {
        actions: HINT_FORWARD,
        label: "run",
    },
    HintGroup {
        actions: HINT_BACK,
        label: "cancel",
    },
    HintGroup {
        actions: HINT_TOP_BOTTOM,
        label: "top/bottom",
    },
    HintGroup {
        actions: HINT_HELP,
        label: "help",
    },
    HintGroup {
        actions: HINT_QUIT,
        label: "quit",
    },
];

const SAVED_AGGREGATION_HINTS: &[HintGroup] = SAVED_QUERY_HINTS;

const ADD_CONNECTION_SCOPE_HINTS: &[HintGroup] = &[
    HintGroup {
        actions: HINT_MOVE,
        label: "move",
    },
    HintGroup {
        actions: HINT_FORWARD,
        label: "select",
    },
    HintGroup {
        actions: HINT_BACK,
        label: "cancel",
    },
    HintGroup {
        actions: HINT_TOP_BOTTOM,
        label: "top/bottom",
    },
    HintGroup {
        actions: HINT_HELP,
        label: "help",
    },
    HintGroup {
        actions: HINT_QUIT,
        label: "quit",
    },
];

const SAVE_SCOPE_HINTS: &[HintGroup] = ADD_CONNECTION_SCOPE_HINTS;

const DOCUMENT_VIEW_HINTS: &[HintGroup] = &[
    HintGroup {
        actions: HINT_SCROLL,
        label: "scroll",
    },
    HintGroup {
        actions: HINT_BACK,
        label: "back",
    },
    HintGroup {
        actions: HINT_EDIT_DELETE,
        label: "edit/delete",
    },
    HintGroup {
        actions: HINT_TOP_BOTTOM,
        label: "top/bottom",
    },
    HintGroup {
        actions: HINT_HELP,
        label: "help",
    },
    HintGroup {
        actions: HINT_QUIT,
        label: "quit",
    },
];

#[derive(Debug, Clone, Copy)]
struct Theme {
    text: Color,
    accent: Color,
    border: Color,
    selection_fg: Color,
    selection_bg: Color,
    warning: Color,
    error: Color,
}

impl Theme {
    fn text_style(self) -> Style {
        Style::default().fg(self.text)
    }

    fn title_style(self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    fn border_style(self) -> Style {
        Style::default().fg(self.border)
    }

    fn selection_style(self) -> Style {
        Style::default()
            .fg(self.selection_fg)
            .bg(self.selection_bg)
            .add_modifier(Modifier::BOLD)
    }

    fn warning_style(self) -> Style {
        Style::default().fg(self.warning)
    }

    fn error_style(self) -> Style {
        Style::default().fg(self.error)
    }
}

const THEME_CLASSIC: Theme = Theme {
    text: Color::Gray,
    accent: Color::Cyan,
    border: Color::DarkGray,
    selection_fg: Color::Black,
    selection_bg: Color::Cyan,
    warning: Color::Yellow,
    error: Color::Red,
};

const THEME_EMBER: Theme = Theme {
    text: Color::White,
    accent: Color::LightRed,
    border: Color::Red,
    selection_fg: Color::Black,
    selection_bg: Color::LightRed,
    warning: Color::LightYellow,
    error: Color::LightRed,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Connections,
    Databases,
    Collections,
    Documents,
    DocumentView,
    SavedQuerySelect,
    SavedAggregationSelect,
    SaveQueryScopeSelect,
    SaveAggregationScopeSelect,
    AddConnectionScopeSelect,
}

#[derive(Debug, Clone)]
struct ConfirmState {
    prompt: String,
    action: ConfirmAction,
    input: String,
    required: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct EditorPromptState {
    prompt: String,
    input: String,
    action: PendingEditorAction,
}

#[derive(Debug, Clone)]
enum ConfirmAction {
    DeleteDocument {
        spec: DocumentDeleteSpec,
        return_to_documents: bool,
    },
    OverwriteQuery {
        query: SavedQuery,
    },
    OverwriteAggregation {
        aggregation: SavedAggregation,
    },
}

#[derive(Debug, Clone)]
enum PendingEditorAction {
    Insert {
        connection: String,
        database: String,
        collection: String,
    },
    Edit {
        connection: String,
        database: String,
        collection: String,
        document: Document,
    },
    SaveQuery {
        template: SavedQuery,
    },
    SaveAggregation {
        template: SavedAggregation,
    },
    AddConnection {
        scope: ConnectionPersistenceScope,
        template: ConnectionSpec,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionPersistenceScope {
    SessionOnly,
    Repo,
    Global,
}

#[derive(Debug, Clone)]
enum LoadState {
    Idle,
    Loading,
    Failed(String),
}

#[derive(Debug)]
enum LoadResult {
    Databases {
        id: u64,
        result: Result<Vec<String>, String>,
    },
    Collections {
        id: u64,
        result: Result<Vec<String>, String>,
    },
    Documents {
        id: u64,
        result: Result<Vec<Document>, String>,
    },
    SavedQuery {
        id: u64,
        name: String,
        result: Result<Vec<Document>, String>,
    },
    SavedAggregation {
        id: u64,
        name: String,
        result: Result<Vec<Document>, String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocumentLoadReason {
    EnterCollection,
    NavigateNext,
    NavigatePrevious,
    Refresh,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DocumentResultSource {
    Collection,
    SavedQuery { name: String },
    SavedAggregation { name: String },
}

struct ListView<'a> {
    title: &'a str,
    items: &'a [String],
    selected: Option<usize>,
    load_state: &'a LoadState,
    loading_label: &'a str,
}

struct App {
    paths: ConfigPaths,
    storage: StorageSnapshot,
    executor: MongoExecutor,
    runtime: Runtime,
    theme: Theme,
    read_only: bool,
    screen: Screen,
    connection_index: Option<usize>,
    database_items: Vec<String>,
    database_index: Option<usize>,
    collection_items: Vec<String>,
    collection_index: Option<usize>,
    documents: Vec<Document>,
    document_index: Option<usize>,
    document_page: u64,
    document_lines: Vec<String>,
    document_scroll: u16,
    last_g: bool,
    help_visible: bool,
    message: Option<String>,
    confirm: Option<ConfirmState>,
    editor_prompt: Option<EditorPromptState>,
    editor_command: Option<String>,
    warnings: VecDeque<String>,
    load_tx: Sender<LoadResult>,
    load_rx: Receiver<LoadResult>,
    next_load_id: u64,
    database_load_id: Option<u64>,
    collection_load_id: Option<u64>,
    document_load_id: Option<u64>,
    saved_query_load_id: Option<u64>,
    saved_agg_load_id: Option<u64>,
    database_state: LoadState,
    collection_state: LoadState,
    document_state: LoadState,
    saved_query_state: LoadState,
    saved_agg_state: LoadState,
    document_pending_index: Option<usize>,
    document_load_reason: DocumentLoadReason,
    document_result_source: DocumentResultSource,
    saved_query_index: Option<usize>,
    saved_agg_index: Option<usize>,
    save_query_scope_index: Option<usize>,
    save_agg_scope_index: Option<usize>,
    add_connection_scope_index: Option<usize>,
}

impl App {
    fn new(paths: ConfigPaths, storage: StorageSnapshot, read_only: bool) -> Result<Self> {
        let runtime = Runtime::new().context("unable to start async runtime")?;
        let (load_tx, load_rx) = mpsc::channel();
        let (theme, theme_warning) = resolve_theme(&storage.config);
        let mut warnings = VecDeque::from(storage.warnings.clone());
        if let Some(warning) = theme_warning {
            warnings.push_back(warning);
        }
        let connection_index = if storage.config.connections.is_empty() {
            None
        } else {
            Some(0)
        };
        let message = if connection_index.is_none() {
            Some("no connections configured".to_string())
        } else {
            None
        };

        Ok(Self {
            paths,
            storage,
            executor: MongoExecutor::new(),
            runtime,
            theme,
            read_only,
            screen: Screen::Connections,
            connection_index,
            database_items: Vec::new(),
            database_index: None,
            collection_items: Vec::new(),
            collection_index: None,
            documents: Vec::new(),
            document_index: None,
            document_page: 0,
            document_lines: Vec::new(),
            document_scroll: 0,
            last_g: false,
            help_visible: false,
            message,
            confirm: None,
            editor_prompt: None,
            editor_command: None,
            warnings,
            load_tx,
            load_rx,
            next_load_id: 0,
            database_load_id: None,
            collection_load_id: None,
            document_load_id: None,
            saved_query_load_id: None,
            saved_agg_load_id: None,
            database_state: LoadState::Idle,
            collection_state: LoadState::Idle,
            document_state: LoadState::Idle,
            saved_query_state: LoadState::Idle,
            saved_agg_state: LoadState::Idle,
            document_pending_index: None,
            document_load_reason: DocumentLoadReason::Refresh,
            document_result_source: DocumentResultSource::Collection,
            saved_query_index: None,
            saved_agg_index: None,
            save_query_scope_index: Some(0),
            save_agg_scope_index: Some(0),
            add_connection_scope_index: Some(0),
        })
    }

    fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            self.drain_load_results();
            terminal.draw(|frame| self.draw(frame))?;
            if event::poll(Duration::from_millis(200))? {
                match event::read()? {
                    Event::Key(key) => {
                        if self.handle_key(key, terminal)? {
                            return Ok(());
                        }
                    }
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }
        }
    }

    fn drain_load_results(&mut self) {
        loop {
            match self.load_rx.try_recv() {
                Ok(result) => self.apply_load_result(result),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    fn apply_load_result(&mut self, result: LoadResult) {
        match result {
            LoadResult::Databases { id, result } => {
                if self.database_load_id != Some(id) {
                    return;
                }
                self.database_load_id = None;
                match result {
                    Ok(mut databases) => {
                        databases.sort();
                        self.database_items = databases;
                        self.database_index = if self.database_items.is_empty() {
                            None
                        } else {
                            Some(0)
                        };
                        self.database_state = LoadState::Idle;
                    }
                    Err(error) => {
                        let message =
                            format_error_message(&error, is_network_error_message(&error));
                        self.database_state = LoadState::Failed(message.clone());
                        self.message = Some(message);
                    }
                }
            }
            LoadResult::Collections { id, result } => {
                if self.collection_load_id != Some(id) {
                    return;
                }
                self.collection_load_id = None;
                match result {
                    Ok(mut collections) => {
                        collections.sort();
                        self.collection_items = collections;
                        self.collection_index = if self.collection_items.is_empty() {
                            None
                        } else {
                            Some(0)
                        };
                        self.collection_state = LoadState::Idle;
                    }
                    Err(error) => {
                        let message =
                            format_error_message(&error, is_network_error_message(&error));
                        self.collection_state = LoadState::Failed(message.clone());
                        self.message = Some(message);
                    }
                }
            }
            LoadResult::Documents { id, result } => {
                if self.document_load_id != Some(id) {
                    return;
                }
                self.document_load_id = None;
                match result {
                    Ok(documents) => {
                        self.documents = documents;
                        if self.documents.is_empty() && self.document_page > 0 {
                            if self.document_load_reason == DocumentLoadReason::NavigateNext {
                                self.message = Some("no more documents".to_string());
                            }
                            let pending_index = self.document_pending_index.take();
                            self.document_page -= 1;
                            let _ = self
                                .start_load_documents(pending_index, DocumentLoadReason::Refresh);
                            return;
                        }
                        self.document_state = LoadState::Idle;
                        if let Some(index) = self.document_pending_index.take() {
                            Self::select_index(
                                &mut self.document_index,
                                self.documents.len(),
                                index,
                            );
                        } else if self.documents.is_empty() {
                            self.document_index = None;
                        } else {
                            self.document_index = Some(0);
                        }
                        if self.documents.is_empty() {
                            self.document_lines.clear();
                            self.document_scroll = 0;
                        }
                        if self.screen == Screen::DocumentView {
                            self.prepare_document_view();
                        }
                    }
                    Err(error) => {
                        let message =
                            format_error_message(&error, is_network_error_message(&error));
                        self.document_state = LoadState::Failed(message.clone());
                        self.message = Some(message);
                    }
                }
            }
            LoadResult::SavedQuery { id, name, result } => {
                if self.saved_query_load_id != Some(id) {
                    return;
                }
                self.saved_query_load_id = None;
                self.saved_query_state = LoadState::Idle;
                match result {
                    Ok(documents) => {
                        self.documents = documents;
                        self.document_index = if self.documents.is_empty() {
                            None
                        } else {
                            Some(0)
                        };
                        self.document_page = 0;
                        self.document_lines.clear();
                        self.document_scroll = 0;
                        self.document_result_source = DocumentResultSource::SavedQuery { name };
                        self.screen = Screen::Documents;
                        self.message = Some(format!(
                            "query returned {} document(s)",
                            self.documents.len()
                        ));
                    }
                    Err(error) => {
                        let message =
                            format_error_message(&error, is_network_error_message(&error));
                        self.saved_query_state = LoadState::Failed(message.clone());
                        self.message = Some(message);
                    }
                }
            }
            LoadResult::SavedAggregation { id, name, result } => {
                if self.saved_agg_load_id != Some(id) {
                    return;
                }
                self.saved_agg_load_id = None;
                self.saved_agg_state = LoadState::Idle;
                match result {
                    Ok(documents) => {
                        self.documents = documents;
                        self.document_index = if self.documents.is_empty() {
                            None
                        } else {
                            Some(0)
                        };
                        self.document_page = 0;
                        self.document_lines.clear();
                        self.document_scroll = 0;
                        self.document_result_source =
                            DocumentResultSource::SavedAggregation { name };
                        self.screen = Screen::Documents;
                        self.message = Some(format!(
                            "aggregation returned {} document(s)",
                            self.documents.len()
                        ));
                    }
                    Err(error) => {
                        let message =
                            format_error_message(&error, is_network_error_message(&error));
                        self.saved_agg_state = LoadState::Failed(message.clone());
                        self.message = Some(message);
                    }
                }
            }
        }
    }

    fn handle_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<bool> {
        if self.editor_prompt.is_some() {
            return self.handle_editor_prompt_key(key, terminal);
        }
        if self.confirm.is_some() {
            return self.handle_confirm_key(key, terminal);
        }

        // Clear warnings and messages on any non-confirm keypress
        if !self.warnings.is_empty() {
            self.warnings.pop_front();
        }
        self.message = None;

        if self.help_visible {
            if key.code == KeyCode::Esc {
                self.help_visible = false;
                self.last_g = false;
                return Ok(false);
            }
            if let Some(action) = action_for_key(key) {
                match action {
                    KeyAction::ToggleHelp => {
                        self.help_visible = false;
                    }
                    KeyAction::Quit => return Ok(true),
                    _ => {}
                }
            }
            self.last_g = false;
            return Ok(false);
        }

        if let Some(action) = self.resolve_action(key) {
            return self.apply_action(action, terminal);
        }

        Ok(false)
    }

    fn resolve_action(&mut self, key: KeyEvent) -> Option<KeyAction> {
        if key.code == KeyCode::Char('g') && key.modifiers == KeyModifiers::NONE {
            if self.last_g {
                self.last_g = false;
                return Some(KeyAction::GoTop);
            }
            self.last_g = true;
            return None;
        }

        self.last_g = false;
        action_for_key(key)
    }

    fn apply_action(
        &mut self,
        action: KeyAction,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<bool> {
        match action {
            KeyAction::Quit => return Ok(true),
            KeyAction::MoveDown => self.move_down(),
            KeyAction::MoveUp => self.move_up(),
            KeyAction::Back => self.go_back(),
            KeyAction::Forward => self.go_forward(terminal)?,
            KeyAction::GoTop => self.go_top(),
            KeyAction::GoBottom => self.go_bottom(),
            KeyAction::NextPage => self.next_page()?,
            KeyAction::PreviousPage => self.previous_page()?,
            KeyAction::Insert => self.insert_document(terminal)?,
            KeyAction::Edit => self.edit_document(terminal)?,
            KeyAction::Delete => self.request_delete_document()?,
            KeyAction::SaveQuery => self.save_query(terminal)?,
            KeyAction::SaveAggregation => self.save_aggregation(terminal)?,
            KeyAction::RunSavedQuery => self.run_saved_query()?,
            KeyAction::RunSavedAggregation => self.run_saved_aggregation()?,
            KeyAction::ClearApplied => self.clear_applied_documents()?,
            KeyAction::ToggleHelp => self.help_visible = !self.help_visible,
            KeyAction::AddConnection => self.start_add_connection()?,
        }

        Ok(false)
    }

    fn handle_confirm_key(
        &mut self,
        key: KeyEvent,
        _terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<bool> {
        let Some(mut confirm) = self.confirm.take() else {
            return Ok(false);
        };

        if let Some(required) = confirm.required {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.message = Some("cancelled".to_string());
                }
                KeyCode::Backspace => {
                    confirm.input.pop();
                    self.confirm = Some(confirm);
                }
                KeyCode::Enter => {
                    if confirm.input.trim().eq_ignore_ascii_case(required) {
                        if let Err(error) = self.perform_confirm_action(confirm.action) {
                            self.set_error_message(&error);
                        }
                    } else {
                        self.message = Some(format!("must type '{}' to confirm", required));
                        self.confirm = Some(confirm);
                    }
                }
                KeyCode::Char(ch) => {
                    if !ch.is_control() {
                        confirm.input.push(ch);
                    }
                    self.confirm = Some(confirm);
                }
                _ => {
                    self.confirm = Some(confirm);
                }
            }

            self.last_g = false;
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Err(error) = self.perform_confirm_action(confirm.action) {
                    self.set_error_message(&error);
                }
            }
            KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc => {
                self.message = Some("cancelled".to_string());
            }
            _ => {
                self.confirm = Some(confirm);
            }
        }

        self.last_g = false;
        Ok(false)
    }

    fn handle_editor_prompt_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<bool> {
        let Some(mut prompt) = self.editor_prompt.take() else {
            return Ok(false);
        };

        match key.code {
            KeyCode::Esc => {
                self.message = Some("cancelled".to_string());
            }
            KeyCode::Backspace => {
                prompt.input.pop();
                self.editor_prompt = Some(prompt);
            }
            KeyCode::Enter => {
                let editor = prompt.input.trim();
                if editor.is_empty() {
                    self.editor_prompt = Some(prompt);
                } else {
                    self.editor_command = Some(editor.to_string());
                    if let Err(error) = self.perform_editor_action(prompt.action, terminal) {
                        self.set_error_message(&error);
                    }
                }
            }
            KeyCode::Char(ch) => {
                if !ch.is_control() {
                    prompt.input.push(ch);
                }
                self.editor_prompt = Some(prompt);
            }
            _ => {
                self.editor_prompt = Some(prompt);
            }
        }

        self.last_g = false;
        Ok(false)
    }

    fn perform_confirm_action(&mut self, action: ConfirmAction) -> Result<()> {
        let guard = WriteGuard::new(self.read_only, self.storage.config.allow_pipeline_writes());
        match action {
            ConfirmAction::DeleteDocument {
                spec,
                return_to_documents,
            } => {
                if let Err(error) = guard.ensure_write_allowed("delete documents") {
                    self.message = Some(error.to_string());
                    return Ok(());
                }
                self.runtime
                    .block_on(self.executor.delete_document(&self.storage.config, &spec))?;
                if return_to_documents {
                    self.screen = Screen::Documents;
                }
                self.reload_documents_after_change()?;
                self.message = Some("document deleted".to_string());
            }
            ConfirmAction::OverwriteQuery { query } => {
                if let Err(error) = guard.ensure_write_allowed("save queries") {
                    self.message = Some(error.to_string());
                    return Ok(());
                }
                let path = write_saved_query(&self.paths, &query, true)?;
                self.upsert_query(query);
                self.message = Some(format!("saved query to {}", path.display()));
            }
            ConfirmAction::OverwriteAggregation { aggregation } => {
                if let Err(error) = guard.ensure_write_allowed("save aggregations") {
                    self.message = Some(error.to_string());
                    return Ok(());
                }
                let path = write_saved_aggregation(&self.paths, &aggregation, true)?;
                self.upsert_aggregation(aggregation);
                self.message = Some(format!("saved aggregation to {}", path.display()));
            }
        }
        Ok(())
    }

    fn set_error_message(&mut self, error: &anyhow::Error) {
        let message = format_error_message(&error.to_string(), is_network_error(error));
        self.message = Some(message);
    }

    fn block_if_read_only(&mut self, action: &str) -> bool {
        let guard = WriteGuard::new(self.read_only, self.storage.config.allow_pipeline_writes());
        if let Err(error) = guard.ensure_write_allowed(action) {
            self.message = Some(error.to_string());
            return true;
        }
        false
    }

    fn request_delete_document(&mut self) -> Result<()> {
        if !matches!(self.screen, Screen::Documents | Screen::DocumentView) {
            return Ok(());
        }
        if self.block_if_read_only("delete documents") {
            return Ok(());
        }
        let result = (|| -> Result<()> {
            let (connection, database, collection) = self.selected_context()?;
            let document = self.selected_document()?;
            let id = document_id(document)?;
            let prompt = format!(
                "delete document {} (Conn: {connection}, Db: {database}, Coll: {collection})",
                format_bson(&id)
            );
            let spec = DocumentDeleteSpec {
                connection: Some(connection),
                database,
                collection,
                id,
            };
            self.confirm = Some(ConfirmState {
                prompt,
                action: ConfirmAction::DeleteDocument {
                    spec,
                    return_to_documents: self.screen == Screen::DocumentView,
                },
                input: String::new(),
                required: Some("delete"),
            });
            Ok(())
        })();

        if let Err(error) = result {
            self.set_error_message(&error);
        }
        Ok(())
    }

    fn insert_document(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        if self.screen != Screen::Documents {
            return Ok(());
        }
        if self.block_if_read_only("insert documents") {
            return Ok(());
        }
        let result = (|| -> Result<()> {
            let (connection, database, collection) = self.selected_context()?;
            let action = PendingEditorAction::Insert {
                connection,
                database,
                collection,
            };
            let Some(_) = self.ensure_editor_command(action.clone())? else {
                return Ok(());
            };
            self.perform_editor_action(action, terminal)
        })();

        if let Err(error) = result {
            self.set_error_message(&error);
        }
        Ok(())
    }

    fn edit_document(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        if !matches!(self.screen, Screen::Documents | Screen::DocumentView) {
            return Ok(());
        }
        if self.block_if_read_only("edit documents") {
            return Ok(());
        }
        let result = (|| -> Result<()> {
            let (connection, database, collection) = self.selected_context()?;
            let document = self.selected_document()?.clone();
            let action = PendingEditorAction::Edit {
                connection,
                database,
                collection,
                document,
            };
            let Some(_) = self.ensure_editor_command(action.clone())? else {
                return Ok(());
            };
            self.perform_editor_action(action, terminal)
        })();

        if let Err(error) = result {
            self.set_error_message(&error);
        }
        Ok(())
    }

    fn save_query(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        let _ = terminal;
        if self.screen != Screen::Documents {
            return Ok(());
        }
        if self.block_if_read_only("save queries") {
            return Ok(());
        }
        self.save_query_scope_index = Some(0);
        self.screen = Screen::SaveQueryScopeSelect;
        self.message = Some("select save mode for query".to_string());
        Ok(())
    }

    fn select_query_save_scope(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
        let scope = match self.save_query_scope_index {
            Some(0) => SavedScope::Shared,
            Some(1) => {
                let (_, database, collection) = self.selected_context()?;
                SavedScope::Scoped {
                    database,
                    collection,
                }
            }
            _ => return Ok(()),
        };
        let template = SavedQuery {
            id: default_saved_id("query", &scope),
            scope,
            filter: None,
            projection: None,
            sort: None,
            limit: None,
        };
        self.screen = Screen::Documents;
        let action = PendingEditorAction::SaveQuery { template };
        let Some(_) = self.ensure_editor_command(action.clone())? else {
            return Ok(());
        };
        self.perform_editor_action(action, terminal)?;
        Ok(())
    }

    fn save_aggregation(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
        let _ = terminal;
        if self.screen != Screen::Documents {
            return Ok(());
        }
        if self.block_if_read_only("save aggregations") {
            return Ok(());
        }
        self.save_agg_scope_index = Some(0);
        self.screen = Screen::SaveAggregationScopeSelect;
        self.message = Some("select save mode for aggregation".to_string());
        Ok(())
    }

    fn select_aggregation_save_scope(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
        let scope = match self.save_agg_scope_index {
            Some(0) => SavedScope::Shared,
            Some(1) => {
                let (_, database, collection) = self.selected_context()?;
                SavedScope::Scoped {
                    database,
                    collection,
                }
            }
            _ => return Ok(()),
        };
        let template = SavedAggregation {
            id: default_saved_id("aggregation", &scope),
            scope,
            pipeline: "[]".to_string(),
        };
        self.screen = Screen::Documents;
        let action = PendingEditorAction::SaveAggregation { template };
        let Some(_) = self.ensure_editor_command(action.clone())? else {
            return Ok(());
        };
        self.perform_editor_action(action, terminal)?;
        Ok(())
    }

    fn run_saved_query(&mut self) -> Result<()> {
        if self.screen != Screen::Documents {
            return Ok(());
        }
        if self.storage.queries.is_empty() {
            self.message = Some("no saved queries".to_string());
            return Ok(());
        }
        self.saved_query_index = if self.storage.queries.is_empty() {
            None
        } else {
            Some(0)
        };
        self.saved_query_state = LoadState::Idle;
        self.screen = Screen::SavedQuerySelect;
        Ok(())
    }

    fn run_saved_aggregation(&mut self) -> Result<()> {
        if self.screen != Screen::Documents {
            return Ok(());
        }
        if self.storage.aggregations.is_empty() {
            self.message = Some("no saved aggregations".to_string());
            return Ok(());
        }
        self.saved_agg_index = if self.storage.aggregations.is_empty() {
            None
        } else {
            Some(0)
        };
        self.saved_agg_state = LoadState::Idle;
        self.screen = Screen::SavedAggregationSelect;
        Ok(())
    }

    fn start_add_connection(&mut self) -> Result<()> {
        if self.screen != Screen::Connections {
            return Ok(());
        }
        self.add_connection_scope_index = Some(0);
        self.screen = Screen::AddConnectionScopeSelect;
        Ok(())
    }

    fn select_connection_scope(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
        let scope = match self.add_connection_scope_index {
            Some(0) => ConnectionPersistenceScope::SessionOnly,
            Some(1) => ConnectionPersistenceScope::Repo,
            Some(2) => ConnectionPersistenceScope::Global,
            _ => return Ok(()),
        };

        let template = ConnectionSpec {
            name: "new_connection".to_string(),
            uri: "mongodb://localhost:27017".to_string(),
            default_database: Some("test".to_string()),
        };

        let action = PendingEditorAction::AddConnection { scope, template };
        let Some(_) = self.ensure_editor_command(action.clone())? else {
            return Ok(());
        };
        self.perform_editor_action(action, terminal)
    }

    fn perform_add_connection_editor_action(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        scope: ConnectionPersistenceScope,
        template: ConnectionSpec,
    ) -> Result<()> {
        let editor = self
            .editor_command
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("editor command missing"))?;
        let initial =
            toml::to_string_pretty(&template).context("unable to render connection template")?;
        let contents = self.open_editor(terminal, editor, "connection", &initial)?;
        if is_editor_cancelled(&contents, &initial) {
            self.message = Some("cancelled".to_string());
            self.screen = Screen::Connections;
            return Ok(());
        }
        let connection: ConnectionSpec =
            toml::from_str(&contents).context("invalid TOML for connection")?;

        // Validate the connection
        if connection.name.trim().is_empty() {
            anyhow::bail!("connection name cannot be empty");
        }
        if connection.uri.trim().is_empty() {
            anyhow::bail!("connection uri cannot be empty");
        }

        // Check for duplicate names
        if self
            .storage
            .config
            .connections
            .iter()
            .any(|c| c.name == connection.name)
        {
            anyhow::bail!("connection with name '{}' already exists", connection.name);
        }

        match scope {
            ConnectionPersistenceScope::SessionOnly => {
                self.storage.config.connections.push(connection.clone());
                // Update connection_index to point to the new connection
                self.connection_index = Some(self.storage.config.connections.len() - 1);
                self.message = Some(format!(
                    "added connection '{}' (session only)",
                    connection.name
                ));
                self.screen = Screen::Connections;
            }
            ConnectionPersistenceScope::Repo => {
                if self.paths.repo_config_root().is_none() {
                    anyhow::bail!("no repo config found; run inside a repo with .lazycompass");
                }
                let paths = self.paths.clone();
                let new_connection = connection.clone();

                // Write to repo config
                let result = self
                    .runtime
                    .block_on(append_connection_to_repo_config(&paths, &new_connection));
                if let Err(e) = result {
                    self.set_error_message(&e);
                    self.screen = Screen::Connections;
                    return Ok(());
                }

                // Update in-memory config
                self.storage.config.connections.push(connection.clone());
                self.connection_index = Some(self.storage.config.connections.len() - 1);
                self.message = Some(format!(
                    "added connection '{}' to repo config",
                    connection.name
                ));
                self.screen = Screen::Connections;
            }
            ConnectionPersistenceScope::Global => {
                let paths = self.paths.clone();
                let new_connection = connection.clone();

                // Write to global config
                let result = self
                    .runtime
                    .block_on(append_connection_to_global_config(&paths, &new_connection));
                if let Err(e) = result {
                    self.set_error_message(&e);
                    self.screen = Screen::Connections;
                    return Ok(());
                }

                // Update in-memory config
                self.storage.config.connections.push(connection.clone());
                self.connection_index = Some(self.storage.config.connections.len() - 1);
                self.message = Some(format!(
                    "added connection '{}' to global config",
                    connection.name
                ));
                self.screen = Screen::Connections;
            }
        }

        Ok(())
    }

    fn ensure_editor_command(&mut self, action: PendingEditorAction) -> Result<Option<String>> {
        if let Some(editor) = &self.editor_command {
            return Ok(Some(editor.clone()));
        }

        match resolve_editor() {
            Ok(editor) => {
                self.editor_command = Some(editor.clone());
                Ok(Some(editor))
            }
            Err(_) => {
                self.editor_prompt = Some(EditorPromptState {
                    prompt: "editor command required (set $VISUAL/$EDITOR or enter here)"
                        .to_string(),
                    input: String::new(),
                    action,
                });
                Ok(None)
            }
        }
    }

    fn perform_editor_action(
        &mut self,
        action: PendingEditorAction,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
        match action {
            PendingEditorAction::Insert {
                connection,
                database,
                collection,
            } => self.insert_document_with_context(terminal, connection, database, collection),
            PendingEditorAction::Edit {
                connection,
                database,
                collection,
                document,
            } => self
                .edit_document_with_context(terminal, connection, database, collection, document),
            PendingEditorAction::SaveQuery { template } => {
                self.save_query_with_template(terminal, template)
            }
            PendingEditorAction::SaveAggregation { template } => {
                self.save_aggregation_with_template(terminal, template)
            }
            PendingEditorAction::AddConnection { scope, template } => {
                self.perform_add_connection_editor_action(terminal, scope, template)
            }
        }
    }

    fn insert_document_with_context(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        connection: String,
        database: String,
        collection: String,
    ) -> Result<()> {
        let editor = self
            .editor_command
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("editor command missing"))?;
        let initial = "{}";
        let contents = self.open_editor(terminal, editor, "insert", initial)?;
        if is_editor_cancelled(&contents, initial) {
            self.message = Some("cancelled".to_string());
            return Ok(());
        }
        let document = parse_json_document("document", &contents)?;
        let spec = DocumentInsertSpec {
            connection: Some(connection),
            database,
            collection,
            document,
        };
        let inserted_id = self
            .runtime
            .block_on(self.executor.insert_document(&self.storage.config, &spec))?;
        self.reload_documents_after_change()?;
        self.message = Some(format!("inserted document {}", format_bson(&inserted_id)));
        Ok(())
    }

    fn edit_document_with_context(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        connection: String,
        database: String,
        collection: String,
        document: Document,
    ) -> Result<()> {
        let editor = self
            .editor_command
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("editor command missing"))?;
        let original_id = document_id(&document)?;
        let initial =
            serde_json::to_string_pretty(&document).context("unable to serialize document")?;
        let contents = self.open_editor(terminal, editor, "edit", &initial)?;
        if is_editor_cancelled(&contents, &initial) {
            self.message = Some("cancelled".to_string());
            return Ok(());
        }
        let mut updated = parse_json_document("document", &contents)?;
        let mut id_changed = false;
        match updated.get("_id") {
            Some(value) if value == &original_id => {}
            _ => {
                updated.insert("_id", original_id.clone());
                id_changed = true;
            }
        }
        let spec = DocumentReplaceSpec {
            connection: Some(connection),
            database,
            collection,
            id: original_id,
            document: updated,
        };
        self.runtime
            .block_on(self.executor.replace_document(&self.storage.config, &spec))?;
        self.reload_documents_after_change()?;
        self.message = Some(if id_changed {
            "updated document (kept original _id)".to_string()
        } else {
            "updated document".to_string()
        });
        Ok(())
    }

    fn save_query_with_template(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        template: SavedQuery,
    ) -> Result<()> {
        let editor = self
            .editor_command
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("editor command missing"))?;
        let initial =
            render_query_payload_template(&template).context("unable to render query template")?;
        let contents = self.open_editor(terminal, editor, "query", &initial)?;
        if is_editor_cancelled(&contents, &initial) {
            self.message = Some("cancelled".to_string());
            return Ok(());
        }
        let query = parse_query_payload_input(&contents, &template)?;
        query.validate().context("invalid saved query")?;
        let path = saved_query_path(&self.paths, &query.id)?;
        if path.exists() {
            self.confirm = Some(ConfirmState {
                prompt: format!("overwrite saved query '{}'? (y/n)", query.id),
                action: ConfirmAction::OverwriteQuery { query },
                input: String::new(),
                required: None,
            });
            return Ok(());
        }
        let path = write_saved_query(&self.paths, &query, false)?;
        self.upsert_query(query);
        self.message = Some(format!("saved query to {}", path.display()));
        Ok(())
    }

    fn save_aggregation_with_template(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        template: SavedAggregation,
    ) -> Result<()> {
        let editor = self
            .editor_command
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("editor command missing"))?;
        let initial = render_aggregation_payload_template(&template)
            .context("unable to render aggregation template")?;
        let contents = self.open_editor(terminal, editor, "aggregation", &initial)?;
        if is_editor_cancelled(&contents, &initial) {
            self.message = Some("cancelled".to_string());
            return Ok(());
        }
        let aggregation = parse_aggregation_payload_input(&contents, &template)?;
        aggregation
            .validate()
            .context("invalid saved aggregation")?;
        let path = saved_aggregation_path(&self.paths, &aggregation.id)?;
        if path.exists() {
            self.confirm = Some(ConfirmState {
                prompt: format!("overwrite saved aggregation '{}'? (y/n)", aggregation.id),
                action: ConfirmAction::OverwriteAggregation { aggregation },
                input: String::new(),
                required: None,
            });
            return Ok(());
        }
        let path = write_saved_aggregation(&self.paths, &aggregation, false)?;
        self.upsert_aggregation(aggregation);
        self.message = Some(format!("saved aggregation to {}", path.display()));
        Ok(())
    }

    fn open_editor(
        &self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        editor: &str,
        label: &str,
        initial: &str,
    ) -> Result<String> {
        let path = editor_temp_path(label);
        write_editor_temp_file(&path, initial)
            .with_context(|| format!("unable to write temporary file {}", path.display()))?;

        suspend_terminal(terminal)?;
        let status = run_editor_command(editor, &path);
        let resume = resume_terminal(terminal);
        let status = match status {
            Ok(status) => status,
            Err(error) => {
                let _ = resume;
                let _ = fs::remove_file(&path);
                return Err(error);
            }
        };
        resume?;
        if !status.success() {
            let _ = fs::remove_file(&path);
            anyhow::bail!("editor exited with non-zero status");
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("unable to read temporary file {}", path.display()))?;
        let _ = fs::remove_file(&path);
        Ok(contents)
    }

    fn reload_documents_after_change(&mut self) -> Result<()> {
        if !matches!(self.screen, Screen::Documents | Screen::DocumentView) {
            return Ok(());
        }
        let selected_index = self.document_index;
        self.start_load_documents(selected_index, DocumentLoadReason::Refresh)?;
        Ok(())
    }

    fn selected_context(&self) -> Result<(String, String, String)> {
        let connection = self
            .selected_connection()
            .ok_or_else(|| anyhow::anyhow!("select a connection"))?;
        let database = self
            .selected_database()
            .ok_or_else(|| anyhow::anyhow!("select a database"))?;
        let collection = self
            .selected_collection()
            .ok_or_else(|| anyhow::anyhow!("select a collection"))?;
        Ok((
            connection.name.clone(),
            database.to_string(),
            collection.to_string(),
        ))
    }

    fn selected_document(&self) -> Result<&Document> {
        let index = self
            .document_index
            .ok_or_else(|| anyhow::anyhow!("select a document"))?;
        self.documents
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("select a document"))
    }

    fn upsert_query(&mut self, query: SavedQuery) {
        if let Some(existing) = self
            .storage
            .queries
            .iter_mut()
            .find(|saved| saved.id == query.id)
        {
            *existing = query;
        } else {
            self.storage.queries.push(query);
        }
    }

    fn upsert_aggregation(&mut self, aggregation: SavedAggregation) {
        if let Some(existing) = self
            .storage
            .aggregations
            .iter_mut()
            .find(|saved| saved.id == aggregation.id)
        {
            *existing = aggregation;
        } else {
            self.storage.aggregations.push(aggregation);
        }
    }

    fn go_top(&mut self) {
        match self.screen {
            Screen::Connections => Self::select_index(
                &mut self.connection_index,
                self.storage.config.connections.len(),
                0,
            ),
            Screen::Databases => {
                Self::select_index(&mut self.database_index, self.database_items.len(), 0)
            }
            Screen::Collections => {
                Self::select_index(&mut self.collection_index, self.collection_items.len(), 0)
            }
            Screen::Documents => {
                Self::select_index(&mut self.document_index, self.documents.len(), 0)
            }
            Screen::DocumentView => self.document_scroll = 0,
            Screen::SavedQuerySelect => {
                Self::select_index(&mut self.saved_query_index, self.storage.queries.len(), 0)
            }
            Screen::SavedAggregationSelect => Self::select_index(
                &mut self.saved_agg_index,
                self.storage.aggregations.len(),
                0,
            ),
            Screen::SaveQueryScopeSelect => {
                Self::select_index(&mut self.save_query_scope_index, 2, 0)
            }
            Screen::SaveAggregationScopeSelect => {
                Self::select_index(&mut self.save_agg_scope_index, 2, 0)
            }
            Screen::AddConnectionScopeSelect => {
                Self::select_index(&mut self.add_connection_scope_index, 3, 0)
            }
        }
    }

    fn go_bottom(&mut self) {
        match self.screen {
            Screen::Connections => Self::select_last(
                &mut self.connection_index,
                self.storage.config.connections.len(),
            ),
            Screen::Databases => {
                Self::select_last(&mut self.database_index, self.database_items.len())
            }
            Screen::Collections => {
                Self::select_last(&mut self.collection_index, self.collection_items.len())
            }
            Screen::Documents => Self::select_last(&mut self.document_index, self.documents.len()),
            Screen::DocumentView => self.document_scroll = self.max_document_scroll(),
            Screen::SavedQuerySelect => {
                Self::select_last(&mut self.saved_query_index, self.storage.queries.len())
            }
            Screen::SavedAggregationSelect => {
                Self::select_last(&mut self.saved_agg_index, self.storage.aggregations.len())
            }
            Screen::SaveQueryScopeSelect => Self::select_last(&mut self.save_query_scope_index, 2),
            Screen::SaveAggregationScopeSelect => {
                Self::select_last(&mut self.save_agg_scope_index, 2)
            }
            Screen::AddConnectionScopeSelect => {
                Self::select_last(&mut self.add_connection_scope_index, 3)
            }
        }
    }

    fn move_up(&mut self) {
        match self.screen {
            Screen::Connections => Self::move_selection(
                &mut self.connection_index,
                self.storage.config.connections.len(),
                -1,
            ),
            Screen::Databases => {
                Self::move_selection(&mut self.database_index, self.database_items.len(), -1)
            }
            Screen::Collections => {
                Self::move_selection(&mut self.collection_index, self.collection_items.len(), -1)
            }
            Screen::Documents => {
                Self::move_selection(&mut self.document_index, self.documents.len(), -1)
            }
            Screen::DocumentView => self.scroll_document(-1),
            Screen::SavedQuerySelect => {
                Self::move_selection(&mut self.saved_query_index, self.storage.queries.len(), -1)
            }
            Screen::SavedAggregationSelect => Self::move_selection(
                &mut self.saved_agg_index,
                self.storage.aggregations.len(),
                -1,
            ),
            Screen::SaveQueryScopeSelect => {
                Self::move_selection(&mut self.save_query_scope_index, 2, -1)
            }
            Screen::SaveAggregationScopeSelect => {
                Self::move_selection(&mut self.save_agg_scope_index, 2, -1)
            }
            Screen::AddConnectionScopeSelect => Self::move_selection(
                &mut self.add_connection_scope_index,
                3, // Session-only, Repo, Global
                -1,
            ),
        }
    }

    fn move_down(&mut self) {
        match self.screen {
            Screen::Connections => Self::move_selection(
                &mut self.connection_index,
                self.storage.config.connections.len(),
                1,
            ),
            Screen::Databases => {
                Self::move_selection(&mut self.database_index, self.database_items.len(), 1)
            }
            Screen::Collections => {
                Self::move_selection(&mut self.collection_index, self.collection_items.len(), 1)
            }
            Screen::Documents => {
                Self::move_selection(&mut self.document_index, self.documents.len(), 1)
            }
            Screen::DocumentView => self.scroll_document(1),
            Screen::SavedQuerySelect => {
                Self::move_selection(&mut self.saved_query_index, self.storage.queries.len(), 1)
            }
            Screen::SavedAggregationSelect => Self::move_selection(
                &mut self.saved_agg_index,
                self.storage.aggregations.len(),
                1,
            ),
            Screen::SaveQueryScopeSelect => {
                Self::move_selection(&mut self.save_query_scope_index, 2, 1)
            }
            Screen::SaveAggregationScopeSelect => {
                Self::move_selection(&mut self.save_agg_scope_index, 2, 1)
            }
            Screen::AddConnectionScopeSelect => Self::move_selection(
                &mut self.add_connection_scope_index,
                3, // Session-only, Repo, Global
                1,
            ),
        }
    }

    fn go_back(&mut self) {
        match self.screen {
            Screen::Connections => {}
            Screen::Databases => self.screen = Screen::Connections,
            Screen::Collections => self.screen = Screen::Databases,
            Screen::Documents => self.screen = Screen::Collections,
            Screen::DocumentView => self.screen = Screen::Documents,
            Screen::SavedQuerySelect => self.screen = Screen::Documents,
            Screen::SavedAggregationSelect => self.screen = Screen::Documents,
            Screen::SaveQueryScopeSelect => {
                self.screen = Screen::Documents;
                self.save_query_scope_index = Some(0);
            }
            Screen::SaveAggregationScopeSelect => {
                self.screen = Screen::Documents;
                self.save_agg_scope_index = Some(0);
            }
            Screen::AddConnectionScopeSelect => {
                self.screen = Screen::Connections;
                self.add_connection_scope_index = Some(0);
            }
        }
    }

    fn go_forward(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        match self.screen {
            Screen::Connections => {
                if self.connection_index.is_some() {
                    if let Err(error) = self.start_load_databases() {
                        self.set_error_message(&error);
                        return Ok(());
                    }
                    self.screen = Screen::Databases;
                }
            }
            Screen::Databases => {
                if self.database_index.is_some() {
                    if let Err(error) = self.start_load_collections() {
                        self.set_error_message(&error);
                        return Ok(());
                    }
                    self.screen = Screen::Collections;
                }
            }
            Screen::Collections => {
                if self.collection_index.is_some() {
                    self.document_page = 0;
                    if let Err(error) =
                        self.start_load_documents(None, DocumentLoadReason::EnterCollection)
                    {
                        self.set_error_message(&error);
                        return Ok(());
                    }
                    self.screen = Screen::Documents;
                }
            }
            Screen::Documents => {
                if self.document_index.is_some() {
                    self.prepare_document_view();
                    self.screen = Screen::DocumentView;
                }
            }
            Screen::DocumentView => {}
            Screen::SavedQuerySelect => {
                if let Err(error) = self.start_execute_saved_query() {
                    self.set_error_message(&error);
                }
            }
            Screen::SavedAggregationSelect => {
                if let Err(error) = self.start_execute_saved_aggregation() {
                    self.set_error_message(&error);
                }
            }
            Screen::SaveQueryScopeSelect => {
                if let Err(error) = self.select_query_save_scope(terminal) {
                    self.set_error_message(&error);
                }
            }
            Screen::SaveAggregationScopeSelect => {
                if let Err(error) = self.select_aggregation_save_scope(terminal) {
                    self.set_error_message(&error);
                }
            }
            Screen::AddConnectionScopeSelect => {
                if let Err(error) = self.select_connection_scope(terminal) {
                    self.set_error_message(&error);
                }
            }
        }

        Ok(())
    }

    fn next_page(&mut self) -> Result<()> {
        if self.screen != Screen::Documents {
            return Ok(());
        }
        self.document_page += 1;
        if let Err(error) = self.start_load_documents(None, DocumentLoadReason::NavigateNext) {
            self.set_error_message(&error);
            return Ok(());
        }
        Ok(())
    }

    fn previous_page(&mut self) -> Result<()> {
        if self.screen != Screen::Documents {
            return Ok(());
        }
        if self.document_page == 0 {
            return Ok(());
        }
        self.document_page -= 1;
        if let Err(error) = self.start_load_documents(None, DocumentLoadReason::NavigatePrevious) {
            self.set_error_message(&error);
        }
        Ok(())
    }

    fn next_load_id(&mut self) -> u64 {
        self.next_load_id = self.next_load_id.saturating_add(1);
        self.next_load_id
    }

    fn start_load_databases(&mut self) -> Result<()> {
        let connection = self
            .selected_connection()
            .ok_or_else(|| anyhow::anyhow!("select a connection"))?;
        let config = self.storage.config.clone();
        let connection_name = connection.name.clone();
        let request_id = self.next_load_id();
        self.database_load_id = Some(request_id);
        self.database_state = LoadState::Loading;
        self.database_items.clear();
        self.database_index = None;
        self.message = None;
        let sender = self.load_tx.clone();
        self.runtime.spawn(async move {
            let executor = MongoExecutor::new();
            let result = executor
                .list_databases(&config, Some(&connection_name))
                .await
                .map_err(|error| error.to_string());
            let _ = sender.send(LoadResult::Databases {
                id: request_id,
                result,
            });
        });
        Ok(())
    }

    fn start_execute_saved_query(&mut self) -> Result<()> {
        let query_index = self
            .saved_query_index
            .ok_or_else(|| anyhow::anyhow!("select a saved query"))?;
        let saved = self
            .storage
            .queries
            .get(query_index)
            .ok_or_else(|| anyhow::anyhow!("select a saved query"))?
            .clone();
        let connection = self.selected_connection().map(|c| c.name.clone());
        let (database, collection) = match &saved.scope {
            SavedScope::Scoped {
                database,
                collection,
            } => (database.clone(), collection.clone()),
            SavedScope::Shared => (
                self.selected_database()
                    .ok_or_else(|| anyhow::anyhow!("select a database for shared saved queries"))?
                    .to_string(),
                self.selected_collection()
                    .ok_or_else(|| anyhow::anyhow!("select a collection for shared saved queries"))?
                    .to_string(),
            ),
        };

        let spec = lazycompass_mongo::QuerySpec {
            connection,
            database,
            collection,
            filter: saved.filter.clone(),
            projection: saved.projection.clone(),
            sort: saved.sort.clone(),
            limit: saved.limit,
        };

        let config = self.storage.config.clone();
        let request_id = self.next_load_id();
        self.saved_query_load_id = Some(request_id);
        self.saved_query_state = LoadState::Loading;
        self.message = Some(format!("executing saved query '{}'...", saved.id));
        let saved_name = saved.id.clone();
        let sender = self.load_tx.clone();
        self.runtime.spawn(async move {
            let executor = MongoExecutor::new();
            let result = executor
                .execute_query(&config, &spec)
                .await
                .map_err(|error| error.to_string());
            let _ = sender.send(LoadResult::SavedQuery {
                id: request_id,
                name: saved_name,
                result,
            });
        });
        Ok(())
    }

    fn start_execute_saved_aggregation(&mut self) -> Result<()> {
        let agg_index = self
            .saved_agg_index
            .ok_or_else(|| anyhow::anyhow!("select a saved aggregation"))?;
        let saved = self
            .storage
            .aggregations
            .get(agg_index)
            .ok_or_else(|| anyhow::anyhow!("select a saved aggregation"))?
            .clone();
        let connection = self.selected_connection().map(|c| c.name.clone());
        let (database, collection) = match &saved.scope {
            SavedScope::Scoped {
                database,
                collection,
            } => (database.clone(), collection.clone()),
            SavedScope::Shared => (
                self.selected_database()
                    .ok_or_else(|| {
                        anyhow::anyhow!("select a database for shared saved aggregations")
                    })?
                    .to_string(),
                self.selected_collection()
                    .ok_or_else(|| {
                        anyhow::anyhow!("select a collection for shared saved aggregations")
                    })?
                    .to_string(),
            ),
        };

        let spec = lazycompass_mongo::AggregationSpec {
            connection,
            database,
            collection,
            pipeline: saved.pipeline.clone(),
        };

        let config = self.storage.config.clone();
        let request_id = self.next_load_id();
        self.saved_agg_load_id = Some(request_id);
        self.saved_agg_state = LoadState::Loading;
        self.message = Some(format!("executing saved aggregation '{}'...", saved.id));
        let saved_name = saved.id.clone();
        let sender = self.load_tx.clone();
        self.runtime.spawn(async move {
            let executor = MongoExecutor::new();
            let result = executor
                .execute_aggregation(&config, &spec)
                .await
                .map_err(|error| error.to_string());
            let _ = sender.send(LoadResult::SavedAggregation {
                id: request_id,
                name: saved_name,
                result,
            });
        });
        Ok(())
    }

    fn start_load_collections(&mut self) -> Result<()> {
        let connection = self
            .selected_connection()
            .ok_or_else(|| anyhow::anyhow!("select a connection"))?;
        let database = self
            .selected_database()
            .ok_or_else(|| anyhow::anyhow!("select a database"))?;
        let config = self.storage.config.clone();
        let connection_name = connection.name.clone();
        let database_name = database.to_string();
        let request_id = self.next_load_id();
        self.collection_load_id = Some(request_id);
        self.collection_state = LoadState::Loading;
        self.collection_items.clear();
        self.collection_index = None;
        self.message = None;
        let sender = self.load_tx.clone();
        self.runtime.spawn(async move {
            let executor = MongoExecutor::new();
            let result = executor
                .list_collections(&config, Some(&connection_name), &database_name)
                .await
                .map_err(|error| error.to_string());
            let _ = sender.send(LoadResult::Collections {
                id: request_id,
                result,
            });
        });
        Ok(())
    }

    fn start_load_documents(
        &mut self,
        pending_index: Option<usize>,
        reason: DocumentLoadReason,
    ) -> Result<()> {
        let connection = self
            .selected_connection()
            .ok_or_else(|| anyhow::anyhow!("select a connection"))?;
        let database = self
            .selected_database()
            .ok_or_else(|| anyhow::anyhow!("select a database"))?;
        let collection = self
            .selected_collection()
            .ok_or_else(|| anyhow::anyhow!("select a collection"))?;
        let spec = DocumentListSpec {
            connection: Some(connection.name.clone()),
            database: database.to_string(),
            collection: collection.to_string(),
            skip: self.document_page * PAGE_SIZE,
            limit: PAGE_SIZE,
        };
        let config = self.storage.config.clone();
        let request_id = self.next_load_id();
        self.document_load_id = Some(request_id);
        self.document_state = LoadState::Loading;
        self.document_result_source = DocumentResultSource::Collection;
        self.document_load_reason = reason;
        self.document_pending_index = pending_index;
        self.documents.clear();
        self.document_index = None;
        self.document_lines.clear();
        self.document_scroll = 0;
        self.message = None;
        let sender = self.load_tx.clone();
        self.runtime.spawn(async move {
            let executor = MongoExecutor::new();
            let result = executor
                .list_documents(&config, &spec)
                .await
                .map_err(|error| error.to_string());
            let _ = sender.send(LoadResult::Documents {
                id: request_id,
                result,
            });
        });
        Ok(())
    }

    fn prepare_document_view(&mut self) {
        let Some(index) = self.document_index else {
            return;
        };
        let Some(document) = self.documents.get(index) else {
            return;
        };
        self.document_lines = format_document(document);
        self.document_scroll = 0;
    }

    fn selected_connection(&self) -> Option<&ConnectionSpec> {
        self.connection_index
            .and_then(|index| self.storage.config.connections.get(index))
    }

    fn selected_database(&self) -> Option<&str> {
        self.database_index
            .and_then(|index| self.database_items.get(index))
            .map(String::as_str)
    }

    fn selected_collection(&self) -> Option<&str> {
        self.collection_index
            .and_then(|index| self.collection_items.get(index))
            .map(String::as_str)
    }

    fn move_selection(selected: &mut Option<usize>, len: usize, delta: i32) {
        if len == 0 {
            *selected = None;
            return;
        }
        let current = selected.unwrap_or(0);
        let next = if delta < 0 {
            current.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            (current + delta as usize).min(len.saturating_sub(1))
        };
        *selected = Some(next);
    }

    fn select_index(selected: &mut Option<usize>, len: usize, index: usize) {
        if len == 0 {
            *selected = None;
        } else {
            *selected = Some(index.min(len - 1));
        }
    }

    fn select_last(selected: &mut Option<usize>, len: usize) {
        if len == 0 {
            *selected = None;
        } else {
            *selected = Some(len - 1);
        }
    }

    fn scroll_document(&mut self, delta: i16) {
        if self.document_lines.is_empty() {
            self.document_scroll = 0;
            return;
        }
        let max_scroll = self.max_document_scroll();
        let next = if delta < 0 {
            self.document_scroll.saturating_sub(delta.unsigned_abs())
        } else {
            self.document_scroll.saturating_add(delta as u16)
        };
        self.document_scroll = next.min(max_scroll);
    }

    fn max_document_scroll(&self) -> u16 {
        let max = self.document_lines.len().saturating_sub(1);
        max.min(u16::MAX as usize) as u16
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(2),
                Constraint::Length(3),
            ])
            .split(frame.area());

        let header = Paragraph::new(self.header_lines())
            .style(self.theme.text_style())
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(self.theme.border_style()),
            );
        frame.render_widget(header, layout[0]);

        match self.screen {
            Screen::Connections => {
                let items = self
                    .storage
                    .config
                    .connections
                    .iter()
                    .map(connection_label)
                    .collect::<Vec<_>>();
                self.render_list(
                    frame,
                    layout[1],
                    ListView {
                        title: "Connections",
                        items: &items,
                        selected: self.connection_index,
                        load_state: &LoadState::Idle,
                        loading_label: "loading connections...",
                    },
                );
            }
            Screen::Databases => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(layout[1]);
                let connections = self
                    .storage
                    .config
                    .connections
                    .iter()
                    .map(connection_label)
                    .collect::<Vec<_>>();
                self.render_list(
                    frame,
                    panes[0],
                    ListView {
                        title: "Connections",
                        items: &connections,
                        selected: self.connection_index,
                        load_state: &LoadState::Idle,
                        loading_label: "loading connections...",
                    },
                );
                self.render_list(
                    frame,
                    panes[1],
                    ListView {
                        title: "Databases",
                        items: &self.database_items,
                        selected: self.database_index,
                        load_state: &self.database_state,
                        loading_label: "loading databases...",
                    },
                );
            }
            Screen::Collections => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(layout[1]);
                self.render_list(
                    frame,
                    panes[0],
                    ListView {
                        title: "Databases",
                        items: &self.database_items,
                        selected: self.database_index,
                        load_state: &self.database_state,
                        loading_label: "loading databases...",
                    },
                );
                self.render_list(
                    frame,
                    panes[1],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                );
            }
            Screen::Documents => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(layout[1]);
                self.render_list(
                    frame,
                    panes[0],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                );
                let items = self
                    .documents
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = self.documents_list_title();
                self.render_list(
                    frame,
                    panes[1],
                    ListView {
                        title: &title,
                        items: &items,
                        selected: self.document_index,
                        load_state: &self.document_state,
                        loading_label: "loading documents...",
                    },
                );
            }
            Screen::DocumentView => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(layout[1]);
                let items = self
                    .documents
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = self.documents_list_title();
                self.render_list(
                    frame,
                    panes[0],
                    ListView {
                        title: &title,
                        items: &items,
                        selected: self.document_index,
                        load_state: &self.document_state,
                        loading_label: "loading documents...",
                    },
                );
                let max_scroll = self.max_document_scroll();
                if self.document_scroll > max_scroll {
                    self.document_scroll = max_scroll;
                }
                let lines = self
                    .document_lines
                    .iter()
                    .map(|line| Line::from(line.clone()))
                    .collect::<Vec<_>>();
                let body = Paragraph::new(lines)
                    .style(self.theme.text_style())
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(self.theme.border_style())
                            .title(Line::from(Span::styled(
                                "Document",
                                self.theme.title_style(),
                            ))),
                    )
                    .wrap(Wrap { trim: false })
                    .scroll((self.document_scroll, 0));
                frame.render_widget(body, panes[1]);
            }
            Screen::SavedQuerySelect => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(layout[1]);
                self.render_list(
                    frame,
                    panes[0],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                );
                let right_panes = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(15), Constraint::Percentage(85)])
                    .split(panes[1]);
                let documents = self
                    .documents
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let document_title = self.documents_list_title();
                let items: Vec<String> = self
                    .storage
                    .queries
                    .iter()
                    .map(|q| format!("{} ({})", q.id, saved_scope_label(&q.scope)))
                    .collect();
                self.render_list(
                    frame,
                    right_panes[0],
                    ListView {
                        title: "Select Saved Query to Run",
                        items: &items,
                        selected: self.saved_query_index,
                        load_state: &self.saved_query_state,
                        loading_label: "executing query...",
                    },
                );
                self.render_list_with_focus(
                    frame,
                    right_panes[1],
                    ListView {
                        title: &document_title,
                        items: &documents,
                        selected: self.document_index,
                        load_state: &self.document_state,
                        loading_label: "loading documents...",
                    },
                    false,
                );
            }
            Screen::SavedAggregationSelect => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(layout[1]);
                self.render_list(
                    frame,
                    panes[0],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                );
                let right_panes = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(15), Constraint::Percentage(85)])
                    .split(panes[1]);
                let documents = self
                    .documents
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let document_title = self.documents_list_title();
                let items: Vec<String> = self
                    .storage
                    .aggregations
                    .iter()
                    .map(|a| format!("{} ({})", a.id, saved_scope_label(&a.scope)))
                    .collect();
                self.render_list(
                    frame,
                    right_panes[0],
                    ListView {
                        title: "Select Saved Aggregation to Run",
                        items: &items,
                        selected: self.saved_agg_index,
                        load_state: &self.saved_agg_state,
                        loading_label: "executing aggregation...",
                    },
                );
                self.render_list_with_focus(
                    frame,
                    right_panes[1],
                    ListView {
                        title: &document_title,
                        items: &documents,
                        selected: self.document_index,
                        load_state: &self.document_state,
                        loading_label: "loading documents...",
                    },
                    false,
                );
            }
            Screen::SaveQueryScopeSelect => {
                let items = vec![
                    "Shared (uses current db/collection when running)".to_string(),
                    "Scoped (encode current db/collection in filename)".to_string(),
                ];
                self.render_list(
                    frame,
                    layout[1],
                    ListView {
                        title: "Select Query Save Scope",
                        items: &items,
                        selected: self.save_query_scope_index,
                        load_state: &LoadState::Idle,
                        loading_label: "",
                    },
                );
            }
            Screen::SaveAggregationScopeSelect => {
                let items = vec![
                    "Shared (uses current db/collection when running)".to_string(),
                    "Scoped (encode current db/collection in filename)".to_string(),
                ];
                self.render_list(
                    frame,
                    layout[1],
                    ListView {
                        title: "Select Aggregation Save Scope",
                        items: &items,
                        selected: self.save_agg_scope_index,
                        load_state: &LoadState::Idle,
                        loading_label: "",
                    },
                );
            }
            Screen::AddConnectionScopeSelect => {
                let items = vec![
                    "Session only (not persisted)".to_string(),
                    "Save to repo config (.lazycompass/config.toml)".to_string(),
                    "Save to global config (~/.config/lazycompass/config.toml)".to_string(),
                ];
                self.render_list(
                    frame,
                    layout[1],
                    ListView {
                        title: "Select Persistence Scope for New Connection",
                        items: &items,
                        selected: self.add_connection_scope_index,
                        load_state: &LoadState::Idle,
                        loading_label: "",
                    },
                );
            }
        }

        if self.help_visible {
            self.render_help(frame, layout[1]);
        }

        let footer = Paragraph::new(self.footer_lines())
            .style(self.theme.text_style())
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(self.theme.border_style()),
            );
        frame.render_widget(footer, layout[2]);
    }

    fn render_list(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        view: ListView<'_>,
    ) {
        self.render_list_with_focus(frame, area, view, true);
    }

    fn render_list_with_focus(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        view: ListView<'_>,
        focused: bool,
    ) {
        let title_style = if focused {
            self.theme.title_style()
        } else {
            self.theme
                .text_style()
                .add_modifier(Modifier::DIM)
                .add_modifier(Modifier::BOLD)
        };
        let border_style = if focused {
            self.theme.border_style()
        } else {
            self.theme.border_style().add_modifier(Modifier::DIM)
        };
        let title = Line::from(Span::styled(view.title.to_string(), title_style));
        if view.items.is_empty() {
            let (text, style) = match view.load_state {
                LoadState::Loading => (view.loading_label.to_string(), self.theme.text_style()),
                LoadState::Failed(message) => {
                    (format!("error: {message}"), self.theme.error_style())
                }
                LoadState::Idle => ("no items".to_string(), self.theme.text_style()),
            };
            let placeholder = Paragraph::new(text).style(style).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(title),
            );
            frame.render_widget(placeholder, area);
            return;
        }

        let items = view
            .items
            .iter()
            .map(|item| ListItem::new(item.clone()))
            .collect::<Vec<_>>();
        let list = List::new(items)
            .style(self.theme.text_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(title),
            )
            .highlight_style(self.theme.selection_style())
            .highlight_symbol("> ");
        let mut state = ListState::default();
        state.select(if focused { view.selected } else { None });
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_help(&self, frame: &mut ratatui::Frame, area: Rect) {
        let help_area = centered_rect(70, 70, area);
        frame.render_widget(Clear, help_area);
        let help = Paragraph::new(self.help_lines())
            .style(self.theme.text_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.border_style())
                    .title(Line::from(Span::styled("Help", self.theme.title_style()))),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(help, help_area);
    }

    fn help_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for group in hint_groups(self.screen) {
            let keys = keys_for_actions(group.actions);
            lines.push(Line::from(format!("{keys:<12} {}", group.label)));
        }
        lines.push(Line::from(" "));
        lines.push(Line::from("press ? or Esc to close"));
        lines
    }

    fn hint_line(&self) -> String {
        hint_groups(self.screen)
            .iter()
            .map(|group| format!("{} {}", keys_for_actions(group.actions), group.label))
            .collect::<Vec<_>>()
            .join("  ")
    }

    fn header_lines(&self) -> Vec<Line<'static>> {
        let title = match self.screen {
            Screen::Connections => "Connections",
            Screen::Databases => "Databases",
            Screen::Collections => "Collections",
            Screen::Documents => "Documents",
            Screen::DocumentView => "Document",
            Screen::SavedQuerySelect => "Run Saved Query",
            Screen::SavedAggregationSelect => "Run Saved Aggregation",
            Screen::SaveQueryScopeSelect => "Save Query",
            Screen::SaveAggregationScopeSelect => "Save Aggregation",
            Screen::AddConnectionScopeSelect => "Add Connection",
        };
        let connection = self
            .selected_connection()
            .map(|connection| connection.name.as_str())
            .unwrap_or("-");
        let database = self.selected_database().unwrap_or("-");
        let collection = self.selected_collection().unwrap_or("-");
        let path = format!("Conn: {connection}  Db: {database}  Coll: {collection}");

        let mut lines = vec![
            Line::from(Span::styled(title.to_string(), self.theme.title_style())),
            Line::from(Span::styled(path, self.theme.text_style())),
        ];

        if self.read_only {
            lines.push(Line::from(Span::styled(
                "MODE: READ-ONLY",
                self.theme.warning_style().add_modifier(Modifier::BOLD),
            )));
        }

        lines
    }

    fn footer_lines(&self) -> Vec<Line<'static>> {
        let hint = self.hint_line();

        if let Some(editor_prompt) = &self.editor_prompt {
            let input_display = if editor_prompt.input.is_empty() {
                "[type below]".to_string()
            } else {
                format!("'{}'", editor_prompt.input)
            };
            vec![
                Line::from(Span::styled(
                    editor_prompt.prompt.clone(),
                    self.theme.warning_style(),
                )),
                Line::from(format!(
                    "Enter to launch editor (current: {input_display})  Esc to cancel"
                )),
            ]
        } else if let Some(confirm) = &self.confirm {
            let action_line = if let Some(required) = confirm.required {
                let input_display = if confirm.input.is_empty() {
                    "[type below]".to_string()
                } else {
                    format!("'{}'", confirm.input)
                };
                format!(
                    "Confirm: type '{}' then press Enter (currently: {})  Esc to cancel",
                    required, input_display
                )
            } else {
                "y confirm  n cancel  Esc to cancel".to_string()
            };
            vec![
                Line::from(Span::styled(
                    confirm.prompt.clone(),
                    self.theme.warning_style(),
                )),
                Line::from(action_line),
            ]
        } else if let Some(message) = &self.message {
            vec![
                Line::from(Span::styled(message.clone(), self.theme.error_style())),
                Line::from(hint),
            ]
        } else if let Some(warning) = &self.warnings.front() {
            vec![
                Line::from(Span::styled(
                    format!("warning: {warning}"),
                    self.theme.warning_style(),
                )),
                Line::from(hint),
            ]
        } else {
            vec![Line::from(hint), Line::from(" ")]
        }
    }

    fn documents_list_title(&self) -> String {
        let base = format!("Documents (page {})", self.document_page + 1);
        match &self.document_result_source {
            DocumentResultSource::Collection => base,
            DocumentResultSource::SavedQuery { name } => {
                format!("{base} [saved query: {name}] [c clear applied]")
            }
            DocumentResultSource::SavedAggregation { name } => {
                format!("{base} [saved aggregation: {name}] [c clear applied]")
            }
        }
    }

    fn clear_applied_documents(&mut self) -> Result<()> {
        if !matches!(self.screen, Screen::Documents | Screen::DocumentView) {
            return Ok(());
        }
        if matches!(
            self.document_result_source,
            DocumentResultSource::Collection
        ) {
            return Ok(());
        }

        self.document_result_source = DocumentResultSource::Collection;
        self.document_page = 0;
        if let Err(error) = self.start_load_documents(None, DocumentLoadReason::Refresh) {
            self.set_error_message(&error);
            return Ok(());
        }
        self.message = Some("cleared applied saved results".to_string());
        Ok(())
    }
}

fn render_query_payload_template(template: &SavedQuery) -> Result<String> {
    let mut object = serde_json::Map::new();
    if let Some(filter) = template.filter.as_deref() {
        object.insert(
            "filter".to_string(),
            serde_json::from_str(filter).context("saved query filter must be valid JSON")?,
        );
    }
    if let Some(projection) = template.projection.as_deref() {
        object.insert(
            "projection".to_string(),
            serde_json::from_str(projection)
                .context("saved query projection must be valid JSON")?,
        );
    }
    if let Some(sort) = template.sort.as_deref() {
        object.insert(
            "sort".to_string(),
            serde_json::from_str(sort).context("saved query sort must be valid JSON")?,
        );
    }
    if let Some(limit) = template.limit {
        object.insert("limit".to_string(), serde_json::Value::from(limit));
    }
    serde_json::to_string_pretty(&serde_json::Value::Object(object))
        .context("unable to serialize query template")
}

fn parse_query_payload_input(contents: &str, template: &SavedQuery) -> Result<SavedQuery> {
    let value: serde_json::Value =
        serde_json::from_str(contents).context("invalid JSON for saved query")?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("saved query payload must be a JSON object"))?;
    for key in object.keys() {
        if !matches!(key.as_str(), "filter" | "projection" | "sort" | "limit") {
            anyhow::bail!("unknown field '{key}' in saved query payload");
        }
    }
    let filter = json_field_as_string(object, "filter")?;
    let projection = json_field_as_string(object, "projection")?;
    let sort = json_field_as_string(object, "sort")?;
    let limit = match object.get("limit") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("field 'limit' must be a non-negative integer"))?,
        ),
    };
    Ok(SavedQuery {
        id: template.id.clone(),
        scope: template.scope.clone(),
        filter,
        projection,
        sort,
        limit,
    })
}

fn render_aggregation_payload_template(template: &SavedAggregation) -> Result<String> {
    let pipeline: serde_json::Value = serde_json::from_str(&template.pipeline)
        .context("saved aggregation pipeline must be valid JSON")?;
    if !pipeline.is_array() {
        anyhow::bail!("saved aggregation pipeline must be a JSON array");
    }
    serde_json::to_string_pretty(&pipeline).context("unable to serialize aggregation template")
}

fn parse_aggregation_payload_input(
    contents: &str,
    template: &SavedAggregation,
) -> Result<SavedAggregation> {
    let pipeline: serde_json::Value =
        serde_json::from_str(contents).context("invalid JSON for saved aggregation")?;
    if !pipeline.is_array() {
        anyhow::bail!("saved aggregation payload must be a JSON array");
    }
    Ok(SavedAggregation {
        id: template.id.clone(),
        scope: template.scope.clone(),
        pipeline: serde_json::to_string(&pipeline).context("unable to serialize pipeline JSON")?,
    })
}

fn json_field_as_string(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<Option<String>> {
    let Some(value) = object.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let serialized = serde_json::to_string(value)
        .with_context(|| format!("unable to serialize field '{field}'"))?;
    Ok(Some(serialized))
}

fn saved_scope_label(scope: &SavedScope) -> String {
    match scope {
        SavedScope::Shared => "shared".to_string(),
        SavedScope::Scoped {
            database,
            collection,
        } => format!("{database}.{collection}"),
    }
}

fn default_saved_id(kind: &str, scope: &SavedScope) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let name = format!("{kind}_{millis}");
    match scope {
        SavedScope::Shared => name,
        SavedScope::Scoped {
            database,
            collection,
        } => format!("{database}.{collection}.{name}"),
    }
}

impl KeyBinding {
    fn matches(&self, key: KeyEvent) -> bool {
        self.code == key.code && self.modifiers == key.modifiers
    }
}

fn action_for_key(key: KeyEvent) -> Option<KeyAction> {
    KEY_BINDINGS
        .iter()
        .find(|binding| binding.matches(key))
        .map(|binding| binding.action)
}

fn is_network_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        let message = cause.to_string().to_ascii_lowercase();
        message.contains("unable to connect")
            || message.contains("failed to connect")
            || message.contains("server selection")
            || message.contains("network")
            || message.contains("timed out")
            || message.contains("timeout")
    })
}

fn is_network_error_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("unable to connect")
        || message.contains("failed to connect")
        || message.contains("server selection")
        || message.contains("network")
        || message.contains("timed out")
        || message.contains("timeout")
}

fn format_error_message(message: &str, is_network: bool) -> String {
    let mut output = redact_sensitive_text(message);
    if is_network {
        output.push_str(" (network error: retry read-only operations)");
    }
    output
}

fn hint_groups(screen: Screen) -> &'static [HintGroup] {
    match screen {
        Screen::Connections => CONNECTION_HINTS,
        Screen::Databases => DATABASE_HINTS,
        Screen::Collections => COLLECTION_HINTS,
        Screen::Documents => DOCUMENT_HINTS,
        Screen::DocumentView => DOCUMENT_VIEW_HINTS,
        Screen::SavedQuerySelect => SAVED_QUERY_HINTS,
        Screen::SavedAggregationSelect => SAVED_AGGREGATION_HINTS,
        Screen::SaveQueryScopeSelect => SAVE_SCOPE_HINTS,
        Screen::SaveAggregationScopeSelect => SAVE_SCOPE_HINTS,
        Screen::AddConnectionScopeSelect => ADD_CONNECTION_SCOPE_HINTS,
    }
}

fn action_keys(action: KeyAction) -> &'static [&'static str] {
    match action {
        KeyAction::Quit => &["q"],
        KeyAction::MoveDown => &["j"],
        KeyAction::MoveUp => &["k"],
        KeyAction::Back => &["h"],
        KeyAction::Forward => &["l", "Enter"],
        KeyAction::GoTop => &["gg"],
        KeyAction::GoBottom => &["G"],
        KeyAction::NextPage => &["PgDn"],
        KeyAction::PreviousPage => &["PgUp"],
        KeyAction::Insert => &["i"],
        KeyAction::Edit => &["e"],
        KeyAction::Delete => &["d"],
        KeyAction::SaveQuery => &["Q"],
        KeyAction::SaveAggregation => &["A"],
        KeyAction::RunSavedQuery => &["r"],
        KeyAction::RunSavedAggregation => &["a"],
        KeyAction::ClearApplied => &["c"],
        KeyAction::ToggleHelp => &["?"],
        KeyAction::AddConnection => &["n"],
    }
}

fn keys_for_actions(actions: &[KeyAction]) -> String {
    let mut keys = Vec::new();
    for action in actions {
        keys.extend_from_slice(action_keys(*action));
    }
    keys.join("/")
}

fn resolve_theme(config: &Config) -> (Theme, Option<String>) {
    let name = config.theme.name.as_deref().unwrap_or_default();
    if name.trim().is_empty() {
        return (THEME_CLASSIC, None);
    }
    match theme_by_name(name) {
        Some(theme) => (theme, None),
        None => (
            THEME_CLASSIC,
            Some(format!("unknown theme '{name}', using classic")),
        ),
    }
}

fn theme_by_name(name: &str) -> Option<Theme> {
    match name.trim().to_ascii_lowercase().as_str() {
        "classic" | "default" => Some(THEME_CLASSIC),
        "ember" => Some(THEME_EMBER),
        _ => None,
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    let vertical = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1]);

    vertical[1]
}

pub fn run(config: Config) -> Result<()> {
    let cwd = std::env::current_dir().context("unable to resolve current directory")?;
    let paths = ConfigPaths::resolve_from(&cwd)?;
    let read_only = config.read_only();
    let storage = load_storage_with_config(&paths, config)?;
    let mut app = App::new(paths, storage, read_only)?;

    let mut terminal = setup_terminal()?;
    let result = app.run(&mut terminal);
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("unable to enable raw mode")?;
    let mut output = stdout();
    execute!(output, EnterAlternateScreen, Hide).context("unable to enter alternate screen")?;
    let backend = CrosstermBackend::new(output);
    Terminal::new(backend).context("unable to start terminal")
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("unable to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, Show)
        .context("unable to leave alternate screen")?;
    terminal.show_cursor().context("unable to restore cursor")?;
    Ok(())
}

fn suspend_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("unable to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, Show)
        .context("unable to leave alternate screen")?;
    terminal.show_cursor().context("unable to show cursor")?;
    Ok(())
}

fn resume_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    enable_raw_mode().context("unable to enable raw mode")?;
    execute!(terminal.backend_mut(), EnterAlternateScreen, Hide)
        .context("unable to enter alternate screen")?;
    terminal.clear().context("unable to clear terminal")?;
    Ok(())
}

fn resolve_editor() -> Result<String> {
    std::env::var("VISUAL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .ok_or_else(|| anyhow::anyhow!("$VISUAL or $EDITOR is required for editing"))
}

fn parse_editor_command(editor: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = editor.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                current.push(ch);
            }
            continue;
        }

        if in_double {
            match ch {
                '"' => in_double = false,
                '\\' => {
                    let next = chars
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("unterminated escape in editor command"))?;
                    current.push(next);
                }
                _ => current.push(ch),
            }
            continue;
        }

        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '\\' => {
                let next = chars
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("unterminated escape in editor command"))?;
                current.push(next);
            }
            ch if ch.is_whitespace() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if in_single || in_double {
        anyhow::bail!("unterminated quote in editor command");
    }
    if !current.is_empty() {
        args.push(current);
    }
    if args.is_empty() {
        anyhow::bail!("editor command is empty");
    }
    Ok(args)
}

fn run_editor_command(editor: &str, path: &Path) -> Result<std::process::ExitStatus> {
    let args = parse_editor_command(editor)?;
    let (program, rest) = args
        .split_first()
        .ok_or_else(|| anyhow::anyhow!("editor command is empty"))?;
    Command::new(program)
        .args(rest)
        .arg(path)
        .status()
        .context("failed to launch editor")
}

fn write_editor_temp_file(path: &Path, contents: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::fs::PermissionsExt;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .with_context(|| format!("unable to open temporary file {}", path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("unable to write temporary file {}", path.display()))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("unable to set permissions on {}", path.display()))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        fs::write(path, contents)
            .with_context(|| format!("unable to write temporary file {}", path.display()))?;
        Ok(())
    }
}

fn editor_temp_path(label: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.push(format!("lazycompass_{label}_{pid}_{nanos}.tmp"));
    path
}

fn document_id(document: &Document) -> Result<Bson> {
    document
        .get("_id")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("document is missing _id"))
}

fn format_bson(value: &Bson) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(value)) => value,
        Ok(serde_json::Value::Null) => "null".to_string(),
        Ok(value) => value.to_string(),
        Err(_) => format!("{value:?}"),
    }
}

fn connection_label(connection: &ConnectionSpec) -> String {
    match connection.default_database.as_deref() {
        Some(default_db) => format!("{} ({default_db})", connection.name),
        None => connection.name.clone(),
    }
}

fn document_preview(document: &Document) -> String {
    let mut json = serde_json::to_string(document).unwrap_or_else(|_| format!("{document:?}"));
    json = json.replace('\n', " ");
    if json.len() > 120 {
        json.truncate(117);
        json.push_str("...");
    }
    json
}

fn format_document(document: &Document) -> Vec<String> {
    match serde_json::to_string_pretty(document) {
        Ok(output) => output.lines().map(|line| line.to_string()).collect(),
        Err(_) => vec![format!("{document:?}")],
    }
}

fn is_editor_cancelled(contents: &str, initial: &str) -> bool {
    let trimmed = contents.trim();
    trimmed.is_empty() || trimmed == initial.trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn key_bindings_are_unique() {
        let mut seen = HashSet::new();
        for binding in KEY_BINDINGS {
            let key = format!("{:?}:{:?}", binding.code, binding.modifiers);
            assert!(seen.insert(key), "duplicate key binding: {binding:?}");
        }
    }

    #[test]
    fn resolve_theme_warns_on_unknown() {
        let config = Config {
            connections: Vec::new(),
            theme: lazycompass_core::ThemeConfig {
                name: Some("mystery".to_string()),
            },
            logging: lazycompass_core::LoggingConfig::default(),
            read_only: None,
            allow_pipeline_writes: None,
            allow_insecure: None,
            timeouts: lazycompass_core::TimeoutConfig::default(),
        };
        let (theme, warning) = resolve_theme(&config);
        assert!(warning.is_some());
        assert_eq!(theme.border, THEME_CLASSIC.border);
    }

    #[test]
    fn resolve_theme_uses_ember() {
        let config = Config {
            connections: Vec::new(),
            theme: lazycompass_core::ThemeConfig {
                name: Some("ember".to_string()),
            },
            logging: lazycompass_core::LoggingConfig::default(),
            read_only: None,
            allow_pipeline_writes: None,
            allow_insecure: None,
            timeouts: lazycompass_core::TimeoutConfig::default(),
        };
        let (theme, warning) = resolve_theme(&config);
        assert!(warning.is_none());
        assert_eq!(theme.accent, THEME_EMBER.accent);
    }

    #[test]
    fn parse_editor_command_handles_quotes() {
        let args = parse_editor_command("nvim -c \"set ft=json\"").expect("parse editor command");
        assert_eq!(args, vec!["nvim", "-c", "set ft=json"]);
        let args = parse_editor_command("code --wait").expect("parse editor command");
        assert_eq!(args, vec!["code", "--wait"]);
        let args = parse_editor_command("edit 'arg with spaces'").expect("parse editor command");
        assert_eq!(args, vec!["edit", "arg with spaces"]);
    }

    #[test]
    fn parse_editor_command_rejects_unclosed_quotes() {
        assert!(parse_editor_command("nvim -c \"oops").is_err());
    }
}
