use crate::{common::*, generic};
use futures::future::BoxFuture;
use serde_loader::Json5Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Config {
    pub file: Json5Path<generic::Config>,
}

impl Config {
    pub fn build_sender(&self) -> BoxFuture<'static, Result<generic::Sender>> {
        let file = self.file.clone();
        async move { file.build_sender().await }.boxed()
    }

    pub fn build_receiver(&self) -> BoxFuture<'static, Result<generic::Receiver>> {
        let file = self.file.clone();
        async move { file.build_receiver().await }.boxed()
    }
}
