use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::config_model::{AppConfig, Focus, Mode, ShortcutStatus, ValidationState};
use crate::highlighting::{
    colorized_spans, render_diff_line, render_setting_line, render_theme_preview_line,
    validation_style,
};
use crate::keybinding_editor::{build_shortcut_rows, display_action, shortcut_status_counts};
use crate::tui_app::{filtered_shortcut_rows, filtered_theme_indices};
use crate::validator::validate;

pub fn render(frame: &mut Frame, app: &AppConfig) {
    let root = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(5)])
        .split(root);
    let body = chunks[0];
    let footer = chunks[1];

    match app.mode {
        Mode::Diff => render_diff_mode(frame, body, app),
        Mode::Themes => render_themes_mode(frame, body, app),
        Mode::Keybindings => render_keybindings_mode(frame, body, app),
        Mode::Help => {
            render_primary_browser(frame, body, app);
            render_help_popup(frame, body);
        }
        _ => render_primary_browser(frame, body, app),
    }
    render_footer(frame, footer, app);
    if matches!(
        app.mode,
        Mode::Search | Mode::Edit | Mode::Confirm | Mode::EnumPicker
    ) {
        render_input_popup(frame, root, app);
    }
}

fn render_primary_browser(frame: &mut Frame, area: Rect, app: &AppConfig) {
    if app.live_theme_edit {
        render_theme_edit_mode(frame, area, app);
    } else {
        render_browse(frame, area, app);
    }
}

fn render_browse(frame: &mut Frame, area: Rect, app: &AppConfig) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24),
            Constraint::Length(42),
            Constraint::Min(30),
        ])
        .split(area);

    let counts = app.category_counts();
    let category_items: Vec<ListItem> = app
        .categories
        .iter()
        .enumerate()
        .map(|(idx, cat)| {
            let style = if idx == app.selected_category && app.focus == Focus::Categories {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let label = format!("{} ({})", cat, counts.get(cat).copied().unwrap_or(0));
            ListItem::new(Line::from(Span::styled(label, style)))
        })
        .collect();
    let categories =
        List::new(category_items).block(Block::default().title("Categories").borders(Borders::ALL));
    let mut category_state = list_state(app.selected_category, app.categories.len());
    frame.render_stateful_widget(categories, cols[0], &mut category_state);

    let setting_items: Vec<ListItem> = app
        .search_results
        .iter()
        .enumerate()
        .filter_map(|(visible_idx, idx)| {
            let meta = app.metadata.get(*idx)?;
            let value = app.current_value_for(&meta.key);
            let validation = value
                .as_deref()
                .map(|v| validate(meta, v))
                .unwrap_or(ValidationState::Unknown);
            let line = render_setting_line(
                meta,
                value.as_deref(),
                app.is_changed(&meta.key),
                &validation,
            );
            let style = if visible_idx == app.selected_setting && app.focus == Focus::Settings {
                Style::default().bg(Color::Blue)
            } else {
                Style::default()
            };
            Some(ListItem::new(line).style(style))
        })
        .collect();
    let settings = List::new(setting_items).block(
        Block::default()
            .title(format!("Settings ({})", app.search_results.len()))
            .borders(Borders::ALL),
    );
    let mut settings_state = list_state(app.selected_setting, app.search_results.len());
    frame.render_stateful_widget(settings, cols[1], &mut settings_state);

    let details = render_details(app);
    frame.render_widget(details, cols[2]);
}

fn render_theme_edit_mode(frame: &mut Frame, area: Rect, app: &AppConfig) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(44), Constraint::Min(30)])
        .split(area);

    let setting_items: Vec<ListItem> = app
        .search_results
        .iter()
        .enumerate()
        .filter_map(|(visible_idx, idx)| {
            let meta = app.metadata.get(*idx)?;
            let value = app.current_value_for(&meta.key);
            let validation = value
                .as_deref()
                .map(|v| validate(meta, v))
                .unwrap_or(ValidationState::Unknown);
            let line = render_setting_line(
                meta,
                value.as_deref(),
                app.is_changed(&meta.key),
                &validation,
            );
            let style = if visible_idx == app.selected_setting && app.focus == Focus::Settings {
                Style::default().bg(Color::Blue)
            } else {
                Style::default()
            };
            Some(ListItem::new(line).style(style))
        })
        .collect();
    let title = format!("Theme Colors ({})", app.search_results.len());
    let settings =
        List::new(setting_items).block(Block::default().title(title).borders(Borders::ALL));
    let mut settings_state = list_state(app.selected_setting, app.search_results.len());
    frame.render_stateful_widget(settings, cols[0], &mut settings_state);

    let details = render_details(app);
    frame.render_widget(details, cols[1]);
}

