use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;
const PATH: & str = ".node-id";

pub fn load_node_id_from_file() -> Result<String> {
    if Path::new(PATH).exists() {
        let id = std::fs::read_to_string(PATH)
            .with_context(|| format!("failed to read node id file: {PATH}"))?;
        info!("Loaded server node id from {PATH}");
        Ok(id.trim().to_string())
    } else {
        panic!("Server node id not found. Pass it as an argument on first run.");
    }
}

pub fn write_node_id_to_file(id: &str) -> Result<()> {
    std::fs::write(PATH, id)
        .with_context(|| format!("failed to write node id file: {PATH}"))?;
    info!("Saved server node id to {PATH}");
    Ok(())
}