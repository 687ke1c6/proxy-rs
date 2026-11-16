use std::str::FromStr;

use anyhow::{Context, Result};
use dialoguer::{Confirm, Input, Select};
use iroh::EndpointId;
use rand::Rng;
use tracing::{info, warn};

use crate::config::{self, save_server};
use crate::identity::short_id;

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

/// One `[servers]` entry: a server node id and its optional name.
struct KnownServer {
    id: EndpointId,
    name: Option<String>,
}

fn generate_name() -> String {
    let mut rng = rand::thread_rng();
    let adj = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let animal = ANIMALS[rng.gen_range(0..ANIMALS.len())];
    format!("{adj} {animal}")
}

/// How a server is shown on the console: `"Name" [3f1eb...58d]`, or just `[3f1eb...58d]`.
pub fn server_label(name: Option<&str>, id: EndpointId) -> String {
    match name {
        Some(name) => format!("\"{name}\" [{}]", short_id(id)),
        None => format!("[{}]", short_id(id)),
    }
}

const NEW_ENTRY_LABEL: &str = "<new>";

fn select_known_server() -> Result<(Option<String>, EndpointId)> {
    let servers = load()?;

    let mut labels: Vec<String> = servers
        .iter()
        .map(|s| match &s.name {
            Some(name) => format!("{name} [{}]", short_id(s.id)),
            None => format!("[{}]", short_id(s.id)),
        })
        .collect();
    labels.push(NEW_ENTRY_LABEL.to_string());

    let selection = Select::new()
        .with_prompt("Select server node ID")
        .items(&labels)
        .default(0)
        .interact()
        .with_context(|| "Failed to get user selection")?;

    match servers.get(selection) {
        Some(server) => Ok((server.name.clone(), server.id)),
        None => add_new_server(&servers),
    }
}

/// The `<new>` menu entry: prompts for a node id, then a name, and saves them. If a different
/// saved server already has that name, asks before replacing that entry (names are unique);
/// answering no asks for another name. If the id is already saved, its line is renamed rather
/// than duplicated (the table is keyed by node id), and its current name is the prompt's default.
fn add_new_server(servers: &[KnownServer]) -> Result<(Option<String>, EndpointId)> {
    let id = prompt_node_id()?;
    let default_name = servers
        .iter()
        .find(|s| s.id == id)
        .and_then(|s| s.name.clone())
        .unwrap_or_else(generate_name);
    loop {
        let name: String = Input::new()
            .with_prompt("Name")
            .default(default_name.clone())
            .interact_text()
            .with_context(|| "Failed to read name")?
            .trim()
            .to_string();
        let name = (!name.is_empty()).then_some(name);

        let clashes: Vec<EndpointId> = servers
            .iter()
            .filter(|s| s.id != id && s.name.is_some() && s.name == name)
            .map(|s| s.id)
            .collect();
        if let (Some(name), false) = (&name, clashes.is_empty()) {
            let ids: Vec<String> = clashes.iter().map(|&c| format!("[{}]", short_id(c))).collect();
            let replace = Confirm::new()
                .with_prompt(format!(
                    "\"{name}\" is already saved for {}. Replace it with [{}]?",
                    ids.join(", "),
                    short_id(id)
                ))
                .default(false)
                .interact()
                .with_context(|| "Failed to read confirmation")?;
            if !replace {
                continue;
            }
        }
        save_server(id, name.as_deref(), &clashes)?;
        return Ok((name, id));
    }
}

fn prompt_node_id() -> Result<EndpointId> {
    let id: String = Input::new()
        .with_prompt("Enter a server node ID")
        .validate_with(|input: &String| -> Result<(), &str> {
            EndpointId::from_str(input.trim())
                .map(|_| ())
                .map_err(|_| "Not a valid node ID")
        })
        .interact_text()
        .with_context(|| "Failed to read server node ID")?;
    EndpointId::from_str(id.trim()).with_context(|| "Could not parse server node id")
}

/// Resolves the server node id for a client mode from `-n`/`--name`, returning
/// `(name, node_id)` so callers can print a human-readable name instead of just the id:
/// - both given: use `id` if known (renaming its entry to `name`), else add it as `name`.
/// - only `-n`: use `id` if known, else add it under a generated name.
/// - only `--name`: use the saved id for `name` if known, else prompt for an id and save it as `name`.
/// - neither: interactive `Select` menu of known servers, plus `<new>`.
pub fn resolve_node_id(node_id: Option<String>, name: Option<String>) -> Result<(Option<String>, EndpointId)> {
    let node_id = node_id
        .map(|id| EndpointId::from_str(id.trim()).with_context(|| "Could not parse server node id"))
        .transpose()?;
    match (node_id, name) {
        (Some(id), Some(name)) => {
            let servers = load()?;
            if let Some(other) = servers.iter().find(|s| s.name.as_deref() == Some(&name) && s.id != id) {
                anyhow::bail!(
                    "Name \"{name}\" is already used by a different saved server [{}]; pick a different --name",
                    short_id(other.id)
                );
            }
            match servers.iter().find(|s| s.id == id) {
                Some(known) if known.name.as_deref() == Some(&name) => {}
                Some(_) => {
                    info!("Naming saved server [{}] \"{name}\"", short_id(id));
                    save_server(id, Some(&name), &[])?;
                }
                None => save_server(id, Some(&name), &[])?,
            }
            Ok((Some(name), id))
        }
        (Some(id), None) => match load()?.into_iter().find(|s| s.id == id) {
            Some(known) => Ok((known.name, id)),
            None => {
                let name = generate_name();
                save_server(id, Some(&name), &[])?;
                Ok((Some(name), id))
            }
        },
        (None, Some(name)) => {
            let servers = load()?;
            let mut matches = servers.iter().filter(|s| s.name.as_deref() == Some(&name));
            match matches.next() {
                Some(known) => {
                    if matches.next().is_some() {
                        warn!(
                            "Multiple saved servers are named \"{name}\"; using the first match [{}]",
                            short_id(known.id)
                        );
                    }
                    Ok((Some(name), known.id))
                }
                None => {
                    let id = prompt_node_id()?;
                    save_server(id, Some(&name), &[])?;
                    Ok((Some(name), id))
                }
            }
        }
        (None, None) => select_known_server(),
    }
}

/// Reads the config file's `[servers]`, in file order.
fn load() -> Result<Vec<KnownServer>> {
    let entries = config::read_ids(&config::config_path()?, config::SERVERS)?;
    Ok(entries.into_iter().map(|(id, name)| KnownServer { id, name }).collect())
}
