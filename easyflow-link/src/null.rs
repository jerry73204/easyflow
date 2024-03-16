use crate::common::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_recv")]
    pub recv: ReceiverKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiverKind {
    Empty,
    Block,
    Error,
}

fn default_recv() -> ReceiverKind {
    ReceiverKind::Empty
}

impl Config {
    pub fn build_sender(&self) -> Sender {
        Sender { _private: [] }
    }

    pub fn build_receiver(&self) -> Receiver {
        Receiver { recv: self.recv }
    }
}

#[derive(Debug)]
pub struct Sender {
    _private: [u8; 0],
}

impl Sender {
    pub fn send(&self, _payload: &[u8]) {}

    pub fn into_sink(self) -> impl Sink<Vec<u8>, Error = Error> {
        sink::unfold(self, |sender, payload: Vec<u8>| async move {
            sender.send(&payload);
            Ok(sender)
        })
    }
}

#[derive(Debug)]
pub struct Receiver {
    recv: ReceiverKind,
}

impl Receiver {
    pub async fn recv(&self) -> Result<Option<Vec<u8>>> {
        match self.recv {
            ReceiverKind::Empty => Ok(None),
            ReceiverKind::Block => {
                let () = future::pending().await;
                unreachable!();
            }
            ReceiverKind::Error => {
                bail!("the null exchange cannot be read");
            }
        }
    }

    pub fn into_stream(self) -> impl Stream<Item = Result<Vec<u8>>> {
        stream::try_unfold(self, |rx| async move {
            let item = rx.recv().await?;
            anyhow::Ok(item.map(|item| (item, rx)))
        })
    }
}
