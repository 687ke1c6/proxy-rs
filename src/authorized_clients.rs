use anyhow::{Context, Result};
use iroh::EndpointId;
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::{error, warn};

pub const FILENAME: &str = "authorized-clients";

/// Written to a new `authorized-clients` file; only comments, so it allows no one.
const TEMPLATE: &str = "\
# Client node ids allowed to connect to this proxy-rs server, one per line.
# Anything after the id is a comment. Get a client's id with `proxy-rs client whoami`.
# This file is re-read on every connection: edit it to add or revoke clients, no restart needed.
#
# 7c41d2...9e0b  laptop
";

/// Which client node IDs may connect to the server: `--allow` IDs (fixed at startup)
/// plus the `authorized-clients` file, which is re-read on every connection so clients
/// can be added or revoked without a restart.
pub struct ClientAllowlist {
    fixed: HashSet<EndpointId>,
    file: PathBuf,
}

/// Parses `authorized-clients`: one node ID per line, optionally followed by whitespace
/// and a free-form comment (e.g. a name). Blank lines and `#` comments are ignored.
fn parse(content: &str) -> Result<HashSet<EndpointId>> {
    let mut ids = HashSet::new();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let id = line.split_whitespace().next().unwrap_or(line);
        let id = EndpointId::from_str(id)
            .with_context(|| format!("line {}: invalid node id {id:?}", i + 1))?;
        ids.insert(id);
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

    /// Reads the `authorized-clients` file. A missing file counts as empty.
    pub fn read_file(&self) -> Result<HashSet<EndpointId>> {
        let content = match std::fs::read_to_string(&self.file) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashSet::new()),
            Err(e) => return Err(e).with_context(|| format!("failed to read {}", self.file.display())),
        };
        parse(&content).with_context(|| format!("invalid {}", self.file.display()))
    }

    pub fn fixed_count(&self) -> usize {
        self.fixed.len()
    }

    pub fn file(&self) -> &PathBuf {
        &self.file
    }

    /// Fails closed: if the file can't be read or parsed, only `--allow` IDs get in.
    pub fn allows(&self, id: EndpointId) -> bool {
        let allowed = self.fixed.contains(&id)
            || match self.read_file() {
                Ok(ids) => ids.contains(&id),
                Err(e) => {
                    error!("{e:#}; denying clients not given via --allow");
                    false
                }
            };
        if !allowed {
            warn!("Rejected connection from unauthorized client {id}");
        }
        allowed
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
        let content = format!("# laptop and phone\n\n{a}\n  {b}   phone  \n");
        let ids = parse(&content).unwrap();
        assert_eq!(ids, HashSet::from([a, b]));
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
        assert!(list.allows(a));
        assert!(!list.allows(id()));
    }

    #[test]
    fn file_is_reread_and_bad_file_fails_closed() {
        let dir = std::env::temp_dir().join(format!("proxy-rs-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILENAME);
        let (a, b) = (id(), id());
        let list = ClientAllowlist::new(&[], path.clone()).unwrap();

        std::fs::write(&path, format!("{a}\n")).unwrap();
        assert!(list.allows(a));
        assert!(!list.allows(b));

        std::fs::write(&path, format!("{b}\n")).unwrap();
        assert!(!list.allows(a), "revoked by editing the file");
        assert!(list.allows(b), "added by editing the file");

        std::fs::write(&path, format!("{b}\ngarbage\n")).unwrap();
        assert!(!list.allows(b), "unparseable file denies everyone");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
