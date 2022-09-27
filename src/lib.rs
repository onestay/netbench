#![allow(dead_code)]
mod client;
mod server;

pub use crate::client::{Client, ClientConfig};
pub use crate::server::{Server, ServerConfig};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
