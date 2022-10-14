use crate::{Direction, NBError, Role};
use anyhow::Result;
use async_trait::async_trait;
use std::boxed::Box;
use std::fmt::Debug;
use std::io::{Result as IOResult, ErrorKind, Error as IOError};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use time::{Duration, OffsetDateTime};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::time as t_time;

#[derive(Debug)]
struct UDPTest;

impl UDPTest {
    pub fn new() -> Self {
        UDPTest {}
    }
}


#[async_trait]
impl TestFunction for UDPTest {
    async fn connect(&mut self) -> Result<()> {
        todo!()
    }

    async fn accept(&mut self) -> Result<()> {
        todo!()
    }

    fn write(&self, buf: &[u8], sent: &mut usize) -> Result<()> {
        todo!()
    }

    fn read(&self, mut buf: &mut [u8], sent: &mut usize) -> Result<()> {
        todo!()
    }

    async fn readable(&self) -> IOResult<()> {
        todo!()
    }

    async fn writable(&self) -> IOResult<()> {
        todo!()
    }
}

#[derive(Debug)]
struct TCPTest {
    socket: Option<TcpStream>,
    addr: SocketAddr,
}

impl TCPTest {
    pub async fn new(setup: &TestSetup) -> Self {
        let mut tcp_test = TCPTest {
            socket: None,
            addr: setup.addr,
        };
        if setup.role == Role::Server {
            tcp_test.accept().await.unwrap();
        } else {
            tcp_test.connect().await.unwrap();
        }

        tcp_test
    }
}

#[async_trait]
impl TestFunction for TCPTest {
    async fn connect(&mut self) -> Result<()> {
        println!("Calling connect");
        self.socket = Some(TcpStream::connect(self.addr).await?);
        Ok(())
    }

    async fn accept(&mut self) -> Result<()> {
        println!("Calling bind");
        let listener = TcpListener::bind(self.addr).await?;
        println!("calling accept");
        let (socket, _) = listener.accept().await?;
        self.socket = Some(socket);
        Ok(())
    }

    fn write(&self, buf: &[u8], sent: &mut usize) -> Result<()> {
        //println!("Called send");
        if let Some(socket) = &self.socket {
            let result = socket.try_write(buf)?;
            *sent += result;
        }

        Err(NBError::NotConnected.into())
    }

    fn read(&self, buf: &mut [u8], recv: &mut usize) -> Result<()> {
        //println!("Called recv");
        if let Some(socket) = &self.socket {
            let result = socket.try_read(buf)?;
            *recv += result;
        }

        Err(NBError::NotConnected.into())
    }

    async fn readable(&self) -> IOResult<()> {
        self.socket.as_ref().unwrap().readable().await
    }

    async fn writable(&self) -> IOResult<()> {
        self.socket.as_ref().unwrap().writable().await
    }
}


#[async_trait]
trait TestFunction: Debug + Send {
    async fn connect(&mut self) -> Result<()>;
    async fn accept(&mut self) -> Result<()>;
    fn write(&self, buf: &[u8], sent: &mut usize) -> Result<()>;
    fn read(&self, buf: &mut [u8], recv: &mut usize) -> Result<()>;
    async fn readable(&self) -> IOResult<()>;
    async fn writable(&self) -> IOResult<()>;
}

#[derive(Debug)]
pub struct TestSetup {
    pub direction: Direction,
    pub role: Role,
    pub protocol: crate::Protocol,
    pub addr: SocketAddr,
    pub duration: Duration,
    pub intervals: Duration,
}

#[derive(Debug)]
pub struct TestInterval {
    bytes_sent: usize,
    bytes_recv: usize,
    bits_per_sec: usize,
    start: Duration,
    end: Duration,
}

#[derive(Debug)]
pub struct TestResult {
    start_time: OffsetDateTime,
    intervals: Vec<TestInterval>,
}

#[derive(Debug)]
pub struct Test {
    funcs: Box<dyn TestFunction>,
    setup: Arc<TestSetup>,
}

impl Test {
    pub async fn new(setup: TestSetup) -> Result<Self> {
        let funcs: Box<dyn TestFunction> = match setup.protocol {
            crate::Protocol::TCP => {
                println!("Creating new TCP test");
                Box::new(TCPTest::new(&setup).await) as Box<dyn TestFunction>
            }
            crate::Protocol::UDP => Box::new(UDPTest::new()),
            _ => {
                todo!()
            }
        };
        Ok(Test {
            funcs,
            setup: Arc::new(setup),
        })
    }

    pub async fn run(mut self) -> Result<TestResult> {
        println!("Starting test");
        let mut results = TestResult {
            start_time: OffsetDateTime::now_utc(),
            intervals: Vec::new(),
        };
        // setup control channels
        let (rx, tx) = mpsc::channel::<(ControlMessage, oneshot::Sender<TestInterval>)>(100);
        let tmp_funcs = self.funcs;
        let setup_clone = self.setup.clone();
        let send_task = tokio::spawn(async move { Self::run_rw(tmp_funcs, setup_clone, tx).await });

        let mut time_this_interval = OffsetDateTime::now_utc();
        let mut remaining = self.setup.duration;

        while !(remaining.is_zero() || remaining.is_negative()) {
            if OffsetDateTime::now_utc() - time_this_interval >= self.setup.intervals {
                let (oneshot_rx, oneshot_tx) = oneshot::channel();
                rx.send((ControlMessage::GetInterval, oneshot_rx)).await?;

                let result = oneshot_tx.await?;
                results.intervals.push(result);

                time_this_interval = OffsetDateTime::now_utc();
                remaining -= self.setup.intervals;
            }
            t_time::sleep(StdDuration::from_millis(100)).await;
        }

        let (oneshot_rx, oneshot_tx) = oneshot::channel();
        rx.send((ControlMessage::StopTest, oneshot_rx)).await?;
        let result = oneshot_tx.await?;

        results.intervals.push(result);
        println!("{:?}", results.intervals);
        
        Ok(results)
    }

