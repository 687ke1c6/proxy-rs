use std::path::Path;

use anyhow::{Context, Result};
use dialoguer::Select;
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::info;

const PATH: &str = ".node-ids.yaml";

const ADJECTIVES: &[&str] = &[
    "Amber", "Bouncy", "Cosmic", "Dapper", "Eager", "Focal", "Groovy",
    "Hardy", "Intrepid", "Jammy", "Kinetic", "Lunar", "Mantic", "Noble",
    "Oracular", "Plucky", "Questing", "Radiant", "Snappy", "Trusty",
    "Utopic", "Vivid", "Wily", "Xenial", "Yakkety", "Zesty",
];

const ANIMALS: &[&str] = &[
    "Alpaca", "Badger", "Capybara", "Dingo", "Ermine", "Fossa", "Gecko",
    "Heron", "Ibis", "Jellyfish", "Kirin", "Lynx", "Meerkat", "Narwhal",
    "Ocelot", "Pangolin", "Quokka", "Ringtail", "Salamander", "Tapir",
    "Unicorn", "Viper", "Walrus", "Xerus", "Yak", "Zebrafish",
];

#[derive(Serialize, Deserialize)]
struct NodeEntry {
    name: String,
    key: String,
}

#[derive(Serialize, Deserialize, Default)]
struct ClientConfig {
    #[serde(default)]
    node_entries: Vec<NodeEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_used: Option<String>,
}

fn generate_name() -> String {
    let mut rng = rand::thread_rng();
    let adj = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let animal = ANIMALS[rng.gen_range(0..ANIMALS.len())];
    format!("{adj} {animal}")
}

pub fn load_node_id_from_file() -> Result<String> {
    let mut config = load_config()?;
    match config.node_entries.len() {
        0 => anyhow::bail!("No saved server node IDs. Pass one with -n on first run."),
        1 => Ok(config.node_entries.into_iter().next().unwrap().key),
        _ => {
            // Rotate last-used entry to the front of the display list.
            let mut ordered: Vec<&NodeEntry> = config.node_entries.iter().collect();
            if let Some(last_key) = &config.last_used {
                if let Some(pos) = ordered.iter().position(|e| &e.key == last_key) {
                    let entry = ordered.remove(pos);
                    ordered.insert(0, entry);
                }
            }
            let labels: Vec<String> = ordered
                .iter()
                .map(|e| format!("{} [{}...]", e.name, &e.key[..16]))
                .collect();
            let selection = Select::new()
                .with_prompt("Select server node ID")
                .items(&labels)
                .default(0)
                .interact()
                .with_context(|| "Failed to get user selection")?;
            let selected_key = ordered[selection].key.clone();
            config.last_used = Some(selected_key.clone());
            save_config(&config)?;
            Ok(selected_key)
        }
    }
}

pub fn write_node_id_to_file(id: &str) -> Result<()> {
    let mut config = load_config().unwrap_or_default();
    if config.node_entries.iter().any(|e| e.key == id) {
        info!("Server node id already saved in {PATH}");
        return Ok(());
    }
    let name = generate_name();
    info!("Saving server node id as \"{name}\" to {PATH}");
    config.node_entries.push(NodeEntry {
        name,
        key: id.to_string(),
    });
    save_config(&config)
}

fn load_config() -> Result<ClientConfig> {
    if !Path::new(PATH).exists() {
        return Ok(ClientConfig::default());
    }
    let content = std::fs::read_to_string(PATH)
        .with_context(|| format!("failed to read config file: {PATH}"))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("failed to parse config file: {PATH}"))
}

fn save_config(config: &ClientConfig) -> Result<()> {
    let yaml =
        serde_yaml::to_string(config).with_context(|| "failed to serialize client config")?;
    std::fs::write(PATH, yaml)
        .with_context(|| format!("failed to write config file: {PATH}"))
}
