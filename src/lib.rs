#![allow(dead_code)]
#![warn(missing_debug_implementations)]
extern crate core;

mod client;
mod server;
mod token_bucket;

pub use crate::client::{Client, ClientConfig};
pub use crate::server::{ControlMessage, Server, ServerConfig};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(crate) const CONTROL_MSG_SIZE: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum MessageType {
    NewTest(usize),
    CancelTest(usize),
    MsgError(usize),
    Close(usize),
}

impl MessageType {
    fn new(id: u16, len: usize) -> Result<Self, NBError> {
        match id {
            0x0 => Ok(MessageType::NewTest(len)),
            0x1 => Ok(MessageType::CancelTest(len)),
            0x2 => Ok(MessageType::MsgError(len)),
            0xFFFF => Ok(MessageType::Close(len)),
            _ => Err(NBError::InvalidMessageType(id)),
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
