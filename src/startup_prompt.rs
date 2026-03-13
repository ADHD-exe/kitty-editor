use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::cli::Cli;
use crate::runtime::TerminalSession;

#[derive(Debug)]
pub struct StartupPaths {
    pub current: PathBuf,
    pub themes_dir: Option<PathBuf>,
    pub create_backup: bool,
    pub create_new: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptField {
    Current,
    NewConfigDir,
    CreateBackup,
    CreateNew,
    Themes,
}

struct StartupPromptState {
    current_input: String,
    new_config_input: String,
    themes_input: String,
    create_backup: bool,
    create_new: bool,
    focus: PromptField,
    status: String,
}

pub fn resolve_startup_paths(
    terminal: &mut TerminalSession,
    cli: &Cli,
    tick_rate: Duration,
) -> Result<StartupPaths> {
    if cli.current.is_some() && cli.themes_dir.is_some() {
        let paths = validate_paths(
            cli.current
                .as_deref()
                .map(display_path)
                .as_deref()
                .unwrap_or_default(),
            cli.current
                .as_deref()
                .and_then(|path| path.parent())
                .map(display_path)
                .as_deref()
                .unwrap_or_default(),
            true,
            false,
            cli.themes_dir
                .as_deref()
                .map(display_path)
                .as_deref()
                .unwrap_or_default(),
        )?;
        persist_last_current_input(&paths.current);
        return Ok(paths);
    }

    let mut state = StartupPromptState {
        current_input: cli
            .current
            .as_deref()
            .map(display_path)
            .or_else(load_saved_current_input)
            .unwrap_or_else(default_current_input),
        new_config_input: cli
            .current
            .as_deref()
            .and_then(|path| path.parent())
            .map(display_path)
            .or_else(load_saved_new_config_input)
            .unwrap_or_else(default_new_config_input),
        themes_input: cli
            .themes_dir
            .as_deref()
            .map(display_path)
            .unwrap_or_else(default_themes_input),
        create_backup: true,
        create_new: false,
        focus: if cli.current.is_some() {
            PromptField::Themes
        } else {
            PromptField::Current
        },
        status: String::from(
            "Choose an existing config or a new-config directory, then set the backup option and optional themes directory.",
        ),
    };

    loop {
        terminal
            .terminal_mut()
            .draw(|frame| render_prompt(frame, &state))?;

        if !event::poll(tick_rate)? {
            continue;
        }

        match event::read()? {
            Event::Key(key) => {
                if let Some(result) = handle_key(&mut state, key)? {
                    return Ok(result);
                }
            }
            Event::Paste(text) => {
                if let Some(input) = active_input_mut(&mut state) {
                    input.push_str(&text);
                }
            }
            Event::Resize(_, _) | Event::FocusGained | Event::FocusLost | Event::Mouse(_) => {}
        }
    }
}

fn handle_key(state: &mut StartupPromptState, key: KeyEvent) -> Result<Option<StartupPaths>> {
    if matches!(key.kind, KeyEventKind::Release) {
        return Ok(None);
    }

    match key.code {
        KeyCode::Esc => return Err(anyhow!("startup cancelled")),
        KeyCode::Tab | KeyCode::Down => {
            state.focus = next_field(state);
        }
        KeyCode::Up => {
            state.focus = previous_field(state);
        }
        KeyCode::Enter => match state.focus {
            PromptField::Current | PromptField::NewConfigDir => {
                state.focus = next_field(state);
            }
            PromptField::CreateBackup => {
                state.create_backup = !state.create_backup;
            }
            PromptField::CreateNew => {
                toggle_create_new(state);
            }
            PromptField::Themes => match validate_paths(
                &state.current_input,
                &state.new_config_input,
                state.create_backup,
                state.create_new,
                &state.themes_input,
            ) {
                Ok(paths) => {
                    persist_last_current_input(&paths.current);
                    return Ok(Some(paths));
                }
                Err(err) => state.status = err.to_string(),
            },
        },
        KeyCode::Backspace => {
            if let Some(input) = active_input_mut(state) {
                input.pop();
            }
        }
        KeyCode::Char(ch) => {
            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                match state.focus {
                    PromptField::CreateBackup if matches!(ch, ' ' | 'x' | 'X') => {
                        state.create_backup = !state.create_backup;
                    }
                    PromptField::CreateNew if matches!(ch, ' ' | 'x' | 'X') => {
                        toggle_create_new(state);
                    }
                    _ => {
                        if let Some(input) = active_input_mut(state) {
                            input.push(ch);
                        }
                    }
                }
            }
        }
        KeyCode::Left | KeyCode::Right if matches!(state.focus, PromptField::CreateBackup) => {
            state.create_backup = !state.create_backup;
        }
        KeyCode::Left | KeyCode::Right if matches!(state.focus, PromptField::CreateNew) => {
            toggle_create_new(state);
        }
        _ => {}
    }

