#![cfg(feature = "unix-sock")]
#![cfg(unix)]

use crate::common::*;
use anyhow::{ensure, Context};
use async_std::os::unix::net::{UnixListener, UnixStream};
use derivative::Derivative;
use log::{debug, error};
use std::{fs, pin::Pin, time::Instant};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Config {
    pub path: PathBuf,
    #[serde(default = "default_force")]
    pub force: bool,
    #[serde(with = "humantime_serde", default)]
    pub connect_timeout: Option<Duration>,
}

fn default_force() -> bool {
    false
}

impl Config {
    pub async fn build_sender(&self) -> Result<Sender> {
        let deadline = self
            .connect_timeout
            .map(|duration| Instant::now() + duration);
        let mut cnt = 0;
        let stream = loop {
            if let Some(deadline) = deadline {
                ensure!(Instant::now() < deadline, "connection timeout");
            }

            match UnixStream::connect(&self.path).await {
                Ok(stream) => break stream,
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    debug!(
                        "the socket file '{}' does not exist. retrying ...",
                        self.path.display()
                    );
                    async_std::task::sleep(Duration::from_millis(100)).await;
                }
                Err(err) => {
                    if cnt < 10000 {
                        cnt += 1;
                        continue;
                    }
                    return Err(err.into());
                }
            }
        };

        debug!(
            "connection to '{}' socket file established",
            self.path.display()
        );

        Ok(Sender { stream })
    }

    pub async fn build_receiver(&self) -> Result<Receiver> {
        if self.force && self.path.exists() {
            async_std::fs::remove_file(&self.path)
                .await
                .with_context(|| format!("unable to remove file '{}'", self.path.display()))?;
        }

        let listener = UnixListener::bind(&self.path).await.with_context(|| {
            format!(
                "unable to create socket file at '{}'. you may remove this file manually.",
                self.path.display()
            )
        })?;

        let (tx, rx) = flume::bounded(2);

        let accept_future = async move {
            let handle_stream = move |mut stream: UnixStream| {
                let tx = tx.clone();

                async move {
                    loop {
                        let len = {
                            let mut len_buf = [0u8; 8];
                            let mut len_ref = len_buf.as_mut();

                            while !len_ref.is_empty() {
                                let num_bytes = stream.read(len_ref).await?;
                                if num_bytes == 0 {
                                    break;
                                }
                                len_ref = &mut len_ref[num_bytes..];
                            }

                            match len_ref.len() {
                                0 => u64::from_le_bytes(len_buf) as usize,
                                len if len == len_buf.len() => return Ok(()),
                                _ => {
                                    return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into())
                                }
                            }
                        };

                        let mut payload = vec![0; len];
                        stream.read_exact(&mut payload).await?;

                        let result = tx.send_async(payload).await;
                        if result.is_err() {
                            break;
                        }
                    }

                    anyhow::Ok(())
                }
                .boxed()
            };

            listener
                .incoming()
                .map(move |stream| {
                    let handle_stream = handle_stream.clone();
                    let future = async move {
                        handle_stream(stream?).await?;
                        anyhow::Ok(())
                    };

                    anyhow::Ok(future)
                })
                .try_buffer_unordered(1024)
                .try_for_each(|()| future::ok(()))
                .await
        };

        let accept_stream = accept_future
            .map(|result: Result<_>| result.map(|()| None))
            .into_stream();
        let data_stream = rx.into_stream().map(|payload| Ok(Some(payload)));
        let stream = futures::stream::select(accept_stream, data_stream)
            .try_filter_map(future::ok)
            .boxed();

        Ok(Receiver {
            path: self.path.clone(),
            stream,
        })
    }
}

#[derive(Debug)]
pub struct Sender {
    stream: UnixStream,
}

impl Sender {
    pub async fn send(&mut self, payload: &[u8]) -> Result<()> {
        let len = payload.len() as u64;
        self.stream.write_all(&len.to_le_bytes()).await?;
        self.stream.write_all(payload).await?;
        Ok(())
    }

    pub fn into_sink(self) -> impl Sink<Vec<u8>, Error = Error> {
        sink::unfold(self, |mut sender, payload: Vec<u8>| async move {
            sender.send(&payload).await.map(|_| sender)
        })
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Receiver {
    path: PathBuf,
    #[derivative(Debug = "ignore")]
    stream: Pin<Box<dyn Stream<Item = Result<Vec<u8>>> + Send>>,
}

impl Receiver {
    pub async fn recv(&mut self) -> Result<Option<Vec<u8>>> {
        self.stream.next().await.transpose()
    }

    pub fn into_stream(self) -> impl Stream<Item = Result<Vec<u8>>> {
        stream::try_unfold(self, |mut rx| async move {
            let item = rx.recv().await?;
            anyhow::Ok(item.map(|item| (item, rx)))
        })
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            error!(
                "unable to remove socket file '{}': {:?}",
                self.path.display(),
                err
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;
    use std::sync::Arc;

    #[async_std::test]
    pub async fn unix_socket_test() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.as_ref().join("test.socket");

        let config = Config {
            path,
            force: false,
            connect_timeout: None,
        };
        let mut rx = config.build_receiver().await?;
        let mut tx = config.build_sender().await?;

        let mut rng = rand::thread_rng();

        let payloads: Vec<_> = (0..1000)
            .map(|_| {
                let len: usize = rng.gen_range(1..=100);
                let mut data = vec![0u8; len];
                rng.fill(&mut *data);
                data
            })
            .collect();

        let payloads = Arc::new(payloads);

        let send_future = {
            let payloads = payloads.clone();

            async move {
                for payload in &*payloads {
                    tx.send(payload).await?;
                }
                anyhow::Ok(())
            }
        };

        let recv_future = async move {
            for expect in &*payloads {
                let payload = rx.recv().await?;
                ensure!(payload.as_ref() == Some(expect));
            }

            let result = async_std::future::timeout(Duration::ZERO, rx.recv()).await;
            ensure!(result.is_err());

            anyhow::Ok(())
        };

        futures::try_join!(send_future, recv_future)?;
        Ok(())
    }
}
