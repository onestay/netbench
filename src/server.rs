use std::net::SocketAddr;

use anyhow::Result;
use tokio::io::AsyncReadExt;
use tracing::{info, trace};

use crate::{MessageType, NewTestMessage, CONTROL_MSG_SIZE};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

struct ConnectedClient {
    socket: TcpStream,
    addr: SocketAddr,
}

impl ConnectedClient {
    pub(crate) async fn init(socket: TcpStream, addr: SocketAddr) {
        info!("Handling new client from {:?}", addr);
        ConnectedClient::msg_loop(socket).await;
    }

    async fn msg_loop(mut socket: TcpStream) {
        // read control information:
        let msg_type = ConnectedClient::read_control_info(&mut socket)
            .await
            .unwrap();
        match msg_type {
            MessageType::NewTest(len) => {
                let mut buf = vec![0; len];
                socket.read_exact(buf.as_mut_slice()).await.unwrap();
                let message: NewTestMessage = serde_json::from_slice(buf.as_slice()).unwrap();
                trace!("{message:?}");
                
            }
            _ => {
                todo!()
            }
        }
    }
    async fn handle_new_test(message: NewTestMessage) -> Result<()> {
        Ok(())
    }
    async fn read_control_info(socket: &mut TcpStream) -> Result<MessageType> {
        let mut buf = [0; CONTROL_MSG_SIZE];
        socket.read_exact(&mut buf).await?;
        trace!("{buf:?}");

        let msg_type = u16::from_be_bytes(buf[..2].try_into()?);
        let len = u32::from_be_bytes(buf[2..6].try_into()?);

        let msg_type = MessageType::new(msg_type, len as usize)?;
        Ok(msg_type)
    }
}

#[derive(Debug)]
pub struct ServerConfig {
    pub addr: SocketAddr,
}

#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
    com_rx: mpsc::Receiver<ControlMessage>,
}

impl Server {
    pub async fn new(config: ServerConfig) -> Result<(Self, mpsc::Sender<ControlMessage>)> {
        let (com_tx, com_rx) = mpsc::channel(10);
        let listener = TcpListener::bind(&config.addr).await?;
        info!("Server listening on {}", config.addr);

        Ok((Server { listener, com_rx }, com_tx))
    }

    pub async fn accept(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                Ok((socket, addr)) = self.listener.accept() => {
                    tokio::spawn(async move {
                        info!("New connection from {addr}");
                        ConnectedClient::init(socket, addr).await
                    });
                }
                msg = self.com_rx.recv() => {
                    // Currently there's only the stop message, that's why this is okay
                    if msg.is_some() {
                        info!("Shutting down server...");
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum ControlMessage {
    Stop,
}