    Ok(None)
}

fn validate_paths(
    current_input: &str,
    new_config_input: &str,
    create_backup: bool,
    create_new: bool,
    themes_input: &str,
) -> Result<StartupPaths> {
    let current = if create_new {
        resolve_new_config_path(new_config_input)?
    } else {
        let current = resolve_current_path(current_input)?;
        if !current.exists() {
            return Err(anyhow!(
                "current config not found: {} (enable Create new config from defaults to initialize it)",
                current.display()
            ));
        }
        current
    };
    if create_new && current.exists() {
        return Err(anyhow!(
            "new config target already exists: {}",
            current.display()
        ));
    }
    let themes_dir = resolve_themes_path(themes_input)?;
    Ok(StartupPaths {
        current,
        themes_dir,
        create_backup,
        create_new,
    })
}

fn resolve_current_path(raw: &str) -> Result<PathBuf> {
    let trimmed = raw.trim();
    let expanded = if trimmed.is_empty() {
        default_current_path()
    } else {
        expand_path(trimmed)
    };
    let resolved = if expanded.is_dir() {
        expanded.join("kitty.conf")
    } else {
        expanded
    };

    if resolved.exists() && !resolved.is_file() {
        Err(anyhow!(
            "current config path is not a file: {}",
            resolved.display()
        ))
    } else {
        Ok(resolved)
    }
}

fn resolve_new_config_path(raw: &str) -> Result<PathBuf> {
    Ok(resolve_new_config_dir(raw)?.join("kitty.conf"))
}

fn resolve_new_config_dir(raw: &str) -> Result<PathBuf> {
    let trimmed = raw.trim();
    let expanded = if trimmed.is_empty() {
        default_new_config_dir()
    } else {
        expand_path(trimmed)
    };

    if looks_like_conf_path(&expanded) {
        return Err(anyhow!(
            "new config target must be a directory, not a kitty.conf path: {}",
            expanded.display()
        ));
    }

    if expanded.exists() && !expanded.is_dir() {
        Err(anyhow!(
            "new config target is not a directory: {}",
            expanded.display()
        ))
    } else {
        Ok(expanded)
    }
}

fn resolve_themes_path(raw: &str) -> Result<Option<PathBuf>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let expanded = expand_path(trimmed);
    if expanded.is_dir() {
        Ok(Some(expanded))
    } else {
        Err(anyhow!(
            "themes directory not found: {}",
            expanded.display()
        ))
    }
}

fn expand_path(raw: &str) -> PathBuf {
    PathBuf::from(shellexpand::tilde(raw).into_owned())
}

fn active_input_mut(state: &mut StartupPromptState) -> Option<&mut String> {
    match state.focus {
        PromptField::Current if !state.create_new => Some(&mut state.current_input),
        PromptField::NewConfigDir if state.create_new => Some(&mut state.new_config_input),
        PromptField::Current | PromptField::NewConfigDir => None,
        PromptField::CreateBackup => None,
        PromptField::CreateNew => None,
        PromptField::Themes => Some(&mut state.themes_input),
    }
}

fn next_field(state: &StartupPromptState) -> PromptField {
    match state.focus {
        PromptField::Current => PromptField::CreateBackup,
        PromptField::NewConfigDir => PromptField::CreateBackup,
        PromptField::CreateBackup => PromptField::CreateNew,
        PromptField::CreateNew => PromptField::Themes,
        PromptField::Themes => {
            if state.create_new {
                PromptField::NewConfigDir
            } else {
                PromptField::Current
            }
        }
    }
}

