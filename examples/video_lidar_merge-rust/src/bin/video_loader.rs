//! A fake video data loader.

use anyhow::Result;
use easyflow::Dataflow;
use rand::prelude::*;
use std::time::Duration;
use tokio::time::sleep;
use video_lidar_merge::VideoPacket;

#[tokio::main]
async fn main() -> Result<()> {
    let mut rng = rand::thread_rng();

    let config_file = concat!(env!("CARGO_MANIFEST_DIR"), "/dataflow.json5");
    let dataflow = Dataflow::open(config_file)?;
    let mut sender = dataflow.build_sender("video_loader").await?;

    loop {
        let h = 640;
        let w = 480;
        let dimension = [w, h];

        let image: Vec<u8> = (0..(h * w)).map(|_| rng.gen()).collect();
        let packet = VideoPacket { dimension, image };

        let payload = bincode::serialize(&packet)?;
        sender.send(payload).await?;

        eprintln!("sent one video packet");

        sleep(Duration::from_secs(1)).await;
    }
}
