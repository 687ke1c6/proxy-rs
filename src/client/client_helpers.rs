use std::path::Path;

use anyhow::{Context, Result};
use dialoguer::Select;
use tracing::info;

const PATH: &str = ".node-id";

pub fn load_node_id_from_file() -> Result<String> {
    let ids = load_node_ids()?;
    match ids.len() {
        0 => anyhow::bail!("No saved server node IDs. Pass one with -n on first run."),
        1 => Ok(ids.into_iter().next().unwrap()),
        _ => {
            let selection = Select::new()
                .with_prompt("Select server node ID")
                .items(&ids)
                .default(0)
                .interact()
                .with_context(|| "Failed to get user selection")?;
            Ok(ids[selection].clone())
        }
    }
}

pub fn write_node_id_to_file(id: &str) -> Result<()> {
    let mut ids = load_node_ids().unwrap_or_default();
    if ids.iter().any(|existing| existing == id) {
        info!("Server node id already saved in {PATH}");
        return Ok(());
    }
    ids.push(id.to_string());
    std::fs::write(PATH, ids.join("\n"))
        .with_context(|| format!("failed to write node id file: {PATH}"))?;
    info!("Saved server node id to {PATH}");
    Ok(())
}

fn load_node_ids() -> Result<Vec<String>> {
    if !Path::new(PATH).exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(PATH)
        .with_context(|| format!("failed to read node id file: {PATH}"))?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|&s| !s.is_empty())
        .map(String::from)
        .collect())
}