fn render_details(app: &AppConfig) -> Paragraph<'static> {
    let Some(meta) = app.current_setting() else {
        return Paragraph::new("No setting selected")
            .block(Block::default().title("Details").borders(Borders::ALL));
    };
    let current = app
        .current_value_for(&meta.key)
        .unwrap_or_else(|| "<unset>".into());
    let validation = validate(meta, &current);
    let source = app
        .effective
        .last_value(&meta.key)
        .map(|v| format!("{}:{}", v.source_file.display(), v.line_no))
        .unwrap_or_else(|| "default".into());

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Key: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(meta.key.clone()),
        ]),
        Line::from(vec![
            Span::styled("Category: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(meta.category.clone()),
        ]),
        {
            let mut spans = vec![Span::styled(
                "Current: ",
                Style::default().add_modifier(Modifier::BOLD),
            )];
            spans.extend(colorized_spans(
                &current,
                Style::default().fg(Color::Yellow),
            ));
            Line::from(spans)
        },
        {
            let default_value = meta
                .default_value
                .clone()
                .unwrap_or_else(|| "<none>".into());
            let mut spans = vec![Span::styled(
                "Default: ",
                Style::default().add_modifier(Modifier::BOLD),
            )];
            spans.extend(colorized_spans(&default_value, Style::default()));
            Line::from(spans)
        },
        Line::from(vec![
            Span::styled("Source: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(source),
        ]),
        Line::from(vec![
            Span::styled(
                "Validation: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                match validation.clone() {
                    ValidationState::Valid => "valid".to_string(),
                    ValidationState::Invalid(e) => e,
                    ValidationState::Unknown => "unknown".to_string(),
                },
                validation_style(&validation),
            ),
        ]),
        Line::from(vec![
            Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:?}", meta.value_type)),
        ]),
        Line::raw(""),
        Line::from(Span::styled(
            "Description",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    for line in meta.description.lines() {
        lines.push(Line::raw(line.to_string()));
    }
    if !meta.enum_choices.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "Enum choices",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::raw(meta.enum_choices.join(", ")));
    }
    if !meta.examples.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "Examples",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for example in &meta.examples {
            lines.push(Line::from(colorized_spans(
                example,
                Style::default().fg(Color::DarkGray),
            )));
        }
    }
    Paragraph::new(Text::from(lines))
        .block(Block::default().title("Details").borders(Borders::ALL))
        .scroll((app.detail_scroll, 0))
        .wrap(Wrap { trim: false })
}

fn render_footer(frame: &mut Frame, area: Rect, app: &AppConfig) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(32)])
        .split(area);
    let mode_box = Paragraph::new(Text::from(vec![
        Line::raw(format!("Mode: {}", session_mode_label(app))),
        Line::raw(format!("Status: {}", app.status)),
    ]))
    .block(Block::default().title("Session").borders(Borders::ALL))
    .wrap(Wrap { trim: true })
    .style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(mode_box, cols[0]);

    let keys_box = Paragraph::new(footer_keys(app))
        .block(Block::default().title("Keys").borders(Borders::ALL))
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(keys_box, cols[1]);
}

fn session_mode_label(app: &AppConfig) -> &'static str {
    if app.live_theme_edit {
        "ThemeEdit"
    } else {
        match app.mode {
            Mode::Browse => "Browse",
            Mode::Search => "Search",
            Mode::Edit => "Edit",
            Mode::EnumPicker => "EnumPicker",
            Mode::Diff => "Diff",
            Mode::Themes => "Themes",
            Mode::Keybindings => "Keybindings",
            Mode::Confirm => "Confirm",
            Mode::Help => "Help",
        }
    }
}

fn render_input_popup(frame: &mut Frame, area: Rect, app: &AppConfig) {
    let popup = centered_rect(72, 40, area);
    frame.render_widget(Clear, popup);
    match app.mode {
        Mode::EnumPicker => render_enum_popup(frame, popup, app),
        _ => {
            let title = match app.mode {
                Mode::Search => "Search",
                Mode::Edit => match app.edit_target {
                    Some(crate::config_model::EditTarget::ThemeSaveAs) => "Save theme as",
                    _ => "Edit value",
                },
                Mode::Confirm => "Confirm",
                _ => "Input",
            };
            let widget = Paragraph::new(app.edit_buffer.clone())
                .block(Block::default().title(title).borders(Borders::ALL))
                .wrap(Wrap { trim: false });
            frame.render_widget(widget, popup);
        }
    }
}

