use super::*;
pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainPaneLayout {
    Single,
    Double,
    Triple,
}

impl App {
    pub(crate) fn draw(&mut self, frame: &mut ratatui::Frame) {
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
            Screen::Connections => self.render_connections_screen(frame, layout[1]),
            Screen::Databases => self.render_databases_screen(frame, layout[1]),
            Screen::Collections => self.render_collections_screen(frame, layout[1]),
            Screen::Indexes => self.render_indexes_screen(frame, layout[1]),
            Screen::IndexView => self.render_index_view_screen(frame, layout[1]),
            Screen::Documents => self.render_documents_screen(frame, layout[1]),
            Screen::DocumentView => self.render_document_view_screen(frame, layout[1]),
            Screen::ExportFormatSelect => self.render_export_format_select_screen(frame, layout[1]),
            Screen::SavedQuerySelect => self.render_saved_query_select_screen(frame, layout[1]),
            Screen::SavedAggregationSelect => {
                self.render_saved_aggregation_select_screen(frame, layout[1])
            }
            Screen::SaveQueryScopeSelect => self.render_save_query_scope_screen(frame, layout[1]),
            Screen::SaveAggregationScopeSelect => {
                self.render_save_aggregation_scope_screen(frame, layout[1])
            }
            Screen::AddConnectionScopeSelect => {
                self.render_add_connection_scope_screen(frame, layout[1])
            }
        }

        if self.help_visible {
            self.render_help(frame, layout[1]);
        }
        if self.quick_query_modal.is_some() {
            self.render_quick_query_modal(frame, layout[1]);
            if let Some((x, y)) = self.quick_query_cursor(layout[1]) {
                frame.set_cursor_position((x, y));
            }
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

    fn hierarchy_layout(&self, screen: Screen, width: u16) -> MainPaneLayout {
        match screen {
            Screen::Connections => MainPaneLayout::Single,
            Screen::Databases => {
                if width >= 60 {
                    MainPaneLayout::Double
                } else {
                    MainPaneLayout::Single
                }
            }
            Screen::Collections
            | Screen::Indexes
            | Screen::IndexView
            | Screen::Documents
            | Screen::DocumentView
            | Screen::SavedQuerySelect
            | Screen::SavedAggregationSelect => {
                if width >= 90 {
                    MainPaneLayout::Triple
                } else if width >= 60 {
                    MainPaneLayout::Double
                } else {
                    MainPaneLayout::Single
                }
            }
            Screen::ExportFormatSelect
            | Screen::SaveQueryScopeSelect
            | Screen::SaveAggregationScopeSelect
            | Screen::AddConnectionScopeSelect => MainPaneLayout::Single,
        }
    }

    fn render_connections_screen(&self, frame: &mut ratatui::Frame, area: Rect) {
        let items = self
            .storage
            .config
            .connections
            .iter()
            .map(connection_label)
            .collect::<Vec<_>>();
        self.render_list(
            frame,
            area,
            ListView {
                title: "Connections",
                items: &items,
                selected: self.connection_index,
                load_state: &LoadState::Idle,
                loading_label: "loading connections...",
            },
        );
    }

    fn render_databases_screen(&self, frame: &mut ratatui::Frame, area: Rect) {
        let mode = self.hierarchy_layout(Screen::Databases, area.width);
        match mode {
            MainPaneLayout::Single => {
                self.render_list(
                    frame,
                    area,
                    ListView {
                        title: "Databases",
                        items: &self.database_items,
                        selected: self.database_index,
                        load_state: &self.database_state,
                        loading_label: "loading databases...",
                    },
                );
            }
            MainPaneLayout::Double | MainPaneLayout::Triple => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(area);
                let connections = self
                    .storage
                    .config
                    .connections
                    .iter()
                    .map(connection_label)
                    .collect::<Vec<_>>();
                self.render_list_with_focus(
                    frame,
                    panes[0],
                    ListView {
                        title: "Connections",
                        items: &connections,
                        selected: self.connection_index,
                        load_state: &LoadState::Idle,
                        loading_label: "loading connections...",
                    },
                    false,
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
        }
    }

    fn render_collections_screen(&self, frame: &mut ratatui::Frame, area: Rect) {
        match self.hierarchy_layout(Screen::Collections, area.width) {
            MainPaneLayout::Single => {
                self.render_list(
                    frame,
                    area,
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                );
            }
            MainPaneLayout::Double => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(area);
                self.render_list_with_focus(
                    frame,
                    panes[0],
                    ListView {
                        title: "Databases",
                        items: &self.database_items,
                        selected: self.database_index,
                        load_state: &self.database_state,
                        loading_label: "loading databases...",
                    },
                    false,
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
            MainPaneLayout::Triple => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(10),
                        Constraint::Percentage(20),
                        Constraint::Percentage(70),
                    ])
                    .split(area);
                let connections = self
                    .storage
                    .config
                    .connections
                    .iter()
                    .map(connection_label)
                    .collect::<Vec<_>>();
                self.render_list_with_focus(
                    frame,
                    panes[0],
                    ListView {
                        title: "Connections",
                        items: &connections,
                        selected: self.connection_index,
                        load_state: &LoadState::Idle,
                        loading_label: "loading connections...",
                    },
                    false,
                );
                self.render_list_with_focus(
                    frame,
                    panes[1],
                    ListView {
                        title: "Databases",
                        items: &self.database_items,
                        selected: self.database_index,
                        load_state: &self.database_state,
                        loading_label: "loading databases...",
                    },
                    false,
                );
                self.render_list(
                    frame,
                    panes[2],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                );
            }
        }
    }

