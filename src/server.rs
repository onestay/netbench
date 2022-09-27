use std::net::{SocketAddr, ToSocketAddrs};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};

use crate::NBError;
use crate::{MessageType, NewTestMessage};

#[derive(Debug)]
pub struct ServerConfig {}

#[derive(Debug)]
pub struct Server {
    addr: SocketAddr,
}

struct ConnectedClient {
    socket: TcpStream,
    addr: SocketAddr,
}

async fn read_n(socket: &mut TcpStream, result: &mut Vec<u8>, n: usize) -> Result<()> {
    const BUF_SIZE: usize = 1024;
    let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];
    let mut to_be_read = n;
    while to_be_read != 0 {
        let read_buf_size = if to_be_read < BUF_SIZE {
            to_be_read
        } else {
            BUF_SIZE
        };

        socket.readable().await?;

        match socket.try_read(&mut buf[..read_buf_size])? {
            0 => return Err(NBError::ConnectionReset.into()),
            val => {
                to_be_read -= val;
                result.extend_from_slice(&buf[0..val]);
            }
        }
    }

    Ok(())
}

// package format:
// msg_type(2 bytes) len(4 bytes) data
const CONTROL_MSG_SIZE: usize = 6;

impl ConnectedClient {
    pub(crate) async fn init(socket: TcpStream, addr: SocketAddr) {
        println!("Handling new client from {:?}", addr);
        ConnectedClient::read_loop(socket).await;
    }

    async fn read_loop(mut socket: TcpStream) {
        // read control information:
        let (msg_type, len) = ConnectedClient::read_control_info(&mut socket)
            .await
            .unwrap();
        match msg_type {
            MessageType::NewTest => {
                println!("New Test!");
                let mut buf: Vec<u8> = Vec::with_capacity(len as usize);
                read_n(&mut socket, &mut buf, len as usize).await.unwrap();
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
        read_n(socket, &mut buf, CONTROL_MSG_SIZE).await?;

        let msg_type = u16::from_be_bytes(buf[..2].try_into()?);
        let msg_type = MessageType::try_from(msg_type)?;
        let len = u32::from_be_bytes(buf[2..6].try_into()?);

        Ok((msg_type, len))
    }
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        Server {
            addr: "127.0.0.1:8080".parse().unwrap(),
        }
    }

    pub async fn listen(&mut self) -> Result<()> {
        let listen = TcpListener::bind(self.addr).await?;
        loop {
            let (socket, addr) = listen.accept().await?;
            tokio::spawn(async move { ConnectedClient::init(socket, addr).await });
        }
    }
}
