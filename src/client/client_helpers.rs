use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use dialoguer::{Input, Select};
use iroh::EndpointId;
use rand::Rng;
use tracing::{info, warn};

use crate::config_dir::config_dir;
use crate::id_file;
use crate::identity::short_id;

const FILENAME: &str = "known-servers";

/// Written at the top of a new `known-servers` file.
const TEMPLATE: &str = "\
# Server node ids this client knows, one per line. Anything after the id is an optional
# name: pass it as `--name` instead of `-n <node-id>`, and it's shown when connecting.
";

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

/// One `known-servers` line: a server node id and its optional name.
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

    match servers.into_iter().nth(selection) {
        Some(server) => Ok((server.name, server.id)),
        None => {
            let id = prompt_node_id()?;
            let name: String = Input::new()
                .with_prompt("Name")
                .default(generate_name())
                .interact_text()
                .with_context(|| "Failed to read name")?
                .trim()
                .to_string();
            let name = (!name.is_empty()).then_some(name);
            append(id, name.as_deref())?;
            Ok((name, id))
        }
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
                    rename(id, &name)?;
                }
                None => append(id, Some(&name))?,
            }
            Ok((Some(name), id))
        }
        (Some(id), None) => match load()?.into_iter().find(|s| s.id == id) {
            Some(known) => Ok((known.name, id)),
            None => {
                let name = generate_name();
                append(id, Some(&name))?;
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
                    append(id, Some(&name))?;
                    Ok((Some(name), id))
                }
            }
        }
        (None, None) => select_known_server(),
    }
}

/// Reads `known-servers`, in file order. A missing file counts as empty.
fn load() -> Result<Vec<KnownServer>> {
    let path = path()?;
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("failed to read {}", path.display())),
    };
    let entries = id_file::parse(&content).with_context(|| format!("invalid {}", path.display()))?;
    Ok(entries.into_iter().map(|(id, name)| KnownServer { id, name }).collect())
}

/// Appends a server to `known-servers`, creating it (with the comment header) if missing.
fn append(id: EndpointId, name: Option<&str>) -> Result<()> {
    let path = path()?;
    info!("Saving server [{}] to {}", short_id(id), path.display());
    let is_new = !path.exists();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    let header = if is_new { TEMPLATE } else { "" };
    writeln!(file, "{header}{}", id_file::format_line(id, name))
        .with_context(|| format!("failed to write {}", path.display()))
}

/// Sets the name on `id`'s line(s) in `known-servers`, leaving every other line
/// (comments included) as it was.
fn rename(id: EndpointId, name: &str) -> Result<()> {
    let path = path()?;
    let content = std::fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut out = String::with_capacity(content.len());
    for line in content.lines() {
        match id_file::parse_line(line) {
            Ok(Some((line_id, _))) if line_id == id => out += &id_file::format_line(id, Some(name)),
            _ => out += line,
        }
        out.push('\n');
    }
    std::fs::write(&path, out).with_context(|| format!("failed to write {}", path.display()))
}
