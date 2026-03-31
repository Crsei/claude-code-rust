#![allow(unused)]
//! Remote session — connects to a cloud container
use anyhow::Result;
use super::{RemoteConfig, RemoteStatus};

pub struct RemoteSession {
    pub config: RemoteConfig,
    pub status: RemoteStatus,
}

impl RemoteSession {
    pub fn new(config: RemoteConfig) -> Self {
        Self { config, status: RemoteStatus::Disconnected }
    }

    pub async fn connect(&mut self) -> Result<()> {
        anyhow::bail!("Remote sessions require network feature")
    }

    pub async fn disconnect(&mut self) {
        self.status = RemoteStatus::Disconnected;
    }

    pub fn is_connected(&self) -> bool {
        self.status == RemoteStatus::Connected
    }
}
