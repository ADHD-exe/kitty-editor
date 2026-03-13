mod cli;
mod config_model;
mod default_reference;
mod diff_engine;
mod fuzzy_search;
mod highlighting;
mod include_loader;
mod keybinding_editor;
mod kitty;
mod metadata_extractor;
mod parser;
mod reload;
mod runtime;
mod startup_prompt;
mod theme_browser;
mod tui_app;
mod ui_renderer;
mod validator;
mod writer;

use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;

use crate::cli::Cli;
use crate::default_reference::{load_reference_keymaps, load_reference_metadata};
use crate::parser::parse_current_config;
use crate::startup_prompt::resolve_startup_paths;
use crate::theme_browser::discover_themes;
use crate::tui_app::{build_app, run, RuntimeOptions};
use crate::writer::ensure_starter_config;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let capabilities = kitty::detect::detect(&cli);

    runtime::install_panic_hook();
    let mut terminal = runtime::TerminalSession::enter(&cli, &capabilities)?;
    let run_result = (|| -> Result<()> {
        let startup =
            resolve_startup_paths(&mut terminal, &cli, Duration::from_millis(cli.tick_rate_ms))?;
        let created_starter = if startup.create_new {
            ensure_starter_config(&startup.current)
                .with_context(|| format!("failed to initialize {}", startup.current.display()))?
        } else {
            false
        };
        let metadata = load_reference_metadata(None)
            .context("failed to load bundled kitty reference metadata")?;
        let default_keymaps =
            load_reference_keymaps(None).context("failed to load bundled kitty default keymaps")?;
        let effective = parse_current_config(&startup.current)
            .context("failed to parse current kitty config")?;
        let themes =
            discover_themes(startup.themes_dir.as_deref()).context("failed to discover themes")?;
        let mut app = build_app(
            metadata,
            effective,
            default_keymaps,
            themes,
            startup.themes_dir.clone(),
        );
        let mut status_parts = Vec::new();
        if created_starter {
            status_parts.push(format!(
                "starter config created at {}",
                startup.current.display()
            ));
        }
        status_parts.push(String::from("ready"));
        status_parts.push(format!("runtime {}", capabilities.runtime_label()));
        status_parts.push(format!(
            "terminal {}",
            if capabilities.is_kitty_terminal {
                "kitty"
            } else {
                "generic"
            }
        ));
        if let Some(window_id) = capabilities.kitty_window_id.as_deref() {
            status_parts.push(format!("window {}", window_id));
        }
        if capabilities.kitty_listen_on.is_some() {
            status_parts.push(String::from("remote socket detected"));
        }
        app.status = status_parts.join(" | ");

        let opts = RuntimeOptions {
            out_full: cli.out_full.clone(),
            out_minimal: cli.out_minimal.clone(),
            create_backup: startup.create_backup,
            enable_reload: cli.enable_reload,
            reload_command: cli.reload_command.clone(),
        };

        run(
            &mut terminal,
            app,
            opts,
            Duration::from_millis(cli.tick_rate_ms),
        )
    })();
    let restore_result = terminal.restore();
    run_result?;
    restore_result?;
    Ok(())
}
