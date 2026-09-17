use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::OnceLock;

static OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// Overrides the directory proxy-rs keeps its persistent state in.
///
/// Must be called (at most once, before any call to `config_dir()`) — normally
/// right at startup in `main`, from the `--config-dir` CLI flag.
pub fn set_config_dir_override(dir: PathBuf) {
    let _ = OVERRIDE.set(dir);
}

/// Directory proxy-rs keeps its persistent state in: `~/.proxy-rs`, unless
/// overridden via `set_config_dir_override`.
///
/// Creates the directory if it doesn't exist yet.
pub fn config_dir() -> Result<PathBuf> {
    let dir = match OVERRIDE.get() {
        Some(dir) => dir.clone(),
        None => dirs::home_dir()
            .context("failed to determine home directory")?
            .join(".proxy-rs"),
    };
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create config dir: {}", dir.display()))?;
    Ok(dir)
}
