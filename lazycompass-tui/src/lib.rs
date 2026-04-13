use anyhow::{Context, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use lazycompass_core::{
    Config, ConnectionSpec, OutputFormat, SavedAggregation, SavedQuery, SavedScope, WriteGuard,
    redact_sensitive_text,
};
use lazycompass_mongo::{
    Bson, Document, DocumentDeleteSpec, DocumentInsertSpec, DocumentListSpec, DocumentReplaceSpec,
    MongoExecutor, parse_json_document,
};
use lazycompass_output::{
    ExportNameSource, render_documents, suggested_export_filename, write_rendered_output,
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
mod clipboard;
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
    create_secure_editor_temp_file, is_editor_cancelled, resolve_editor, run_editor_command,
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
    Indexes,
    IndexView,
    Documents,
    DocumentView,
    ExportFormatSelect,
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
struct PathPromptState {
    prompt: String,
    input: String,
    rendered: String,
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
    OverwriteExport {
        path: PathBuf,
        rendered: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuickQueryField {
    Filter,
    Sort,
    Limit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuickQueryModalState {
    filter: String,
    projection: Option<String>,
    sort: String,
    limit: String,
    focus: QuickQueryField,
    filter_cursor: usize,
    sort_cursor: usize,
    limit_cursor: usize,
}

impl QuickQueryModalState {
    fn from_draft(draft: Option<&InlineQueryDraft>) -> Self {
        let payload = draft.and_then(|draft| draft.parsed.as_ref());
        let filter = payload
            .and_then(|payload| payload.filter.clone())
            .unwrap_or_else(|| "{}".to_string());
        let projection = payload.and_then(|payload| payload.projection.clone());
        let sort = payload
            .and_then(|payload| payload.sort.clone())
            .unwrap_or_else(|| r#"{"_id": -1}"#.to_string());
        let limit = payload
            .and_then(|payload| payload.limit.map(|limit| limit.to_string()))
            .unwrap_or_else(|| "20".to_string());
        let filter_cursor = filter.len();
        let sort_cursor = sort.len();
        let limit_cursor = limit.len();
        Self {
            filter,
            projection,
            sort,
            limit,
            focus: QuickQueryField::Filter,
            filter_cursor,
            sort_cursor,
            limit_cursor,
        }
    }

    fn focus_next(&mut self) {
        self.focus = match self.focus {
            QuickQueryField::Filter => QuickQueryField::Sort,
            QuickQueryField::Sort => QuickQueryField::Limit,
            QuickQueryField::Limit => QuickQueryField::Filter,
        };
    }

    fn focus_prev(&mut self) {
        self.focus = match self.focus {
            QuickQueryField::Filter => QuickQueryField::Limit,
            QuickQueryField::Sort => QuickQueryField::Filter,
            QuickQueryField::Limit => QuickQueryField::Sort,
        };
    }

    fn field_text(&self, field: QuickQueryField) -> &str {
        match field {
            QuickQueryField::Filter => &self.filter,
            QuickQueryField::Sort => &self.sort,
            QuickQueryField::Limit => &self.limit,
        }
    }

    fn field_text_mut(&mut self, field: QuickQueryField) -> &mut String {
        match field {
            QuickQueryField::Filter => &mut self.filter,
            QuickQueryField::Sort => &mut self.sort,
            QuickQueryField::Limit => &mut self.limit,
        }
    }

    fn cursor_mut(&mut self, field: QuickQueryField) -> &mut usize {
        match field {
            QuickQueryField::Filter => &mut self.filter_cursor,
            QuickQueryField::Sort => &mut self.sort_cursor,
            QuickQueryField::Limit => &mut self.limit_cursor,
        }
    }

    fn cursor(&self, field: QuickQueryField) -> usize {
        match field {
            QuickQueryField::Filter => self.filter_cursor,
            QuickQueryField::Sort => self.sort_cursor,
            QuickQueryField::Limit => self.limit_cursor,
        }
    }

    fn move_left(&mut self) {
        let field = self.focus;
        let cursor = self.cursor(field);
        let text = self.field_text(field);
        let new_cursor = prev_char_boundary(text, cursor);
        *self.cursor_mut(field) = new_cursor;
    }

    fn move_right(&mut self) {
        let field = self.focus;
        let cursor = self.cursor(field);
        let text = self.field_text(field);
        let new_cursor = next_char_boundary(text, cursor);
        *self.cursor_mut(field) = new_cursor;
    }

    fn move_home(&mut self) {
        let field = self.focus;
        *self.cursor_mut(field) = 0;
    }

    fn move_end(&mut self) {
        let field = self.focus;
        let len = self.field_text(field).len();
        *self.cursor_mut(field) = len;
    }

    fn insert_char(&mut self, ch: char) {
        let field = self.focus;
        let cursor = self.cursor(field);
        let text = self.field_text_mut(field);
        text.insert(cursor, ch);
        *self.cursor_mut(field) = cursor + ch.len_utf8();
    }

    fn backspace(&mut self) {
        let field = self.focus;
        let cursor = self.cursor(field);
        if cursor == 0 {
            return;
        }
        let text = self.field_text_mut(field);
        let start = prev_char_boundary(text, cursor);
        text.drain(start..cursor);
        *self.cursor_mut(field) = start;
    }

    fn delete(&mut self) {
        let field = self.focus;
        let cursor = self.cursor(field);
        let text = self.field_text_mut(field);
        if cursor >= text.len() {
            return;
        }
        let end = next_char_boundary(text, cursor);
        text.drain(cursor..end);
    }

    fn normalized_text(value: &str, default: &str) -> String {
        if value.trim().is_empty() {
            default.to_string()
        } else {
            value.trim().to_string()
        }
    }

    fn rendered_contents(&self) -> String {
        let filter = Self::normalized_text(&self.filter, "{}");
        let sort = Self::normalized_text(&self.sort, r#"{"_id": -1}"#);
        let limit = Self::normalized_text(&self.limit, "20");
        let mut lines = vec![format!("  \"filter\": {filter},")];
        if let Some(projection) = &self.projection {
            if !projection.trim().is_empty() {
                lines.push(format!("  \"projection\": {projection},"));
            }
        }
        lines.push(format!("  \"sort\": {sort},"));
        lines.push(format!("  \"limit\": {limit}"));
        format!("{{\n{}\n}}", lines.join("\n"))
    }

    fn build_payload(&self) -> Result<InlineQueryPayload> {
        parse_inline_query_payload(&self.rendered_contents())
    }
}

fn prev_char_boundary(text: &str, index: usize) -> usize {
    if index == 0 {
        return 0;
    }
    text[..index]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn next_char_boundary(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    text[index..]
        .chars()
        .next()
        .map(|ch| index + ch.len_utf8())
        .unwrap_or(text.len())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportAction {
    File,
    Clipboard,
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
    Indexes {
        id: u64,
        result: Result<Vec<Document>>,
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

struct ExportTarget {
    documents: Vec<Document>,
    source: ExportNameSource,
    single_document: bool,
}

struct App {
    paths: ConfigPaths,
    storage: StorageSnapshot,
    executor: MongoExecutor,
    runtime: Runtime,
    theme: Theme,
    write_enabled: bool,
    allow_pipeline_writes: bool,
    screen: Screen,
    connection_index: Option<usize>,
    database_items: Vec<String>,
    database_index: Option<usize>,
    collection_items: Vec<String>,
    collection_index: Option<usize>,
    indexes: Vec<Document>,
    index_index: Option<usize>,
    index_lines: Vec<String>,
    index_scroll: u16,
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
    path_prompt: Option<PathPromptState>,
    editor_command: Option<String>,
    warnings: VecDeque<String>,
    load_tx: Sender<LoadResult>,
    load_rx: Receiver<LoadResult>,
    next_load_id: u64,
    database_load_id: Option<u64>,
    collection_load_id: Option<u64>,
    index_load_id: Option<u64>,
    document_load_id: Option<u64>,
    saved_query_load_id: Option<u64>,
    saved_agg_load_id: Option<u64>,
    inline_query_load_id: Option<u64>,
    inline_agg_load_id: Option<u64>,
    database_state: LoadState,
    collection_state: LoadState,
    index_state: LoadState,
    document_state: LoadState,
    saved_query_state: LoadState,
    saved_agg_state: LoadState,
    document_pending_index: Option<usize>,
    document_load_reason: DocumentLoadReason,
    document_result_source: DocumentResultSource,
    saved_query_index: Option<usize>,
    saved_agg_index: Option<usize>,
    export_action: Option<ExportAction>,
    export_format_index: Option<usize>,
    export_return_screen: Option<Screen>,
    save_query_scope_index: Option<usize>,
    save_agg_scope_index: Option<usize>,
    add_connection_scope_index: Option<usize>,
    inline_query_draft: Option<InlineQueryDraft>,
    inline_aggregation_draft: Option<InlineAggregationDraft>,
    active_inline_draft: Option<InlineDraftKind>,
    quick_query_modal: Option<QuickQueryModalState>,
    query_save_source: QuerySaveSource,
    aggregation_save_source: AggregationSaveSource,
}

pub fn run(config: Config, write_enabled: bool, allow_pipeline_writes: bool) -> Result<()> {
    let cwd = std::env::current_dir().context("unable to resolve current directory")?;
    let paths = ConfigPaths::resolve_from(&cwd)?;
    let storage = load_storage_with_config(&paths, config)?;
    let mut app = App::new(paths, storage, write_enabled, allow_pipeline_writes)?;

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
        Self::new(
            ConfigPaths {
                global_root: root.join("global"),
                repo_root: None,
            },
            storage,
            false,
            false,
        )
        .expect("build app")
    }
}
