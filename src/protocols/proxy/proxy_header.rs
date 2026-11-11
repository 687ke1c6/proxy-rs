use serde::{Deserialize, Serialize};

/// `Ack.ack` codes the server replies with after a `ProxyHeader` (0 = `Ack::ack()`, connected).
pub const ACK_NOT_ALLOWED: u8 = 1;
pub const ACK_CONNECT_FAILED: u8 = 2;

#[derive(Serialize, Deserialize)]
pub struct ProxyHeader {
    pub host: String,
    pub port: u16,
}
