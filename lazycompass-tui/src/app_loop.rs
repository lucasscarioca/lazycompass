use super::*;

impl App {
    pub(crate) fn new(
        paths: ConfigPaths,
        storage: StorageSnapshot,
        read_only: bool,
    ) -> Result<Self> {
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

    pub(crate) fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
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

    pub(crate) fn drain_load_results(&mut self) {
        loop {
            match self.load_rx.try_recv() {
                Ok(result) => self.apply_load_result(result),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    pub(crate) fn apply_load_result(&mut self, result: LoadResult) {
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

    pub(crate) fn handle_key(
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

    pub(crate) fn resolve_action(&mut self, key: KeyEvent) -> Option<KeyAction> {
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

    pub(crate) fn apply_action(
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

    pub(crate) fn handle_confirm_key(
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

    pub(crate) fn handle_editor_prompt_key(
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

    pub(crate) fn perform_confirm_action(&mut self, action: ConfirmAction) -> Result<()> {
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

    pub(crate) fn set_error_message(&mut self, error: &anyhow::Error) {
        let message = format_error_message(&error.to_string(), is_network_error(error));
        self.message = Some(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use lazycompass_core::Config;
    use lazycompass_storage::StorageSnapshot;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lazycompass_tui_loop_{prefix}_{nonce}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn test_app() -> App {
        let root = temp_dir("app");
        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: None,
        };
        let storage = StorageSnapshot {
            config: Config::default(),
            queries: Vec::new(),
            aggregations: Vec::new(),
            warnings: Vec::new(),
        };
        App::new(paths, storage, false).expect("build app")
    }

    #[test]
    fn resolve_action_requires_double_g_for_top_navigation() {
        let mut app = test_app();
        assert_eq!(
            app.resolve_action(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            app.resolve_action(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)),
            Some(KeyAction::GoTop)
        );
    }

    #[test]
    fn apply_load_result_ignores_stale_database_payloads() {
        let mut app = test_app();
        app.database_load_id = Some(2);
        app.database_state = LoadState::Loading;
        app.database_items = vec!["kept".to_string()];

        app.apply_load_result(LoadResult::Databases {
            id: 1,
            result: Ok(vec!["new".to_string()]),
        });

        assert_eq!(app.database_load_id, Some(2));
        assert_eq!(app.database_items, vec!["kept".to_string()]);
        assert!(matches!(app.database_state, LoadState::Loading));
    }
}
