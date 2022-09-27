#![allow(dead_code)]
mod client;
mod server;

pub use crate::client::{Client, ClientConfig};
pub use crate::server::{Server, ServerConfig, ControlMessage};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::net::TcpStream;
use std::io;
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Direction {
    ClientToServer,
    ServerToClient,
    Bidirectional,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    TCP,
    UDP,
    DCCP,
    SCTP,
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
    #[error("Connection reset")]
    ConnectionReset,
    #[error("Invalid message type {0}")]
    InvalidMessageType(u16),
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
            Ok(0) => return Err(NBError::ConnectionReset.into()),
            Ok(val) => {
                to_be_read -= val;
                result.extend_from_slice(&buf[0..val]);
            },
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(e.into())
        }
    }

    Ok(())
}

async fn write_n(socket: &TcpStream, mut data: &mut [u8], n: usize) -> Result<()> {
    let mut to_be_written = n;

    while to_be_written != 0 {
        socket.writable().await?;

        match socket.try_write(data) {
            Ok(0) => return Err(NBError::ConnectionReset.into()),
            Ok(val) => {
                to_be_written -= val;
                data = &mut data[to_be_written..];
            },
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(e.into())
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
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
}
