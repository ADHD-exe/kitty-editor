use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::config_model::{
    AppConfig, EditTarget, Focus, KeymapField, MapEntry, Mode, SaveMode, SearchTarget,
    SettingMetadata, ShortcutRow, ShortcutView, ThemeEntry, ValidationState,
};
use crate::diff_engine::simple_diff;
use crate::fuzzy_search;
use crate::keybinding_editor::{
    build_shortcut_rows, filter_shortcut_rows, map_field_value, shortcut_status_counts,
    update_map_field, validate_map_entry,
};
use crate::parser::parse_current_config;
use crate::reload::{preview_theme, preview_theme_artifact, reload_config};
use crate::runtime::TerminalSession;
use crate::theme_browser::{
    apply_theme_include, detect_current_theme_include, discover_themes, find_current_theme_index,
    is_theme_setting_key, theme_artifact_path,
};
use crate::ui_renderer;
use crate::validator::validate;
use crate::writer::{
    backup_config_root, render_output, render_output_to_path, render_theme_artifact,
    replace_or_insert_theme_include_in_text, save_to_path, uses_theme_wrapper,
    write_selected_theme_artifact,
};

pub struct RuntimeOptions {
    pub out_full: Option<PathBuf>,
    pub out_minimal: Option<PathBuf>,
    pub create_backup: bool,
    pub enable_reload: bool,
    pub reload_command: Option<String>,
}

pub fn build_app(
    metadata: Vec<SettingMetadata>,
    effective: crate::config_model::EffectiveConfig,
    default_keymaps: Vec<MapEntry>,
    themes: Vec<ThemeEntry>,
    themes_dir: Option<PathBuf>,
) -> AppConfig {
    let categories: Vec<String> = metadata
        .iter()
        .map(|m| m.category.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let metadata_by_key = metadata
        .iter()
        .cloned()
        .map(|m| (m.key.clone(), m))
        .collect::<HashMap<_, _>>();
    let current_theme_include = detect_current_theme_include(&effective);
    let current_theme_artifact = theme_artifact_path(&effective.main_file);
    let selected_theme = find_current_theme_index(
        &themes,
        &effective,
        current_theme_include.as_deref(),
        &current_theme_artifact,
    );
    let edited_keymaps = effective.keymaps.clone();
    let mut app = AppConfig {
        metadata,
        metadata_by_key,
        effective,
        default_keymaps,
        edited_values: HashMap::new(),
        edited_keymaps,
        selected_category: 0,
        selected_setting: 0,
        categories,
        search_query: String::new(),
        search_target: SearchTarget::Settings,
        search_results: vec![],
        theme_query: String::new(),
        mode: Mode::Browse,
        focus: Focus::Settings,
        edit_buffer: String::new(),
        edit_target: None,
        enum_index: 0,
        keymap_field: KeymapField::Shortcut,
        detail_scroll: 0,
        diff_scroll: 0,
        status: String::from("ready"),
        diff_lines: vec![],
        themes,
        themes_dir,
        current_theme_artifact,
        selected_theme,
        shortcut_view: ShortcutView::Custom,
        shortcut_query: String::new(),
        selected_shortcut: 0,
        pending_quit: false,
        current_theme_include,
        pending_theme_path: None,
        theme_edit_save_path: None,
        theme_preview_active: false,
        live_theme_edit: false,
        theme_edit_keys: vec![],
        theme_edit_dirty: false,
    };
    sync_visible_settings(&mut app);
    app
}

pub fn run(
    terminal: &mut TerminalSession,
    mut app: AppConfig,
    opts: RuntimeOptions,
    tick_rate: Duration,
) -> Result<()> {
    loop {
        terminal
            .terminal_mut()
            .draw(|frame| ui_renderer::render(frame, &app))?;
        if event::poll(tick_rate)? {
            if handle_event(&mut app, &opts, event::read()?)? {
                break;
            }
        }
    }
    Ok(())
}

fn handle_event(app: &mut AppConfig, opts: &RuntimeOptions, event: Event) -> Result<bool> {
    match event {
        Event::Key(key) => handle_key(app, opts, key),
        Event::Paste(text) => {
            handle_paste(app, &text);
            Ok(false)
        }
        Event::Resize(cols, rows) => {
            app.status = format!("resized to {cols}x{rows}");
            Ok(false)
        }
        Event::FocusGained => {
            app.status = "focus gained".into();
            Ok(false)
        }
        Event::FocusLost => {
            app.status = "focus lost".into();
            Ok(false)
        }
        Event::Mouse(_) => Ok(false),
    }
}

fn handle_key(app: &mut AppConfig, opts: &RuntimeOptions, key: KeyEvent) -> Result<bool> {
    if matches!(key.kind, KeyEventKind::Release) {
        return Ok(false);
    }

    match app.mode {
        Mode::Search => return handle_search_mode(app, key),
        Mode::Edit => return handle_edit_mode(app, key),
        Mode::EnumPicker => return handle_enum_mode(app, key),
        Mode::Confirm => return handle_confirm_mode(app, key),
        _ => {}
    }

    if app.mode == Mode::Keybindings {
        return handle_keybindings_mode(app, key);
    }

    match key.code {
        KeyCode::Char('q') => {
            if app.theme_preview_active {
                restore_theme_preview(app, opts)?;
            }
            return Ok(true);
        }
        KeyCode::Char('?') => app.mode = Mode::Help,
        KeyCode::Esc => {
            if app.live_theme_edit && app.mode == Mode::Browse {
                finish_theme_edit_session(app)?;
            } else if app.mode == Mode::Themes {
                restore_theme_preview(app, opts)?;
                app.mode = Mode::Browse;
            } else {
                app.mode = Mode::Browse;
            }
        }
        KeyCode::Tab => cycle_focus(app),
        KeyCode::Left if app.mode == Mode::Browse => move_focus_left(app),
        KeyCode::Right if app.mode == Mode::Browse => move_focus_right(app),
        KeyCode::Up => move_up(app, opts)?,
        KeyCode::Down => move_down(app, opts)?,
        KeyCode::Enter => {
            if app.mode == Mode::Themes {
                apply_selected_theme(app)?;
            } else {
                begin_edit(app);
            }
        }
        KeyCode::Char('e') => {
            if app.mode == Mode::Themes {
                begin_theme_live_edit(app)?;
            } else {
                begin_edit(app);
            }
        }
        KeyCode::Char('/') => {
            app.search_target = if app.mode == Mode::Themes {
                SearchTarget::Themes
            } else {
                SearchTarget::Settings
            };
            app.mode = Mode::Search;
            app.edit_buffer = match app.search_target {
                SearchTarget::Settings => app.search_query.clone(),
                SearchTarget::Shortcuts => app.shortcut_query.clone(),
                SearchTarget::Themes => app.theme_query.clone(),
            };
            app.edit_target = None;
        }
        KeyCode::Char('d') => open_diff(app),
        KeyCode::Char('t') => {
            if app.live_theme_edit && app.mode == Mode::Browse {
                finish_theme_edit_session(app)?;
            } else {
                open_theme_browser(app);
            }
        }
        KeyCode::Char('k') => open_keybindings(app),
        KeyCode::Char('r') => {
            reset_current_to_default(app);
            maybe_preview_live_theme_edit(app)?;
        }
        KeyCode::Char('c') => {
            clear_current(app);
            maybe_preview_live_theme_edit(app)?;
        }
        KeyCode::Char('s') => {
            prepare_theme_selection_for_save(app)?;
            save_current(app, opts, SaveMode::Full, false)?;
        }
        KeyCode::Char('S') => {
            prepare_theme_selection_for_save(app)?;
            save_current(app, opts, SaveMode::Minimal, false)?;
        }
        KeyCode::Char('R') => {
            prepare_theme_selection_for_save(app)?;
            save_current(app, opts, SaveMode::Full, true)?;
        }
        KeyCode::Char('w') if app.live_theme_edit && app.mode == Mode::Browse => {
            app.live_theme_edit = false;
            app.theme_edit_dirty = false;
            begin_theme_save_as(app)?
        }
        KeyCode::Char('w') | KeyCode::Char('n') if app.mode == Mode::Themes => {
            begin_theme_save_as(app)?
        }
        KeyCode::Char(' ') if app.mode == Mode::Themes => apply_selected_theme(app)?,
        _ => {}
    }
    Ok(false)
}

fn handle_keybindings_mode(app: &mut AppConfig, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Esc => app.mode = Mode::Browse,
        KeyCode::Up | KeyCode::Char('k') => {
            app.selected_shortcut = app.selected_shortcut.saturating_sub(1)
        }
        KeyCode::Down => {
            let visible = filtered_shortcut_rows(app);
            if !visible.is_empty() {
                app.selected_shortcut =
                    (app.selected_shortcut + 1).min(visible.len().saturating_sub(1))
            }
        }
        KeyCode::Char('j') => {
            let visible = filtered_shortcut_rows(app);
            if !visible.is_empty() {
                app.selected_shortcut =
                    (app.selected_shortcut + 1).min(visible.len().saturating_sub(1))
            }
        }
        KeyCode::PageDown => {
            let visible = filtered_shortcut_rows(app);
            if !visible.is_empty() {
                app.selected_shortcut =
                    (app.selected_shortcut + 10).min(visible.len().saturating_sub(1))
            }
        }
        KeyCode::PageUp => app.selected_shortcut = app.selected_shortcut.saturating_sub(10),
        KeyCode::Home => app.selected_shortcut = 0,
        KeyCode::End => {
            let visible = filtered_shortcut_rows(app);
            if !visible.is_empty() {
                app.selected_shortcut = visible.len() - 1;
            }
        }
        KeyCode::Char('/') => {
            app.search_target = SearchTarget::Shortcuts;
            app.mode = Mode::Search;
            app.edit_buffer = app.shortcut_query.clone();
            app.edit_target = None;
        }
        KeyCode::Char('1') => {
            app.shortcut_view = ShortcutView::Custom;
            app.selected_shortcut = 0;
        }
        KeyCode::Char('2') => {
            app.shortcut_view = ShortcutView::Effective;
            app.selected_shortcut = 0;
        }
        KeyCode::Char('3') => {
            app.shortcut_view = ShortcutView::Defaults;
            app.selected_shortcut = 0;
        }
        KeyCode::Enter | KeyCode::Char('e') => begin_shortcut_edit(app),
        KeyCode::Char('a') => add_keymap(app),
        KeyCode::Char('x') => delete_keymap(app),
        _ => {}
    }
    Ok(false)
}

