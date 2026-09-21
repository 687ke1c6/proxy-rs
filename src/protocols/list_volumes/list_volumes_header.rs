use crate::protocols::codec::StreamCodec;

#[derive(Debug, StreamCodec)]
pub struct ListVolumesRequest {
    pub version: u8,
}

#[derive(Debug, StreamCodec)]
pub struct VolumeEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, StreamCodec)]
pub struct ListVolumesResponse {
    pub version: u8,
    pub volumes: Vec<VolumeEntry>,
}
