use std::env;

use crossterm::terminal;

use crate::cli::{Cli, RuntimeFlavor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveRuntime {
    Plain,
    KittyOptimized,
    KittenShim,
}

#[derive(Debug, Clone)]
pub struct KittyCapabilities {
    pub effective_runtime: EffectiveRuntime,
    pub is_kitty_terminal: bool,
    pub kitty_window_id: Option<String>,
    pub kitty_listen_on: Option<String>,
    pub supports_keyboard_enhancement: bool,
}

impl KittyCapabilities {
    pub fn runtime_label(&self) -> &'static str {
        match self.effective_runtime {
            EffectiveRuntime::Plain => "plain",
            EffectiveRuntime::KittyOptimized => "kitty-optimized",
            EffectiveRuntime::KittenShim => "kitten",
        }
    }
}

pub fn detect(cli: &Cli) -> KittyCapabilities {
    detect_from_parts(
        env::var("TERM").ok().as_deref(),
        env::var("KITTY_WINDOW_ID").ok().as_deref(),
        env::var("KITTY_LISTEN_ON").ok().as_deref(),
        cli.runtime,
        matches!(terminal::supports_keyboard_enhancement(), Ok(true)),
    )
}

fn detect_from_parts(
    term: Option<&str>,
    kitty_window_id: Option<&str>,
    kitty_listen_on: Option<&str>,
    runtime: RuntimeFlavor,
    supports_keyboard_enhancement: bool,
) -> KittyCapabilities {
    let is_kitty_terminal = term == Some("xterm-kitty") || kitty_window_id.is_some();
    let effective_runtime = match runtime {
        RuntimeFlavor::Plain => EffectiveRuntime::Plain,
        RuntimeFlavor::Kitty => EffectiveRuntime::KittyOptimized,
        RuntimeFlavor::Kitten => EffectiveRuntime::KittenShim,
        RuntimeFlavor::Auto if is_kitty_terminal => EffectiveRuntime::KittyOptimized,
        RuntimeFlavor::Auto => EffectiveRuntime::Plain,
    };

    KittyCapabilities {
        effective_runtime,
        is_kitty_terminal,
        kitty_window_id: kitty_window_id.map(ToOwned::to_owned),
        kitty_listen_on: kitty_listen_on.map(ToOwned::to_owned),
        supports_keyboard_enhancement,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_mode_uses_plain_runtime_outside_kitty() {
        let caps = detect_from_parts(None, None, None, RuntimeFlavor::Auto, false);
        assert_eq!(caps.effective_runtime, EffectiveRuntime::Plain);
        assert!(!caps.is_kitty_terminal);
    }

    #[test]
    fn auto_mode_switches_to_kitty_inside_kitty() {
        let caps = detect_from_parts(
            Some("xterm-kitty"),
            Some("42"),
            None,
            RuntimeFlavor::Auto,
            true,
        );
        assert_eq!(caps.effective_runtime, EffectiveRuntime::KittyOptimized);
        assert!(caps.is_kitty_terminal);
        assert!(caps.supports_keyboard_enhancement);
    }

    #[test]
    fn explicit_kitten_mode_wins_even_without_terminal_markers() {
        let caps = detect_from_parts(None, None, None, RuntimeFlavor::Kitten, false);
        assert_eq!(caps.effective_runtime, EffectiveRuntime::KittenShim);
    }
}
