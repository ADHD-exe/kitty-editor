use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

pub fn reload_config(command: Option<&str>, config_path: &Path) -> Result<String> {
    let default_cmd = format!("kitty @ load-config {}", shell_quote(config_path));
    let cmd = command.unwrap_or(&default_cmd);
    run_command(cmd, "reload")
}

pub fn preview_theme(theme_path: &Path) -> Result<String> {
    let cmd = format!(
        "kitty @ set-colors --all --configured {}",
        shell_quote(theme_path)
    );
    run_command(&cmd, "theme preview")
}

pub fn preview_theme_artifact(body: &str) -> Result<String> {
    let preview_path = std::env::temp_dir().join("kitty-config-editor-live-preview.conf");
    fs::write(&preview_path, body)
        .with_context(|| format!("writing {}", preview_path.display()))?;
    let preview_result = preview_theme(&preview_path);
    let _ = fs::remove_file(&preview_path);
    preview_result
}

fn run_command(cmd: &str, label: &str) -> Result<String> {
    let output = Command::new("sh")
        .arg("-lc")
        .arg(cmd)
        .output()
        .with_context(|| format!("running {label} command: {cmd}"))?;
    if output.status.success() {
        Ok(format!("{label} ok"))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(format!("{label} failed: {}", stderr.trim()))
    }
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}
