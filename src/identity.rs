/// Shortened node id for console output: first 5 and last 3 characters, e.g. `3f1eb...58d`.
/// Full ids are only printed where they must be copied: the server's startup line and
/// `proxy-rs whoami`.
pub fn short_id(id: impl std::fmt::Display) -> String {
    let id = id.to_string();
    if id.len() <= 11 || !id.is_ascii() {
        return id;
    }
    format!("{}...{}", &id[..5], &id[id.len() - 3..])
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
