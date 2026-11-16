use serde::{Deserialize, Serialize};

/// `Ack.ack` codes the server replies with after an `Open` (0 = `Ack::ack()`, go ahead).
/// The request doesn't fit the server's mode, or the server's own checks refused it.
pub const ACK_REFUSED: u8 = 1;
/// Tunnel mode: the server couldn't connect to its target.
pub const ACK_CONNECT_FAILED: u8 = 2;

/// First frame of every stream, sent by the client before any payload. It makes the stream
/// exist for the server even when the local application waits for the other side to speak
/// first (ssh, SMTP: a QUIC stream is invisible to the acceptor until something is written).
///
/// The client doesn't know which mode the server runs: it sends `Open`, gets an `Ack`, and
/// from then on the stream is raw bytes that only the server interprets.
#[derive(Debug, Serialize, Deserialize)]
pub struct Open {
    /// Extra arguments given to `--pipe`, forwarded as-is. Empty for `--listen`. Only an
    /// `rsync` server takes any: what rsync appends to its `-e` command, i.e.
    /// `<host> rsync --server <flags> . <volume[/dir]>`.
    pub args: Vec<String>,
}
