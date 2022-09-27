use std::net::{SocketAddr, ToSocketAddrs};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug)]
pub struct ClientConfig {
    pub addr: SocketAddr,
}

#[derive(Debug)]
pub struct Client {
    stream: TcpStream,
}

impl Client {
    pub async fn new(config: ClientConfig) -> Result<Self> {
        let stream = TcpStream::connect(&config.addr).await?;
        Ok(Client { stream })
    }
}
