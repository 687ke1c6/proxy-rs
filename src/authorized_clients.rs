use anyhow::{Context, Result};
use iroh::{EndpointId, endpoint::{AfterHandshakeOutcome, ConnectionInfo, EndpointHooks}};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::{error, warn};

use crate::identity::short_id;
use crate::protocols::ping::alpn::PING_ALPN_V1;

pub const FILENAME: &str = "authorized-clients";

/// Written to a new `authorized-clients` file; only comments, so it allows no one.
const TEMPLATE: &str = "\
# Client node ids allowed to connect to this proxy-rs server, one per line.
# Anything after the id is a label, shown when that client connects.
# Get a client's id with `proxy-rs client whoami`.
# This file is re-read on every connection: edit it to add or revoke clients, no restart needed.
#
# 7c41d2...9e0b  laptop
";

/// Which client node IDs may connect to the server: `--allow` IDs (fixed at startup)
/// plus the `authorized-clients` file, which is re-read on every connection so clients
/// can be added or revoked without a restart.
#[derive(Debug)]
pub struct ClientAllowlist {
    fixed: HashSet<EndpointId>,
    file: PathBuf,
}

/// Whether a client may connect, and its label from `authorized-clients` if it has one.
#[derive(Debug, PartialEq)]
pub enum Access {
    Allowed { label: Option<String> },
    Denied,
}

/// Parses `authorized-clients`: one node ID per line, optionally followed by whitespace
/// and a free-form label (e.g. `Dan's PC`). Blank lines and `#` comments are ignored.
fn parse(content: &str) -> Result<HashMap<EndpointId, Option<String>>> {
    let mut ids = HashMap::new();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (id, label) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let id = EndpointId::from_str(id)
            .with_context(|| format!("line {}: invalid node id {id:?}", i + 1))?;
        let label = label.trim();
        ids.insert(id, (!label.is_empty()).then(|| label.to_string()));
    }
    Ok(ids)
}

impl ClientAllowlist {
    pub fn new(allow: &[String], file: PathBuf) -> Result<Self> {
        let fixed = allow
            .iter()
            .map(|id| EndpointId::from_str(id.trim()).with_context(|| format!("invalid --allow node id {id:?}")))
            .collect::<Result<_>>()?;
        Ok(ClientAllowlist { fixed, file })
    }

    /// Creates the `authorized-clients` file with a commented template if it doesn't exist,
    /// so operators can find it and see the format. Never touches an existing file.
    pub fn create_file_if_missing(&self) -> Result<()> {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&self.file) {
            Ok(mut file) => {
                file.write_all(TEMPLATE.as_bytes())
                    .with_context(|| format!("failed to write {}", self.file.display()))?;
                println!("Created {}", self.file.display());
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(e) => Err(e).with_context(|| format!("failed to create {}", self.file.display())),
        }
    }

    /// Reads the `authorized-clients` file: id -> optional label. A missing file counts as empty.
    pub fn read_file(&self) -> Result<HashMap<EndpointId, Option<String>>> {
        let content = match std::fs::read_to_string(&self.file) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
            Err(e) => return Err(e).with_context(|| format!("failed to read {}", self.file.display())),
        };
        parse(&content).with_context(|| format!("invalid {}", self.file.display()))
    }

    /// Number of distinct clients currently allowed: `--allow` IDs plus the file's.
    pub fn count(&self) -> Result<usize> {
        let mut ids: HashSet<EndpointId> = self.read_file()?.into_keys().collect();
        ids.extend(&self.fixed);
        Ok(ids.len())
    }


    pub fn file(&self) -> &PathBuf {
        &self.file
    }

    /// Re-reads the file, so edits take effect on the next connection. Fails closed: if the
    /// file can't be read or parsed, only `--allow` IDs get in (without labels).
    pub fn check(&self, id: EndpointId) -> Access {
        let file_label = match self.read_file() {
            Ok(mut ids) => ids.remove(&id),
            Err(e) => {
                error!("{e:#}; denying clients not given via --allow");
                None
            }
        };
        match file_label {
            Some(label) => Access::Allowed { label },
            None if self.fixed.contains(&id) => Access::Allowed { label: None },
            None => {
                warn!("Rejected connection from unauthorized client {}", short_id(id));
                Access::Denied
            }
        }
    }
}

