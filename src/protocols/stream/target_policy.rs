use anyhow::{Context, Result, bail, ensure};
use std::fmt;
use std::str::FromStr;

/// Host half of a `-t/--target` pattern. Matched against the hostname *string* the
/// client sends, before any DNS resolution — so `localhost` does not admit `127.0.0.1`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum HostPattern {
    /// `*`
    Any,
    /// `localhost`, `10.0.0.5`, `[::1]` (stored normalized, without brackets)
    Exact(String),
    /// `*.lan` → stores `.lan`; matches one or more labels in front of it
    Suffix(String),
}

/// Port half of a `-t/--target` pattern: `*`, `22`, or an inclusive range `8000-8100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortPattern {
    Any,
    Range(u16, u16),
}

/// One `-t/--target` entry: `<host-pattern>:<port-pattern>`, or bare `*` (= `*:*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPattern {
    host: HostPattern,
    port: PortPattern,
}

/// Lowercases and strips a trailing `.` and IPv6 brackets, so client-sent hosts and
/// pattern hosts compare on equal terms.
fn normalize_host(host: &str) -> String {
    let host = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase()
}

impl FromStr for HostPattern {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "" => bail!("empty host"),
            "*" => Ok(HostPattern::Any),
            "*.*" => bail!("'*.*' is not supported; use '*' to match any host"),
            _ => {
                if let Some(suffix) = s.strip_prefix('*') {
                    let suffix = normalize_host(suffix);
                    ensure!(
                        suffix.starts_with('.') && suffix.len() > 1 && !suffix.contains('*'),
                        "host wildcard must be a leading '*.' followed by a domain, e.g. '*.lan'"
                    );
                    Ok(HostPattern::Suffix(suffix))
                } else {
                    ensure!(!s.contains('*'), "'*' is only allowed as a leading '*.' label or on its own");
                    Ok(HostPattern::Exact(normalize_host(s)))
                }
            }
        }
    }
}

impl FromStr for PortPattern {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        if s == "*" {
            return Ok(PortPattern::Any);
        }
        let parse = |p: &str| p.parse::<u16>().with_context(|| format!("invalid port {p:?}"));
        let (lo, hi) = match s.split_once('-') {
            Some((lo, hi)) => (parse(lo)?, parse(hi)?),
            None => { let p = parse(s)?; (p, p) }
        };
        ensure!(lo <= hi, "invalid port range {s:?}: start is greater than end");
        Ok(PortPattern::Range(lo, hi))
    }
}

impl FromStr for TargetPattern {
    type Err = anyhow::Error;

    fn from_str(raw: &str) -> Result<Self> {
        let s = raw.trim();
        if s == "*" {
            return Ok(TargetPattern { host: HostPattern::Any, port: PortPattern::Any });
        }
        ensure!(s != "*.*", "invalid --target {raw:?}: use '*' to match any host and port");
        let (host, port) = if s.starts_with('[') {
            let (host, port) = s
                .split_once("]:")
                .with_context(|| format!("invalid --target {raw:?}: expected [ipv6]:port"))?;
            (format!("{host}]"), port)
        } else {
            let (host, port) = s
                .rsplit_once(':')
                .with_context(|| format!("invalid --target {raw:?}: expected host:port (or '*')"))?;
            ensure!(!host.contains(':'), "invalid --target {raw:?}: IPv6 hosts must be bracketed, e.g. [::1]:22");
            (host.to_string(), port)
        };
        let host = host.parse().with_context(|| format!("invalid --target {raw:?}"))?;
        let port = port.parse().with_context(|| format!("invalid --target {raw:?}"))?;
        Ok(TargetPattern { host, port })
    }
}

impl TargetPattern {
    fn allows(&self, host: &str, port: u16) -> bool {
        let port_ok = match self.port {
            PortPattern::Any => true,
            PortPattern::Range(lo, hi) => (lo..=hi).contains(&port),
        };
        port_ok
            && match &self.host {
                HostPattern::Any => true,
                HostPattern::Exact(h) => *h == host,
                HostPattern::Suffix(suffix) => host.len() > suffix.len() && host.ends_with(suffix.as_str()),
            }
    }

    fn is_open(&self) -> bool {
        self.host == HostPattern::Any && self.port == PortPattern::Any
    }
}

impl fmt::Display for TargetPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.host {
            HostPattern::Any => write!(f, "*")?,
            HostPattern::Exact(h) if h.contains(':') => write!(f, "[{h}]")?,
            HostPattern::Exact(h) => write!(f, "{h}")?,
            HostPattern::Suffix(s) => write!(f, "*{s}")?,
        }
        match self.port {
            PortPattern::Any => write!(f, ":*"),
            PortPattern::Range(lo, hi) if lo == hi => write!(f, ":{lo}"),
            PortPattern::Range(lo, hi) => write!(f, ":{lo}-{hi}"),
        }
    }
}

