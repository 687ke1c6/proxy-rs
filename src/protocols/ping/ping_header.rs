use proxy_rs_derive::StreamCodec;

#[derive(StreamCodec)]
pub struct PingHeader {
    pub version: u8,
    pub msg: String,
}