fn previous_field(state: &StartupPromptState) -> PromptField {
    match state.focus {
        PromptField::Current => PromptField::Themes,
        PromptField::NewConfigDir => PromptField::CreateNew,
        PromptField::CreateBackup => {
            if state.create_new {
                PromptField::NewConfigDir
            } else {
                PromptField::Current
            }
        }
        PromptField::CreateNew => PromptField::CreateBackup,
        PromptField::Themes => PromptField::CreateNew,
    }
}

fn default_current_input() -> String {
    String::from("~/.config/kitty/kitty.conf")
}

fn default_current_path() -> PathBuf {
    expand_path("~/.config/kitty/kitty.conf")
}

fn default_themes_input() -> String {
    String::from("~/.config/kitty/themes")
}

fn default_new_config_input() -> String {
    String::from("~/.config/kitty")
}

fn default_new_config_dir() -> PathBuf {
    expand_path("~/.config/kitty")
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn load_saved_current_input() -> Option<String> {
    let path = startup_state_file()?;
    load_saved_current_input_from(&path)
}

fn load_saved_new_config_input() -> Option<String> {
    let saved = load_saved_current_input()?;
    PathBuf::from(saved)
        .parent()
        .map(display_path)
        .or_else(|| Some(default_new_config_input()))
}

fn load_saved_current_input_from(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn persist_last_current_input(current: &Path) {
    let Some(path) = startup_state_file() else {
        return;
    };
    let _ = persist_last_current_input_to(&path, current);
}

fn persist_last_current_input_to(path: &Path, current: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, current.display().to_string())?;
    Ok(())
}

fn startup_state_file() -> Option<PathBuf> {
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(root.join("kitty-config-editor").join("last-current-path"))
}

fn toggle_create_new(state: &mut StartupPromptState) {
    state.create_new = !state.create_new;
    state.focus = if state.create_new {
        PromptField::NewConfigDir
    } else {
        PromptField::Current
    };
}

fn looks_like_conf_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("kitty.conf"))
}

