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
use std::net::SocketAddr;
use std::path::PathBuf;
use std::{fmt, ops};
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

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
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

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub enum Base {
    Base2,
    Base10,
}

impl Default for Base {
    fn default() -> Self {
        Base::Base2
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub enum SizePreference {
    Auto,
    K,
    M,
    G,
    T,
}

impl Default for SizePreference {
    fn default() -> Self {
        SizePreference::Auto
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub enum BasePreference {
    Base2,
    Base10,
}

impl Default for BasePreference {
    fn default() -> Self {
        BasePreference::Base2
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
#[allow(non_camel_case_types)]
pub enum Unit {
    B,
    KB,
    MB,
    GB,
    TB,
    KiB,
    MiB,
    GiB,
    TiB,
    b,
    Kb,
    Mb,
    Gb,
    Tb,
    Kib,
    Mib,
    Gib,
    Tib,
}

impl Unit {
    pub const fn value(&self) -> u64 {
        match self {
            Unit::B | Unit::b => 1,
            Unit::KB | Unit::Kb => 1000,
            Unit::MB | Unit::Mb => 1000 * 1000,
            Unit::GB | Unit::Gb => 1000 * 1000 * 1000,
            Unit::TB | Unit::Tb => 1000 * 1000 * 1000 * 1000,
            Unit::KiB | Unit::Kib => 1024,
            Unit::MiB | Unit::Mib => 1024 * 1024,
            Unit::GiB | Unit::Gib => 1024 * 1024 * 1024,
            Unit::TiB | Unit::Tib => 1024 * 1024 * 1024 * 1024,
        }
    }

    pub const fn by_index_byte(base_preference: BasePreference, index: usize) -> Option<Self> {
        match index {
            0 if matches!(base_preference, BasePreference::Base2) => Some(Unit::B),
            0 if matches!(base_preference, BasePreference::Base10) => Some(Unit::B),
            1 if matches!(base_preference, BasePreference::Base2) => Some(Unit::KiB),
            1 if matches!(base_preference, BasePreference::Base10) => Some(Unit::KB),
            2 if matches!(base_preference, BasePreference::Base2) => Some(Unit::MiB),
            2 if matches!(base_preference, BasePreference::Base10) => Some(Unit::MB),
            3 if matches!(base_preference, BasePreference::Base2) => Some(Unit::GiB),
            3 if matches!(base_preference, BasePreference::Base10) => Some(Unit::GB),
            _ if matches!(base_preference, BasePreference::Base2) => Some(Unit::TiB),
            _ if matches!(base_preference, BasePreference::Base10) => Some(Unit::TB),
            _ => None,
        }
    }

    pub const fn by_index_bit(base_preference: BasePreference, index: usize) -> Option<Self> {
        match index {
            0 if matches!(base_preference, BasePreference::Base2) => Some(Unit::b),
            0 if matches!(base_preference, BasePreference::Base10) => Some(Unit::b),
            1 if matches!(base_preference, BasePreference::Base2) => Some(Unit::Kib),
            1 if matches!(base_preference, BasePreference::Base10) => Some(Unit::Kb),
            2 if matches!(base_preference, BasePreference::Base2) => Some(Unit::Mib),
            2 if matches!(base_preference, BasePreference::Base10) => Some(Unit::Mb),
            3 if matches!(base_preference, BasePreference::Base2) => Some(Unit::Gib),
            3 if matches!(base_preference, BasePreference::Base10) => Some(Unit::Gb),
            _ if matches!(base_preference, BasePreference::Base2) => Some(Unit::Tib),
            _ if matches!(base_preference, BasePreference::Base10) => Some(Unit::Tb),
            _ => None,
        }
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let out = match self {
            Unit::B => "B",
            Unit::KB => "KB",
            Unit::MB => "MB",
            Unit::GB => "GB",
            Unit::TB => "TB",
            Unit::KiB => "KiB",
            Unit::MiB => "MiB",
            Unit::GiB => "GiB",
            Unit::TiB => "TiB",
            Unit::b => "b",
            Unit::Kb => "Kb",
            Unit::Mb => "Mb",
            Unit::Gb => "Gb",
            Unit::Tb => "Tb",
            Unit::Kib => "Kib",
            Unit::Mib => "Mib",
            Unit::Gib => "Gib",
            Unit::Tib => "Tib",
        };

        write!(f, "{out}")
    }
}

fn u64_to_f64_checked(n: u64) -> Result<f64, NBError> {
    let res = n as f64;
    if res as u64 != n {
        return Err(NBError::ConversionError(n, "f64"));
    }

    Ok(res)
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy, Default)]
pub(crate) struct NBytes {
    n: u64,
    size_preference: SizePreference,
    base_preference: BasePreference,
}

pub(crate) struct NBytesDisplay {
    n: f64,
    unit: Unit,
}

impl fmt::Display for NBytesDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3} {}", self.n, self.unit)
    }
}

impl NBytes {
    fn format_as_bytes(&self) -> NBytesDisplay {
        let n = u64_to_f64_checked(self.n).unwrap();
        NBytes::format(n, self.base_preference, self.size_preference, false)
    }

    fn format_as_bits(&self) -> NBytesDisplay {
        let n = u64_to_f64_checked(self.n.checked_mul(8).unwrap()).unwrap();
        NBytes::format(n, self.base_preference, self.size_preference, true)
    }

    fn format(
        n: f64,
        base_preference: BasePreference,
        size_preference: SizePreference,
        as_bits: bool,
    ) -> NBytesDisplay {
        let bytes = n.abs();
        if !bytes.is_normal() {
            return NBytesDisplay {
                unit: Unit::B,
                n: 0.0,
            };
        }

        let place = match size_preference {
            SizePreference::Auto => bytes.log(1024.0).floor() as i32,
            SizePreference::K => 1,
            SizePreference::M => 2,
            SizePreference::G => 3,
            SizePreference::T => 4,
        };

        let n = match base_preference {
            BasePreference::Base2 => bytes / 1024_f64.powi(place),
            BasePreference::Base10 => bytes / 1000_f64.powi(place),
        };
        let unit = if as_bits {
            Unit::by_index_bit(base_preference, place.try_into().unwrap()).unwrap()
        } else {
            Unit::by_index_byte(base_preference, place.try_into().unwrap()).unwrap()
        };

        NBytesDisplay { n, unit }
    }
}

impl ops::Add<u64> for NBytes {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        NBytes {
            n: self.n + rhs,
            ..self
        }
    }
}

impl ops::AddAssign<u64> for NBytes {
    fn add_assign(&mut self, rhs: u64) {
        self.n += rhs;
    }
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
    #[error("Conversion error: failed to parse {0} as a {1}")]
    ConversionError(u64, &'static str),
}

#[derive(Debug)]
pub struct CommonConfig {
    pub format: SizePreference,
    pub base: BasePreference,
    pub file: Option<PathBuf>,
}

#[derive(Debug)]
pub struct ClientConfig {
    pub common: CommonConfig,
    pub bw: Option<u64>,
    pub addr: SocketAddr,
    pub proto: Protocol,
    pub direction: Direction,
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

pub fn parse_u64_with_suffix(s: &str) -> Result<u64, NBError> {
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

pub(crate) fn should_send(direction: Direction, role: Role) -> bool {
    match direction {
        Direction::ClientToServer if role == Role::Client => true,
        Direction::ServerToClient if role == Role::Server => true,
        Direction::Bidirectional => true,
        _ => false,
    }
}

pub(crate) fn should_recv(direction: Direction, role: Role) -> bool {
    match direction {
        Direction::Bidirectional => true,
        _ => !should_send(direction, role),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::parse_u64_with_suffix;
    #[test]
    fn test_parse_u64_from_suffix() {
        let result = parse_u64_with_suffix("123").unwrap();
        assert_eq!(result, 123);

        let result = parse_u64_with_suffix("1k").unwrap();
        assert_eq!(result, 1024);

        let result = parse_u64_with_suffix("1K").unwrap();
        assert_eq!(result, 1000);

        let result = parse_u64_with_suffix("15t").unwrap();
        assert_eq!(result, 16492674416640);

        let result = parse_u64_with_suffix("1Z");
        assert!(result.is_err());

        let result = parse_u64_with_suffix("17a6k");
        println!("{result:?}");
        assert!(result.is_err());
    }

    #[test]
    fn test_should_send() {
        let role = Role::Client;
        let direction = Direction::ClientToServer;
        assert!(should_send(direction, role));

        let role = Role::Server;
        let direction = Direction::ClientToServer;
        assert!(!should_send(direction, role));

        let role = Role::Client;
        let direction = Direction::ServerToClient;
        assert!(!should_send(direction, role));

        let role = Role::Server;
        let direction = Direction::ServerToClient;
        assert!(should_send(direction, role));

        let role = Role::Client;
        let direction = Direction::Bidirectional;
        assert!(should_send(direction, role));

        let role = Role::Server;
        let direction = Direction::Bidirectional;
        assert!(should_send(direction, role));
    }

    #[test]
    fn test_should_recv() {
        let role = Role::Client;
        let direction = Direction::ClientToServer;
        assert!(!should_recv(direction, role));

        let role = Role::Server;
        let direction = Direction::ClientToServer;
        assert!(should_recv(direction, role));

        let role = Role::Client;
        let direction = Direction::ServerToClient;
        assert!(should_recv(direction, role));

        let role = Role::Server;
        let direction = Direction::ServerToClient;
        assert!(!should_recv(direction, role));

        let role = Role::Client;
        let direction = Direction::Bidirectional;
        assert!(should_recv(direction, role));

        let role = Role::Server;
        let direction = Direction::Bidirectional;
        assert!(should_recv(direction, role));
    }

    #[test]
    fn test_n_bytes_display() {
        let nbytes = NBytes {
            n: 1024,
            size_preference: SizePreference::K,
            base_preference: BasePreference::Base2,
        };

        assert_eq!(nbytes.format_as_bytes().to_string(), "1.000 KiB");

        let nbytes = NBytes {
            n: 23570212479560,
            size_preference: SizePreference::G,
            base_preference: BasePreference::Base10,
        };

        assert_eq!(nbytes.format_as_bytes().to_string(), "23570.212 GB");

        let nbytes = NBytes {
            n: 6346712,
            size_preference: SizePreference::M,
            base_preference: BasePreference::Base2,
        };

        assert_eq!(nbytes.format_as_bits().to_string(), "48.422 Mib");
    }
}
