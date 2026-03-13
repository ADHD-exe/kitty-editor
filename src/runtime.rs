use std::io;
use std::panic;

use anyhow::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::cli::Cli;
use crate::kitty::detect::KittyCapabilities;

pub struct TerminalSession {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    paste_enabled: bool,
    kitty_keyboard_enabled: bool,
    restored: bool,
}

impl TerminalSession {
    pub fn enter(cli: &Cli, capabilities: &KittyCapabilities) -> Result<Self> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, Hide)?;

        let paste_enabled = !cli.no_bracketed_paste;
        if paste_enabled {
            execute!(stdout, EnableBracketedPaste)?;
        }

        let kitty_keyboard_enabled =
            capabilities.supports_keyboard_enhancement && !cli.no_kitty_keyboard;
        if kitty_keyboard_enabled {
            execute!(
                stdout,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                )
            )?;
        }

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        Ok(Self {
            terminal,
            paste_enabled,
            kitty_keyboard_enabled,
            restored: false,
        })
    }

    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }

    pub fn restore(&mut self) -> Result<()> {
        if self.restored {
            return Ok(());
        }

        if self.kitty_keyboard_enabled {
            execute!(self.terminal.backend_mut(), PopKeyboardEnhancementFlags)?;
        }
        if self.paste_enabled {
            execute!(self.terminal.backend_mut(), DisableBracketedPaste)?;
        }

        execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen)?;
        disable_raw_mode()?;
        self.terminal.show_cursor()?;
        self.restored = true;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub fn install_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal_best_effort();
        previous(info);
    }));
}

fn restore_terminal_best_effort() -> Result<()> {
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    execute!(
        stdout,
        PopKeyboardEnhancementFlags,
        DisableBracketedPaste,
        Show,
        LeaveAlternateScreen
    )?;
    Ok(())
}
