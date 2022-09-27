use std::net::{SocketAddr};

use anyhow::Result;
use crate::{NewTestMessage, Protocol, Direction};

use tokio::net::{TcpStream};

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

    pub async fn start_new_test(&self) -> Result<()> {
        let msg = NewTestMessage{
            bw: 0,
            direction: Direction::ClientToServer,
            protocol: Protocol::TCP
        };
        let mut msg = serde_json::to_vec(&msg)?;
        let mut data = Vec::with_capacity(msg.len() + 6);
        data.extend_from_slice(&0_u16.to_be_bytes());
        data.extend_from_slice(&(msg.len() as u32).to_be_bytes());
        data.append(&mut msg);
        let n = data.len();
        crate::write_n(&self.stream, data.as_mut_slice(), n).await?;
        Ok(())
    }
}
