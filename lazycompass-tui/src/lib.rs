use anyhow::{Context, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use lazycompass_core::{ConnectionSpec, SavedAggregation, SavedQuery};
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
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use std::collections::VecDeque;
use std::fs;
use std::io::{Stdout, stdout};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;

const PAGE_SIZE: u64 = 20;

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
    message: Option<String>,
    confirm: Option<ConfirmState>,
    warnings: VecDeque<String>,
}

impl App {
    fn new(paths: ConfigPaths, storage: StorageSnapshot) -> Result<Self> {
        let runtime = Runtime::new().context("unable to start async runtime")?;
        let warnings = VecDeque::from(storage.warnings.clone());
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

        if key.code == KeyCode::Char('q') {
            return Ok(true);
        }

        match key.code {
            KeyCode::Char('g') if key.modifiers == KeyModifiers::NONE => {
                if self.last_g {
                    self.last_g = false;
                    self.go_top();
                } else {
                    self.last_g = true;
                }
                return Ok(false);
            }
            _ => {
                self.last_g = false;
            }
        }

        match key.code {
            KeyCode::Char('G') => self.go_bottom(),
            KeyCode::Char('j') => self.move_down(),
            KeyCode::Char('k') => self.move_up(),
            KeyCode::Char('h') => self.go_back(),
            KeyCode::Char('l') | KeyCode::Enter => self.go_forward()?,
            KeyCode::PageDown => self.next_page()?,
            KeyCode::PageUp => self.previous_page()?,
            KeyCode::Char('i') => self.insert_document(terminal)?,
            KeyCode::Char('e') => self.edit_document(terminal)?,
            KeyCode::Char('d') => self.request_delete_document()?,
            KeyCode::Char('Q') => self.save_query(terminal)?,
            KeyCode::Char('A') => self.save_aggregation(terminal)?,
            _ => {}
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

        let header =
            Paragraph::new(self.header_lines()).block(Block::default().borders(Borders::BOTTOM));
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
                    .block(Block::default().borders(Borders::ALL).title("Document"))
                    .wrap(Wrap { trim: false })
                    .scroll((self.document_scroll, 0));
                frame.render_widget(body, layout[1]);
            }
        }

        let footer =
            Paragraph::new(self.footer_lines()).block(Block::default().borders(Borders::TOP));
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
        if items.is_empty() {
            let placeholder = Paragraph::new("no items")
                .block(Block::default().borders(Borders::ALL).title(title));
            frame.render_widget(placeholder, area);
            return;
        }

        let items = items
            .iter()
            .map(|item| ListItem::new(item.clone()))
            .collect::<Vec<_>>();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        let mut state = ListState::default();
        state.select(selected);
        frame.render_stateful_widget(list, area, &mut state);
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
            Line::from(Span::styled(
                title.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(path),
        ]
    }

    fn footer_lines(&self) -> Vec<Line<'static>> {
        let hint = match self.screen {
            Screen::Connections => "j/k move  l enter  gg/G top/bottom  q quit",
            Screen::Databases => "j/k move  l enter  h back  gg/G top/bottom  q quit",
            Screen::Collections => "j/k move  l enter  h back  gg/G top/bottom  q quit",
            Screen::Documents => {
                "j/k move  l view  h back  i insert  e edit  d delete  Q save query  A save agg  pgup/pgdn page  gg/G top/bottom  q quit"
            }
            Screen::DocumentView => "j/k scroll  h back  e edit  d delete  gg/G top/bottom  q quit",
        };

        if let Some(confirm) = &self.confirm {
            vec![
                Line::from(Span::styled(
                    confirm.prompt.clone(),
                    Style::default().fg(Color::Yellow),
                )),
                Line::from("y confirm  n cancel"),
            ]
        } else if let Some(message) = &self.message {
            vec![
                Line::from(Span::styled(
                    message.clone(),
                    Style::default().fg(Color::Red),
                )),
                Line::from(hint.to_string()),
            ]
        } else if let Some(warning) = &self.warnings.front() {
            vec![
                Line::from(Span::styled(
                    format!("warning: {warning}"),
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(hint.to_string()),
            ]
        } else {
            vec![Line::from(hint.to_string()), Line::from(" ")]
        }
    }
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
