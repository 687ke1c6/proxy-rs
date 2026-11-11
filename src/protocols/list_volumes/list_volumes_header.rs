use serde::{Deserialize, Serialize};

/// Empty for now; still sent so the client's request is an explicit message.
#[derive(Debug, Serialize, Deserialize)]
pub struct ListVolumesRequest {}

#[derive(Debug, Serialize, Deserialize)]
pub struct VolumeEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListVolumesResponse {
    pub volumes: Vec<VolumeEntry>,
}