fn handle_search_mode(app: &mut AppConfig, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => {
            app.mode = match app.search_target {
                SearchTarget::Settings => Mode::Browse,
                SearchTarget::Shortcuts => Mode::Keybindings,
                SearchTarget::Themes => Mode::Themes,
            };
            app.edit_buffer.clear();
            app.edit_target = None;
        }
        KeyCode::Enter => {
            match app.search_target {
                SearchTarget::Settings => app.search_query = app.edit_buffer.clone(),
                SearchTarget::Shortcuts => app.shortcut_query = app.edit_buffer.clone(),
                SearchTarget::Themes => app.theme_query = app.edit_buffer.clone(),
            }
            refresh_search(app);
            app.mode = match app.search_target {
                SearchTarget::Settings => Mode::Browse,
                SearchTarget::Shortcuts => Mode::Keybindings,
                SearchTarget::Themes => Mode::Themes,
            };
            app.edit_target = None;
        }
        KeyCode::Backspace => {
            app.edit_buffer.pop();
            match app.search_target {
                SearchTarget::Settings => app.search_query = app.edit_buffer.clone(),
                SearchTarget::Shortcuts => app.shortcut_query = app.edit_buffer.clone(),
                SearchTarget::Themes => app.theme_query = app.edit_buffer.clone(),
            }
            refresh_search(app);
        }
        KeyCode::Char(ch) => {
            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                app.edit_buffer.push(ch);
                match app.search_target {
                    SearchTarget::Settings => app.search_query = app.edit_buffer.clone(),
                    SearchTarget::Shortcuts => app.shortcut_query = app.edit_buffer.clone(),
                    SearchTarget::Themes => app.theme_query = app.edit_buffer.clone(),
                }
                refresh_search(app);
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_edit_mode(app: &mut AppConfig, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => {
            app.mode = if matches!(
                app.edit_target,
                Some(EditTarget::KeymapShortcut(_)) | Some(EditTarget::ShortcutOverride(_, _))
            ) {
                Mode::Keybindings
            } else if matches!(app.edit_target, Some(EditTarget::ThemeSaveAs)) {
                Mode::Themes
            } else {
                Mode::Browse
            };
            app.edit_target = None;
            app.edit_buffer.clear();
        }
        KeyCode::Enter => {
            commit_edit(app)?;
        }
        KeyCode::Backspace => {
            app.edit_buffer.pop();
        }
        KeyCode::Char(ch) => {
            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                app.edit_buffer.push(ch);
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_enum_mode(app: &mut AppConfig, key: KeyEvent) -> Result<bool> {
    let choices_len = app
        .current_setting()
        .map(|m| m.enum_choices.len())
        .unwrap_or_default();
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Browse;
            app.edit_target = None;
        }
        KeyCode::Up => {
            app.enum_index = app.enum_index.saturating_sub(1);
        }
        KeyCode::Down => {
            if choices_len > 0 {
                app.enum_index = (app.enum_index + 1).min(choices_len - 1);
            }
        }
        KeyCode::Enter => {
            if let Some((key, choice)) = app.current_setting().and_then(|meta| {
                meta.enum_choices
                    .get(app.enum_index)
                    .cloned()
                    .map(|choice| (meta.key.clone(), choice))
            }) {
                app.edited_values.insert(key.clone(), choice);
                app.status = format!("updated {key}");
            }
            app.mode = Mode::Browse;
            app.edit_target = None;
        }
        _ => {}
    }
    Ok(false)
}

fn handle_confirm_mode(app: &mut AppConfig, key: KeyEvent) -> Result<bool> {
    if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
        app.mode = Mode::Browse;
    }
    Ok(false)
}

fn handle_paste(app: &mut AppConfig, text: &str) {
    match app.mode {
        Mode::Search => {
            app.edit_buffer.push_str(text);
            match app.search_target {
                SearchTarget::Settings => app.search_query = app.edit_buffer.clone(),
                SearchTarget::Shortcuts => app.shortcut_query = app.edit_buffer.clone(),
                SearchTarget::Themes => app.theme_query = app.edit_buffer.clone(),
            }
            refresh_search(app);
            app.status = "search updated from paste".into();
        }
        Mode::Edit => {
            app.edit_buffer.push_str(text);
            app.status = "edit buffer updated from paste".into();
        }
        _ => {
            app.status = "paste ignored outside input mode".into();
        }
    }
}

fn commit_edit(app: &mut AppConfig) -> Result<()> {
    match app.edit_target.clone() {
        Some(EditTarget::Setting(key)) => {
            if let Some(meta) = app.metadata_by_key.get(&key).cloned() {
                let state = validate(&meta, &app.edit_buffer);
                if let ValidationState::Invalid(err) = state {
                    app.status = format!("invalid: {err}");
                    return Ok(());
                }
                app.edited_values
                    .insert(meta.key.clone(), app.edit_buffer.clone());
                app.status = format!("updated {}", meta.key);
                if is_theme_setting_key(&meta.key) {
                    if app.live_theme_edit {
                        app.theme_edit_dirty = true;
                    }
                    maybe_preview_live_theme_edit(app)?;
                }
                app.mode = Mode::Browse;
                app.edit_target = None;
                app.edit_buffer.clear();
            }
        }
        Some(EditTarget::KeymapShortcut(index)) => {
            if let Some(entry) = app.edited_keymaps.get_mut(index) {
                let old_shortcut = entry.shortcut.clone();
                update_map_field(entry, KeymapField::Shortcut, app.edit_buffer.clone());
                if let Err(err) = validate_map_entry(&entry.shortcut, &entry.action) {
                    entry.shortcut = old_shortcut;
                    app.status = format!("invalid keymap: {err}");
                    return Ok(());
                }
                app.status = "updated keymap".into();
                app.mode = Mode::Keybindings;
                app.edit_target = None;
                app.edit_buffer.clear();
                select_shortcut_by_edited_index(app, index);
            }
        }
        Some(EditTarget::ShortcutOverride(row, field)) => {
            let input = app.edit_buffer.trim().to_string();
            match field {
                KeymapField::Shortcut => {
                    let shortcut = input;
                    if let Err(err) = validate_map_entry(&shortcut, &row.action) {
                        app.status = format!("invalid keymap: {err}");
                        return Ok(());
                    }
                    let remove_default = MapEntry {
                        mode: row.mode.clone(),
                        shortcut: row.raw_shortcut.clone(),
                        action: String::new(),
                        option_prefix: row.option_prefix.clone(),
                        source_file: app.effective.main_file.clone(),
                        line_no: 0,
                    };
                    let new_entry = MapEntry {
                        mode: row.mode.clone(),
                        shortcut: shortcut.clone(),
                        action: row.action.clone(),
                        option_prefix: row.option_prefix.clone(),
                        source_file: app.effective.main_file.clone(),
                        line_no: 0,
                    };
                    app.edited_keymaps.push(remove_default);
                    app.edited_keymaps.push(new_entry);
                    app.shortcut_view = ShortcutView::Custom;
                    app.status = "created shortcut override".into();
                    select_shortcut_by_edited_index(app, app.edited_keymaps.len() - 1);
                }
                KeymapField::Action => {
                    if let Err(err) = validate_map_entry(&row.raw_shortcut, &input) {
                        app.status = format!("invalid keymap: {err}");
                        return Ok(());
                    }
                    let new_entry = MapEntry {
                        mode: row.mode.clone(),
                        shortcut: row.raw_shortcut.clone(),
                        action: input.clone(),
                        option_prefix: row.option_prefix.clone(),
                        source_file: app.effective.main_file.clone(),
                        line_no: 0,
                    };
                    app.edited_keymaps.push(new_entry);
                    app.shortcut_view = ShortcutView::Custom;
                    app.status = "created action override".into();
                    select_shortcut_by_edited_index(app, app.edited_keymaps.len() - 1);
                }
            }
            app.mode = Mode::Keybindings;
            app.edit_target = None;
            app.edit_buffer.clear();
        }
        Some(EditTarget::ThemeSaveAs) => {
            save_theme_as(app)?;
            app.edit_buffer.clear();
        }
        None => {
            app.mode = Mode::Browse;
        }
    }
    Ok(())
}

fn refresh_search(app: &mut AppConfig) {
    match app.search_target {
        SearchTarget::Settings => {
            sync_visible_settings(app);
            app.selected_setting = 0;
            app.detail_scroll = 0;
        }
        SearchTarget::Shortcuts => {
            app.selected_shortcut = 0;
        }
        SearchTarget::Themes => {
            sync_theme_selection(app);
        }
    }
}

fn cycle_focus(app: &mut AppConfig) {
    if app.live_theme_edit {
        app.focus = if app.focus == Focus::Details {
            Focus::Settings
        } else {
            Focus::Details
        };
        return;
    }
    app.focus = match app.focus {
        Focus::Categories => Focus::Settings,
        Focus::Settings => Focus::Details,
        Focus::Details => Focus::Categories,
    }
}

fn move_focus_left(app: &mut AppConfig) {
    if app.live_theme_edit {
        app.focus = Focus::Settings;
        return;
    }
    app.focus = match app.focus {
        Focus::Categories => Focus::Categories,
        Focus::Settings => Focus::Categories,
        Focus::Details => Focus::Settings,
    };
}

fn move_focus_right(app: &mut AppConfig) {
    if app.live_theme_edit {
        app.focus = Focus::Details;
        return;
    }
    app.focus = match app.focus {
        Focus::Categories => Focus::Settings,
        Focus::Settings => Focus::Details,
        Focus::Details => Focus::Details,
    };
}

fn move_up(app: &mut AppConfig, opts: &RuntimeOptions) -> Result<()> {
    match app.mode {
        Mode::Diff => app.diff_scroll = app.diff_scroll.saturating_sub(1),
        Mode::Themes => {
            let visible = filtered_theme_indices(app);
            if let Some(current) = visible.iter().position(|idx| *idx == app.selected_theme) {
                let next = current.saturating_sub(1);
                if next != current {
                    app.selected_theme = visible[next];
                    preview_selected_theme(app, opts)?;
                }
            } else if let Some(first) = visible.first().copied() {
                app.selected_theme = first;
                preview_selected_theme(app, opts)?;
            }
        }
        Mode::Keybindings => app.selected_shortcut = app.selected_shortcut.saturating_sub(1),
        _ => match app.focus {
            Focus::Categories => {
                app.selected_category = app.selected_category.saturating_sub(1);
                sync_visible_settings(app);
                app.selected_setting = 0;
                app.detail_scroll = 0;
            }
            Focus::Settings => {
                app.selected_setting = app.selected_setting.saturating_sub(1);
                app.detail_scroll = 0;
            }
            Focus::Details => app.detail_scroll = app.detail_scroll.saturating_sub(1),
        },
    }
    Ok(())
}

fn move_down(app: &mut AppConfig, opts: &RuntimeOptions) -> Result<()> {
    match app.mode {
        Mode::Diff => app.diff_scroll = app.diff_scroll.saturating_add(1),
        Mode::Themes => {
            let visible = filtered_theme_indices(app);
            if let Some(current) = visible.iter().position(|idx| *idx == app.selected_theme) {
                let next = (current + 1).min(visible.len().saturating_sub(1));
                if next != current {
                    app.selected_theme = visible[next];
                    preview_selected_theme(app, opts)?;
                }
            } else if let Some(first) = visible.first().copied() {
                app.selected_theme = first;
                preview_selected_theme(app, opts)?;
            }
        }
        Mode::Keybindings => {
            let visible = filtered_shortcut_rows(app);
            if !visible.is_empty() {
                app.selected_shortcut =
                    (app.selected_shortcut + 1).min(visible.len().saturating_sub(1))
            }
        }
        _ => match app.focus {
            Focus::Categories => {
                if !app.categories.is_empty() {
                    app.selected_category =
                        (app.selected_category + 1).min(app.categories.len() - 1);
                    sync_visible_settings(app);
                    app.selected_setting = 0;
                    app.detail_scroll = 0;
                }
            }
            Focus::Settings => {
                if !app.search_results.is_empty() {
                    app.selected_setting =
                        (app.selected_setting + 1).min(app.search_results.len() - 1)
                }
                app.detail_scroll = 0;
            }
            Focus::Details => app.detail_scroll = app.detail_scroll.saturating_add(1),
        },
    }
    Ok(())
}

fn begin_edit(app: &mut AppConfig) {
    if app.mode == Mode::Keybindings {
        begin_shortcut_edit(app);
        return;
    }

    if let Some(meta) = app.current_setting().cloned() {
        if !meta.enum_choices.is_empty() {
            let current_value = app.current_value_for(&meta.key);
            app.enum_index = meta
                .enum_choices
                .iter()
                .position(|choice| Some(choice.clone()) == current_value)
                .unwrap_or(0);
            app.edit_buffer = meta.key.clone();
            app.edit_target = Some(EditTarget::Setting(meta.key.clone()));
            app.mode = Mode::EnumPicker;
            return;
        }
        app.edit_buffer = app
            .current_value_for(&meta.key)
            .or_else(|| meta.default_value.clone())
            .unwrap_or_default();
        app.edit_target = Some(EditTarget::Setting(meta.key.clone()));
        app.mode = Mode::Edit;
    }
}

fn begin_shortcut_edit(app: &mut AppConfig) {
    let Some(row) = selected_shortcut_row(app) else {
        app.status = "no shortcut selected".into();
        return;
    };

    if row.status == crate::config_model::ShortcutStatus::Removed {
        app.status = "removed shortcuts cannot be reassigned from the TUI yet".into();
        return;
    }

    if let Some(index) = row.edited_index {
        if let Some(map) = app.edited_keymaps.get(index) {
            app.edit_buffer = map_field_value(map, KeymapField::Shortcut);
            app.edit_target = Some(EditTarget::KeymapShortcut(index));
        }
    } else {
        app.edit_buffer = row.raw_shortcut.clone();
        app.edit_target = Some(EditTarget::ShortcutOverride(row, KeymapField::Shortcut));
    }
    app.mode = Mode::Edit;
}

fn open_diff(app: &mut AppConfig) {
    let old_text = std::fs::read_to_string(&app.effective.main_file).unwrap_or_default();
    let new_text = render_output(app, SaveMode::Full);
    app.diff_lines = simple_diff(&old_text, &new_text);
    app.diff_scroll = 0;
    app.mode = Mode::Diff;
}

fn reset_current_to_default(app: &mut AppConfig) {
    if let Some(meta) = app.current_setting().cloned() {
        if let Some(default) = meta.default_value {
            app.edited_values.insert(meta.key.clone(), default);
            if app.live_theme_edit && is_theme_setting_key(&meta.key) {
                app.theme_edit_dirty = true;
            }
            app.status = format!("reset {} to default", meta.key);
        }
    }
}

fn clear_current(app: &mut AppConfig) {
    if let Some(meta) = app.current_setting().cloned() {
        app.edited_values.insert(meta.key.clone(), String::new());
        if app.live_theme_edit && is_theme_setting_key(&meta.key) {
            app.theme_edit_dirty = true;
        }
        app.status = format!("cleared {}", meta.key);
    }
}

fn save_current(
    app: &mut AppConfig,
    opts: &RuntimeOptions,
    mode: SaveMode,
    reload_after: bool,
) -> Result<()> {
    let out_path = match mode {
        SaveMode::Full => opts.out_full.as_ref().unwrap_or(&app.effective.main_file),
        SaveMode::Minimal => opts
            .out_minimal
            .as_ref()
            .unwrap_or(&app.effective.main_file),
    }
    .clone();
    let rendered = render_output_to_path(app, mode, &out_path);
    if should_backup_before_save(
        &app.effective.main_file,
        &out_path,
        mode,
        opts.create_backup,
    ) {
        let backup = backup_config_root(&out_path)?;
        app.status = format!("backup: {}", backup.display());
    }
    save_to_path(&out_path, &rendered)?;
    let mut status = format!("saved {}", out_path.display());
    let mut wrote_theme_artifact = None;
    if out_path == app.effective.main_file && uses_theme_wrapper(app) {
        wrote_theme_artifact = write_selected_theme_artifact(app, &out_path)?;
    }
    if let Some(theme_path) = wrote_theme_artifact.as_ref() {
        status = format!("{} | theme {}", status, theme_path.display());
    }
    if mode == SaveMode::Full {
        app.pending_theme_path = None;
    }
    if out_path == app.effective.main_file {
        refresh_effective_state(app)?;
    }
    if reload_after || opts.enable_reload || out_path == app.effective.main_file {
        let reload_msg = reload_config(opts.reload_command.as_deref(), &out_path)?;
        status = format!("{} | {}", status, reload_msg);
    }
    app.status = status;
    Ok(())
}

fn add_keymap(app: &mut AppConfig) {
    let entry = MapEntry {
        mode: "main".into(),
        shortcut: "ctrl+shift+t".into(),
        action: "new_tab".into(),
        option_prefix: String::new(),
        source_file: app.effective.main_file.clone(),
        line_no: 0,
    };
    app.edited_keymaps.push(entry);
    app.shortcut_view = ShortcutView::Custom;
    app.keymap_field = KeymapField::Shortcut;
    app.status = "added keymap template".into();
    select_shortcut_by_edited_index(app, app.edited_keymaps.len() - 1);
}

fn delete_keymap(app: &mut AppConfig) {
    if let Some(row) = selected_shortcut_row(app) {
        if let Some(index) = row.edited_index {
            app.edited_keymaps.remove(index);
            app.status = "deleted keymap".into();
            sync_shortcut_selection(app);
        } else {
            let entry = MapEntry {
                mode: row.mode.clone(),
                shortcut: row.raw_shortcut.clone(),
                action: String::new(),
                option_prefix: row.option_prefix.clone(),
                source_file: app.effective.main_file.clone(),
                line_no: 0,
            };
            app.edited_keymaps.push(entry);
            app.shortcut_view = ShortcutView::Custom;
            app.status = "removed default shortcut".into();
            select_shortcut_by_edited_index(app, app.edited_keymaps.len() - 1);
        }
    }
}

fn apply_selected_theme(app: &mut AppConfig) -> Result<()> {
    let Some((theme_name, theme_path)) = app
        .themes
        .get(app.selected_theme)
        .map(|theme| (theme.name.clone(), theme.path.clone()))
    else {
        app.status = "no theme selected".into();
        return Ok(());
    };
    if app
        .theme_edit_save_path
        .as_ref()
        .is_some_and(|path| !same_path(path, &theme_path))
    {
        app.theme_edit_save_path = None;
    }
    stage_theme_values_from_file(app, &theme_path)?;
    app.pending_theme_path = Some(theme_path);
    let wrapper_include = "current-theme.conf";
    let include_line = apply_theme_include(
        app.current_theme_include.as_deref(),
        Path::new(wrapper_include),
    );
    app.effective.leading_block = replace_or_insert_theme_include_in_text(
        &app.effective.leading_block,
        app.current_theme_include.as_deref(),
        &include_line,
    );
    app.current_theme_include = Some(wrapper_include.into());
    app.theme_edit_dirty = false;
    app.status = format!("theme selected: {} | press e to edit colors", theme_name);
    Ok(())
}

fn open_theme_browser(app: &mut AppConfig) {
    app.mode = Mode::Themes;
    app.live_theme_edit = false;
    app.theme_edit_dirty = false;
    app.selected_theme = find_current_theme_index(
        &app.themes,
        &app.effective,
        app.current_theme_include.as_deref(),
        &app.current_theme_artifact,
    );
    sync_theme_selection(app);
    let visible = filtered_theme_indices(app);
    app.status = if app.themes.is_empty() {
        "no themes found".into()
    } else if visible.is_empty() {
        "no themes match the current filter".into()
    } else {
        format!(
            "theme browser: {} | / filter, Up/Down preview, Enter select, e edit theme",
            app.themes[app.selected_theme].name
        )
    };
}

fn open_keybindings(app: &mut AppConfig) {
    app.mode = Mode::Keybindings;
    app.search_target = SearchTarget::Shortcuts;
    sync_shortcut_selection(app);
    let rows = shortcut_rows(app);
    let (added, changed, removed) = shortcut_status_counts(&build_shortcut_rows(
        &app.default_keymaps,
        &app.edited_keymaps,
        &current_kitty_mod(app),
        ShortcutView::Custom,
    ));
    app.status = if rows.is_empty() {
        "no shortcuts available".into()
    } else {
        format!(
            "shortcut browser: {} rows | {} added {} changed {} removed",
            rows.len(),
            added,
            changed,
            removed
        )
    };
}

fn preview_selected_theme(app: &mut AppConfig, _opts: &RuntimeOptions) -> Result<()> {
    let Some(theme) = app.themes.get(app.selected_theme) else {
        return Ok(());
    };
    let preview_status = preview_theme(&theme.path)?;
    app.theme_preview_active = true;
    app.live_theme_edit = false;
    app.theme_edit_dirty = false;
    app.status = format!("{} | {}", theme.name, preview_status);
    Ok(())
}

fn restore_theme_preview(app: &mut AppConfig, opts: &RuntimeOptions) -> Result<()> {
    if !app.theme_preview_active {
        return Ok(());
    }
    let restore_status = reload_config(opts.reload_command.as_deref(), &app.effective.main_file)?;
    app.theme_preview_active = false;
    app.live_theme_edit = false;
    app.theme_edit_dirty = false;
    app.status = format!("theme preview cleared | {}", restore_status);
    Ok(())
}

fn stage_theme_values_from_file(app: &mut AppConfig, path: &Path) -> Result<()> {
    app.theme_edit_keys = theme_setting_key_order(path)?;
    for key in app
        .metadata
        .iter()
        .filter(|meta| is_theme_setting_key(&meta.key))
        .map(|meta| meta.key.clone())
        .collect::<Vec<_>>()
    {
        app.edited_values.remove(&key);
    }

    let parsed = parse_current_config(path)?;
    for (key, values) in parsed.values {
        if !is_theme_setting_key(&key) {
            continue;
        }
        if let Some(value) = values.last() {
            app.edited_values.insert(key, value.value.clone());
        }
    }
    Ok(())
}

fn prepare_theme_selection_for_save(app: &mut AppConfig) -> Result<()> {
    if app.mode == Mode::Themes {
        apply_selected_theme(app)?;
    }
    Ok(())
}

fn refresh_effective_state(app: &mut AppConfig) -> Result<()> {
    let effective = parse_current_config(&app.effective.main_file)?;
    app.current_theme_artifact = theme_artifact_path(&effective.main_file);
    app.current_theme_include = detect_current_theme_include(&effective);
    app.selected_theme = find_current_theme_index(
        &app.themes,
        &effective,
        app.current_theme_include.as_deref(),
        &app.current_theme_artifact,
    );
    sync_theme_selection(app);
    app.edited_keymaps = effective.keymaps.clone();
    app.edited_values.clear();
    app.detail_scroll = 0;
    app.selected_shortcut = 0;
    app.pending_theme_path = None;
    app.theme_preview_active = false;
    app.live_theme_edit = false;
    app.theme_edit_keys.clear();
    app.theme_edit_dirty = false;
    app.effective = effective;
    sync_visible_settings(app);
    Ok(())
}

fn begin_theme_live_edit(app: &mut AppConfig) -> Result<()> {
    let theme_name = app
        .themes
        .get(app.selected_theme)
        .map(|theme| theme.name.clone())
        .unwrap_or_else(|| "theme".into());
    apply_selected_theme(app)?;
    app.live_theme_edit = true;
    app.theme_edit_dirty = false;
    focus_theme_editor(app);
    let preview_status = preview_current_theme_values(app)?;
    app.mode = Mode::Browse;
    app.status = format!(
        "editing theme: {} | Esc finishes and asks to save | {}",
        theme_name, preview_status
    );
    Ok(())
}

fn preview_current_theme_values(app: &mut AppConfig) -> Result<String> {
    let preview_status = preview_theme_artifact(&render_theme_artifact(app))?;
    app.theme_preview_active = true;
    Ok(preview_status)
}

fn maybe_preview_live_theme_edit(app: &mut AppConfig) -> Result<()> {
    let Some(meta) = app.current_setting().cloned() else {
        return Ok(());
    };
    if !app.live_theme_edit || !is_theme_setting_key(&meta.key) {
        return Ok(());
    }
    let preview_status = preview_current_theme_values(app)?;
    app.status = format!("{} | {}", app.status, preview_status);
    if let Some(path) = maybe_auto_save_live_theme(app)? {
        app.status = format!("{} | saved theme {}", app.status, path.display());
    }
    Ok(())
}

fn focus_theme_editor(app: &mut AppConfig) {
    app.search_query.clear();
    sync_visible_settings(app);
    app.selected_setting = 0;
    app.focus = Focus::Settings;
    app.detail_scroll = 0;
}

fn begin_theme_save_as(app: &mut AppConfig) -> Result<()> {
    if app.themes_dir.is_none() {
        app.status = "set a themes directory at startup to save custom themes".into();
        return Ok(());
    }
    if app.pending_theme_path.is_none() && !app.themes.is_empty() {
        apply_selected_theme(app)?;
    }
    app.edit_buffer = app
        .themes
        .get(app.selected_theme)
        .map(|theme| theme.name.clone())
        .unwrap_or_else(|| "custom-theme".into());
    app.edit_target = Some(EditTarget::ThemeSaveAs);
    app.mode = Mode::Edit;
    Ok(())
}

fn save_theme_as(app: &mut AppConfig) -> Result<()> {
    let Some(themes_dir) = app.themes_dir.as_ref() else {
        app.status = "no themes directory configured".into();
        app.mode = Mode::Themes;
        app.edit_target = None;
        return Ok(());
    };

    let raw_name = app.edit_buffer.trim();
    if raw_name.is_empty() {
        app.status = "theme name is required".into();
        return Ok(());
    }

    let file_name = if Path::new(raw_name).extension().is_some() {
        raw_name.to_string()
    } else {
        format!("{raw_name}.conf")
    };
    let save_path = themes_dir.join(file_name);
    save_to_path(&save_path, &render_theme_artifact(app))?;
    app.themes = discover_themes(Some(themes_dir))?;
    if let Some(idx) = app.themes.iter().position(|theme| theme.path == save_path) {
        app.selected_theme = idx;
    }
    app.pending_theme_path = Some(save_path.clone());
    app.theme_edit_save_path = Some(save_path.clone());
    app.live_theme_edit = false;
    app.theme_edit_dirty = false;
    app.theme_edit_keys = theme_setting_key_order(&save_path)?;
    app.status = format!(
        "saved theme {} | press e to edit colors",
        save_path.display()
    );
    app.mode = Mode::Themes;
    app.edit_target = None;
    Ok(())
}

fn should_backup_before_save(
    main_file: &Path,
    out_path: &Path,
    mode: SaveMode,
    create_backup: bool,
) -> bool {
    create_backup && mode == SaveMode::Full && out_path == main_file
}

fn current_kitty_mod(app: &AppConfig) -> String {
    app.current_value_for("kitty_mod")
        .unwrap_or_else(|| String::from("ctrl+shift"))
}

pub(crate) fn filtered_theme_indices(app: &AppConfig) -> Vec<usize> {
    let query = app.theme_query.trim().to_ascii_lowercase();
    app.themes
        .iter()
        .enumerate()
        .filter_map(|(idx, theme)| {
            if query.is_empty() {
                return Some(idx);
            }

            let name = theme.name.to_ascii_lowercase();
            let path = theme.path.display().to_string().to_ascii_lowercase();
            (name.contains(&query) || path.contains(&query)).then_some(idx)
        })
        .collect()
}

pub fn shortcut_rows(app: &AppConfig) -> Vec<ShortcutRow> {
    build_shortcut_rows(
        &app.default_keymaps,
        &app.edited_keymaps,
        &current_kitty_mod(app),
        app.shortcut_view,
    )
}

pub fn filtered_shortcut_rows(app: &AppConfig) -> Vec<ShortcutRow> {
    filter_shortcut_rows(&shortcut_rows(app), &app.shortcut_query)
}

fn selected_shortcut_row(app: &AppConfig) -> Option<ShortcutRow> {
    let rows = filtered_shortcut_rows(app);
    rows.get(app.selected_shortcut).cloned()
}

fn sync_theme_selection(app: &mut AppConfig) {
    let visible = filtered_theme_indices(app);
    if visible.is_empty() {
        if app.themes.is_empty() {
            app.selected_theme = 0;
        } else {
            app.selected_theme = app.selected_theme.min(app.themes.len().saturating_sub(1));
        }
    } else if !visible.contains(&app.selected_theme) {
        app.selected_theme = visible[0];
    }
}

fn sync_shortcut_selection(app: &mut AppConfig) {
    let rows = filtered_shortcut_rows(app);
    if rows.is_empty() {
        app.selected_shortcut = 0;
    } else {
        app.selected_shortcut = app.selected_shortcut.min(rows.len().saturating_sub(1));
    }
}

fn select_shortcut_by_edited_index(app: &mut AppConfig, edited_index: usize) {
    let rows = filtered_shortcut_rows(app);
    if let Some(idx) = rows
        .iter()
        .position(|row| row.edited_index == Some(edited_index))
    {
        app.selected_shortcut = idx;
    } else {
        sync_shortcut_selection(app);
    }
}

fn finish_theme_edit_session(app: &mut AppConfig) -> Result<()> {
    if app.theme_edit_dirty && app.theme_edit_save_path.is_none() {
        app.live_theme_edit = false;
        app.theme_edit_dirty = false;
        begin_theme_save_as(app)?;
        app.status = "save edited theme as a new preset".into();
    } else {
        app.live_theme_edit = false;
        app.theme_edit_dirty = false;
        app.search_query.clear();
        open_theme_browser(app);
    }
    Ok(())
}

fn maybe_auto_save_live_theme(app: &mut AppConfig) -> Result<Option<PathBuf>> {
    if !app.live_theme_edit {
        return Ok(None);
    }
    let Some(path) = app.theme_edit_save_path.clone() else {
        return Ok(None);
    };
    let Some(meta) = app.current_setting() else {
        return Ok(None);
    };
    if !is_theme_setting_key(&meta.key) {
        return Ok(None);
    }

    save_to_path(&path, &render_theme_artifact(app))?;
    Ok(Some(path))
}

fn theme_setting_key_order(path: &Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    let mut keys = Vec::new();
    let mut seen = HashSet::new();
    for raw in text.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(key) = trimmed.split_whitespace().next() else {
            continue;
        };
        if is_theme_setting_key(key) && seen.insert(key.to_string()) {
            keys.push(key.to_string());
        }
    }
    Ok(keys)
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn sync_visible_settings(app: &mut AppConfig) {
    if app.live_theme_edit {
        sync_theme_edit_settings(app);
        return;
    }

    if !app.search_query.trim().is_empty() {
        app.search_results = fuzzy_search::search(&app.metadata, &app.search_query);
        return;
    }

    if let Some(category) = app.categories.get(app.selected_category) {
        app.search_results = app
            .metadata
            .iter()
            .enumerate()
            .filter_map(|(idx, meta)| (meta.category == *category).then_some(idx))
            .collect();
    } else {
        app.search_results = (0..app.metadata.len()).collect();
    }
}

fn sync_theme_edit_settings(app: &mut AppConfig) {
    if !app.search_query.trim().is_empty() {
        app.search_results = fuzzy_search::search(&app.metadata, &app.search_query)
            .into_iter()
            .filter(|idx| {
                app.metadata
                    .get(*idx)
                    .is_some_and(|meta| is_theme_setting_key(&meta.key))
            })
            .collect();
        return;
    }

    let mut ordered = Vec::new();
    let mut seen = HashSet::new();

    for key in &app.theme_edit_keys {
        if let Some((idx, _)) = app
            .metadata
            .iter()
            .enumerate()
            .find(|(_, meta)| meta.key == *key && is_theme_setting_key(&meta.key))
        {
            if seen.insert(idx) {
                ordered.push(idx);
            }
        }
    }

    for (idx, meta) in app.metadata.iter().enumerate() {
        if is_theme_setting_key(&meta.key) && seen.insert(idx) {
            ordered.push(idx);
        }
    }

    app.search_results = ordered;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_model::{
        EffectiveConfig, KeymapField, Mode, ShortcutStatus, ThemeEntry, ValueType,
    };
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn app_for_diff() -> AppConfig {
        let meta = SettingMetadata {
            key: "font_size".into(),
            category: "Fonts".into(),
            description: String::new(),
            default_value: Some("11.0".into()),
            examples: vec![],
            enum_choices: vec![],
            repeatable: false,
            value_type: ValueType::Number,
            order: 0,
        };
        let metadata_by_key = HashMap::from([(meta.key.clone(), meta.clone())]);
        AppConfig {
            metadata: vec![meta],
            metadata_by_key,
            effective: EffectiveConfig {
                values: HashMap::new(),
                keymaps: vec![],
                includes: vec![],
                leading_block: String::new(),
                main_file: PathBuf::from("kitty.conf"),
            },
            default_keymaps: vec![],
            edited_values: HashMap::new(),
            edited_keymaps: vec![],
            selected_category: 0,
            selected_setting: 0,
            categories: vec!["Fonts".into()],
            search_query: String::new(),
            search_target: SearchTarget::Settings,
            search_results: vec![0],
            theme_query: String::new(),
            mode: Mode::Browse,
            focus: Focus::Settings,
            edit_buffer: String::new(),
            edit_target: None,
            enum_index: 0,
            keymap_field: KeymapField::Shortcut,
            detail_scroll: 0,
            diff_scroll: 0,
            status: String::new(),
            diff_lines: vec![],
            themes: vec![ThemeEntry {
                name: "x".into(),
                path: PathBuf::from("x.conf"),
                preview: vec![],
            }],
            themes_dir: None,
            current_theme_artifact: PathBuf::from("current-theme.conf"),
            selected_theme: 0,
            shortcut_view: ShortcutView::Custom,
            shortcut_query: String::new(),
            selected_shortcut: 0,
            pending_quit: false,
            current_theme_include: None,
            pending_theme_path: None,
            theme_edit_save_path: None,
            theme_preview_active: false,
            live_theme_edit: false,
            theme_edit_keys: vec![],
            theme_edit_dirty: false,
        }
    }

    #[test]
    fn backs_up_when_writing_main_file() {
        assert!(should_backup_before_save(
            Path::new("/tmp/kitty.conf"),
            Path::new("/tmp/kitty.conf"),
            SaveMode::Full,
            true,
        ));
    }

    #[test]
    fn skips_backup_for_custom_full_output() {
        assert!(!should_backup_before_save(
            Path::new("/home/rabbit/.config/kitty/kitty.conf"),
            Path::new("/tmp/kitty.out.conf"),
            SaveMode::Full,
            true,
        ));
    }

    #[test]
    fn never_backs_up_minimal_saves() {
        assert!(!should_backup_before_save(
            Path::new("/tmp/kitty.conf"),
            Path::new("/tmp/kitty.conf"),
            SaveMode::Minimal,
            true,
        ));
    }

    #[test]
    fn skips_backup_when_startup_checkbox_is_off() {
        assert!(!should_backup_before_save(
            Path::new("/tmp/kitty.conf"),
            Path::new("/tmp/kitty.conf"),
            SaveMode::Full,
            false,
        ));
    }

    #[test]
    fn diff_mode_uses_up_down_for_scroll() {
        let mut app = app_for_diff();
        app.mode = Mode::Diff;
        let opts = RuntimeOptions {
            out_full: None,
            out_minimal: None,
            create_backup: true,
            enable_reload: false,
            reload_command: None,
        };

        move_down(&mut app, &opts).expect("scroll down");
        move_down(&mut app, &opts).expect("scroll down twice");
        move_up(&mut app, &opts).expect("scroll up");

        assert_eq!(app.diff_scroll, 1);
    }

    #[test]
    fn open_diff_resets_scroll_offset() {
        let dir = tempdir().expect("tempdir");
        let main_file = dir.path().join("kitty.conf");
        fs::write(&main_file, "font_size 12.0\n").expect("write config");

        let mut app = app_for_diff();
        app.effective.main_file = main_file;
        app.edited_values.insert("font_size".into(), "13.0".into());
        app.diff_scroll = 7;

        open_diff(&mut app);

        assert_eq!(app.mode, Mode::Diff);
        assert_eq!(app.diff_scroll, 0);
        assert!(!app.diff_lines.is_empty());
    }

    #[test]
    fn left_right_focus_moves_between_panes() {
        let mut app = app_for_diff();
        app.focus = Focus::Categories;

        move_focus_right(&mut app);
        assert_eq!(app.focus, Focus::Settings);

        move_focus_right(&mut app);
        assert_eq!(app.focus, Focus::Details);

        move_focus_left(&mut app);
        assert_eq!(app.focus, Focus::Settings);
    }

    #[test]
    fn focus_theme_editor_clears_search_and_selects_first_theme_key() {
        let font_meta = SettingMetadata {
            key: "font_size".into(),
            category: "Fonts".into(),
            description: String::new(),
            default_value: Some("12.0".into()),
            examples: vec![],
            enum_choices: vec![],
            repeatable: false,
            value_type: ValueType::Number,
            order: 0,
        };
        let color_meta = SettingMetadata {
            key: "foreground".into(),
            category: "Colors".into(),
            description: String::new(),
            default_value: Some("#ebdbb2".into()),
            examples: vec![],
            enum_choices: vec![],
            repeatable: false,
            value_type: ValueType::Color,
            order: 1,
        };
        let mut app = AppConfig {
            metadata: vec![font_meta.clone(), color_meta.clone()],
            metadata_by_key: HashMap::from([
                (font_meta.key.clone(), font_meta),
                (color_meta.key.clone(), color_meta),
            ]),
            effective: EffectiveConfig {
                values: HashMap::new(),
                keymaps: vec![],
                includes: vec![],
                leading_block: String::new(),
                main_file: PathBuf::from("kitty.conf"),
            },
            default_keymaps: vec![],
            edited_values: HashMap::new(),
            edited_keymaps: vec![],
            selected_category: 0,
            selected_setting: 0,
            categories: vec!["Fonts".into(), "Colors".into()],
            search_query: "font".into(),
            search_target: SearchTarget::Settings,
            search_results: vec![0],
            theme_query: String::new(),
            mode: Mode::Browse,
            focus: Focus::Categories,
            edit_buffer: String::new(),
            edit_target: None,
            enum_index: 0,
            keymap_field: KeymapField::Shortcut,
            detail_scroll: 0,
            diff_scroll: 0,
            status: String::new(),
            diff_lines: vec![],
            themes: vec![ThemeEntry {
                name: "x".into(),
                path: PathBuf::from("x.conf"),
                preview: vec![],
            }],
            themes_dir: None,
            current_theme_artifact: PathBuf::from("current-theme.conf"),
            selected_theme: 0,
            shortcut_view: ShortcutView::Custom,
            shortcut_query: String::new(),
            selected_shortcut: 0,
            pending_quit: false,
            current_theme_include: None,
            pending_theme_path: None,
            theme_edit_save_path: None,
            theme_preview_active: false,
            live_theme_edit: true,
            theme_edit_keys: vec!["foreground".into()],
            theme_edit_dirty: false,
        };

        focus_theme_editor(&mut app);

        assert_eq!(app.focus, Focus::Settings);
        assert!(app.search_query.is_empty());
        assert_eq!(app.search_results, vec![1]);
        assert_eq!(
            app.current_setting().map(|meta| meta.key.as_str()),
            Some("foreground")
        );
    }

    #[test]
    fn focus_theme_editor_keeps_theme_file_keys_first_then_rest() {
        let tab_meta = SettingMetadata {
            key: "active_tab_foreground".into(),
            category: "Tab bar".into(),
            description: String::new(),
            default_value: Some("#000000".into()),
            examples: vec![],
            enum_choices: vec![],
            repeatable: false,
            value_type: ValueType::Color,
            order: 0,
        };
        let color_meta = SettingMetadata {
            key: "foreground".into(),
            category: "Color scheme".into(),
            description: String::new(),
            default_value: Some("#ebdbb2".into()),
            examples: vec![],
            enum_choices: vec![],
            repeatable: false,
            value_type: ValueType::Color,
            order: 1,
        };
        let mut app = AppConfig {
            metadata: vec![tab_meta.clone(), color_meta.clone()],
            metadata_by_key: HashMap::from([
                (tab_meta.key.clone(), tab_meta),
                (color_meta.key.clone(), color_meta),
            ]),
            effective: EffectiveConfig {
                values: HashMap::new(),
                keymaps: vec![],
                includes: vec![],
                leading_block: String::new(),
                main_file: PathBuf::from("kitty.conf"),
            },
            default_keymaps: vec![],
            edited_values: HashMap::new(),
            edited_keymaps: vec![],
            selected_category: 0,
            selected_setting: 0,
            categories: vec!["Tab bar".into(), "Color scheme".into()],
            search_query: "tab".into(),
            search_target: SearchTarget::Settings,
            search_results: vec![0],
            theme_query: String::new(),
            mode: Mode::Browse,
            focus: Focus::Categories,
            edit_buffer: String::new(),
            edit_target: None,
            enum_index: 0,
            keymap_field: KeymapField::Shortcut,
            detail_scroll: 0,
            diff_scroll: 0,
            status: String::new(),
            diff_lines: vec![],
            themes: vec![ThemeEntry {
                name: "x".into(),
                path: PathBuf::from("x.conf"),
                preview: vec![],
            }],
            themes_dir: None,
            current_theme_artifact: PathBuf::from("current-theme.conf"),
            selected_theme: 0,
            shortcut_view: ShortcutView::Custom,
            shortcut_query: String::new(),
            selected_shortcut: 0,
            pending_quit: false,
            current_theme_include: None,
            pending_theme_path: None,
            theme_edit_save_path: None,
            theme_preview_active: false,
            live_theme_edit: true,
            theme_edit_keys: vec!["foreground".into()],
            theme_edit_dirty: false,
        };

        focus_theme_editor(&mut app);

        assert_eq!(app.search_results, vec![1, 0]);
        assert_eq!(
            app.current_setting().map(|meta| meta.key.as_str()),
            Some("foreground")
        );
    }

    #[test]
    fn theme_browser_search_uses_slash_filter() {
        let mut app = app_for_diff();
        app.mode = Mode::Themes;
        app.themes = vec![
            ThemeEntry {
                name: "gruvbox".into(),
                path: PathBuf::from("gruvbox.conf"),
                preview: vec![],
            },
            ThemeEntry {
                name: "tokyonight".into(),
                path: PathBuf::from("tokyonight.conf"),
                preview: vec![],
            },
            ThemeEntry {
                name: "nightfox".into(),
                path: PathBuf::from("nightfox.conf"),
                preview: vec![],
            },
        ];
        app.selected_theme = 2;

        let opts = RuntimeOptions {
            out_full: None,
            out_minimal: None,
            create_backup: false,
            enable_reload: false,
            reload_command: None,
        };

        handle_key(
            &mut app,
            &opts,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        )
        .expect("enter theme search");
        handle_key(
            &mut app,
            &opts,
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        )
        .expect("filter themes");
        handle_key(
            &mut app,
            &opts,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        )
        .expect("refine theme filter");
        handle_key(
            &mut app,
            &opts,
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE),
        )
        .expect("refine theme filter again");

        assert_eq!(app.mode, Mode::Search);
        assert_eq!(app.search_target, SearchTarget::Themes);
        assert_eq!(app.theme_query, "gru");
        assert_eq!(filtered_theme_indices(&app), vec![0]);
        assert_eq!(app.selected_theme, 0);

        handle_key(
            &mut app,
            &opts,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        )
        .expect("apply theme filter");

        assert_eq!(app.mode, Mode::Themes);
    }

    #[test]
    fn live_theme_edit_auto_saves_saved_theme_path() {
        let dir = tempdir().expect("tempdir");
        let save_path = dir.path().join("custom.conf");
        let color_meta = SettingMetadata {
            key: "foreground".into(),
            category: "Colors".into(),
            description: String::new(),
            default_value: Some("#ebdbb2".into()),
            examples: vec![],
            enum_choices: vec![],
            repeatable: false,
            value_type: ValueType::Color,
            order: 0,
        };
        let mut app = AppConfig {
            metadata: vec![color_meta.clone()],
            metadata_by_key: HashMap::from([(color_meta.key.clone(), color_meta)]),
            effective: EffectiveConfig {
                values: HashMap::new(),
                keymaps: vec![],
                includes: vec![],
                leading_block: String::new(),
                main_file: PathBuf::from("kitty.conf"),
            },
            default_keymaps: vec![],
            edited_values: HashMap::from([("foreground".into(), "#ffffff".into())]),
            edited_keymaps: vec![],
            selected_category: 0,
            selected_setting: 0,
            categories: vec!["Colors".into()],
            search_query: String::new(),
            search_target: SearchTarget::Settings,
            search_results: vec![0],
            theme_query: String::new(),
            mode: Mode::Browse,
            focus: Focus::Settings,
            edit_buffer: String::new(),
            edit_target: None,
            enum_index: 0,
            keymap_field: KeymapField::Shortcut,
            detail_scroll: 0,
            diff_scroll: 0,
            status: String::new(),
            diff_lines: vec![],
            themes: vec![],
            themes_dir: None,
            current_theme_artifact: PathBuf::from("current-theme.conf"),
            selected_theme: 0,
            shortcut_view: ShortcutView::Custom,
            shortcut_query: String::new(),
            selected_shortcut: 0,
            pending_quit: false,
            current_theme_include: Some("current-theme.conf".into()),
            pending_theme_path: None,
            theme_edit_save_path: Some(save_path.clone()),
            theme_preview_active: false,
            live_theme_edit: true,
            theme_edit_keys: vec!["foreground".into()],
            theme_edit_dirty: false,
        };

        maybe_preview_live_theme_edit(&mut app).expect("preview live edit");

        let body = fs::read_to_string(save_path).expect("read saved theme");
        assert!(body.contains("foreground #ffffff"));
    }

    #[test]
    fn stage_theme_values_from_file_keeps_file_order_first() {
        let dir = tempdir().expect("tempdir");
        let theme_path = dir.path().join("ordered.conf");
        fs::write(
            &theme_path,
            "background #000000\nforeground #ffffff\ncursor #ff00ff\n",
        )
        .expect("write theme");

        let foreground = SettingMetadata {
            key: "foreground".into(),
            category: "Colors".into(),
            description: String::new(),
            default_value: Some("#ebdbb2".into()),
            examples: vec![],
            enum_choices: vec![],
            repeatable: false,
            value_type: ValueType::Color,
            order: 0,
        };
        let background = SettingMetadata {
            key: "background".into(),
            category: "Colors".into(),
            description: String::new(),
            default_value: Some("#282828".into()),
            examples: vec![],
            enum_choices: vec![],
            repeatable: false,
            value_type: ValueType::Color,
            order: 1,
        };
        let cursor = SettingMetadata {
            key: "cursor".into(),
            category: "Colors".into(),
            description: String::new(),
            default_value: Some("#d0d0d0".into()),
            examples: vec![],
            enum_choices: vec![],
            repeatable: false,
            value_type: ValueType::Color,
            order: 2,
        };
        let selection = SettingMetadata {
            key: "selection_background".into(),
            category: "Colors".into(),
            description: String::new(),
            default_value: Some("#444444".into()),
            examples: vec![],
            enum_choices: vec![],
            repeatable: false,
            value_type: ValueType::Color,
            order: 3,
        };
        let mut app = AppConfig {
            metadata: vec![
                foreground.clone(),
                background.clone(),
                cursor.clone(),
                selection.clone(),
            ],
            metadata_by_key: HashMap::from([
                (foreground.key.clone(), foreground),
                (background.key.clone(), background),
                (cursor.key.clone(), cursor),
                (selection.key.clone(), selection),
            ]),
            effective: EffectiveConfig {
                values: HashMap::new(),
                keymaps: vec![],
                includes: vec![],
                leading_block: String::new(),
                main_file: PathBuf::from("kitty.conf"),
            },
            default_keymaps: vec![],
            edited_values: HashMap::new(),
            edited_keymaps: vec![],
            selected_category: 0,
            selected_setting: 0,
            categories: vec!["Colors".into()],
            search_query: String::new(),
            search_target: SearchTarget::Settings,
            search_results: vec![],
            theme_query: String::new(),
            mode: Mode::Browse,
            focus: Focus::Settings,
            edit_buffer: String::new(),
            edit_target: None,
            enum_index: 0,
            keymap_field: KeymapField::Shortcut,
            detail_scroll: 0,
            diff_scroll: 0,
            status: String::new(),
            diff_lines: vec![],
            themes: vec![],
            themes_dir: None,
            current_theme_artifact: PathBuf::from("current-theme.conf"),
            selected_theme: 0,
            shortcut_view: ShortcutView::Custom,
            shortcut_query: String::new(),
            selected_shortcut: 0,
            pending_quit: false,
            current_theme_include: None,
            pending_theme_path: None,
            theme_edit_save_path: None,
            theme_preview_active: false,
            live_theme_edit: true,
            theme_edit_keys: vec![],
            theme_edit_dirty: false,
        };

        stage_theme_values_from_file(&mut app, &theme_path).expect("stage theme values");
        sync_visible_settings(&mut app);

        assert_eq!(
            app.search_results,
            vec![1, 0, 2, 3],
            "theme file keys should stay first, then remaining theme settings"
        );
        assert_eq!(
            app.current_value_for("background").as_deref(),
            Some("#000000")
        );
        assert_eq!(
            app.current_value_for("foreground").as_deref(),
            Some("#ffffff")
        );
    }

    #[test]
    fn finish_theme_edit_session_prompts_save_as_for_unsaved_changes() {
        let dir = tempdir().expect("tempdir");
        let themes_dir = dir.path().join("themes");
        fs::create_dir_all(&themes_dir).expect("create themes dir");
        let theme_path = themes_dir.join("gruvbox.conf");
        fs::write(&theme_path, "foreground #ffffff\n").expect("write theme");

        let mut app = app_for_diff();
        app.themes_dir = Some(themes_dir);
        app.themes = vec![ThemeEntry {
            name: "gruvbox".into(),
            path: theme_path.clone(),
            preview: vec![],
        }];
        app.pending_theme_path = Some(theme_path);
        app.live_theme_edit = true;
        app.theme_edit_dirty = true;

        finish_theme_edit_session(&mut app).expect("finish theme edit");

        assert_eq!(app.mode, Mode::Edit);
        assert_eq!(app.edit_target, Some(EditTarget::ThemeSaveAs));
        assert_eq!(app.edit_buffer, "gruvbox");
        assert!(!app.live_theme_edit);
    }

    #[test]
    fn select_shortcut_by_edited_index_keeps_mode_specific_row() {
        let mut app = app_for_diff();
        app.mode = Mode::Keybindings;
        app.shortcut_view = ShortcutView::Custom;
        app.edited_keymaps = vec![
            MapEntry {
                mode: "resize".into(),
                shortcut: "ctrl+t".into(),
                action: "resize_window wider".into(),
                option_prefix: "--mode=resize".into(),
                source_file: PathBuf::from("kitty.conf"),
                line_no: 1,
            },
            MapEntry {
                mode: "main".into(),
                shortcut: "ctrl+t".into(),
                action: "new_tab".into(),
                option_prefix: String::new(),
                source_file: PathBuf::from("kitty.conf"),
                line_no: 2,
            },
        ];

        select_shortcut_by_edited_index(&mut app, 1);

        let row = selected_shortcut_row(&app).expect("selected row");
        assert_eq!(row.mode, "main");
        assert_eq!(row.action, "new_tab");
        assert_eq!(row.edited_index, Some(1));
    }

    #[test]
    fn begin_shortcut_edit_blocks_removed_rows() {
        let mut app = app_for_diff();
        app.mode = Mode::Keybindings;
        app.shortcut_view = ShortcutView::Custom;
        app.edited_keymaps = vec![MapEntry {
            mode: "main".into(),
            shortcut: "ctrl+t".into(),
            action: String::new(),
            option_prefix: String::new(),
            source_file: PathBuf::from("kitty.conf"),
            line_no: 1,
        }];

        begin_shortcut_edit(&mut app);

        assert_eq!(app.mode, Mode::Keybindings);
        assert_eq!(
            app.status,
            "removed shortcuts cannot be reassigned from the TUI yet"
        );
        assert!(matches!(
            selected_shortcut_row(&app).as_ref().map(|row| row.status),
            Some(ShortcutStatus::Removed)
        ));
    }

    #[test]
    fn begin_shortcut_edit_targets_shortcut_field_only() {
        let mut app = app_for_diff();
        app.mode = Mode::Keybindings;
        app.shortcut_view = ShortcutView::Custom;
        app.edited_keymaps = vec![MapEntry {
            mode: "main".into(),
            shortcut: "ctrl+t".into(),
            action: "new_tab".into(),
            option_prefix: String::new(),
            source_file: PathBuf::from("kitty.conf"),
            line_no: 1,
        }];

        begin_shortcut_edit(&mut app);

        assert_eq!(app.mode, Mode::Edit);
        assert_eq!(app.edit_buffer, "ctrl+t");
        assert_eq!(app.edit_target, Some(EditTarget::KeymapShortcut(0)));
    }

    #[test]
    fn applying_different_theme_clears_auto_save_target() {
        let dir = tempdir().expect("tempdir");
        let saved_path = dir.path().join("saved.conf");
        let builtin_path = dir.path().join("builtin.conf");
        fs::write(&saved_path, "foreground #ffffff\nbackground #000000\n").expect("write saved");
        fs::write(&builtin_path, "foreground #ebdbb2\nbackground #282828\n")
            .expect("write builtin");

        let mut app = app_for_diff();
        app.themes = vec![
            ThemeEntry {
                name: "saved".into(),
                path: saved_path.clone(),
                preview: vec![],
            },
            ThemeEntry {
                name: "builtin".into(),
                path: builtin_path.clone(),
                preview: vec![],
            },
        ];
        app.selected_theme = 1;
        app.theme_edit_save_path = Some(saved_path);

        apply_selected_theme(&mut app).expect("apply theme");

        assert!(app.theme_edit_save_path.is_none());
    }
}
