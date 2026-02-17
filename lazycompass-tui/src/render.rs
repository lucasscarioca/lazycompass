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
                    ListView {
                        title: "Connections",
                        items: &items,
                        selected: self.connection_index,
                        load_state: &LoadState::Idle,
                        loading_label: "loading connections...",
                    },
                );
            }
            Screen::Databases => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(layout[1]);
                let connections = self
                    .storage
                    .config
                    .connections
                    .iter()
                    .map(connection_label)
                    .collect::<Vec<_>>();
                self.render_list(
                    frame,
                    panes[0],
                    ListView {
                        title: "Connections",
                        items: &connections,
                        selected: self.connection_index,
                        load_state: &LoadState::Idle,
                        loading_label: "loading connections...",
                    },
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
            Screen::Collections => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(layout[1]);
                self.render_list(
                    frame,
                    panes[0],
                    ListView {
                        title: "Databases",
                        items: &self.database_items,
                        selected: self.database_index,
                        load_state: &self.database_state,
                        loading_label: "loading databases...",
                    },
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
            Screen::Documents => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(layout[1]);
                self.render_list(
                    frame,
                    panes[0],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
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
            Screen::DocumentView => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(layout[1]);
                let items = self
                    .documents
                    .iter()
                    .map(document_preview)
                    .collect::<Vec<_>>();
                let title = self.documents_list_title();
                self.render_list(
                    frame,
                    panes[0],
                    ListView {
                        title: &title,
                        items: &items,
                        selected: self.document_index,
                        load_state: &self.document_state,
                        loading_label: "loading documents...",
                    },
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
            Screen::SavedQuerySelect => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(layout[1]);
                self.render_list(
                    frame,
                    panes[0],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                );
                let right_panes = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(15), Constraint::Percentage(85)])
                    .split(panes[1]);
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
            Screen::SavedAggregationSelect => {
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(layout[1]);
                self.render_list(
                    frame,
                    panes[0],
                    ListView {
                        title: "Collections",
                        items: &self.collection_items,
                        selected: self.collection_index,
                        load_state: &self.collection_state,
                        loading_label: "loading collections...",
                    },
                );
                let right_panes = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(15), Constraint::Percentage(85)])
                    .split(panes[1]);
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
            Screen::SaveQueryScopeSelect => {
                let items = vec![
                    "Shared (uses current db/collection when running)".to_string(),
                    "Scoped (encode current db/collection in filename)".to_string(),
                ];
                self.render_list(
                    frame,
                    layout[1],
                    ListView {
                        title: "Select Query Save Scope",
                        items: &items,
                        selected: self.save_query_scope_index,
                        load_state: &LoadState::Idle,
                        loading_label: "",
                    },
                );
            }
            Screen::SaveAggregationScopeSelect => {
                let items = vec![
                    "Shared (uses current db/collection when running)".to_string(),
                    "Scoped (encode current db/collection in filename)".to_string(),
                ];
                self.render_list(
                    frame,
                    layout[1],
                    ListView {
                        title: "Select Aggregation Save Scope",
                        items: &items,
                        selected: self.save_agg_scope_index,
                        load_state: &LoadState::Idle,
                        loading_label: "",
                    },
                );
            }
            Screen::AddConnectionScopeSelect => {
                let items = vec![
                    "Session only (not persisted)".to_string(),
                    "Save to repo config (.lazycompass/config.toml)".to_string(),
                    "Save to global config (~/.config/lazycompass/config.toml)".to_string(),
                ];
                self.render_list(
                    frame,
                    layout[1],
                    ListView {
                        title: "Select Persistence Scope for New Connection",
                        items: &items,
                        selected: self.add_connection_scope_index,
                        load_state: &LoadState::Idle,
                        loading_label: "",
                    },
                );
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
        focused: bool,
    ) {
        let title_style = if focused {
            self.theme.title_style()
        } else {
            self.theme
                .text_style()
                .add_modifier(Modifier::DIM)
                .add_modifier(Modifier::BOLD)
        };
        let border_style = if focused {
            self.theme.border_style()
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
        let list = List::new(items)
            .style(self.theme.text_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(title),
            )
            .highlight_style(self.theme.selection_style())
            .highlight_symbol("> ");
        let mut state = ListState::default();
        state.select(if focused { view.selected } else { None });
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
            Screen::Documents => "Documents",
            Screen::DocumentView => "Document",
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

        if self.read_only {
            lines.push(Line::from(Span::styled(
                "MODE: READ-ONLY",
                self.theme.warning_style().add_modifier(Modifier::BOLD),
            )));
        }

        lines
    }

    pub(crate) fn footer_lines(&self) -> Vec<Line<'static>> {
        let hint = self.hint_line();

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

    pub(crate) fn documents_list_title(&self) -> String {
        let base = format!("Documents (page {})", self.document_page + 1);
        match &self.document_result_source {
            DocumentResultSource::Collection => base,
            DocumentResultSource::SavedQuery { name } => {
                format!("{base} [saved query: {name}] [c clear applied]")
            }
            DocumentResultSource::SavedAggregation { name } => {
                format!("{base} [saved aggregation: {name}] [c clear applied]")
            }
        }
    }
}
