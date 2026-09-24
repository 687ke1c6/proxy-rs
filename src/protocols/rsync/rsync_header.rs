use crate::protocols::codec::StreamCodec;

/// Sent once by `sync-rsh` before the stream becomes a raw byte pipe carrying the
/// actual rsync protocol. `argv` is everything rsync handed to `sync-rsh` after the
/// placeholder host token — i.e. `--server`, the negotiated flags, and (as the last
/// element) the destination-path token, e.g. `volume[/dir]`.
///
/// The server never trusts that trailing path element as a real filesystem path: it
/// parses it the same way `--target` is parsed for file-send, and resolves it through
/// the configured volumes before spawning anything. Everything before it (the flags)
/// is passed through to the spawned `rsync --server` verbatim, since flags like
/// compression or archive-mode aren't filesystem-referencing by themselves.
#[derive(Debug, StreamCodec)]
pub struct RsyncHeader {
    pub version: u8,
    pub argv: Vec<String>,
}
