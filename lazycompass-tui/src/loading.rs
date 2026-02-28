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
                .await;
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
            let result = executor.execute_query(&config, &spec).await;
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
            let result = executor.execute_aggregation(&config, &spec).await;
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
                .await;
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
            let result = executor.list_documents(&config, &spec).await;
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

#[cfg(test)]
mod tests {
    use lazycompass_core::{Config, ConnectionSpec};
    use lazycompass_storage::StorageSnapshot;

    use super::*;

    fn app_with_context() -> App {
        let storage = StorageSnapshot {
            config: Config {
                connections: vec![ConnectionSpec {
                    name: "local".to_string(),
                    uri: "mongodb://localhost:27017".to_string(),
                    default_database: Some("app".to_string()),
                }],
                read_only: Some(false),
                ..Config::default()
            },
            queries: Vec::new(),
            aggregations: Vec::new(),
            warnings: Vec::new(),
        };
        let mut app = App::test_app_with_storage(storage);
        app.connection_index = Some(0);
        app.database_items = vec!["app".to_string()];
        app.database_index = Some(0);
        app.collection_items = vec!["users".to_string()];
        app.collection_index = Some(0);
        app
    }

    #[test]
    fn start_load_documents_resets_document_state() {
        let mut app = app_with_context();
        app.documents = vec![Document::from_iter([("_id".to_string(), Bson::Int32(1))])];
        app.document_index = Some(0);
        app.document_lines = vec!["old".to_string()];
        app.document_scroll = 4;
        app.message = Some("stale".to_string());

        app.start_load_documents(Some(2), DocumentLoadReason::Refresh)
            .expect("start load");

        assert!(app.document_load_id.is_some());
        assert!(matches!(app.document_state, LoadState::Loading));
        assert!(app.documents.is_empty());
        assert_eq!(app.document_index, None);
        assert!(app.document_lines.is_empty());
        assert_eq!(app.document_scroll, 0);
        assert_eq!(app.document_pending_index, Some(2));
        assert_eq!(app.message, None);
    }

    #[test]
    fn start_load_collections_resets_collection_state() {
        let mut app = app_with_context();
        app.collection_items = vec!["old".to_string()];
        app.collection_index = Some(0);
        app.message = Some("stale".to_string());

        app.start_load_collections().expect("start collections");

        assert!(app.collection_load_id.is_some());
        assert!(matches!(app.collection_state, LoadState::Loading));
        assert!(app.collection_items.is_empty());
        assert_eq!(app.collection_index, None);
        assert_eq!(app.message, None);
    }
}
