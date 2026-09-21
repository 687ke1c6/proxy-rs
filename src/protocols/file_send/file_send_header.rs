use crate::protocols::codec::StreamCodec;

#[derive(Debug, StreamCodec)]
pub struct FileSendHeader {
    pub version: u8,
    pub file_name: String,
    pub file_size: u64,
    /// Server-side volume name (see `-v`/`--volume`); empty means "let the server pick a
    /// default" (only valid when the server exposes zero or one volumes).
    pub volume: String,
    /// Destination directory within the volume, `/`-separated, relative, no `..`; empty
    /// means the volume root.
    pub target_dir: String,
    #[codec(bitpack)]
    pub can_overwrite: bool,
}