/// Server endpoint hook that admits only allowlisted clients (every ALPN, before the router
/// hands the connection to a protocol handler) and prints each client session as it starts.
/// `allowlist: None` (`--allow-any`) admits everyone.
#[derive(Debug)]
pub struct ClientGate {
    pub allowlist: Option<ClientAllowlist>,
}

impl EndpointHooks for ClientGate {
    async fn after_handshake<'a>(&'a self, conn: &'a ConnectionInfo) -> AfterHandshakeOutcome {
        if !conn.side().is_server() {
            return AfterHandshakeOutcome::Accept;
        }
        let id = conn.remote_id();
        let label = match &self.allowlist {
            None => None,
            Some(list) => match list.check(id) {
                Access::Allowed { label } => label,
                // Same close reason as iroh's `AccessLimit`; the client's `ping_server` looks for it.
                Access::Denied => {
                    return AfterHandshakeOutcome::Reject { error_code: 0u32.into(), reason: b"not allowed".to_vec() };
                }
            },
        };
        // Every client operation pings first, so a ping marks a client session starting;
        // other ALPNs (e.g. one connection per proxied socket) would be too noisy to print.
        // Printed rather than logged so it's visible at the default (error-only) log level.
        if conn.alpn() == PING_ALPN_V1 {
            match label {
                Some(label) => println!("Client connected: {label} [{}]", short_id(id)),
                None => println!("Client connected: {}", short_id(id)),
            }
        }
        AfterHandshakeOutcome::Accept
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::SecretKey;

    fn id() -> EndpointId {
        SecretKey::generate().public()
    }

    #[test]
    fn parses_ids_comments_and_blank_lines() {
        let (a, b) = (id(), id());
        let content = format!("# laptop and phone\n\n{a}\n  {b}   Dan Bowers PC  \n");
        let ids = parse(&content).unwrap();
        assert_eq!(ids, HashMap::from([(a, None), (b, Some("Dan Bowers PC".to_string()))]));
    }

    #[test]
    fn template_allows_no_one() {
        assert!(parse(TEMPLATE).unwrap().is_empty());
    }

    #[test]
    fn invalid_line_is_an_error_with_line_number() {
        let content = format!("{}\nnot-a-node-id\n", id());
        let err = parse(&content).unwrap_err();
        assert!(format!("{err:#}").contains("line 2"), "{err:#}");
    }

    #[test]
    fn missing_file_is_empty_and_fixed_ids_still_allowed() {
        let a = id();
        let list = ClientAllowlist::new(&[a.to_string()], PathBuf::from("/nonexistent/authorized-clients")).unwrap();
        assert!(list.read_file().unwrap().is_empty());
        assert_eq!(list.check(a), Access::Allowed { label: None });
        assert_eq!(list.check(id()), Access::Denied);
    }

    #[test]
    fn file_is_reread_and_bad_file_fails_closed() {
        let dir = std::env::temp_dir().join(format!("proxy-rs-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILENAME);
        let (a, b) = (id(), id());
        let list = ClientAllowlist::new(&[], path.clone()).unwrap();

        std::fs::write(&path, format!("{a}\n")).unwrap();
        assert_eq!(list.check(a), Access::Allowed { label: None });
        assert_eq!(list.check(b), Access::Denied);

        std::fs::write(&path, format!("{b} phone\n")).unwrap();
        assert_eq!(list.check(a), Access::Denied, "revoked by editing the file");
        assert_eq!(list.check(b), Access::Allowed { label: Some("phone".to_string()) }, "added and labelled by editing the file");

        std::fs::write(&path, format!("{b}\ngarbage\n")).unwrap();
        assert_eq!(list.check(b), Access::Denied, "unparseable file denies everyone");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
