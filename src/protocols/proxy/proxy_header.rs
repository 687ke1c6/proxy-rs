use proxy_rs_derive::StreamCodec;

pub const PROXY_HEADER_VERSION: u8 = 2;

/// `Ack.ack` codes the server replies with after a `ProxyHeader` (0 = `Ack::ack()`, connected).
pub const ACK_NOT_ALLOWED: u8 = 1;
pub const ACK_CONNECT_FAILED: u8 = 2;
pub const ACK_BAD_VERSION: u8 = 3;

#[derive(StreamCodec)]
pub struct ProxyHeader {
    pub version: u8,
    pub host: String,
    pub port: u16,
}
