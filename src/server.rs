use std::net::{SocketAddr};

use anyhow::Result;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use crate::NBError;
use crate::{MessageType, NewTestMessage};

// package format:
// msg_type(2 bytes) len(4 bytes) data
const CONTROL_MSG_SIZE: usize = 6;
struct ConnectedClient {
    socket: TcpStream,
    addr: SocketAddr,
}

impl ConnectedClient {
    pub(crate) async fn init(socket: TcpStream, addr: SocketAddr) {
        println!("Handling new client from {:?}", addr);
        ConnectedClient::msg_loop(socket).await;
    }

    async fn msg_loop(mut socket: TcpStream) {
        // read control information:
        let (msg_type, len) = ConnectedClient::read_control_info(&mut socket)
            .await
            .unwrap();
        match msg_type {
            MessageType::NewTest => {
                let mut buf: Vec<u8> = Vec::with_capacity(len as usize);
                crate::read_n(&mut socket, &mut buf, len as usize).await.unwrap();
                let message: NewTestMessage = serde_json::from_slice(buf.as_slice()).unwrap();
                println!("{:?}", message);
            }
            _ => {
                todo!()
            }
        }
    }

    async fn read_control_info(socket: &mut TcpStream) -> Result<(MessageType, u32)> {
        let mut buf: Vec<u8> = Vec::with_capacity(CONTROL_MSG_SIZE);
        crate::read_n(socket, &mut buf, CONTROL_MSG_SIZE).await?;

        let msg_type = u16::from_be_bytes(buf[..2].try_into()?);
        let msg_type = MessageType::try_from(msg_type)?;
        let len = u32::from_be_bytes(buf[2..6].try_into()?);

        Ok((msg_type, len))
    }
}

#[derive(Debug)]
pub struct ServerConfig {
    pub addr: SocketAddr
}

#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
    com_rx: mpsc::Receiver<ControlMessage>
}

impl Server {
    pub async fn new(config: ServerConfig) -> Result<(Self, mpsc::Sender<ControlMessage>)> {
        let (com_tx, com_rx) = mpsc::channel(10);
        let listener = TcpListener::bind(&config.addr).await?;
        log::info!("Server listening on {}", config.addr);
        
        Ok((Server {
            listener,
            com_rx
        }, com_tx))
    }

    pub async fn accept(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                Ok((socket, addr)) = self.listener.accept() => {
                    tokio::spawn(async move {
                        println!("New client!");
                        ConnectedClient::init(socket, addr).await
                    });
                }
                msg = self.com_rx.recv() => {
                    // Currently there's only the stop message, that's why this is okay
                    if msg.is_some() {
                        log::info!("Shutting down server...");
                        println!("Shutting down server");
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
    Stop
}