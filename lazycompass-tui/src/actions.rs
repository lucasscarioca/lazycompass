use super::*;

impl App {
    pub(crate) fn block_if_read_only(&mut self, action: &str) -> bool {
        let guard = WriteGuard::new(self.read_only, self.storage.config.allow_pipeline_writes());
        if let Err(error) = guard.ensure_write_allowed(action) {
            self.message = Some(error.to_string());
            return true;
        }
        false
    }

    pub(crate) fn request_delete_document(&mut self) -> Result<()> {
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

    pub(crate) fn insert_document(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
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

    pub(crate) fn edit_document(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
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

    pub(crate) fn save_query(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
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

    pub(crate) fn select_query_save_scope(
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

    pub(crate) fn save_aggregation(
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

    pub(crate) fn select_aggregation_save_scope(
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

    pub(crate) fn run_saved_query(&mut self) -> Result<()> {
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

    pub(crate) fn run_saved_aggregation(&mut self) -> Result<()> {
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

    pub(crate) fn start_add_connection(&mut self) -> Result<()> {
        if self.screen != Screen::Connections {
            return Ok(());
        }
        self.add_connection_scope_index = Some(0);
        self.screen = Screen::AddConnectionScopeSelect;
        Ok(())
    }

    pub(crate) fn select_connection_scope(
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

    pub(crate) fn perform_add_connection_editor_action(
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

    pub(crate) fn ensure_editor_command(
        &mut self,
        action: PendingEditorAction,
    ) -> Result<Option<String>> {
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

    pub(crate) fn perform_editor_action(
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

    pub(crate) fn insert_document_with_context(
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

    pub(crate) fn edit_document_with_context(
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

    pub(crate) fn save_query_with_template(
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

    pub(crate) fn save_aggregation_with_template(
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

    pub(crate) fn open_editor(
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

    pub(crate) fn reload_documents_after_change(&mut self) -> Result<()> {
        if !matches!(self.screen, Screen::Documents | Screen::DocumentView) {
            return Ok(());
        }
        let selected_index = self.document_index;
        self.start_load_documents(selected_index, DocumentLoadReason::Refresh)?;
        Ok(())
    }

    pub(crate) fn selected_context(&self) -> Result<(String, String, String)> {
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

    pub(crate) fn selected_document(&self) -> Result<&Document> {
        let index = self
            .document_index
            .ok_or_else(|| anyhow::anyhow!("select a document"))?;
        self.documents
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("select a document"))
    }

    pub(crate) fn upsert_query(&mut self, query: SavedQuery) {
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

    pub(crate) fn upsert_aggregation(&mut self, aggregation: SavedAggregation) {
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
    pub(crate) fn clear_applied_documents(&mut self) -> Result<()> {
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
