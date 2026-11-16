//! The config file: one TOML file per node (`~/.proxy-rs/config.toml`, or `--config <file>`)
//! holding its secret key, the servers it knows (client side) and the clients it admits
//! (server side). Created on first use; a second node on the same host is just another file.
//!
//! Edits go through `toml_edit`, so comments and hand edits survive a rewrite.

use anyhow::{Context, Result};
use iroh::{EndpointId, SecretKey};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;
use toml_edit::DocumentMut;
use tracing::info;

use crate::identity::short_id;

/// Table of servers this node connects to as a client: `<node-id> = "<name>"`.
pub const SERVERS: &str = "servers";
/// Table of clients admitted when this node runs as a server: `<node-id> = "<label>"`.
pub const CLIENTS: &str = "clients";
const SECRET_KEY: &str = "secret-key";

static OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// Points `config_path()` at `path` (from `--config`). Call at most once, before anything
/// reads the config.
pub fn set_config_override(path: PathBuf) {
    let _ = OVERRIDE.set(path);
}

/// The config file: `~/.proxy-rs/config.toml` unless overridden. Creates its directory.
pub fn config_path() -> Result<PathBuf> {
    let path = match OVERRIDE.get() {
        Some(path) => path.clone(),
        None => dirs::home_dir()
            .context("failed to determine home directory")?
            .join(".proxy-rs")
            .join("config.toml"),
    };
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    }
    Ok(path)
}

/// ` config=<file>` for the end of every startup banner, as an absolute path.
pub fn banner_suffix() -> String {
    match config_path() {
        Ok(path) => format!(" config={}", std::path::absolute(&path).unwrap_or(path).display()),
        Err(e) => format!(" config=? ({e:#})"),
    }
}

/// Content of a new config file. Both tables are empty, so on its own (no `--allow`) a
/// server using it is open to any client.
fn template(secret_key_hex: &str) -> String {
    format!(
        "\
# proxy-rs config: this node's identity, the servers it knows (when run as a client) and
# the clients it admits (when run as a server). Use another file (--config) to run a
# second node with its own node id.

# This node's secret key; its node id (`proxy-rs whoami`) derives from it. Keep it private.
{SECRET_KEY} = \"{secret_key_hex}\"

# Servers this node connects to: <node-id> = \"<name>\" (\"\" for none). Pass the name as
# --name instead of -n <node-id>. proxy-rs adds a server here the first time it's used.
[{SERVERS}]

# Clients allowed to connect to this node: <node-id> = \"<label>\" (\"\" for none; shown when
# that client connects). Get a client's id with `proxy-rs whoami` on its machine.
# Re-read on every connection: edit to add or revoke clients, no restart needed.
# If this table is empty and the server has no --allow, ANY client may connect.
[{CLIENTS}]
# 7c41d2...9e0b = \"laptop\"
"
    )
}

fn new_key_hex() -> String {
    SecretKey::generate().to_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

fn read(path: &Path) -> Result<DocumentMut> {
    let content = std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    content.parse().with_context(|| format!("invalid {}", path.display()))
}

/// Writes `content` to a private (0600) temp file next to `path` and returns its path.
fn write_temp(path: &Path, content: &str) -> Result<PathBuf> {
    let tmp = path.with_file_name(format!(
        "{}.tmp.{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("config"),
        std::process::id()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&tmp).with_context(|| format!("failed to write {}", tmp.display()))?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    Ok(tmp)
}

/// Replaces `path` atomically, so a crash can't leave a half-written file (and lose the key).
fn replace(path: &Path, content: &str) -> Result<()> {
    let tmp = write_temp(path, content)?;
    std::fs::rename(&tmp, path).with_context(|| format!("failed to write {}", path.display()))
}

/// Creates the config file with a new key. Safe against several processes starting at once:
/// the temp file is hard-linked into place, which fails if another process got there first,
/// and everyone then reads the winner's file.
fn create(path: &Path) -> Result<()> {
    let tmp = write_temp(path, &template(&new_key_hex()))?;
    let linked = std::fs::hard_link(&tmp, path);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(()) => {
            info!("Created {} with a new secret key", path.display());
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e).with_context(|| format!("failed to write {}", path.display())),
    }
}

/// This node's secret key, which determines its stable node id. Creates the config file if
/// it's missing, and adds a key to a (hand-written) file that has none.
pub fn load_secret_key() -> Result<SecretKey> {
    let path = config_path()?;
    if !path.exists() {
        create(&path)?;
    }
    let mut doc = read(&path)?;
    let hex = match doc.get(SECRET_KEY) {
        Some(item) => item
            .as_str()
            .with_context(|| format!("{SECRET_KEY} in {} must be a string", path.display()))?
            .to_string(),
        None => {
            let hex = new_key_hex();
            doc.insert(SECRET_KEY, toml_edit::value(&hex));
            replace(&path, &doc.to_string())?;
            info!("Added a new secret key to {}", path.display());
            hex
        }
    };
    SecretKey::from_str(hex.trim()).with_context(|| format!("invalid {SECRET_KEY} in {}", path.display()))
}

/// The `<node-id> = "<label>"` entries of `table`, in file order. An empty label is `None`.
fn parse_ids(doc: &DocumentMut, table: &str) -> Result<Vec<(EndpointId, Option<String>)>> {
    let Some(item) = doc.get(table) else {
        return Ok(Vec::new());
    };
    let entries = item.as_table_like().with_context(|| format!("[{table}] must be a table"))?;
    entries
        .iter()
        .map(|(key, value)| {
            let id = EndpointId::from_str(key).with_context(|| format!("[{table}]: invalid node id {key:?}"))?;
            let label = value.as_str().with_context(|| format!("[{table}] {key}: expected a quoted string"))?.trim();
            Ok((id, (!label.is_empty()).then(|| label.to_string())))
        })
        .collect()
}

/// Reads the ids in `table` of the config file at `path`. A missing file counts as empty.
pub fn read_ids(path: &Path, table: &str) -> Result<Vec<(EndpointId, Option<String>)>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let doc = read(path)?;
    parse_ids(&doc, table).with_context(|| format!("invalid {}", path.display()))
}

