use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RuntimeFlavor {
    Auto,
    Plain,
    Kitty,
    Kitten,
}

#[derive(Debug, Parser)]
#[command(name = "kitty-config-editor")]
#[command(about = "Interactive Kitty config editor")]
pub struct Cli {
    #[arg(
        long = "current",
        help = "Optional path to kitty.conf or its containing directory. When omitted, the TUI prompts for it; new configs are created from the startup prompt's dedicated defaults mode."
    )]
    pub current: Option<PathBuf>,

    #[arg(
        long = "out-full",
        help = "Optional destination for full-format output."
    )]
    pub out_full: Option<PathBuf>,

    #[arg(
        long = "out-minimal",
        help = "Optional destination for minimal override output."
    )]
    pub out_minimal: Option<PathBuf>,

    #[arg(
        long = "themes-dir",
        help = "Optional directory to scan for Kitty themes. When omitted, the TUI prompts for it."
    )]
    pub themes_dir: Option<PathBuf>,

    #[arg(long = "enable-reload", default_value_t = false)]
    pub enable_reload: bool,

    #[arg(long = "reload-command")]
    pub reload_command: Option<String>,

    #[arg(long, value_enum, default_value_t = RuntimeFlavor::Auto)]
    pub runtime: RuntimeFlavor,

    #[arg(long, default_value_t = 200)]
    pub tick_rate_ms: u64,

    #[arg(long, default_value_t = false)]
    pub no_kitty_keyboard: bool,

    #[arg(long, default_value_t = false)]
    pub no_bracketed_paste: bool,
}
