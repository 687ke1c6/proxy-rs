use crate::protocols::codec::StreamCodec;

#[derive(Debug, StreamCodec)]
pub struct FileSendHeader {
    pub version: u8,
    pub file_name: String,
    pub file_size: u64,
    #[codec(bitpack)]
    pub can_overwrite: bool,
}