use super::*;

impl App {
    pub(crate) fn go_top(&mut self) {
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

    pub(crate) fn go_bottom(&mut self) {
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

    pub(crate) fn move_up(&mut self) {
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

    pub(crate) fn move_down(&mut self) {
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

    pub(crate) fn go_back(&mut self) {
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

    pub(crate) fn go_forward(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
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

    pub(crate) fn next_page(&mut self) -> Result<()> {
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

    pub(crate) fn previous_page(&mut self) -> Result<()> {
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
    pub(crate) fn move_selection(selected: &mut Option<usize>, len: usize, delta: i32) {
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

    pub(crate) fn select_index(selected: &mut Option<usize>, len: usize, index: usize) {
        if len == 0 {
            *selected = None;
        } else {
            *selected = Some(index.min(len - 1));
        }
    }

    pub(crate) fn select_last(selected: &mut Option<usize>, len: usize) {
        if len == 0 {
            *selected = None;
        } else {
            *selected = Some(len - 1);
        }
    }

    pub(crate) fn scroll_document(&mut self, delta: i16) {
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

    pub(crate) fn max_document_scroll(&self) -> u16 {
        let max = self.document_lines.len().saturating_sub(1);
        max.min(u16::MAX as usize) as u16
    }
}