    async fn run_rw(
        funcs: Box<dyn TestFunction>,
        setup: Arc<TestSetup>,
        mut tx: mpsc::Receiver<(ControlMessage, oneshot::Sender<TestInterval>)>,
    ) {
        println!("Starting run_rw");
        let should_send = Self::should_send(&*setup);
        let should_receive = Self::should_receive(&*setup);
        let send_buf = vec![0_u8; 1500].into_boxed_slice();
        let mut rcv_buf = vec![0_u8; 128 * 1024].into_boxed_slice();
        let mut sent = 0_usize;
        let mut recv = 0_usize;
        loop {
            tokio::select! {
                res = funcs.writable(), if should_send => {
                    if res.is_ok() {
                        if let Err(e) = funcs.write(&send_buf, &mut sent) {
                            // TODO: make this look a bit nicer when eRFC 2497 is stable
                            if let Some(e) = e.downcast_ref::<IOError>() {
                                if e.kind() == ErrorKind::WouldBlock {
                                    continue;
                                }
                            }
                            log::error!("Error during write: {}", e);
                        }
                    }
                }
                res = funcs.readable(), if should_receive => {
                    if res.is_ok() {
                        if let Err(e) = funcs.read(&mut rcv_buf,  &mut recv) {
                            // TODO: make this look a bit nicer when eRFC 2497 is stable
                            if let Some(e) = e.downcast_ref::<IOError>() {
                                if e.kind() == ErrorKind::WouldBlock {
                                    continue;
                                }
                            }
                            log::error!("Error during write: {}", e);
                        }
                    }
                }
                msg = tx.recv() => {
                    match msg {
                        Some((ControlMessage::GetInterval, ch)) => {
                            let mbps_sent = (sent * 8) as f64 * 1e-6;
                            let mbps_recv = (recv * 8) as f64 * 1e-6;
                            println!("Called GetInterval from {:?}. Sent: {} ({} mbit/s), recv: {} ({} mbit/s).", setup.role, sent, mbps_sent, recv, mbps_recv);
                            let interval = TestInterval {
                                bytes_recv: recv,
                                bytes_sent: sent,
                                bits_per_sec: 0,
                                start: Duration::new(0,0),
                                end: Duration::new(0, 0)
                            };
                            sent = 0;
                            recv = 0;
                            ch.send(interval).unwrap();
                        },
                        Some((ControlMessage::StopTest, ch)) => {
                            println!("Called StopTest from {:?}", setup.role);
                            let mbps_sent = (sent * 8) as f64 * 1e-6;
                            let mbps_recv = (recv * 8) as f64 * 1e-6;
                            println!("Called GetInterval from {:?}. Sent: {} ({} mbit/s), recv: {} ({} mbit/s).", setup.role, sent, mbps_sent, recv, mbps_recv);
                            let interval = TestInterval {
                                bytes_recv: recv,
                                bytes_sent: sent,
                                bits_per_sec: 0,
                                start: Duration::new(0,0),
                                end: Duration::new(0, 0)
                            };
                            sent = 0;
                            recv = 0;
                            ch.send(interval).unwrap();
                            break;
                        },
                        None => {
                            println!("Channel closed from {:?}?", setup.role)
                        }

                    }
                }
            }
        }
    }

    fn should_send(setup: &TestSetup) -> bool {
        setup.direction == Direction::Bidirectional
            || (setup.direction == Direction::ClientToServer && setup.role == Role::Client)
            || (setup.direction == Direction::ServerToClient && setup.role == Role::Server)
    }

    fn should_receive(setup: &TestSetup) -> bool {
        setup.direction == Direction::Bidirectional
            || (setup.direction == Direction::ClientToServer && setup.role == Role::Server)
            || (setup.direction == Direction::ServerToClient && setup.role == Role::Client)
    }
}

#[cfg(test)]
mod test_test {
    use crate::Protocol;

    #[tokio::test]
    async fn test_run() {
        use super::*;
        let setup_server = TestSetup {
            role: Role::Server,
            direction: Direction::ClientToServer,
            protocol: Protocol::TCP,
            duration: Duration::new(10, 0),
            intervals: Duration::new(1, 0),
            addr: "127.0.0.1:2353".parse().unwrap(),
        };

        let setup_client = TestSetup {
            role: Role::Client,
            direction: Direction::ClientToServer,
            protocol: Protocol::TCP,
            duration: Duration::new(10, 0),
            intervals: Duration::new(1, 0),
            addr: "127.0.0.1:2353".parse().unwrap(),
        };

        let result_server = tokio::spawn(async move {
            let test_server = Test::new(setup_server).await.unwrap();

            test_server.run().await.unwrap();
        });

        let result_client = tokio::spawn(async move {
            let test_client = Test::new(setup_client).await.unwrap();

            test_client.run().await.unwrap();
        });

        let (_, _) = tokio::join!(result_server, result_client);
    }
}

#[derive(Debug)]
enum ControlMessage {
    StopTest,
    GetInterval,
}
