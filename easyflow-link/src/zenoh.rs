#![cfg(feature = "zenoh")]

use anyhow::{anyhow, Result};
use futures::{sink, stream, Sink, Stream};
use global::SESSION;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use zenoh::{prelude::r#async::*, publication::Publisher, subscriber::Subscriber};

mod global {
    use std::sync::Arc;

    use once_cell::sync::Lazy;
    use zenoh::Session;

    pub static SESSION: Lazy<Arc<Session>> = Lazy::new(|| {
        use zenoh::prelude::sync::*;

        zenoh::open(zenoh::config::default())
            .res()
            .unwrap()
            .into_arc()
    });
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Config {
    pub key: String,
}

impl Config {
    pub async fn build_sender(&self) -> Result<Sender> {
        let session: Arc<Session> = SESSION.clone();
        let key = self.key.clone();

        let publisher = session
            .declare_publisher(key)
            .res()
            .await
            .map_err(map_err)?;

        Ok(Sender { publisher })
    }

    pub async fn build_receiver(&self) -> Result<Receiver> {
        let session: Arc<Session> = SESSION.clone();
        let key = self.key.clone();

        let subscriber = session
            .declare_subscriber(key)
            .res()
            .await
            .map_err(map_err)?;

        Ok(Receiver { subscriber })
    }
}

#[derive(Debug)]
pub struct Sender {
    publisher: Publisher<'static>,
}

impl Sender {
    pub async fn send(&self, payload: &[u8]) -> Result<()> {
        self.publisher.put(payload).res().await.map_err(map_err)
    }

    pub fn into_sink(self) -> impl Sink<Vec<u8>, Error = anyhow::Error> {
        sink::unfold(self, |sender, payload: Vec<u8>| async move {
            sender.send(&payload).await.map(|_| sender)
        })
    }
}

#[derive(Debug)]
pub struct Receiver {
    subscriber: Subscriber<'static, flume::Receiver<Sample>>,
}

impl Receiver {
    pub async fn recv(&mut self) -> Result<Option<Vec<u8>>> {
        let Ok(sample) = self.subscriber.recv_async().await else {
            return Ok(None);
        };
        let payload: Vec<_> = sample.value.try_into()?;
        Ok(Some(payload))
    }

    pub fn into_stream(self) -> impl Stream<Item = Result<Vec<u8>>> {
        stream::try_unfold(self, |mut rx| async move {
            let item = rx.recv().await?;
            anyhow::Ok(item.map(|item| (item, rx)))
        })
    }
}

fn map_err(err: zenoh::Error) -> anyhow::Error {
    anyhow!("{err}")
}
