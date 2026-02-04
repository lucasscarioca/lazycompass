use anyhow::{Context, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use lazycompass_core::{Config, ConnectionSpec, SavedAggregation, SavedQuery};
use lazycompass_mongo::{
    Bson, Document, DocumentDeleteSpec, DocumentInsertSpec, DocumentListSpec, DocumentReplaceSpec,
    MongoExecutor, parse_json_document,
};
use lazycompass_storage::{
    ConfigPaths, StorageSnapshot, load_storage, saved_aggregation_path, saved_query_path,
    write_saved_aggregation, write_saved_query,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use std::collections::VecDeque;
use std::fs;
use std::io::{Stdout, stdout};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
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
    ToggleHelp,
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
        action: KeyAction::ToggleHelp,
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::ToggleHelp,
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::SHIFT,
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
}

#[derive(Debug, Clone)]
struct ConfirmState {
    prompt: String,
    action: ConfirmAction,
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

struct App {
    paths: ConfigPaths,
    storage: StorageSnapshot,
    executor: MongoExecutor,
    runtime: Runtime,
    theme: Theme,
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
    warnings: VecDeque<String>,
}

impl App {
    fn new(paths: ConfigPaths, storage: StorageSnapshot) -> Result<Self> {
        let runtime = Runtime::new().context("unable to start async runtime")?;
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
            warnings,
        })
    }

    fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;
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

    fn handle_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<bool> {
        if self.confirm.is_some() {
            return self.handle_confirm_key(key, terminal);
        }

        if !self.warnings.is_empty() {
            self.warnings.pop_front();
        }

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
            KeyAction::Forward => self.go_forward()?,
            KeyAction::GoTop => self.go_top(),
            KeyAction::GoBottom => self.go_bottom(),
            KeyAction::NextPage => self.next_page()?,
            KeyAction::PreviousPage => self.previous_page()?,
            KeyAction::Insert => self.insert_document(terminal)?,
            KeyAction::Edit => self.edit_document(terminal)?,
            KeyAction::Delete => self.request_delete_document()?,
            KeyAction::SaveQuery => self.save_query(terminal)?,
            KeyAction::SaveAggregation => self.save_aggregation(terminal)?,
            KeyAction::ToggleHelp => self.help_visible = !self.help_visible,
        }

        Ok(false)
    }

    fn handle_confirm_key(
        &mut self,
        key: KeyEvent,
        _terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<bool> {
        let Some(confirm) = self.confirm.take() else {
            return Ok(false);
        };

        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Err(error) = self.perform_confirm_action(confirm.action) {
                    self.message = Some(error.to_string());
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

    fn perform_confirm_action(&mut self, action: ConfirmAction) -> Result<()> {
        match action {
            ConfirmAction::DeleteDocument {
                spec,
                return_to_documents,
            } => {
                self.runtime
                    .block_on(self.executor.delete_document(&self.storage.config, &spec))?;
                if return_to_documents {
                    self.screen = Screen::Documents;
                }
                self.reload_documents_after_change()?;
                self.message = Some("document deleted".to_string());
            }
            ConfirmAction::OverwriteQuery { query } => {
                let path = write_saved_query(&self.paths, &query, true)?;
                self.upsert_query(query);
                self.message = Some(format!("saved query to {}", path.display()));
            }
            ConfirmAction::OverwriteAggregation { aggregation } => {
                let path = write_saved_aggregation(&self.paths, &aggregation, true)?;
                self.upsert_aggregation(aggregation);
                self.message = Some(format!("saved aggregation to {}", path.display()));
            }
        }
        Ok(())
    }

    fn request_delete_document(&mut self) -> Result<()> {
        if !matches!(self.screen, Screen::Documents | Screen::DocumentView) {
            return Ok(());
        }
        let result = (|| -> Result<()> {
            let (connection, database, collection) = self.selected_context()?;
            let document = self.selected_document()?;
            let id = document_id(document)?;
            let prompt = format!("delete document {}? (y/n)", format_bson(&id));
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
            });
            Ok(())
        })();

        if let Err(error) = result {
            self.message = Some(error.to_string());
        }
        Ok(())
    }

    fn insert_document(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        if self.screen != Screen::Documents {
            return Ok(());
        }
        let result = (|| -> Result<()> {
            let (connection, database, collection) = self.selected_context()?;
            let contents = self.open_editor(terminal, "insert", "{}")?;
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
        })();

        if let Err(error) = result {
            self.message = Some(error.to_string());
        }
        Ok(())
    }

    fn edit_document(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        if !matches!(self.screen, Screen::Documents | Screen::DocumentView) {
            return Ok(());
        }
        let result = (|| -> Result<()> {
            let (connection, database, collection) = self.selected_context()?;
            let document = self.selected_document()?.clone();
            let original_id = document_id(&document)?;
            let initial =
                serde_json::to_string_pretty(&document).context("unable to serialize document")?;
            let contents = self.open_editor(terminal, "edit", &initial)?;
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
        })();

        if let Err(error) = result {
            self.message = Some(error.to_string());
        }
        Ok(())
    }

    fn save_query(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        if self.screen != Screen::Documents {
            return Ok(());
        }
        let result = (|| -> Result<()> {
            let (connection, database, collection) = self.selected_context()?;
            let template = SavedQuery {
                name: "new_query".to_string(),
                connection: Some(connection),
                database,
                collection,
                filter: None,
                projection: None,
                sort: None,
                limit: None,
                notes: None,
            };
            let initial =
                toml::to_string_pretty(&template).context("unable to render query template")?;
            let contents = self.open_editor(terminal, "query", &initial)?;
            let query: SavedQuery =
                toml::from_str(&contents).context("invalid TOML for saved query")?;
            query.validate().context("invalid saved query")?;
            let path = saved_query_path(&self.paths, &query.name)?;
            if path.exists() {
                self.confirm = Some(ConfirmState {
                    prompt: format!("overwrite saved query '{}'? (y/n)", query.name),
                    action: ConfirmAction::OverwriteQuery { query },
                });
                return Ok(());
            }
            let path = write_saved_query(&self.paths, &query, false)?;
            self.upsert_query(query);
            self.message = Some(format!("saved query to {}", path.display()));
            Ok(())
        })();

        if let Err(error) = result {
            self.message = Some(error.to_string());
        }
        Ok(())
    }

    fn save_aggregation(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
        if self.screen != Screen::Documents {
            return Ok(());
        }
        let result = (|| -> Result<()> {
            let (connection, database, collection) = self.selected_context()?;
            let template = SavedAggregation {
                name: "new_aggregation".to_string(),
                connection: Some(connection),
                database,
                collection,
                pipeline: "[]".to_string(),
                notes: None,
            };
            let initial = toml::to_string_pretty(&template)
                .context("unable to render aggregation template")?;
            let contents = self.open_editor(terminal, "aggregation", &initial)?;
            let aggregation: SavedAggregation =
                toml::from_str(&contents).context("invalid TOML for saved aggregation")?;
            aggregation
                .validate()
                .context("invalid saved aggregation")?;
            let path = saved_aggregation_path(&self.paths, &aggregation.name)?;
            if path.exists() {
                self.confirm = Some(ConfirmState {
                    prompt: format!("overwrite saved aggregation '{}'? (y/n)", aggregation.name),
                    action: ConfirmAction::OverwriteAggregation { aggregation },
                });
                return Ok(());
            }
            let path = write_saved_aggregation(&self.paths, &aggregation, false)?;
            self.upsert_aggregation(aggregation);
            self.message = Some(format!("saved aggregation to {}", path.display()));
            Ok(())
        })();

        if let Err(error) = result {
            self.message = Some(error.to_string());
        }
        Ok(())
    }

    fn open_editor(
        &self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        label: &str,
        initial: &str,
    ) -> Result<String> {
        let editor = resolve_editor()?;
        let path = editor_temp_path(label);
        fs::write(&path, initial)
            .with_context(|| format!("unable to write temporary file {}", path.display()))?;

        suspend_terminal(terminal)?;
        let status = run_editor_command(&editor, &path);
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
        self.load_documents()?;
        if self.documents.is_empty() && self.document_page > 0 {
            self.document_page -= 1;
            self.load_documents()?;
        }
        if let Some(index) = selected_index {
            Self::select_index(&mut self.document_index, self.documents.len(), index);
        }
        if self.screen == Screen::DocumentView {
            self.prepare_document_view();
        }
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
            .find(|saved| saved.name == query.name)
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
            .find(|saved| saved.name == aggregation.name)
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
        }
    }

    fn go_back(&mut self) {
        match self.screen {
            Screen::Connections => {}
            Screen::Databases => self.screen = Screen::Connections,
            Screen::Collections => self.screen = Screen::Databases,
            Screen::Documents => self.screen = Screen::Collections,
            Screen::DocumentView => self.screen = Screen::Documents,
        }
    }

    fn go_forward(&mut self) -> Result<()> {
        match self.screen {
            Screen::Connections => {
                if self.connection_index.is_some() {
                    if let Err(error) = self.load_databases() {
                        self.message = Some(error.to_string());
                        return Ok(());
                    }
                    self.screen = Screen::Databases;
                }
            }
            Screen::Databases => {
                if self.database_index.is_some() {
                    if let Err(error) = self.load_collections() {
                        self.message = Some(error.to_string());
                        return Ok(());
                    }
                    self.screen = Screen::Collections;
                }
            }
            Screen::Collections => {
                if self.collection_index.is_some() {
                    self.document_page = 0;
                    if let Err(error) = self.load_documents() {
                        self.message = Some(error.to_string());
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
        }

        Ok(())
    }

    fn next_page(&mut self) -> Result<()> {
        if self.screen != Screen::Documents {
            return Ok(());
        }
        self.document_page += 1;
        if let Err(error) = self.load_documents() {
            self.message = Some(error.to_string());
            return Ok(());
        }
        if self.documents.is_empty() && self.document_page > 0 {
            self.document_page -= 1;
            let _ = self.load_documents();
            self.message = Some("no more documents".to_string());
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
        if let Err(error) = self.load_documents() {
            self.message = Some(error.to_string());
        }
        Ok(())
    }

    fn load_databases(&mut self) -> Result<()> {
        let connection = self
            .selected_connection()
            .ok_or_else(|| anyhow::anyhow!("select a connection"))?;
        let mut databases = self.runtime.block_on(
            self.executor
                .list_databases(&self.storage.config, Some(&connection.name)),
        )?;
        databases.sort();
        self.database_items = databases;
        self.database_index = if self.database_items.is_empty() {
            None
        } else {
            Some(0)
        };
        self.message = None;
        Ok(())
    }

    fn load_collections(&mut self) -> Result<()> {
        let connection = self
            .selected_connection()
            .ok_or_else(|| anyhow::anyhow!("select a connection"))?;
        let database = self
            .selected_database()
            .ok_or_else(|| anyhow::anyhow!("select a database"))?;
        let mut collections = self.runtime.block_on(self.executor.list_collections(
            &self.storage.config,
            Some(&connection.name),
            database,
        ))?;
        collections.sort();
        self.collection_items = collections;
        self.collection_index = if self.collection_items.is_empty() {
            None
        } else {
            Some(0)
        };
        self.message = None;
        Ok(())
    }

    fn load_documents(&mut self) -> Result<()> {
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
        let documents = self
            .runtime
            .block_on(self.executor.list_documents(&self.storage.config, &spec))?;
        self.documents = documents;
        self.document_index = if self.documents.is_empty() {
            None
        } else {
            Some(0)
        };
        self.message = None;
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
                Constraint::Length(2),
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
                    "Connections",
                    &items,
                    self.connection_index,
                );
            }
            Screen::Databases => {
                self.render_list(
                    frame,
                    layout[1],
                    "Databases",
                    &self.database_items,
                    self.database_index,
                );
            }
            Screen::Collections => {
                self.render_list(
                    frame,
                    layout[1],
                    "Collections",
                    &self.collection_items,
                    self.collection_index,
                );
            }
            Screen::Documents => {
                let items = self
                    .documents
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = format!("Documents (page {})", self.document_page + 1);
                self.render_list(frame, layout[1], &title, &items, self.document_index);
            }
            Screen::DocumentView => {
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
                frame.render_widget(body, layout[1]);
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
        title: &str,
        items: &[String],
        selected: Option<usize>,
    ) {
        let title = Line::from(Span::styled(title.to_string(), self.theme.title_style()));
        if items.is_empty() {
            let placeholder = Paragraph::new("no items")
                .style(self.theme.text_style())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(self.theme.border_style())
                        .title(title),
                );
            frame.render_widget(placeholder, area);
            return;
        }

        let items = items
            .iter()
            .map(|item| ListItem::new(item.clone()))
            .collect::<Vec<_>>();
        let list = List::new(items)
            .style(self.theme.text_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.border_style())
                    .title(title),
            )
            .highlight_style(self.theme.selection_style())
            .highlight_symbol("> ");
        let mut state = ListState::default();
        state.select(selected);
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
        };
        let connection = self
            .selected_connection()
            .map(|connection| connection.name.as_str())
            .unwrap_or("-");
        let database = self.selected_database().unwrap_or("-");
        let collection = self.selected_collection().unwrap_or("-");
        let path = format!("Conn: {connection}  Db: {database}  Coll: {collection}");

        vec![
            Line::from(Span::styled(title.to_string(), self.theme.title_style())),
            Line::from(Span::styled(path, self.theme.text_style())),
        ]
    }

    fn footer_lines(&self) -> Vec<Line<'static>> {
        let hint = self.hint_line();

        if let Some(confirm) = &self.confirm {
            vec![
                Line::from(Span::styled(
                    confirm.prompt.clone(),
                    self.theme.warning_style(),
                )),
                Line::from("y confirm  n cancel"),
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

fn hint_groups(screen: Screen) -> &'static [HintGroup] {
    match screen {
        Screen::Connections => CONNECTION_HINTS,
        Screen::Databases => DATABASE_HINTS,
        Screen::Collections => COLLECTION_HINTS,
        Screen::Documents => DOCUMENT_HINTS,
        Screen::DocumentView => DOCUMENT_VIEW_HINTS,
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
        KeyAction::ToggleHelp => &["?"],
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
    let name = config.theme.name.as_deref().unwrap_or("classic");
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

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir().context("unable to resolve current directory")?;
    let paths = ConfigPaths::resolve_from(&cwd)?;
    let storage = load_storage(&paths)?;
    let mut app = App::new(paths, storage)?;

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

fn run_editor_command(editor: &str, path: &Path) -> Result<std::process::ExitStatus> {
    if editor.split_whitespace().count() > 1 {
        Command::new("sh")
            .arg("-c")
            .arg(format!("{editor} \"{}\"", path.display()))
            .status()
            .context("failed to launch editor")
    } else {
        Command::new(editor)
            .arg(path)
            .status()
            .context("failed to launch editor")
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
        };
        let (theme, warning) = resolve_theme(&config);
        assert!(warning.is_none());
        assert_eq!(theme.accent, THEME_EMBER.accent);
    }
}
