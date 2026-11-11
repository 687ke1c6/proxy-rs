/// An error and its causes as one line, like anyhow's `{e:#}`, but without repeats.
///
/// `{e:#}` joins every error in the chain with `: `, and wrapper errors often already
/// include their source's text in their own message (iroh's transparent wrappers repeat
/// it verbatim), so you'd get `timed out: timed out: timed out`. Here a cause is skipped
/// when the message printed before it already contains it.
pub fn display_chain(e: &anyhow::Error) -> String {
    let mut out = String::new();
    let mut prev = String::new();
    for cause in e.chain() {
        let msg = cause.to_string();
        if !prev.is_empty() && prev.contains(&msg) {
            continue;
        }
        if !out.is_empty() {
            out += ": ";
        }
        out += &msg;
        prev = msg;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An error with a fixed message and an optional source.
    #[derive(Debug)]
    struct E(&'static str, Option<Box<E>>);

    impl std::fmt::Display for E {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.0)
        }
    }

    impl std::error::Error for E {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            self.1.as_deref().map(|e| e as _)
        }
    }

    fn chain(msgs: &[&'static str]) -> anyhow::Error {
        let inner = msgs.iter().rev().fold(None, |source, msg| Some(Box::new(E(msg, source))));
        anyhow::Error::new(*inner.unwrap())
    }

    #[test]
    fn collapses_transparent_wrappers() {
        assert_eq!(display_chain(&chain(&["timed out", "timed out", "timed out"])), "timed out");
    }

    #[test]
    fn drops_causes_the_wrapper_already_includes() {
        let e = chain(&["connect failed: refused", "refused"]);
        assert_eq!(display_chain(&e), "connect failed: refused");
    }

    #[test]
    fn keeps_distinct_causes() {
        let e = chain(&["No addressing information available", "No addressing information available", "lookup failed"])
            .context("Failed to list volumes");
        assert_eq!(display_chain(&e), "Failed to list volumes: No addressing information available: lookup failed");
    }
}