/// `content` with `id = "<label>"` in `table`: an existing entry is changed in place, a new
/// one is added at the end of the table; entries for `remove` are dropped. Everything else,
/// comments included, is kept.
fn set_id(content: &str, table: &str, id: EndpointId, label: Option<&str>, remove: &[EndpointId]) -> Result<String> {
    let mut doc: DocumentMut = content.parse()?;
    let entries = doc
        .entry(table)
        .or_insert(toml_edit::table())
        .as_table_like_mut()
        .with_context(|| format!("[{table}] must be a table"))?;
    for id in remove {
        entries.remove(&id.to_string());
    }
    entries.insert(&id.to_string(), toml_edit::value(label.unwrap_or_default()));
    Ok(doc.to_string())
}

/// Saves a server (and its name) to the config file's `[servers]`, dropping the entries for
/// `remove` (servers whose name it takes over).
pub fn save_server(id: EndpointId, name: Option<&str>, remove: &[EndpointId]) -> Result<()> {
    let path = config_path()?;
    if !path.exists() {
        create(&path)?;
    }
    info!("Saving server [{}] to {}", short_id(id), path.display());
    let content = std::fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let updated = set_id(&content, SERVERS, id, name, remove).with_context(|| format!("invalid {}", path.display()))?;
    replace(&path, &updated)
}

/// Whether `item` holds any entry; used to tell "table present but empty".
#[cfg(test)]
fn is_empty_table(item: &toml_edit::Item) -> bool {
    item.as_table_like().is_some_and(|t| t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> (EndpointId, EndpointId, EndpointId) {
        let id = |n: u8| SecretKey::from_bytes(&[n; 32]).public();
        (id(1), id(2), id(3))
    }

    #[test]
    fn template_has_a_key_and_no_ids() {
        let hex = new_key_hex();
        let doc: DocumentMut = template(&hex).parse().unwrap();
        assert_eq!(doc[SECRET_KEY].as_str(), Some(hex.as_str()));
        assert!(SecretKey::from_str(&hex).is_ok());
        assert!(is_empty_table(&doc[SERVERS]) && is_empty_table(&doc[CLIENTS]));
    }

    #[test]
    fn parses_ids_and_labels_in_order() {
        let (a, b, _) = ids();
        let doc: DocumentMut = format!("[clients]\n# laptop and phone\n{a} = \"\"\n{b} = \" Dan Bowers PC \"\n").parse().unwrap();
        assert_eq!(parse_ids(&doc, CLIENTS).unwrap(), vec![(a, None), (b, Some("Dan Bowers PC".to_string()))]);
        assert!(parse_ids(&doc, SERVERS).unwrap().is_empty());
    }

    #[test]
    fn invalid_id_or_value_is_an_error() {
        let (a, _, _) = ids();
        let doc: DocumentMut = "[clients]\nnot-a-node-id = \"x\"\n".parse().unwrap();
        assert!(format!("{:#}", parse_ids(&doc, CLIENTS).unwrap_err()).contains("not-a-node-id"));
        let doc: DocumentMut = format!("[clients]\n{a} = 5\n").parse().unwrap();
        assert!(parse_ids(&doc, CLIENTS).is_err());
    }

    #[test]
    fn renames_in_place_keeping_comments_and_the_key() {
        let (a, b, _) = ids();
        let content = format!("secret-key = \"k\" # mine\n\n# saved\n[servers]\n{a} = \"Old\"\n{b} = \"Other\"\n\n[clients]\n# none\n");
        let out = set_id(&content, SERVERS, a, Some("New Name"), &[]).unwrap();
        assert_eq!(out, content.replace("Old", "New Name"));
    }

    #[test]
    fn adds_unknown_id_and_creates_the_table() {
        let (a, b, _) = ids();
        let out = set_id(&format!("[servers]\n{a} = \"One\"\n"), SERVERS, b, None, &[]).unwrap();
        assert_eq!(out, format!("[servers]\n{a} = \"One\"\n{b} = \"\"\n"));
        let out = set_id("secret-key = \"k\"\n", SERVERS, a, Some("One"), &[]).unwrap();
        assert_eq!(out, format!("secret-key = \"k\"\n\n[servers]\n{a} = \"One\"\n"));
    }

    #[test]
    fn removes_servers_whose_name_is_taken_over() {
        let (a, b, c) = ids();
        let content = format!("[servers]\n{a} = \"Home\"\n{b} = \"Work\"\n");
        let out = set_id(&content, SERVERS, c, Some("Home"), &[a]).unwrap();
        assert_eq!(out, format!("[servers]\n{b} = \"Work\"\n{c} = \"Home\"\n"));
    }
}
