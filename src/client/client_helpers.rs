use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use dialoguer::{Input, Select};
use iroh::EndpointId;
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::config_dir::config_dir;

const FILENAME: &str = "node-ids.yaml";

fn path() -> Result<PathBuf> {
    Ok(config_dir()?.join(FILENAME))
}

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

const NEW_ENTRY_LABEL: &str = "<new>";

pub fn load_node_id_from_file() -> Result<String> {
    let config = load_config()?;

    // Rotate last-used entry to the front of the display list.
    let mut ordered: Vec<&NodeEntry> = config.node_entries.iter().collect();
    if let Some(last_key) = &config.last_used {
        if let Some(pos) = ordered.iter().position(|e| &e.key == last_key) {
            let entry = ordered.remove(pos);
            ordered.insert(0, entry);
        }
    }

    let mut labels: Vec<String> = ordered
        .iter()
        .map(|e| format!("{} [{}...]", e.name, &e.key[..16]))
        .collect();
    labels.push(NEW_ENTRY_LABEL.to_string());

    let selection = Select::new()
        .with_prompt("Select server node ID")
        .items(&labels)
        .default(0)
        .interact()
        .with_context(|| "Failed to get user selection")?;

    let selected_key = if selection == ordered.len() {
        let id = prompt_new_node_id()?;
        write_node_id_to_file(&id)?;
        id
    } else {
        ordered[selection].key.clone()
    };

    let mut config = load_config()?;
    config.last_used = Some(selected_key.clone());
    save_config(&config)?;
    Ok(selected_key)
}

fn prompt_new_node_id() -> Result<String> {
    let id: String = Input::new()
        .with_prompt("Enter a server node ID")
        .validate_with(|input: &String| -> Result<(), &str> {
            EndpointId::from_str(input.trim())
                .map(|_| ())
                .map_err(|_| "Not a valid node ID")
        })
        .interact_text()
        .with_context(|| "Failed to read server node ID")?
        .trim()
        .to_string();
    Ok(id)
}

pub fn write_node_id_to_file(id: &str) -> Result<()> {
    let path = path()?;
    let mut config = load_config().unwrap_or_default();
    if config.node_entries.iter().any(|e| e.key == id) {
        info!("Server node id already saved in {}", path.display());
        return Ok(());
    }
    let name = generate_name();
    info!("Saving server node id as \"{name}\" to {}", path.display());
    config.node_entries.push(NodeEntry {
        name,
        key: id.to_string(),
    });
    save_config(&config)
}

fn load_config() -> Result<ClientConfig> {
    let path = path()?;
    if !path.exists() {
        return Ok(ClientConfig::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("failed to parse config file: {}", path.display()))
}

fn save_config(config: &ClientConfig) -> Result<()> {
    let path = path()?;
    let yaml =
        serde_yaml::to_string(config).with_context(|| "failed to serialize client config")?;
    std::fs::write(&path, yaml)
        .with_context(|| format!("failed to write config file: {}", path.display()))
}
