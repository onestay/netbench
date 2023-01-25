use std::net::SocketAddr;

use crate::{
    tcp_test::TCPTest, test_manager, Direction, MessageID, NewTestMessage, Protocol, Role,
    TestAssociationMessage,
};
use anyhow::Result;
use std::time::Duration as StdDuration;
use time::Duration;
use tokio::net::TcpStream;

#[derive(Debug)]
pub struct ClientConfig {
    pub addr: SocketAddr,
}

#[derive(Debug)]
pub struct Client {
    stream: TcpStream,
    server_addr: SocketAddr,
}

impl Client {
    pub async fn new(config: ClientConfig) -> Result<Self> {
        let stream = TcpStream::connect(&config.addr).await?;
        Ok(Client {
            stream,
            server_addr: config.addr,
        })
    }

    pub async fn start_new_test(&mut self) -> Result<()> {
        let code = rand::random();
        let new_test_message = NewTestMessage {
            bw: 0,
            direction: Direction::ClientToServer,
            protocol: Protocol::TCP,
            code,
            duration: Duration::new(10, 0),
        };

        //let new_test_message_clone = new_test_message.clone();

        crate::send_message(
            new_test_message,
            MessageID::NEW_TEST_MESSAGE,
            &mut self.stream,
        )
        .await?;
        tokio::time::sleep(StdDuration::from_secs(1)).await;
        let mut test_socket = TcpStream::connect(self.server_addr).await.unwrap();
        let msg = TestAssociationMessage { code };
        crate::send_message(msg, MessageID::TEST_ASSOCIATION_MESSAGE, &mut test_socket)
            .await
            .unwrap();

        let test = TCPTest::new(new_test_message, Role::Client, test_socket);
        test_manager::run(test).await;
        Ok(())
    }
}