/// The set of TCP targets the server will dial on a client's behalf, from `-t/--target`.
/// Empty means TCP proxying is disabled.
#[derive(Debug, Clone, Default)]
pub struct TargetPolicy {
    patterns: Vec<TargetPattern>,
}

impl TargetPolicy {
    pub fn parse(raw: &[String]) -> Result<Self> {
        let patterns = raw.iter().map(|s| s.parse()).collect::<Result<_>>()?;
        Ok(TargetPolicy { patterns })
    }

    pub fn allows(&self, host: &str, port: u16) -> bool {
        let host = normalize_host(host);
        self.patterns.iter().any(|p| p.allows(&host, port))
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// True if any pattern admits every host and port (an open proxy).
    pub fn is_open(&self) -> bool {
        self.patterns.iter().any(TargetPattern::is_open)
    }

    pub fn patterns(&self) -> &[TargetPattern] {
        &self.patterns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(patterns: &[&str]) -> TargetPolicy {
        TargetPolicy::parse(&patterns.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    fn rejects(pattern: &str) {
        assert!(pattern.parse::<TargetPattern>().is_err(), "{pattern:?} should be rejected");
    }

    #[test]
    fn exact_host_and_port() {
        let p = policy(&["localhost:22"]);
        assert!(p.allows("localhost", 22));
        assert!(!p.allows("localhost", 23));
        assert!(!p.allows("otherhost", 22));
    }

    #[test]
    fn matches_host_string_not_resolved_address() {
        let p = policy(&["localhost:22"]);
        assert!(!p.allows("127.0.0.1", 22));
    }

    #[test]
    fn host_is_case_insensitive_and_trailing_dot_stripped() {
        let p = policy(&["Example.COM.:443"]);
        assert!(p.allows("example.com", 443));
        assert!(p.allows("EXAMPLE.com.", 443));
    }

    #[test]
    fn suffix_glob_matches_one_or_more_labels() {
        let p = policy(&["*.lan:*"]);
        assert!(p.allows("a.lan", 80));
        assert!(p.allows("a.b.lan", 80));
        assert!(!p.allows("lan", 80));
        assert!(!p.allows(".lan", 80));
        assert!(!p.allows("alan", 80));
    }

    #[test]
    fn port_ranges_are_inclusive() {
        let p = policy(&["db:8000-8100"]);
        assert!(p.allows("db", 8000));
        assert!(p.allows("db", 8050));
        assert!(p.allows("db", 8100));
        assert!(!p.allows("db", 7999));
        assert!(!p.allows("db", 8101));
    }

    #[test]
    fn any_host_fixed_port() {
        let p = policy(&["*:443"]);
        assert!(p.allows("anything.example", 443));
        assert!(!p.allows("anything.example", 80));
        assert!(!p.is_open());
    }

    #[test]
    fn star_is_open() {
        for pattern in ["*", "*:*", " * "] {
            let p = policy(&[pattern]);
            assert!(p.is_open(), "{pattern:?}");
            assert!(p.allows("10.0.0.1", 1));
            assert!(p.allows("::1", 65535));
        }
    }

    #[test]
    fn ipv6_requires_brackets() {
        let p = policy(&["[::1]:22"]);
        assert!(p.allows("::1", 22));
        assert!(p.allows("[::1]", 22));
        assert!(!p.allows("::2", 22));
        rejects("::1:22");
    }

    #[test]
    fn empty_policy_allows_nothing() {
        let p = policy(&[]);
        assert!(p.is_empty());
        assert!(!p.allows("localhost", 22));
    }

    #[test]
    fn invalid_patterns_are_rejected() {
        rejects("*.*");
        rejects("*.*:80");
        rejects("localhost");
        rejects(":22");
        rejects("host:");
        rejects("host:abc");
        rejects("host:70000");
        rejects("host:100-50");
        rejects("a*b:22");
        rejects("*.a*b:22");
        rejects("*lan:22");
        rejects("*.:22");
        rejects("[::1]22");
    }

    #[test]
    fn display_round_trips() {
        for pattern in ["*:*", "localhost:22", "*.lan:8000-8100", "[::1]:22", "*:443"] {
            let parsed: TargetPattern = pattern.parse().unwrap();
            assert_eq!(parsed.to_string(), pattern);
        }
    }
}
