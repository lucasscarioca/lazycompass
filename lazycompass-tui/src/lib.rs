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

mod actions;
mod app_loop;
mod editor;
mod errors;
mod formatting;
mod keymap;
mod loading;
mod navigation;
mod payloads;
mod render;
mod terminal;
mod theme;

use editor::{
    editor_temp_path, is_editor_cancelled, resolve_editor, run_editor_command,
    write_editor_temp_file,
};
use errors::format_error;
use formatting::{connection_label, document_id, document_preview, format_bson, format_document};
use keymap::{KeyAction, action_for_key, hint_groups, keys_for_actions};
use payloads::{
    default_saved_id, parse_aggregation_payload_input, parse_aggregation_save_input,
    parse_inline_aggregation_payload, parse_inline_query_payload, parse_query_payload_input,
    parse_query_save_input, render_aggregation_payload_template, render_aggregation_save_template,
    render_inline_aggregation_template, render_inline_query_template,
    render_query_payload_template, render_query_save_template, saved_scope_label,
};
use terminal::{restore_terminal, resume_terminal, setup_terminal, suspend_terminal};
use theme::{Theme, resolve_theme};

const PAGE_SIZE: u64 = 20;
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct InlineQueryPayload {
    filter: Option<String>,
    projection: Option<String>,
    sort: Option<String>,
    limit: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InlineAggregationPayload {
    pipeline: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InlineQueryDraft {
    raw: String,
    parsed: Option<InlineQueryPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InlineAggregationDraft {
    raw: String,
    parsed: Option<InlineAggregationPayload>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineDraftKind {
    Query,
    Aggregation,
}

#[derive(Debug, Clone)]
enum QuerySaveSource {
    EmptyTemplate,
    InlineDraft(InlineQueryPayload),
}

#[derive(Debug, Clone)]
enum AggregationSaveSource {
    EmptyTemplate,
    InlineDraft(InlineAggregationPayload),
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
    RunInlineQuery,
    RunInlineAggregation,
    SaveInlineQuery {
        scope: SavedScope,
        draft: InlineQueryPayload,
    },
    SaveInlineAggregation {
        scope: SavedScope,
        draft: InlineAggregationPayload,
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
        result: Result<Vec<String>>,
    },
    Collections {
        id: u64,
        result: Result<Vec<String>>,
    },
    Documents {
        id: u64,
        result: Result<Vec<Document>>,
    },
    SavedQuery {
        id: u64,
        name: String,
        result: Result<Vec<Document>>,
    },
    SavedAggregation {
        id: u64,
        name: String,
        result: Result<Vec<Document>>,
    },
    InlineQuery {
        id: u64,
        result: Result<Vec<Document>>,
    },
    InlineAggregation {
        id: u64,
        result: Result<Vec<Document>>,
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
    InlineQuery,
    InlineAggregation,
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
    inline_query_load_id: Option<u64>,
    inline_agg_load_id: Option<u64>,
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
    inline_query_draft: Option<InlineQueryDraft>,
    inline_aggregation_draft: Option<InlineAggregationDraft>,
    active_inline_draft: Option<InlineDraftKind>,
    query_save_source: QuerySaveSource,
    aggregation_save_source: AggregationSaveSource,
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

#[cfg(test)]
impl App {
    pub(crate) fn test_app() -> Self {
        Self::test_app_with_storage(StorageSnapshot {
            config: Config::default(),
            queries: Vec::new(),
            aggregations: Vec::new(),
            warnings: Vec::new(),
        })
    }

    pub(crate) fn test_app_with_storage(storage: StorageSnapshot) -> Self {
        let root = std::env::temp_dir().join(format!(
            "lazycompass_tui_test_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        let read_only = storage.config.read_only();
        Self::new(
            ConfigPaths {
                global_root: root.join("global"),
                repo_root: None,
            },
            storage,
            read_only,
        )
        .expect("build app")
    }
}