fn render_prompt(frame: &mut ratatui::Frame, state: &StartupPromptState) {
    let root = frame.area();
    let popup = centered_rect(root, 84, 72);
    frame.render_widget(Clear, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .split(popup);

    let title = Paragraph::new(vec![
        Line::from(Span::styled(
            "kitty-config-editor",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::raw("Live edit your Kitty terminal configuration file and related theme setup."),
        Line::raw("Press Enter to accept a field, Space/Enter toggles checkboxes, Tab/Up/Down to switch fields, Esc to cancel."),
    ])
    .block(Block::default().borders(Borders::ALL).title("Startup"));
    frame.render_widget(title, rows[0]);

    frame.render_widget(
        field_widget(
            "Current config path",
            &state.current_input,
            state.focus == PromptField::Current,
            !state.create_new,
        ),
        rows[1],
    );
    frame.render_widget(
        checkbox_widget(
            "Create a Backup",
            state.create_backup,
            state.focus == PromptField::CreateBackup,
        ),
        rows[2],
    );
    frame.render_widget(
        checkbox_widget(
            "Create a new config from defaults",
            state.create_new,
            state.focus == PromptField::CreateNew,
        ),
        rows[3],
    );
    frame.render_widget(
        field_widget(
            "New config directory",
            &state.new_config_input,
            state.focus == PromptField::NewConfigDir,
            state.create_new,
        ),
        rows[4],
    );
    frame.render_widget(
        field_widget(
            "Themes directory (blank disables theme browser)",
            &state.themes_input,
            state.focus == PromptField::Themes,
            true,
        ),
        rows[5],
    );

    let help = Paragraph::new(vec![
        Line::raw("Use either an existing kitty.conf path or a new-config directory."),
        Line::raw("The inactive path field is greyed out until its checkbox mode is selected."),
        Line::raw("New config mode creates kitty.conf inside the chosen directory."),
        Line::raw("Leave Themes blank to disable the theme browser."),
    ])
    .block(Block::default().borders(Borders::ALL).title("Notes"))
    .wrap(Wrap { trim: false });
    frame.render_widget(help, rows[6]);

    let status = Paragraph::new(state.status.clone())
        .style(Style::default().fg(Color::White).bg(Color::DarkGray))
        .wrap(Wrap { trim: false });
    frame.render_widget(status, rows[7]);
}

fn field_widget<'a>(title: &'a str, value: &'a str, active: bool, enabled: bool) -> Paragraph<'a> {
    let border_style = if active && enabled {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else if !enabled {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };
    let text_style = if enabled {
        Style::default()
    } else {
        Style::default().fg(Color::DarkGray)
    };

    Paragraph::new(Line::from(Span::styled(value.to_string(), text_style)))
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false })
}

fn checkbox_widget<'a>(title: &'a str, checked: bool, active: bool) -> Paragraph<'a> {
    let border_style = if active {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let label = if checked { "[x] yes" } else { "[ ] no" };
    Paragraph::new(label)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false })
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
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
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use tempfile::tempdir;

    #[test]
    fn resolves_current_directory_to_kitty_conf() {
        let dir = tempdir().expect("tempdir");
        let config = dir.path().join("kitty.conf");
        fs::write(&config, "font_size 12.0\n").expect("write config");

        let resolved =
            resolve_current_path(&dir.path().display().to_string()).expect("resolve current");
        assert_eq!(resolved, config);
    }

    #[test]
    fn accepts_blank_themes_path() {
        let themes = resolve_themes_path("").expect("resolve themes");
        assert!(themes.is_none());
    }

    #[test]
    fn allows_blank_current_path_by_using_default_location() {
        let resolved = resolve_current_path("").expect("resolve current");
        let home = std::env::var("HOME").expect("home");
        assert_eq!(
            resolved,
            PathBuf::from(home)
                .join(".config")
                .join("kitty")
                .join("kitty.conf")
        );
    }

    #[test]
    fn accepts_directory_without_existing_kitty_conf() {
        let dir = tempdir().expect("tempdir");

        let resolved =
            resolve_current_path(&dir.path().display().to_string()).expect("resolve current");
        assert_eq!(resolved, dir.path().join("kitty.conf"));
    }

    #[test]
    fn rejects_missing_current_config_without_create_new() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("missing.conf");
        let err = validate_paths(
            &missing.display().to_string(),
            &dir.path().display().to_string(),
            true,
            false,
            "",
        )
        .expect_err("missing config should fail");
        assert!(err
            .to_string()
            .contains("enable Create new config from defaults"));
    }

    #[test]
    fn accepts_missing_current_config_with_create_new() {
        let dir = tempdir().expect("tempdir");
        let create_dir = dir.path().join("new-kitty");
        let paths = validate_paths("", &create_dir.display().to_string(), true, true, "")
            .expect("missing config should be allowed for create new");
        assert_eq!(paths.current, create_dir.join("kitty.conf"));
        assert!(paths.create_new);
        assert!(paths.create_backup);
    }

    #[test]
    fn rejects_create_new_when_target_exists() {
        let dir = tempdir().expect("tempdir");
        let existing_dir = dir.path().join("kitty");
        fs::create_dir_all(&existing_dir).expect("create existing dir");
        let existing = existing_dir.join("kitty.conf");
        fs::write(&existing, "font_size 12.0\n").expect("write config");
        let err = validate_paths("", &existing_dir.display().to_string(), true, true, "")
            .expect_err("existing target should not be reused for create new");
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn rejects_new_config_file_input() {
        let err = resolve_new_config_path("/tmp/kitty.conf").expect_err("file input should fail");
        assert!(err.to_string().contains("must be a directory"));
    }

    #[test]
    fn persists_last_current_path_to_state_file() {
        let dir = tempdir().expect("tempdir");
        let state_file = dir.path().join("last-current-path");
        let current = dir.path().join("kitty.conf");

        persist_last_current_input_to(&state_file, &current).expect("persist current path");

        assert_eq!(
            load_saved_current_input_from(&state_file),
            Some(current.display().to_string())
        );
    }

    #[test]
    fn ignores_empty_saved_current_path() {
        let dir = tempdir().expect("tempdir");
        let state_file = dir.path().join("last-current-path");
        fs::write(&state_file, "\n").expect("write empty state");

        assert!(load_saved_current_input_from(&state_file).is_none());
    }
}
