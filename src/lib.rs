#![allow(dead_code)]
#![warn(missing_debug_implementations)]
extern crate core;

mod client;
mod server;
mod test;
mod token_bucket;

pub use crate::client::{Client, ClientConfig};
pub use crate::server::{ControlMessage, Server, ServerConfig};
pub use crate::test::{Test, TestBuilder, TestSetup};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io;
use std::net::SocketAddr;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
#[derive(Debug)]
pub enum MessageType {
    NewTest,
    CancelTest,
    MsgError,
    Close,
}

impl TryFrom<u16> for MessageType {
    type Error = NBError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x0 => Ok(MessageType::NewTest),
            0x1 => Ok(MessageType::CancelTest),
            0x2 => Ok(MessageType::MsgError),
            0xFFFF => Ok(MessageType::Close),
            _ => Err(NBError::InvalidMessageType(value)),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    ClientToServer,
    ServerToClient,
    Bidirectional,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    TCP,
    UDP,
    DCCP,
    SCTP,
}

#[derive(Debug, PartialEq)]
pub enum Role {
    Client,
    Server,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTestMessage {
    direction: Direction,
    protocol: Protocol,
    bw: u64,
}

#[derive(Debug, Error)]
pub enum NBError {
    #[error("The connection has been closed")]
    ConnectionClosed,
    #[error("Invalid message type {0}")]
    InvalidMessageType(u16),
    #[error("The socket has not yet been connected")]
    NotConnected,
    #[error("The bucket is empty, no tokens available")]
    BucketEmpty,
}

async fn read_n(socket: &TcpStream, result: &mut Vec<u8>, n: usize) -> Result<()> {
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

        match socket.try_read(&mut buf[..read_buf_size]) {
            Ok(0) => return Err(NBError::ConnectionClosed.into()),
            Ok(val) => {
                to_be_read -= val;
                result.extend_from_slice(&buf[0..val]);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

async fn write_n(socket: &TcpStream, mut data: &mut [u8], n: usize) -> Result<()> {
    let mut to_be_written = n;

    while to_be_written != 0 {
        socket.writable().await?;

        match socket.try_write(data) {
            Ok(0) => return Err(NBError::ConnectionClosed.into()),
            Ok(val) => {
                to_be_written -= val;
                data = &mut data[to_be_written..];
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

#[cfg(test)]
mod unit_tests {
    use crate::token_bucket::TokenBucket;

    use super::*;

    #[test]
    fn test_message_serialize() {
        let message = NewTestMessage {
            direction: Direction::ClientToServer,
            protocol: Protocol::TCP,
            bw: 0,
        };
        println!("{}", serde_json::to_string(&message).unwrap());
    }

    #[tokio::test]
    async fn test_read_n_write_n() {
        use tokio::net::{TcpListener, TcpStream};
        let mut data = String::from("Hello World, how are you?").into_bytes();
        let data_clone = data.clone();
        let n = data.len();
        // client thread
        let server = TcpListener::bind("127.0.0.1:3572").await.unwrap();
        let client_handle = tokio::spawn(async move {
            let client = TcpStream::connect("127.0.0.1:3572").await.unwrap();
            crate::write_n(&client, &mut data, n).await.unwrap();
        });

        let server_handle = tokio::spawn(async move {
            let (sock, _) = server.accept().await.unwrap();
            let mut result = Vec::new();
            crate::read_n(&sock, &mut result, n).await.unwrap();
            assert_eq!(result, data_clone);
        });
        client_handle.await.unwrap();
        server_handle.await.unwrap();
    }
}
