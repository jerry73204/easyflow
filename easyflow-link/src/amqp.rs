#![cfg(feature = "amqp")]

use crate::common::*;
use amq_protocol_types::{LongString, ShortString};
use anyhow::ensure;
use lapin::{
    options::{BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions},
    types::{AMQPValue, FieldTable},
    BasicProperties, Channel, Connection, Consumer, ExchangeKind, Queue,
};
use log::{info, warn};
use std::{borrow::Cow, collections::BTreeMap, env, env::VarError, sync::Arc, time::Instant};

static ENV_ADDRESS: &str = "AMQP_ADDRESS";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Config {
    pub address: String,
    pub exchange: String,
    pub queue: Option<String>,
    pub max_length: Option<usize>,
    pub message_ttl_millis: Option<usize>,
    pub reliable: bool,
    #[serde(default = "default_force")]
    pub force: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Overflow {
    DropHead,
    RejectPublish,
}

impl Config {
    pub async fn build_sender(&self) -> Result<Sender> {
        let Self {
            ref address,
            ref exchange,
            message_ttl_millis,
            reliable,
            force,
            ..
        } = *self;

        let channel = {
            let conn = Connection::connect(address, Default::default()).await?;
            let channel = conn.create_channel().await?;
            channel.basic_qos(1, Default::default()).await?;
            if reliable {
                channel.confirm_select(Default::default()).await?;
            }
            channel
        };

        // create exchange
        declare_exchange(&channel, exchange, force).await?;

        Ok(Sender {
            message_ttl_millis,
            channel,
            exchange: Arc::new(exchange.to_owned()),
            reliable,
        })
    }

    pub async fn build_receiver(&self) -> Result<Receiver> {
        let headers: Vec<(&str, &str)> = vec![];
        self.build_receiver_ext(headers).await
    }

    pub async fn build_receiver_ext<K, V>(
        &self,
        headers: impl IntoIterator<Item = (K, V)>,
    ) -> Result<Receiver>
    where
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let Self {
            ref address,
            ref exchange,
            queue: ref queue_name,
            max_length,
            message_ttl_millis,
            reliable,
            force,
        } = *self;

        let address: Cow<'_, str> = match env::var(ENV_ADDRESS) {
            Ok(addr) => {
                info!(
                    r#"The environment variable "{}" overrides the AMQP address"#,
                    ENV_ADDRESS
                );
                addr.into()
            }
            Err(VarError::NotPresent) => address.into(),
            Err(VarError::NotUnicode(_)) => {
                warn!(
                    r#"The environment variable "{}" is set but not Unicode"#,
                    ENV_ADDRESS
                );
                address.into()
            }
        };

        let overflow = if reliable {
            Overflow::RejectPublish
        } else {
            Overflow::DropHead
        };

        if let Some(queue_name) = queue_name {
            ensure!(
                !queue_name.is_empty(),
                "queue name must not be empty if specified"
            );
        }

        let channel = {
            let conn = Connection::connect(&address, Default::default()).await?;
            let channel = conn.create_channel().await?;
            if reliable {
                channel.confirm_select(Default::default()).await?;
            }
            channel
        };

        // create exchange
        declare_exchange(&channel, exchange, force).await?;

        // create queue
        let queue = declare_queue(
            &channel,
            queue_name.as_ref().map(AsRef::as_ref),
            overflow,
            max_length,
            message_ttl_millis,
        )
        .await?;
        let queue_name = queue.name().as_str();

        // create binding
        {
            let arguments = {
                let mut args = BTreeMap::new();

                let iter = headers.into_iter().map(|(key, value)| {
                    let key = key.as_ref();
                    let value = value.as_ref();
                    (key.into(), AMQPValue::LongString(value.into()))
                });
                args.extend(iter);

                args.into()
            };
            channel
                .queue_bind(
                    queue_name,
                    exchange,
                    "",
                    QueueBindOptions::default(),
                    arguments,
                )
                .await?;
        }

        channel.basic_qos(1, Default::default()).await?;

        let consumer = channel
            .basic_consume(queue_name, "", Default::default(), BTreeMap::new().into())
            .await?;

        Ok(Receiver {
            queue,
            consumer,
            channel,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Sender {
    message_ttl_millis: Option<usize>,
    channel: Channel,
    exchange: Arc<String>,
    reliable: bool,
}

impl Sender {
    pub async fn send(&self, payload: &[u8]) -> Result<()> {
        let headers: Vec<(&str, &str)> = vec![];
        self.send_ext(headers, payload).await
    }

    pub async fn send_ext<K, V>(
        &self,
        headers: impl IntoIterator<Item = (K, V)>,
        payload: &[u8],
    ) -> Result<()>
    where
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let Self {
            ref channel,
            ref exchange,
            message_ttl_millis,
            reliable,
            ..
        } = *self;

        let headers: FieldTable = {
            let headers: BTreeMap<ShortString, AMQPValue> = headers
                .into_iter()
                .map(|(key, value)| {
                    let key = key.as_ref();
                    let value = value.as_ref();
                    (key.into(), LongString::from(value).into())
                })
                .collect();
            headers.into()
        };

        let properties = BasicProperties::default().with_headers(headers);
        let properties = match message_ttl_millis {
            Some(ttl) => properties.with_expiration(ttl.to_string().into()),
            None => properties,
        };

        if reliable {
            let duration = Duration::from_millis(10);

            loop {
                let start = Instant::now();

                let confirm = channel
                    .basic_publish(
                        exchange,
                        "",
                        BasicPublishOptions {
                            mandatory: false,
                            immediate: false,
                        },
                        payload,
                        properties.clone(),
                    )
                    .await?
                    .await?;

                if confirm.is_ack() {
                    break;
                }

                if let Some(remaining) = duration.checked_sub(start.elapsed()) {
                    async_std::task::sleep(remaining).await;
                }
            }
        } else {
            channel
                .basic_publish(
                    exchange,
                    "",
                    BasicPublishOptions {
                        mandatory: false,
                        immediate: false,
                    },
                    payload,
                    properties,
                )
                .await?;
        }

        Ok(())
    }

    pub fn into_sink(self) -> impl Sink<Vec<u8>, Error = Error> {
        sink::unfold(self, |sender, payload: Vec<u8>| async move {
            sender.send(&payload).await.map(|_| sender)
        })
    }
}

#[derive(Debug, Clone)]
pub struct Receiver {
    queue: Queue,
    consumer: Consumer,
    channel: Channel,
}

impl Receiver {
    pub async fn recv(&mut self) -> Result<Option<Vec<u8>>> {
        let Self {
            consumer, channel, ..
        } = self;

        let delivery = match consumer.next().await.transpose()? {
            Some(msg) => msg,
            None => return Ok(None),
        };

        channel
            .basic_ack(delivery.delivery_tag, Default::default())
            .await?;

        Ok(Some(delivery.data))
    }

    pub fn queue_name(&self) -> &str {
        self.queue.name().as_str()
    }

    pub fn into_stream(self) -> impl Stream<Item = Result<Vec<u8>>> {
        stream::try_unfold(self, |mut rx| async move {
            let item = rx.recv().await?;
            anyhow::Ok(item.map(|item| (item, rx)))
        })
    }
}

async fn declare_exchange(channel: &Channel, name: &str, force: bool) -> Result<()> {
    if force {
        channel.exchange_delete(name, Default::default()).await?;
    }
    channel
        .exchange_declare(
            name,
            ExchangeKind::Headers,
            ExchangeDeclareOptions {
                passive: false,
                durable: false,
                auto_delete: false,
                internal: false,
                nowait: false,
            },
            FieldTable::default(),
        )
        .await?;
    Ok(())
}

async fn declare_queue(
    channel: &Channel,
    name: Option<&str>,
    overflow: Overflow,
    max_length: Option<usize>,
    message_ttl_millis: Option<usize>,
) -> Result<Queue> {
    let arguments = {
        let mut args = BTreeMap::new();

        if let Some(len) = max_length {
            args.insert("x-max-length".into(), AMQPValue::LongUInt(len as u32));
        }

        if let Some(ttl) = message_ttl_millis {
            args.insert("x-message-ttl".into(), AMQPValue::LongUInt(ttl as u32));
        }

        let overflow_value = match overflow {
            Overflow::DropHead => "drop-head",
            Overflow::RejectPublish => "reject-publish",
        };
        args.insert(
            "x-overflow".into(),
            AMQPValue::LongString(overflow_value.into()),
        );

        args.into()
    };

    let queue = channel
        .queue_declare(
            name.as_ref().map(AsRef::as_ref).unwrap_or(""),
            QueueDeclareOptions {
                passive: false,
                durable: false,
                exclusive: false,
                auto_delete: true,
                nowait: false,
            },
            arguments,
        )
        .await?;
    Ok(queue)
}

fn default_force() -> bool {
    false
}
