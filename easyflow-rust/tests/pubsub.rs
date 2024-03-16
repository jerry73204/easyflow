use anyhow::{Error, Result};
use easyflow::Dataflow;
use futures::future::FutureExt as _;
use std::{fs, path::Path};
use tokio::task;

type R<T> = Result<T, Error>;

const CONFIG_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pubsub.json5");
const OUTPUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/out");
const PAYLOAD: &[u8] = &[0x00, 0x03, 0x01, 0x04];

#[tokio::test]
async fn pubsub() -> Result<()> {
    if Path::new(OUTPUT_DIR).exists() {
        fs::remove_dir_all(OUTPUT_DIR)?;
    }

    let publisher = task::spawn(async move {
        let graph = Dataflow::open(CONFIG_FILE)?;
        let mut sender = graph.build_sender_to("publisher", "channel").await?;
        sender.send(PAYLOAD).await?;
        R::Ok(())
    })
    .map(|result| result?);

    let consumer = task::spawn(async move {
        let graph = Dataflow::open(CONFIG_FILE)?;
        let mut receiver = graph.build_receiver_from("consumer", "channel").await?;
        let payload = receiver.recv().await?.expect("unexpected end of stream");
        assert_eq!(payload, PAYLOAD);
        R::Ok(())
    })
    .map(|result| result?);

    futures::try_join!(publisher, consumer)?;
    fs::remove_dir_all(OUTPUT_DIR)?;

    Ok(())
}
