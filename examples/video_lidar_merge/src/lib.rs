use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct LidarPacket {
    pub points: Vec<[f32; 3]>,
}

#[derive(Serialize, Deserialize)]
pub struct VideoPacket {
    pub dimension: [u32; 2],
    pub image: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct MergePacket {
    pub points: Vec<[f32; 3]>,
    pub image_dimension: [u32; 2],
    pub image: Vec<u8>,
}
