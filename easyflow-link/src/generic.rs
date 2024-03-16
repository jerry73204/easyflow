#[cfg(feature = "amqp")]
use crate::amqp;
#[cfg(all(unix, feature = "unix-sock"))]
use crate::unix;
#[cfg(feature = "zenoh")]
use crate::zenoh;
use crate::{common::*, file, import, null};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Config {
    File(file::Config),
    #[cfg(feature = "zenoh")]
    Zenoh(zenoh::Config),
    #[cfg(feature = "amqp")]
    Amqp(amqp::Config),
    #[cfg(all(unix, feature = "unix-sock"))]
    Unix(unix::Config),
    Null(null::Config),
    Import(Box<import::Config>),
}

impl Config {
    pub async fn build_sender(&self) -> Result<Sender> {
        let sender = match self {
            Self::File(config) => config.build_sender().await?.into(),
            #[cfg(feature = "zenoh")]
            Self::Zenoh(config) => config.build_sender().await?.into(),
            #[cfg(feature = "amqp")]
            Self::Amqp(config) => config.build_sender().await?.into(),
            #[cfg(all(unix, feature = "unix-sock"))]
            Self::Unix(config) => config.build_sender().await?.into(),
            Self::Null(config) => config.build_sender().into(),
            Self::Import(config) => config.build_sender().await?,
        };
        Ok(sender)
    }

    pub async fn build_receiver(&self) -> Result<Receiver> {
        let receiver = match self {
            Self::File(config) => config.build_receiver().await?.into(),
            #[cfg(feature = "zenoh")]
            Self::Zenoh(config) => config.build_receiver().await?.into(),
            #[cfg(feature = "amqp")]
            Self::Amqp(config) => config.build_receiver().await?.into(),
            #[cfg(all(unix, feature = "unix-sock"))]
            Self::Unix(config) => config.build_receiver().await?.into(),
            Self::Null(config) => config.build_receiver().into(),
            Self::Import(config) => config.build_receiver().await?,
        };
        Ok(receiver)
    }
}

#[derive(Debug)]
pub enum Sender {
    File(file::Sender),
    #[cfg(feature = "zenoh")]
    Zenoh(zenoh::Sender),
    #[cfg(feature = "amqp")]
    Amqp(amqp::Sender),
    #[cfg(all(unix, feature = "unix-sock"))]
    Unix(unix::Sender),
    Null(null::Sender),
}

impl Sender {
    pub async fn send(&mut self, payload: impl Into<Cow<'_, [u8]>>) -> Result<()> {
        match self {
            Self::File(sender) => sender.send(payload.into().borrow()).await,
            #[cfg(feature = "zenoh")]
            Self::Zenoh(sender) => sender.send(payload.into().borrow()).await,
            #[cfg(feature = "amqp")]
            Self::Amqp(sender) => sender.send(payload.into().borrow()).await,
            #[cfg(all(unix, feature = "unix-sock"))]
            Self::Unix(sender) => sender.send(payload.into().borrow()).await,
            Self::Null(sender) => {
                sender.send(payload.into().borrow());
                Ok(())
            }
        }
    }

    pub fn into_sink(self) -> impl Sink<Vec<u8>, Error = Error> {
        sink::unfold(self, |mut sender, payload| async move {
            sender.send(payload).await.map(|_| sender)
        })
    }

    #[cfg(feature = "protobuf")]
    pub fn into_protobuf_encoded<T: prost::Message>(self) -> crate::protobuf::Sender<T> {
        crate::protobuf::Sender::new(self)
    }
}

#[cfg(feature = "zenoh")]
impl From<zenoh::Sender> for Sender {
    fn from(from: zenoh::Sender) -> Self {
        Self::Zenoh(from)
    }
}

impl From<file::Sender> for Sender {
    fn from(from: file::Sender) -> Self {
        Self::File(from)
    }
}

#[cfg(feature = "amqp")]
impl From<amqp::Sender> for Sender {
    fn from(from: amqp::Sender) -> Self {
        Self::Amqp(from)
    }
}

#[cfg(all(unix, feature = "unix-sock"))]
impl From<unix::Sender> for Sender {
    fn from(from: unix::Sender) -> Self {
        Self::Unix(from)
    }
}

impl From<null::Sender> for Sender {
    fn from(from: null::Sender) -> Self {
        Self::Null(from)
    }
}

#[derive(Debug)]
pub enum Receiver {
    File(file::Receiver),
    #[cfg(feature = "zenoh")]
    Zenoh(Box<zenoh::Receiver>),
    #[cfg(feature = "amqp")]
    Amqp(Box<amqp::Receiver>),
    #[cfg(all(unix, feature = "unix-sock"))]
    Unix(unix::Receiver),
    Null(null::Receiver),
}

impl Receiver {
    pub async fn recv(&mut self) -> Result<Option<Vec<u8>>> {
        match self {
            Self::File(receiver) => receiver.recv().await,
            #[cfg(feature = "zenoh")]
            Self::Zenoh(receiver) => receiver.recv().await,
            #[cfg(feature = "amqp")]
            Self::Amqp(receiver) => receiver.recv().await,
            #[cfg(all(unix, feature = "unix-sock"))]
            Self::Unix(receiver) => receiver.recv().await,
            Self::Null(receiver) => receiver.recv().await,
        }
    }

    pub fn into_stream(self) -> impl Stream<Item = Result<Vec<u8>>> {
        stream::try_unfold(self, |mut rx| async move {
            let item = rx.recv().await?;
            anyhow::Ok(item.map(|item| (item, rx)))
        })
    }

    #[cfg(feature = "protobuf")]
    pub fn into_protobuf_decoded<T: prost::Message + Default>(
        self,
    ) -> crate::protobuf::Receiver<T> {
        crate::protobuf::Receiver::new(self)
    }
}

impl From<file::Receiver> for Receiver {
    fn from(from: file::Receiver) -> Self {
        Self::File(from)
    }
}

#[cfg(feature = "zenoh")]
impl From<zenoh::Receiver> for Receiver {
    fn from(from: zenoh::Receiver) -> Self {
        Self::Zenoh(Box::new(from))
    }
}

#[cfg(feature = "amqp")]
impl From<amqp::Receiver> for Receiver {
    fn from(from: amqp::Receiver) -> Self {
        Self::Amqp(Box::new(from))
    }
}

#[cfg(all(unix, feature = "unix-sock"))]
impl From<unix::Receiver> for Receiver {
    fn from(from: unix::Receiver) -> Self {
        Self::Unix(from)
    }
}

impl From<null::Receiver> for Receiver {
    fn from(from: null::Receiver) -> Self {
        Self::Null(from)
    }
}
