use std::net::SocketAddr;

use crate::{Direction, NewTestMessage, Protocol, CONTROL_MSG_SIZE};
use anyhow::Result;

use tokio::{io::AsyncWriteExt, net::TcpStream};

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

    pub async fn start_new_test(&mut self) -> Result<()> {
        let msg = NewTestMessage {
            bw: 0,
            direction: Direction::ClientToServer,
            protocol: Protocol::TCP,
        };
        let mut msg = serde_json::to_vec(&msg)?;
        let mut data = Vec::with_capacity(msg.len() + CONTROL_MSG_SIZE);
        data.extend_from_slice(&0_u16.to_be_bytes());
        data.extend_from_slice(&(msg.len() as u32).to_be_bytes());
        data.append(&mut msg);
        self.stream.write_all(&mut data).await?;
        Ok(())
    }
}
