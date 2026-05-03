use proxy_rs_derive::StreamCodec;

#[derive(StreamCodec)]
pub struct ProxyHeaderV1 {
    pub version: u8,
    pub host: String,
    pub port: u16,
}