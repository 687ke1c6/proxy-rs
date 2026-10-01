use anyhow::{Context, Result};
use iroh::SecretKey;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;
use tracing::info;

use crate::config_dir::config_dir;

pub const SERVER_KEY_FILE: &str = "server-key";
pub const CLIENT_KEY_FILE: &str = "client-key";

/// Shortened node id for console output: first 5 and last 3 characters, e.g. `3f1eb...58d`.
/// Full ids are only printed where they must be copied: the server's startup line and
/// `client whoami`.
pub fn short_id(id: impl std::fmt::Display) -> String {
    let id = id.to_string();
    if id.len() <= 11 || !id.is_ascii() {
        return id;
    }
    format!("{}...{}", &id[..5], &id[id.len() - 4..])
}

/// Loads the hex-encoded secret key at `<config_dir>/<file_name>`, generating and saving
/// a new one if it doesn't exist yet. The key determines this node's stable node ID,
/// which is (re)written to `<file_name>.pub` so it's readable without parsing the key.
///
/// Safe against several processes starting at once: a new key is written to a temp file
/// and hard-linked into place, which fails if another process got there first, in which
/// case that process's key is used. Everyone ends up with the same key.
pub fn load_or_create_secret_key(file_name: &str) -> Result<SecretKey> {
    let path = config_dir()?.join(file_name);
    if !path.exists() {
        create_key_file(&path)?;
    }
    let hex = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read key file: {}", path.display()))?;
    let key = SecretKey::from_str(hex.trim()).with_context(|| format!("invalid key in {}", path.display()))?;

    let pub_path = path.with_file_name(format!("{file_name}.pub"));
    std::fs::write(&pub_path, key.public().to_string())
        .with_context(|| format!("failed to write public key file: {}", pub_path.display()))?;
    Ok(key)
}

fn create_key_file(path: &Path) -> Result<()> {
    let key = SecretKey::generate();
    let hex: String = key.to_bytes().iter().map(|b| format!("{b:02x}")).collect();

    let tmp = path.with_file_name(format!(
        "{}.tmp.{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("key"),
        std::process::id()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options
        .open(&tmp)
        .with_context(|| format!("failed to write key file: {}", tmp.display()))?;
    file.write_all(hex.as_bytes())?;
    file.sync_all()?;
    drop(file);

    let linked = std::fs::hard_link(&tmp, path);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(()) => {
            info!("Generated new secret key, saved to {} {}", path.display(), short_id(key.public()));
            Ok(())
        }
        // Another process created it between our exists() check and now; use theirs.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e).with_context(|| format!("failed to write key file: {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_id_keeps_first_five_and_last_three() {
        let id = "3f1eb5317373e13e8b67e679d40cc2f51539c9055cbd0468998ca4a45771d58d";
        assert_eq!(short_id(id), "3f1eb...58d");
        assert_eq!(short_id("abc"), "abc");
    }
}
