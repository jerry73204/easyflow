//! A fake LiDAR data loader.

use anyhow::Result;
use easyflow::Dataflow;
use rand::prelude::*;
use std::time::Duration;
use tokio::time::sleep;
use video_lidar_merge::LidarPacket;

#[tokio::main]
async fn main() -> Result<()> {
    let mut rng = rand::thread_rng();

    let config_file = concat!(env!("CARGO_MANIFEST_DIR"), "/dataflow.json5");
    let dataflow = Dataflow::open(config_file)?;
    let mut sender = dataflow.build_sender("lidar_loader").await?;

    loop {
        let points: Vec<[f32; 3]> = (0..1000).map(|_| rng.gen()).collect();
        let packet = LidarPacket { points };

        let payload = bincode::serialize(&packet)?;
        sender.send(payload).await?;

        eprintln!("sent one lidar packet");

        sleep(Duration::from_secs(1)).await;
    }
}