fn render_enum_popup(frame: &mut Frame, area: Rect, app: &AppConfig) {
    let cols = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);
    let title = Paragraph::new(app.edit_buffer.clone())
        .block(Block::default().title("Enum choices").borders(Borders::ALL));
    frame.render_widget(title, cols[0]);

    let items: Vec<ListItem> = app
        .current_setting()
        .map(|meta| {
            meta.enum_choices
                .iter()
                .enumerate()
                .map(|(idx, choice)| {
                    let style = if idx == app.enum_index {
                        Style::default()
                            .bg(Color::Blue)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(choice.clone()).style(style)
                })
                .collect()
        })
        .unwrap_or_default();
    let mut enum_state = list_state(app.enum_index, items.len());
    frame.render_stateful_widget(
        List::new(items).block(Block::default().borders(Borders::ALL)),
        cols[1],
        &mut enum_state,
    );
}

fn render_diff_mode(frame: &mut Frame, area: Rect, app: &AppConfig) {
    let lines = app
        .diff_lines
        .iter()
        .map(render_diff_line)
        .collect::<Vec<_>>();
    let diff = Paragraph::new(Text::from(lines))
        .block(Block::default().title("Diff").borders(Borders::ALL))
        .scroll((app.diff_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(diff, area);
}

fn render_themes_mode(frame: &mut Frame, area: Rect, app: &AppConfig) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(32), Constraint::Min(30)])
        .split(area);
    let visible = filtered_theme_indices(app);
    let items: Vec<ListItem> = app
        .themes
        .iter()
        .enumerate()
        .filter(|(idx, _)| visible.contains(idx))
        .map(|(idx, theme)| {
            let style = if idx == app.selected_theme {
                Style::default().bg(Color::Blue)
            } else {
                Style::default()
            };
            ListItem::new(theme.name.clone()).style(style)
        })
        .collect();
    let title = if app.theme_query.trim().is_empty() {
        format!("Themes ({})", app.themes.len())
    } else {
        format!("Themes ({}/{})", visible.len(), app.themes.len())
    };
    let selected = visible
        .iter()
        .position(|idx| *idx == app.selected_theme)
        .unwrap_or(0);
    let mut themes_state = list_state(selected, items.len());
    frame.render_stateful_widget(
        List::new(items).block(Block::default().title(title).borders(Borders::ALL)),
        cols[0],
        &mut themes_state,
    );
    let preview = if app.themes.is_empty() {
        Text::from("No themes found")
    } else if visible.is_empty() {
        Text::from(vec![
            Line::raw("No themes match the current filter."),
            Line::raw(""),
            Line::raw(format!(
                "Filter: {}",
                if app.theme_query.trim().is_empty() {
                    "<none>"
                } else {
                    app.theme_query.as_str()
                }
            )),
            Line::raw(""),
            Line::raw("Press / to change the filter."),
        ])
    } else {
        app.themes
            .get(app.selected_theme)
            .map(|theme| {
                let mut lines = vec![
                    Line::raw(theme.path.display().to_string()),
                    Line::raw(""),
                    Line::raw(format!(
                        "Filter: {}",
                        if app.theme_query.trim().is_empty() {
                            "<none>"
                        } else {
                            app.theme_query.as_str()
                        }
                    )),
                    Line::raw(""),
                    Line::raw("Up/Down preview without saving"),
                    Line::raw("Enter selects a theme"),
                    Line::raw("e opens the editable theme color list"),
                    Line::raw("w saves the current editable theme as a preset"),
                    Line::raw(""),
                ];
                lines.extend(
                    theme
                        .preview
                        .iter()
                        .map(|line| render_theme_preview_line(line)),
                );
                Text::from(lines)
            })
            .unwrap_or_else(|| Text::from("No themes found"))
    };
    frame.render_widget(
        Paragraph::new(preview)
            .block(Block::default().title("Preview").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        cols[1],
    );
}