    fn render_indexes_screen(&self, frame: &mut ratatui::Frame, area: Rect) {
        match self.hierarchy_layout(Screen::Indexes, area.width) {
            MainPaneLayout::Single => {
                let items = self
                    .indexes
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = self.indexes_list_title();
                self.render_list(
                    frame,
                    area,
                    ListView {
                        title: &title,
                        items: &items,
                        selected: self.index_index,
                        load_state: &self.index_state,
                        loading_label: "loading indexes...",
                    },
                );
            }
            MainPaneLayout::Double => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(area);
                self.render_list_with_focus(
                    frame,
                    panes[0],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                    false,
                );
                let items = self
                    .indexes
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = self.indexes_list_title();
                self.render_list(
                    frame,
                    panes[1],
                    ListView {
                        title: &title,
                        items: &items,
                        selected: self.index_index,
                        load_state: &self.index_state,
                        loading_label: "loading indexes...",
                    },
                );
            }
            MainPaneLayout::Triple => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(10),
                        Constraint::Percentage(20),
                        Constraint::Percentage(70),
                    ])
                    .split(area);
                self.render_list_with_focus(
                    frame,
                    panes[0],
                    ListView {
                        title: "Databases",
                        items: &self.database_items,
                        selected: self.database_index,
                        load_state: &self.database_state,
                        loading_label: "loading databases...",
                    },
                    false,
                );
                self.render_list_with_focus(
                    frame,
                    panes[1],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                    false,
                );
                let items = self
                    .indexes
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = self.indexes_list_title();
                self.render_list(
                    frame,
                    panes[2],
                    ListView {
                        title: &title,
                        items: &items,
                        selected: self.index_index,
                        load_state: &self.index_state,
                        loading_label: "loading indexes...",
                    },
                );
            }
        }
    }

    fn render_index_view_screen(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        match self.hierarchy_layout(Screen::IndexView, area.width) {
            MainPaneLayout::Single => {
                let max_scroll = self.max_index_scroll();
                if self.index_scroll > max_scroll {
                    self.index_scroll = max_scroll;
                }
                let lines = self
                    .index_lines
                    .iter()
                    .map(|line| Line::from(line.clone()))
                    .collect::<Vec<_>>();
                let body = Paragraph::new(lines)
                    .style(self.theme.text_style())
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(self.theme.border_style())
                            .title(Line::from(Span::styled("Index", self.theme.title_style()))),
                    )
                    .wrap(Wrap { trim: false })
                    .scroll((self.index_scroll, 0));
                frame.render_widget(body, area);
            }
            MainPaneLayout::Double => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(area);
                let items = self
                    .indexes
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = self.indexes_list_title();
                self.render_list_with_focus(
                    frame,
                    panes[0],
                    ListView {
                        title: &title,
                        items: &items,
                        selected: self.index_index,
                        load_state: &self.index_state,
                        loading_label: "loading indexes...",
                    },
                    false,
                );
                let max_scroll = self.max_index_scroll();
                if self.index_scroll > max_scroll {
                    self.index_scroll = max_scroll;
                }
                let lines = self
                    .index_lines
                    .iter()
                    .map(|line| Line::from(line.clone()))
                    .collect::<Vec<_>>();
                let body = Paragraph::new(lines)
                    .style(self.theme.text_style())
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(self.theme.border_style())
                            .title(Line::from(Span::styled("Index", self.theme.title_style()))),
                    )
                    .wrap(Wrap { trim: false })
                    .scroll((self.index_scroll, 0));
                frame.render_widget(body, panes[1]);
            }
            MainPaneLayout::Triple => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(10),
                        Constraint::Percentage(20),
                        Constraint::Percentage(70),
                    ])
                    .split(area);
                self.render_list_with_focus(
                    frame,
                    panes[0],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                    false,
                );
                let items = self
                    .indexes
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = self.indexes_list_title();
                self.render_list_with_focus(
                    frame,
                    panes[1],
                    ListView {
                        title: &title,
                        items: &items,
                        selected: self.index_index,
                        load_state: &self.index_state,
                        loading_label: "loading indexes...",
                    },
                    false,
                );
                let max_scroll = self.max_index_scroll();
                if self.index_scroll > max_scroll {
                    self.index_scroll = max_scroll;
                }
                let lines = self
                    .index_lines
                    .iter()
                    .map(|line| Line::from(line.clone()))
                    .collect::<Vec<_>>();
                let body = Paragraph::new(lines)
                    .style(self.theme.text_style())
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(self.theme.border_style())
                            .title(Line::from(Span::styled("Index", self.theme.title_style()))),
                    )
                    .wrap(Wrap { trim: false })
                    .scroll((self.index_scroll, 0));
                frame.render_widget(body, panes[2]);
            }
        }
    }

    fn render_documents_screen(&self, frame: &mut ratatui::Frame, area: Rect) {
        match self.hierarchy_layout(Screen::Documents, area.width) {
            MainPaneLayout::Single => {
                let items = self
                    .documents
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = self.documents_list_title();
                self.render_list(
                    frame,
                    area,
                    ListView {
                        title: &title,
                        items: &items,
                        selected: self.document_index,
                        load_state: &self.document_state,
                        loading_label: "loading documents...",
                    },
                );
            }
            MainPaneLayout::Double => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(area);
                self.render_list_with_focus(
                    frame,
                    panes[0],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                    false,
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
            MainPaneLayout::Triple => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(10),
                        Constraint::Percentage(20),
                        Constraint::Percentage(70),
                    ])
                    .split(area);
                self.render_list_with_focus(
                    frame,
                    panes[0],
                    ListView {
                        title: "Databases",
                        items: &self.database_items,
                        selected: self.database_index,
                        load_state: &self.database_state,
                        loading_label: "loading databases...",
                    },
                    false,
                );
                self.render_list_with_focus(
                    frame,
                    panes[1],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                    false,
                );
                let items = self
                    .documents
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = self.documents_list_title();
                self.render_list(
                    frame,
                    panes[2],
                    ListView {
                        title: &title,
                        items: &items,
                        selected: self.document_index,
                        load_state: &self.document_state,
                        loading_label: "loading documents...",
                    },
                );
            }
        }
    }

    fn render_document_view_screen(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        match self.hierarchy_layout(Screen::DocumentView, area.width) {
            MainPaneLayout::Single => {
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
                frame.render_widget(body, area);
            }
            MainPaneLayout::Double => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(area);
                let items = self
                    .documents
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = self.documents_list_title();
                self.render_list_with_focus(
                    frame,
                    panes[0],
                    ListView {
                        title: &title,
                        items: &items,
                        selected: self.document_index,
                        load_state: &self.document_state,
                        loading_label: "loading documents...",
                    },
                    false,
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
            MainPaneLayout::Triple => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(10),
                        Constraint::Percentage(20),
                        Constraint::Percentage(70),
                    ])
                    .split(area);
                self.render_list_with_focus(
                    frame,
                    panes[0],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                    false,
                );
                let items = self
                    .documents
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = self.documents_list_title();
                self.render_list_with_focus(
                    frame,
                    panes[1],
                    ListView {
                        title: &title,
                        items: &items,
                        selected: self.document_index,
                        load_state: &self.document_state,
                        loading_label: "loading documents...",
                    },
                    false,
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
                frame.render_widget(body, panes[2]);
            }
        }
    }

    fn render_export_format_select_screen(&self, frame: &mut ratatui::Frame, area: Rect) {
        let items = vec![
            "JSON (pretty array)".to_string(),
            "CSV (one row per document)".to_string(),
            "Table (plain text)".to_string(),
        ];
        let title = match self.export_action {
            Some(ExportAction::Clipboard) => "Select Format to Copy",
            _ => "Select Format to Export",
        };
        self.render_list(
            frame,
            area,
            ListView {
                title,
                items: &items,
                selected: self.export_format_index,
                load_state: &LoadState::Idle,
                loading_label: "",
            },
        );
    }

    fn render_saved_query_select_screen(&self, frame: &mut ratatui::Frame, area: Rect) {
        let mode = self.hierarchy_layout(Screen::SavedQuerySelect, area.width);
        let (left_context, mid_context, selection_area) = match mode {
            MainPaneLayout::Single => (None, None, area),
            MainPaneLayout::Double => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(area);
                (Some(panes[0]), None, panes[1])
            }
            MainPaneLayout::Triple => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(10),
                        Constraint::Percentage(20),
                        Constraint::Percentage(70),
                    ])
                    .split(area);
                (Some(panes[0]), Some(panes[1]), panes[2])
            }
        };

        match mode {
            MainPaneLayout::Double => {
                if let Some(area) = left_context {
                    self.render_list_with_focus(
                        frame,
                        area,
                        ListView {
                            title: "Collections",
                            items: &self.collection_items,
                            selected: self.collection_index,
                            load_state: &self.collection_state,
                            loading_label: "loading collections...",
                        },
                        false,
                    );
                }
            }
            MainPaneLayout::Triple => {
                if let Some(area) = left_context {
                    self.render_list_with_focus(
                        frame,
                        area,
                        ListView {
                            title: "Databases",
                            items: &self.database_items,
                            selected: self.database_index,
                            load_state: &self.database_state,
                            loading_label: "loading databases...",
                        },
                        false,
                    );
                }
                if let Some(area) = mid_context {
                    self.render_list_with_focus(
                        frame,
                        area,
                        ListView {
                            title: "Collections",
                            items: &self.collection_items,
                            selected: self.collection_index,
                            load_state: &self.collection_state,
                            loading_label: "loading collections...",
                        },
                        false,
                    );
                }
            }
            MainPaneLayout::Single => {}
        }

        let right_panes = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(15), Constraint::Percentage(85)])
            .split(selection_area);
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

    fn render_saved_aggregation_select_screen(&self, frame: &mut ratatui::Frame, area: Rect) {
        let mode = self.hierarchy_layout(Screen::SavedAggregationSelect, area.width);
        let (left_context, mid_context, selection_area) = match mode {
            MainPaneLayout::Single => (None, None, area),
            MainPaneLayout::Double => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(area);
                (Some(panes[0]), None, panes[1])
            }
            MainPaneLayout::Triple => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(10),
                        Constraint::Percentage(20),
                        Constraint::Percentage(70),
                    ])
                    .split(area);
                (Some(panes[0]), Some(panes[1]), panes[2])
            }
        };

        match mode {
            MainPaneLayout::Double => {
                if let Some(area) = left_context {
                    self.render_list_with_focus(
                        frame,
                        area,
                        ListView {
                            title: "Collections",
                            items: &self.collection_items,
                            selected: self.collection_index,
                            load_state: &self.collection_state,
                            loading_label: "loading collections...",
                        },
                        false,
                    );
                }
            }
            MainPaneLayout::Triple => {
                if let Some(area) = left_context {
                    self.render_list_with_focus(
                        frame,
                        area,
                        ListView {
                            title: "Databases",
                            items: &self.database_items,
                            selected: self.database_index,
                            load_state: &self.database_state,
                            loading_label: "loading databases...",
                        },
                        false,
                    );
                }
                if let Some(area) = mid_context {
                    self.render_list_with_focus(
                        frame,
                        area,
                        ListView {
                            title: "Collections",
                            items: &self.collection_items,
                            selected: self.collection_index,
                            load_state: &self.collection_state,
                            loading_label: "loading collections...",
                        },
                        false,
                    );
                }
            }
            MainPaneLayout::Single => {}
        }

        let right_panes = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(15), Constraint::Percentage(85)])
            .split(selection_area);
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

    fn render_save_query_scope_screen(&self, frame: &mut ratatui::Frame, area: Rect) {
        let items = vec![
            "Shared (uses current db/collection when running)".to_string(),
            "Scoped (encode current db/collection in filename)".to_string(),
        ];
        self.render_list(
            frame,
            area,
            ListView {
                title: "Select Query Save Scope",
                items: &items,
                selected: self.save_query_scope_index,
                load_state: &LoadState::Idle,
                loading_label: "",
            },
        );
    }

    fn render_save_aggregation_scope_screen(&self, frame: &mut ratatui::Frame, area: Rect) {
        let items = vec![
            "Shared (uses current db/collection when running)".to_string(),
            "Scoped (encode current db/collection in filename)".to_string(),
        ];
        self.render_list(
            frame,
            area,
            ListView {
                title: "Select Aggregation Save Scope",
                items: &items,
                selected: self.save_agg_scope_index,
                load_state: &LoadState::Idle,
                loading_label: "",
            },
        );
    }

    fn render_add_connection_scope_screen(&self, frame: &mut ratatui::Frame, area: Rect) {
        let items = vec![
            "Session only (not persisted)".to_string(),
            "Save to repo config (.lazycompass/config.toml)".to_string(),
            "Save to global config (~/.config/lazycompass/config.toml)".to_string(),
        ];
        self.render_list(
            frame,
            area,
            ListView {
                title: "Select Persistence Scope for New Connection",
                items: &items,
                selected: self.add_connection_scope_index,
                load_state: &LoadState::Idle,
                loading_label: "",
            },
        );
    }

    pub(crate) fn render_list(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        view: ListView<'_>,
    ) {
        self.render_list_with_focus(frame, area, view, true);
    }

    pub(crate) fn render_list_with_focus(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        view: ListView<'_>,
        active: bool,
    ) {
        let title_style = if active {
            self.theme.title_style()
        } else {
            self.theme
                .text_style()
                .add_modifier(Modifier::DIM)
                .add_modifier(Modifier::BOLD)
        };
        let border_style = if active {
            self.theme.title_style()
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
        let highlight_style = if active {
            self.theme.selection_style()
        } else {
            self.theme.selection_style().add_modifier(Modifier::DIM)
        };
        let list = List::new(items)
            .style(self.theme.text_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(title),
            )
            .highlight_style(highlight_style)
            .highlight_symbol("> ");
        let mut state = ListState::default();
        state.select(view.selected);
        frame.render_stateful_widget(list, area, &mut state);
    }

    pub(crate) fn render_help(&self, frame: &mut ratatui::Frame, area: Rect) {
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

    pub(crate) fn render_quick_query_modal(&self, frame: &mut ratatui::Frame, area: Rect) {
        let modal_area = centered_rect(76, 62, area);
        frame.render_widget(Clear, modal_area);
        let Some(modal) = &self.quick_query_modal else {
            return;
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style())
            .title(Line::from(Span::styled(
                "Quick Query",
                self.theme.title_style(),
            )));
        let inner = block.inner(modal_area);
        frame.render_widget(block, modal_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(inner);

        let instructions =
            Paragraph::new("Tab/Shift+Tab switch fields  Enter run  Ctrl+E editor  Esc cancel")
                .style(self.theme.text_style())
                .wrap(Wrap { trim: false });
        frame.render_widget(instructions, chunks[0]);
        self.render_quick_query_field(
            frame,
            chunks[1],
            "Filter",
            &modal.filter,
            false,
            modal.focus == QuickQueryField::Filter,
        );
        self.render_quick_query_field(
            frame,
            chunks[2],
            "Projection (optional)",
            &modal.projection,
            true,
            modal.focus == QuickQueryField::Projection,
        );
        self.render_quick_query_field(
            frame,
            chunks[3],
            "Sort",
            &modal.sort,
            false,
            modal.focus == QuickQueryField::Sort,
        );
        self.render_quick_query_field(
            frame,
            chunks[4],
            "Limit",
            &modal.limit,
            false,
            modal.focus == QuickQueryField::Limit,
        );
    }

    fn render_quick_query_field(
        &self,
        frame: &mut ratatui::Frame,
        area: Rect,
        title: &str,
        value: &str,
        optional: bool,
        active: bool,
    ) {
        let title = if active {
            format!("▶ {title}")
        } else {
            title.to_string()
        };
        let display = if optional && value.trim().is_empty() {
            "[optional]"
        } else {
            value
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(if active {
                self.theme.title_style()
            } else {
                self.theme.border_style().add_modifier(Modifier::DIM)
            })
            .title(Line::from(Span::styled(
                title,
                if active {
                    self.theme.title_style()
                } else {
                    self.theme.text_style()
                },
            )));
        let style = if active {
            self.theme.text_style().add_modifier(Modifier::BOLD)
        } else if optional && value.trim().is_empty() {
            self.theme.text_style().add_modifier(Modifier::DIM)
        } else {
            self.theme.text_style()
        };
        let input = Paragraph::new(display.to_string())
            .style(style)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(input, area);
    }

    pub(crate) fn help_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for group in hint_groups(self.screen) {
            let keys = keys_for_actions(group.actions);
            lines.push(Line::from(format!("{keys:<12} {}", group.label)));
        }
        lines.push(Line::from(" "));
        lines.push(Line::from("press ? or Esc to close"));
        lines
    }

    pub(crate) fn hint_line(&self) -> String {
        hint_groups(self.screen)
            .iter()
            .map(|group| format!("{} {}", keys_for_actions(group.actions), group.label))
            .collect::<Vec<_>>()
            .join("  ")
    }

    pub(crate) fn header_lines(&self) -> Vec<Line<'static>> {
        let title = match self.screen {
            Screen::Connections => "Connections",
            Screen::Databases => "Databases",
            Screen::Collections => "Collections",
            Screen::Indexes => "Indexes",
            Screen::IndexView => "Index",
            Screen::Documents => "Documents",
            Screen::DocumentView => "Document",
            Screen::ExportFormatSelect => match self.export_action {
                Some(ExportAction::Clipboard) => "Copy Results",
                _ => "Export Results",
            },
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

        if self.write_enabled {
            lines.push(Line::from(Span::styled(
                "MODE: WRITE ENABLED",
                self.theme.warning_style().add_modifier(Modifier::BOLD),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "MODE: READ-ONLY",
                self.theme.warning_style().add_modifier(Modifier::BOLD),
            )));
        }

        lines
    }

    pub(crate) fn footer_lines(&self) -> Vec<Line<'static>> {
        let hint = self.hint_line();

        if self.quick_query_modal.is_some() {
            return vec![
                Line::from("Enter run  Tab next  Shift+Tab prev  Ctrl+E editor"),
                Line::from("Esc cancel"),
            ];
        }

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
        } else if let Some(path_prompt) = &self.path_prompt {
            let input_display = if path_prompt.input.is_empty() {
                "[type below]".to_string()
            } else {
                format!("'{}'", path_prompt.input)
            };
            vec![
                Line::from(Span::styled(
                    path_prompt.prompt.clone(),
                    self.theme.warning_style(),
                )),
                Line::from(format!(
                    "Enter to export (current: {input_display})  Esc to cancel"
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

    pub(crate) fn quick_query_cursor(&self, area: Rect) -> Option<(u16, u16)> {
        let modal = self.quick_query_modal.as_ref()?;
        let modal_area = centered_rect(76, 62, area);
        let inner = Block::default().borders(Borders::ALL).inner(modal_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(inner);
        let field_rect = match modal.focus {
            QuickQueryField::Filter => chunks[1],
            QuickQueryField::Projection => chunks[2],
            QuickQueryField::Sort => chunks[3],
            QuickQueryField::Limit => chunks[4],
        };
        let cursor = match modal.focus {
            QuickQueryField::Filter => modal.filter_cursor,
            QuickQueryField::Projection => modal.projection_cursor,
            QuickQueryField::Sort => modal.sort_cursor,
            QuickQueryField::Limit => modal.limit_cursor,
        };
        let x = field_rect.x.saturating_add(1).saturating_add(cursor as u16);
        let y = field_rect.y.saturating_add(1);
        Some((x.min(field_rect.right().saturating_sub(2)), y))
    }

    pub(crate) fn indexes_list_title(&self) -> String {
        format!("Indexes ({})", self.indexes.len())
    }

    pub(crate) fn documents_list_title(&self) -> String {
        let base = format!("Documents (page {})", self.document_page + 1);
        match &self.document_result_source {
            DocumentResultSource::Collection => base,
            DocumentResultSource::SavedQuery { name } => {
                format!("{base} [saved query: {name}] [x export] [y copy] [c clear applied]")
            }
            DocumentResultSource::SavedAggregation { name } => {
                format!("{base} [saved aggregation: {name}] [x export] [y copy] [c clear applied]")
            }
            DocumentResultSource::InlineQuery => {
                format!(
                    "{base} [inline query] [e edit draft] [x export] [y copy] [c clear applied]"
                )
            }
            DocumentResultSource::InlineAggregation => {
                format!(
                    "{base} [inline aggregation] [e edit draft] [x export] [y copy] [c clear applied]"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_layout_switches_by_width_and_screen() {
        let app = App::test_app();

        assert_eq!(
            app.hierarchy_layout(Screen::Connections, 200),
            MainPaneLayout::Single
        );
        assert_eq!(
            app.hierarchy_layout(Screen::Databases, 80),
            MainPaneLayout::Double
        );
        assert_eq!(
            app.hierarchy_layout(Screen::Databases, 40),
            MainPaneLayout::Single
        );
        assert_eq!(
            app.hierarchy_layout(Screen::Documents, 100),
            MainPaneLayout::Triple
        );
        assert_eq!(
            app.hierarchy_layout(Screen::Documents, 70),
            MainPaneLayout::Double
        );
        assert_eq!(
            app.hierarchy_layout(Screen::Documents, 50),
            MainPaneLayout::Single
        );
        assert_eq!(
            app.hierarchy_layout(Screen::SavedQuerySelect, 95),
            MainPaneLayout::Triple
        );
    }
}
