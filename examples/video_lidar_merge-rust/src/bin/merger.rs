//! A fake video data loader.

use anyhow::Result;
use easyflow::Dataflow;
use futures::try_join;
use video_lidar_merge::{LidarPacket, MergePacket, VideoPacket};

#[tokio::main]
async fn main() -> Result<()> {
    let config_file = concat!(env!("CARGO_MANIFEST_DIR"), "/dataflow.json5");
    let dataflow = Dataflow::open(config_file)?;

    let mut video_receiver = dataflow.build_receiver_from("merger", "VIDEO").await?;
    let mut lidar_receiver = dataflow.build_receiver_from("merger", "LIDAR").await?;
    let mut sender = dataflow.build_sender_to("merger", "OUTPUT").await?;

    loop {
        // Collect input messages
        let packets = try_join!(video_receiver.recv(), lidar_receiver.recv())?;
        let (Some(video_payload), Some(lidar_payload)) = packets else {
            eprintln!("receiver closed");
            break;
        };

        // Decode packets
        let video_packet: VideoPacket = bincode::deserialize(&video_payload)?;
        let lidar_packet: LidarPacket = bincode::deserialize(&lidar_payload)?;

        // Merge input packets into one
        let output_packet = MergePacket {
            points: lidar_packet.points,
            image_dimension: video_packet.dimension,
            image: video_packet.image,
        };

        // Publish the output package
        let output_text = serde_json::to_string_pretty(&output_packet)?;
        let output_payload = output_text.as_bytes();
        sender.send(output_payload).await?;

        eprintln!("sent one merged packet");
    }

    Ok(())
}
