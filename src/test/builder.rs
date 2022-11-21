use std::net::{SocketAddr, ToSocketAddrs};
use anyhow::Result;
use time::Duration;

use crate::{Role, Direction, Protocol};
use super::{IPPref, test::{Test, TestSetup}};

#[derive(Debug)]
pub struct TestBuilder {
    direction: Direction,
    role: Role,
    protocol: crate::Protocol,
    addresses: Vec<SocketAddr>,
    duration: Duration,
    intervals: Duration,
    ip_pref: Option<IPPref>
}

const DEFAULT_TEST_DURATION: Duration = Duration::seconds(10);
const DEFAULT_TEST_INTERVAL: Duration = Duration::seconds(1);

impl TestBuilder {
    pub fn client<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let addresses = addr.to_socket_addrs()?.collect::<Vec<SocketAddr>>();
        if addresses.is_empty() {
            panic!("Couldn't resolve addr to a SocketAddr");
        }

        Ok(TestBuilder {
            direction: Direction::ClientToServer,
            role: Role::Client,
            protocol: Protocol::TCP,
            addresses,
            duration: DEFAULT_TEST_DURATION,
            intervals: DEFAULT_TEST_INTERVAL,
            ip_pref: None
        })
    }

    pub fn set_direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    pub fn set_protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn ipv4_only(mut self) -> Self {
        self.ip_pref = Some(IPPref::V4);
        self
    }

    pub fn ipv6_only(mut self) -> Self {
        self.ip_pref = Some(IPPref::V6);
        self
    }

    pub fn set_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub async fn build(self) -> Result<Test> {
        let (v4, v6): (Vec<SocketAddr>, Vec<SocketAddr>) = self.addresses.into_iter()
                        .partition(|addr| addr.is_ipv4());
                        
        assert!(!v4.is_empty() || !v6.is_empty());

        let addr = match self.ip_pref {
            Some(IPPref::V4) => todo!(),
            Some(IPPref::V6) => todo!(),
            None => {
                if !v6.is_empty() {
                    v6[0]
                } else {
                    v4[0]
                }
            },
        };

        let setup = TestSetup {
            addr,
            direction: self.direction,
            duration: self.duration,
            intervals: self.intervals,
            protocol: self.protocol,
            role: self.role,
        };

        Test::new(setup).await
    }

}