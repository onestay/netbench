#![allow(dead_code)]
#![warn(missing_debug_implementations)]
extern crate core;

mod client;
mod server;
mod tcp_test;
mod test_manager;
mod token_bucket;

pub use crate::client::{Client, ClientConfig};
pub use crate::server::{ControlMessage, Server, ServerConfig};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::Duration;
use tokio::io::{AsyncWrite, AsyncWriteExt};

pub(crate) const CONTROL_MSG_SIZE: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum MessageType {
    NewTest(usize),
    CancelTest(usize),
    MsgError(usize),
    TestAssociation(usize),
    Close(usize),
}

pub(crate) struct MessageID;

impl MessageID {
    pub(crate) const NEW_TEST_MESSAGE: u16 = 0x0;
    pub(crate) const CANCEL_TEST_MESSAGE: u16 = 0x1;
    pub(crate) const MSG_ERROR_MESSAGE: u16 = 0x2;
    pub(crate) const TEST_ASSOCIATION_MESSAGE: u16 = 0x3;
    pub(crate) const CLOSE_MESSAGE: u16 = 0xFFFF;
}

impl MessageType {
    fn new(id: u16, len: usize) -> Result<Self, NBError> {
        match id {
            MessageID::NEW_TEST_MESSAGE => Ok(MessageType::NewTest(len)),
            MessageID::CANCEL_TEST_MESSAGE => Ok(MessageType::CancelTest(len)),
            MessageID::MSG_ERROR_MESSAGE => Ok(MessageType::MsgError(len)),
            MessageID::TEST_ASSOCIATION_MESSAGE => Ok(MessageType::TestAssociation(len)),
            MessageID::CLOSE_MESSAGE => Ok(MessageType::Close(len)),
            _ => Err(NBError::InvalidMessageType(id)),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    ClientToServer,
    ServerToClient,
    Bidirectional,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd, Clone, Copy)]
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

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct NewTestMessage {
    direction: Direction,
    protocol: Protocol,
    bw: u64,
    code: [u8; 32],
    duration: Duration,
}

impl NewTestMessage {}

impl std::cmp::PartialEq<TestAssociationMessage> for NewTestMessage {
    fn eq(&self, other: &TestAssociationMessage) -> bool {
        self.code == other.code
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestAssociationMessage {
    pub(crate) code: [u8; 32],
}

pub(crate) async fn send_message<T, W>(
    message: T,
    message_id: u16,
    writer: &mut W,
) -> Result<(), NBError>
where
    T: Serialize,
    W: AsyncWrite + Sync + Send + Unpin,
{
    let mut message = serde_json::to_vec(&message).unwrap();
    let mut data = Vec::with_capacity(message.len() + CONTROL_MSG_SIZE);
    data.extend_from_slice(&message_id.to_be_bytes());
    data.extend_from_slice(&(message.len() as u32).to_be_bytes());
    data.append(&mut message);

    writer.write_all(data.as_slice()).await.unwrap();
    Ok(())
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
