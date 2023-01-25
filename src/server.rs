use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, trace, warn};

use crate::{
    tcp_test::TCPTest, MessageType, NewTestMessage, Protocol, TestAssociationMessage,
    CONTROL_MSG_SIZE,
};
use crate::{test_manager, Role};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

struct ConnectedClient {
    socket: TcpStream,
    addr: SocketAddr,
    outstanding_tests: Arc<Mutex<Vec<NewTestMessage>>>,
}

impl ConnectedClient {
    pub(crate) async fn init(
        socket: TcpStream,
        addr: SocketAddr,
        outstanding_tests: Arc<Mutex<Vec<NewTestMessage>>>,
    ) {
        debug!("Handling new client from {:?}", addr);
        ConnectedClient {
            socket,
            addr,
            outstanding_tests,
        }
        .msg_loop()
        .await;
    }
    fn get_test_message(
        &mut self,
        test_association_msg: &TestAssociationMessage,
    ) -> Option<NewTestMessage> {
        let mut outstanding_tests = self.outstanding_tests.lock();
        outstanding_tests
            .iter()
            .position(|x| *x == *test_association_msg)
            .map(|index| outstanding_tests.swap_remove(index))
    }

    async fn msg_loop(mut self) {
        // read control information:
        let msg_type = ConnectedClient::read_control_info(&mut self.socket)
            .await
            .unwrap();
        match msg_type {
            MessageType::NewTest(len) => {
                println!("{}\nNew test submitted from {}", SEPARATOR, self.addr);

                trace!("reading NewTestMessage with len {len}");
                let mut buf = vec![0; len];
                self.socket.read_exact(buf.as_mut_slice()).await.unwrap();
                let message: NewTestMessage = serde_json::from_slice(buf.as_slice()).unwrap();
                trace!("read NewTestMessage {message:?}");
                let mut outstanding_tests = self.outstanding_tests.lock();
                outstanding_tests.push(message);
            }

            MessageType::TestAssociation(len) => {
                trace!("reading TestAssociationMessage");
                let mut buf = vec![0; len];
                self.socket.read_exact(buf.as_mut_slice()).await.unwrap();
                let message: TestAssociationMessage =
                    serde_json::from_slice(buf.as_slice()).unwrap();
                trace!("read TestAssociationMessage {message:?}");

                if let Some(test_message) = self.get_test_message(&message) {
                    trace!("found associated TestMessage {test_message:?}");

                    match test_message.protocol {
                        Protocol::TCP => {
                            let test = TCPTest::new(test_message, Role::Server, self.socket);
                            test_manager::run(test).await;
                        }
                        _ => todo!(),
                    }
                } else {
                    warn!("Code of message doesn't exist");
                    self.socket.shutdown().await.expect("shutdown failed");
                }
            }
            _ => {
                todo!()
            }
        }
    }

    async fn read_control_info(socket: &mut TcpStream) -> Result<MessageType> {
        let mut buf = [0; CONTROL_MSG_SIZE];
        socket.read_exact(&mut buf).await?;

        let msg_type = u16::from_be_bytes(buf[..2].try_into()?);
        let len = u32::from_be_bytes(buf[2..6].try_into()?);
        trace!("read control info: msg: {msg_type}, len: {len}");
        let msg_type = MessageType::new(msg_type, len as usize)?;
        Ok(msg_type)
    }
}

#[derive(Debug)]
pub struct ServerConfig {
    pub addr: SocketAddr,
}

#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
    com_rx: mpsc::Receiver<ControlMessage>,
    outstanding_tests: Arc<Mutex<Vec<NewTestMessage>>>,
}

const SEPARATOR: &str = "-----------------------";

impl Server {
    pub async fn new(config: ServerConfig) -> Result<(Self, mpsc::Sender<ControlMessage>)> {
        let (com_tx, com_rx) = mpsc::channel(10);
        let listener = TcpListener::bind(&config.addr).await?;
        debug!("Server listening on {}", config.addr);
        println!("Server ready to accept connections on {}", config.addr);
        Ok((
            Server {
                listener,
                com_rx,
                outstanding_tests: Arc::new(Mutex::new(Vec::new())),
            },
            com_tx,
        ))
    }

    pub async fn accept(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                Ok((socket, addr)) = self.listener.accept() => {
                    let outstanding_tests = self.outstanding_tests.clone();
                    tokio::spawn(async move {
                        debug!("New connection from {addr}");
                        ConnectedClient::init(socket, addr, outstanding_tests).await;
                    });
                }
                msg = self.com_rx.recv() => {
                    // Currently there's only the stop message, that's why this is okay
                    if msg.is_some() {
                        debug!("Shutting down server...");
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum ControlMessage {
    Stop,
}