fn render_keybindings_mode(frame: &mut Frame, area: Rect, app: &AppConfig) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);

    let rows = filtered_shortcut_rows(app);
    let kitty_mod = app
        .current_value_for("kitty_mod")
        .unwrap_or_else(|| String::from("ctrl+shift"));
    let custom_rows = build_shortcut_rows(
        &app.default_keymaps,
        &app.edited_keymaps,
        &kitty_mod,
        crate::config_model::ShortcutView::Custom,
    );
    let (added, changed, removed) = shortcut_status_counts(&custom_rows);
    let title = format!(
        "Shortcuts [{}]  1:Custom 2:Effective 3:Defaults  |  {} added {} changed {} removed",
        app.shortcut_view.title(),
        added,
        changed,
        removed
    );

    let items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .map(|(idx, row)| {
            let style = if idx == app.selected_shortcut {
                Style::default().bg(Color::Blue)
            } else {
                Style::default()
            };
            let status_style = match row.status {
                ShortcutStatus::Added => Style::default().fg(Color::Green),
                ShortcutStatus::Changed => Style::default().fg(Color::Yellow),
                ShortcutStatus::Removed => Style::default().fg(Color::Red),
                ShortcutStatus::Default => Style::default().fg(Color::DarkGray),
            };
            let mode = if row.mode == "main" {
                String::new()
            } else {
                format!("[{}] ", row.mode)
            };
            let prefix = if row.option_prefix.trim().is_empty() {
                String::new()
            } else {
                format!("{} ", row.option_prefix.trim())
            };
            let line = Line::from(vec![
                Span::raw(format!("{}{}{}", mode, prefix, row.shortcut)),
                Span::raw("  "),
                Span::styled(row.status.label(), status_style),
                Span::raw("  "),
                Span::raw(display_action(&row.action)),
            ]);
            ListItem::new(line).style(style)
        })
        .collect();
    let list = List::new(items).block(Block::default().title(title).borders(Borders::ALL));
    let mut shortcuts_state = list_state(app.selected_shortcut, rows.len());
    frame.render_stateful_widget(list, cols[0], &mut shortcuts_state);

    let detail = if let Some(row) = rows.get(app.selected_shortcut) {
        let shortcut_style = Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let action_style = Style::default().fg(Color::Yellow);
        vec![
            Line::from(Span::styled(
                "Selected mapping",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::raw(""),
            Line::from(vec![
                Span::styled("Shortcut: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(row.shortcut.clone(), shortcut_style),
            ]),
            Line::from(vec![
                Span::styled("Action: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(display_action(&row.action), action_style),
            ]),
            Line::from(vec![
                Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(row.status.label()),
            ]),
            Line::from(vec![
                Span::styled("Mode: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(row.mode.clone()),
            ]),
            Line::from(vec![
                Span::styled("Options: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(if row.option_prefix.trim().is_empty() {
                    String::from("<none>")
                } else {
                    row.option_prefix.clone()
                }),
            ]),
            Line::from(vec![
                Span::styled("Source: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{}:{}", row.source_file.display(), row.line_no)),
            ]),
            Line::raw(""),
            Line::raw(format!(
                "Filter: {}",
                if app.shortcut_query.trim().is_empty() {
                    "<none>"
                } else {
                    app.shortcut_query.as_str()
                }
            )),
            Line::raw(if row.detail.is_empty() {
                String::from("Details: built-in default")
            } else {
                format!("Details: {}", row.detail)
            }),
            Line::raw(""),
            Line::raw("Enter/e reassigns the selected shortcut."),
            Line::raw("Default rows become explicit overrides when edited."),
            Line::raw("Removed rows must be edited manually in kitty.conf."),
        ]
    } else {
        vec![Line::raw("No shortcuts match the current filter.")]
    };
    frame.render_widget(
        Paragraph::new(Text::from(detail))
            .block(
                Block::default()
                    .title("Shortcut Details")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: false }),
        cols[1],
    );
}

fn render_help_popup(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(70, 70, area);
    frame.render_widget(Clear, popup);
    let text = "Browse/Search/Edit/Diff/Themes/Keybindings\n\nUp/Down navigate\nLeft/Right or Tab change focus\nEnter select theme or confirm edit\ne edit current setting / start theme editing\nd diff\nr reset to default\nc clear override\ns save and live reload\nS minimal save and live reload\nt themes\nw save the current theme as a preset\nk keybindings\n1/2/3 shortcut views\nR save and force reload\nq quit\nEsc cancel";
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().title("Help").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn footer_keys(app: &AppConfig) -> Text<'static> {
    let text = match app.mode {
        Mode::Themes => {
            "Up/Down preview  / filter  Enter select  e edit theme  w save preset  s save  S minimal save  Esc back  q quit"
        }
        Mode::Keybindings => {
            "Up/Down/j/k move  PgUp/PgDn scroll  Enter/e reassign  / filter  1/2/3 views  a add  x delete/unmap  Esc back  q quit"
        }
        Mode::Diff => "Up/Down scroll  Esc back  q quit",
        Mode::Search => "Type to filter  Enter apply search  Esc cancel",
        Mode::Edit => "Type value  Enter save edit  Esc cancel",
        Mode::EnumPicker => "Up/Down choose  Enter apply  Esc cancel",
        _ if app.live_theme_edit => {
            "Up/Down move  Enter/e edit value  / filter  r reset  c clear  w save preset  Tab focus details  Esc finish  q quit"
        }
        _ => {
            "/ search  Enter/e edit  Left/Right/Tab panes  Up/Down move  d diff  t themes  k keymaps  r reset  c clear  s save  S minimal  R reload  q quit"
        }
    };
    Text::from(text)
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn list_state(selected: usize, len: usize) -> ListState {
    let mut state = ListState::default();
    if len > 0 {
        state.select(Some(selected.min(len.saturating_sub(1))));
    }
    state
}
