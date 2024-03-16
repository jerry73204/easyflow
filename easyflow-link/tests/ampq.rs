#![cfg(feature = "amqp-test")]

use anyhow::{ensure, Result};
use futures::future::FutureExt;
use std::{fs, path::Path, time::Duration};
use wayside_connect::amqp::Config;

#[tokio::test]
async fn amqp_connect_test() -> Result<()> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("amqp.json5");
    let text = fs::read_to_string(&file)?;
    let config: Config = json5::from_str(&text)?;

    let tx = config.build_sender().await?;
    let mut rx = config.build_receiver().await?;

    let n_payloads = 100;

    let producer = {
        tokio::spawn(async move {
            for round in 0..n_payloads {
                let payload: Vec<u8> = (round as u64).to_le_bytes().into();
                tx.send(&payload).await?;
            }
            anyhow::Ok(())
        })
        .map(|result| anyhow::Ok(result??))
    };

    let consumer = {
        tokio::spawn(async move {
            for round in 0..n_payloads {
                tokio::time::sleep(Duration::from_millis(10)).await;

                let received = match rx.recv().await? {
                    Some(msg) => msg,
                    None => break,
                };

                let data = u64::from_le_bytes(received.try_into().unwrap());
                ensure!(data as usize == round);
            }

            anyhow::Ok(())
        })
        .map(|result| anyhow::Ok(result??))
    };

    futures::try_join!(producer, consumer)?;

    Ok(())
}
