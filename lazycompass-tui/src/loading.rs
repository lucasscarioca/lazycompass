use super::*;

impl App {
    pub(crate) fn next_load_id(&mut self) -> u64 {
        self.next_load_id = self.next_load_id.saturating_add(1);
        self.next_load_id
    }

    pub(crate) fn start_load_databases(&mut self) -> Result<()> {
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

    pub(crate) fn start_execute_saved_query(&mut self) -> Result<()> {
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

    pub(crate) fn start_execute_saved_aggregation(&mut self) -> Result<()> {
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

    pub(crate) fn start_load_collections(&mut self) -> Result<()> {
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

    pub(crate) fn start_load_documents(
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

    pub(crate) fn prepare_document_view(&mut self) {
        let Some(index) = self.document_index else {
            return;
        };
        let Some(document) = self.documents.get(index) else {
            return;
        };
        self.document_lines = format_document(document);
        self.document_scroll = 0;
    }

    pub(crate) fn selected_connection(&self) -> Option<&ConnectionSpec> {
        self.connection_index
            .and_then(|index| self.storage.config.connections.get(index))
    }

    pub(crate) fn selected_database(&self) -> Option<&str> {
        self.database_index
            .and_then(|index| self.database_items.get(index))
            .map(String::as_str)
    }

    pub(crate) fn selected_collection(&self) -> Option<&str> {
        self.collection_index
            .and_then(|index| self.collection_items.get(index))
            .map(String::as_str)
    }
}
