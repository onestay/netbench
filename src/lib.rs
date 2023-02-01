#![allow(dead_code)]
#![warn(missing_debug_implementations)]
extern crate core;

mod client;
mod server;
mod tcp_test;
mod test_manager;
mod token_bucket;

pub use crate::client::Client;
pub use crate::server::{ControlMessage, Server};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use thiserror::Error;
use time::Duration;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use once_cell::sync::Lazy;

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

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd, Clone, Copy)]
#[serde(rename_all = "lowercase")]
#[allow(clippy::upper_case_acronyms)]
pub enum Protocol {
    TCP(TCPTestInfo),
    UDP,
    DCCP,
    SCTP,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd, Clone, Copy)]
pub struct TCPTestInfo {
    pub recv_buf_size: u64,
    pub send_buf_size: u64,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::TCP(_) => write!(f, "TCP"),
            Protocol::UDP => write!(f, "UDP"),
            Protocol::DCCP => write!(f, "DCCP"),
            Protocol::SCTP => write!(f, "SCTP"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Role {
    Client,
    Server,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum EndCondition {
    Time(Duration),
    Bytes(u64),
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct NewTestMessage {
    direction: Direction,
    protocol: Protocol,
    bw: u64,
    code: [u8; 32],
    end_condition: EndCondition,
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
    #[error("Parse suffix error")]
    ParseSuffixError(&'static str),
}

#[derive(Debug)]
pub enum SizeFormat {
    Kilo,
    Mega,
    Giga,
    Tera,
    Kibi,
    Mibi,
    Gibi,
    Tebi,
    Auto,
}

impl Default for SizeFormat {
    fn default() -> Self {
        SizeFormat::Auto
    }
}

#[derive(Debug)]
pub struct CommonConfig {
    pub format: SizeFormat,
    pub file: Option<PathBuf>,
}

#[derive(Debug)]
pub struct ClientConfig {
    pub common: CommonConfig,
    pub bw: Option<u64>,
    pub addr: SocketAddr,
    pub proto: Protocol,
}

#[derive(Debug)]
pub struct ServerConfig {
    pub common: CommonConfig,
    pub addr: SocketAddr,
}

static SIZE_MAP: Lazy<HashMap<char, u64>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert('k', 1024);
    m.insert('m', 1024 * 1024);
    m.insert('g', 1024 * 1024 * 1024);
    m.insert('t', 1024 * 1024 * 1024 * 1024);

    m.insert('K', 1000);
    m.insert('M', 1000 * 1000);
    m.insert('G', 1000 * 1000 * 1000);
    m.insert('T', 1000 * 1000 * 1000 * 1000);

    m
});

pub fn parse_u64_from_suffix(s: &str) -> Result<u64, NBError> {
    if !s.is_ascii() {
        return Err(NBError::ParseSuffixError("the input has to be ASCII"));
    }
    let mut output = 0;
    let mut iter = s.chars().rev().peekable();

    let modifier = match iter.peek() {
        Some(c) if c.is_ascii_digit() => 1,
        Some(_) => {
            let c = iter.next().unwrap();
            let modi = SIZE_MAP
                .get(&c)
                .ok_or(NBError::ParseSuffixError("invalid suffix"))?;

            *modi
        }
        None => 1,
    };

    for (i, char) in iter.enumerate() {
        let digit = char.to_digit(10).ok_or(NBError::ParseSuffixError(
            "a char that is not a modifier is not a digit",
        ))? as u64;
        output += digit * (10_u64.pow(i.try_into().unwrap()));
    }

    output *= modifier;

    Ok(output)
}

#[cfg(test)]
mod test {
    use crate::parse_u64_from_suffix;

    #[test]
    fn test_parse_u64_from_suffix() {
        let result = parse_u64_from_suffix("123").unwrap();
        assert_eq!(result, 123);

        let result = parse_u64_from_suffix("1k").unwrap();
        assert_eq!(result, 1024);

        let result = parse_u64_from_suffix("1K").unwrap();
        assert_eq!(result, 1000);

        let result = parse_u64_from_suffix("15t").unwrap();
        assert_eq!(result, 16492674416640);

        let result = parse_u64_from_suffix("1Z");
        assert!(result.is_err());

        let result = parse_u64_from_suffix("17a6k");
        println!("{result:?}");
        assert!(result.is_err());
    }
}
