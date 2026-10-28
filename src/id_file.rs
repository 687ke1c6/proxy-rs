use anyhow::{Context, Result};
use iroh::EndpointId;
use std::str::FromStr;

/// Parses one line of an id file (`authorized-clients`, `known-servers`): a node ID,
/// optionally followed by whitespace and a free-form label (e.g. `Dan's PC`).
/// Blank lines and `#` comments are `None`.
pub fn parse_line(line: &str) -> Result<Option<(EndpointId, Option<String>)>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let (id, label) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let id = EndpointId::from_str(id).with_context(|| format!("invalid node id {id:?}"))?;
    let label = label.trim();
    Ok(Some((id, (!label.is_empty()).then(|| label.to_string()))))
}

/// Parses a whole id file, in file order.
pub fn parse(content: &str) -> Result<Vec<(EndpointId, Option<String>)>> {
    content
        .lines()
        .enumerate()
        .filter_map(|(i, line)| parse_line(line).with_context(|| format!("line {}", i + 1)).transpose())
        .collect()
}

/// Formats an entry as a line of an id file (without the newline).
pub fn format_line(id: EndpointId, label: Option<&str>) -> String {
    match label {
        Some(label) => format!("{id} {label}"),
        None => id.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::SecretKey;

    #[test]
    fn round_trips_ids_and_labels_in_order() {
        let (a, b) = (SecretKey::generate().public(), SecretKey::generate().public());
        let content = format!("# comment\n\n{}\n  {}  \n", format_line(a, Some("Plucky Narwhal")), format_line(b, None));
        assert_eq!(parse(&content).unwrap(), vec![(a, Some("Plucky Narwhal".to_string())), (b, None)]);
    }
}
