use super::ControlMessage;
use crate::token_bucket::TokenBucket;
use crate::{Direction, Role};
use anyhow::Result;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::boxed::Box;
use std::fmt::Debug;
use std::io::{Error as IOError, ErrorKind, Result as IOResult};
use std::mem;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use time::{Duration, OffsetDateTime};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::time as t_time;
// #[derive(Debug)]
// struct UDPTest;

// impl UDPTest {
//     pub fn new() -> Self {
//         UDPTest {}
//     }
// }

// #[async_trait]
// impl TestFunction for UDPTest {
//     async fn connect(&mut self) -> Result<()> {
//         todo!()
//     }

//     async fn accept(&mut self) -> Result<()> {
//         todo!()
//     }

//     fn write(&self, buf: &[u8], sent: &mut usize) -> Result<()> {
//         todo!()
//     }

//     fn read(&self, mut buf: &mut [u8], sent: &mut usize) -> Result<()> {
//         todo!()
//     }

//     async fn readable(&self) -> IOResult<()> {
//         todo!()
//     }

//     async fn writable(&self) -> IOResult<()> {
//         todo!()
//     }
// }

#[derive(Debug)]
struct TCPTest {
    socket: TcpStream,
    addr: SocketAddr,
    current_stats: Mutex<IntervalStats>,
}

impl TCPTest {
    pub async fn new(setup: &TestSetup) -> Result<Self> {
        let socket = if setup.role == Role::Server {
            TCPTest::accept(&setup.addr).await?
        } else {
            TCPTest::connect(&setup.addr).await?
        };

        Ok(TCPTest {
            socket,
            addr: setup.addr,
            current_stats: Mutex::new(IntervalStats::default()),
        })
    }
}

impl TCPTest {
    async fn connect(addr: &SocketAddr) -> Result<TcpStream> {
        println!("Calling connect");
        let socket = TcpStream::connect(addr).await?;
        Ok(socket)
    }

    async fn accept(addr: &SocketAddr) -> Result<TcpStream> {
        println!("Calling bind");
        let listener = TcpListener::bind(addr).await?;
        println!("calling accept");
        let (socket, _) = listener.accept().await?;
        Ok(socket)
    }
}

#[async_trait]
impl TestFunction for TCPTest {
    fn write(&self, buf: &[u8]) -> Result<()> {
        //println!("Called send");
        let result = self.socket.try_write(buf)?;
        let mut stats = self.current_stats.lock();
        stats.bytes_sent += result;
        Ok(())
    }

    fn read(&self, buf: &mut [u8]) -> Result<()> {
        //println!("Called recv");
        let result = self.socket.try_read(buf)?;
        let mut stats = self.current_stats.lock();
        stats.bytes_recv += result;
        Ok(())
    }

    async fn readable(&self) -> IOResult<()> {
        self.socket.readable().await
    }

    async fn writable(&self) -> IOResult<()> {
        self.socket.writable().await
    }

    fn get_stats(&self) -> IntervalStats {
        let mut stats_m = self.current_stats.lock();
        mem::take(&mut *stats_m)
    }
}

#[async_trait]
trait TestFunction: Debug + Send {
    fn write(&self, buf: &[u8]) -> Result<()>;
    fn read(&self, buf: &mut [u8]) -> Result<()>;
    fn get_stats(&self) -> IntervalStats;
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

#[derive(Debug, Default)]
pub struct IntervalStats {
    bytes_sent: usize,
    bytes_recv: usize,
    bits_per_sec: usize,
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
                Box::new(TCPTest::new(&setup).await?) as Box<dyn TestFunction>
            }
            //crate::Protocol::UDP => Box::new(UDPTest::new()),
            _ => {
                todo!()
            }
        };
        Ok(Test {
            funcs,
            setup: Arc::new(setup),
        })
    }

    pub async fn run(self) -> Result<TestResult> {
        println!("Starting test");
        let mut results = TestResult {
            start_time: OffsetDateTime::now_utc(),
            intervals: Vec::with_capacity(
                ((self.setup.duration / self.setup.intervals) + 1.0) as usize,
            ),
        };
        // setup control channels
        let (rx, tx) = mpsc::channel::<(ControlMessage, oneshot::Sender<TestInterval>)>(100);
        let funcs_clone = self.funcs;
        let setup_clone = self.setup.clone();
        let send_task =
            tokio::spawn(async move { Self::run_rw(funcs_clone, setup_clone, tx).await });

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
        send_task.await?;
        Ok(results)
    }

    async fn run_rw(
        funcs: Box<dyn TestFunction>,
        setup: Arc<TestSetup>,
        mut tx: mpsc::Receiver<(ControlMessage, oneshot::Sender<TestInterval>)>,
    ) {
        println!("Starting run_rw");
        let should_send = Self::should_send(&setup);
        let should_receive = Self::should_receive(&setup);
        let send_buf = vec![0_u8; 128 * 1024].into_boxed_slice();
        let mut rcv_buf = vec![0_u8; 128 * 1024].into_boxed_slice();
        let mut bucket = TokenBucket::new(1 * 1024 * 1024 * 1024, 1 * 1024 * 1024 * 1024);

        loop {
            tokio::select! {
                res = funcs.writable(), if should_send && bucket.try_consume(1500).is_ok() => {
                    if res.is_ok() {
                        if let Err(e) = funcs.write(&send_buf) {
                            // TODO: make this look a bit nicer when eRFC 2497 is stable
                            if let Some(e) = e.downcast_ref::<IOError>() {
                                if e.kind() == ErrorKind::WouldBlock {
                                    log::debug!("WouldBlock Error in write");
                                    continue;
                                }
                            }
                            log::error!("Error during write: {}", e);
                        }
                    }
                }
                res = funcs.readable(), if should_receive => {
                    if res.is_ok() {
                        if let Err(e) = funcs.read(&mut rcv_buf) {
                            // TODO: make this look a bit nicer when eRFC 2497 is stable
                            if let Some(e) = e.downcast_ref::<IOError>() {
                                if e.kind() == ErrorKind::WouldBlock {
                                    log::debug!("WouldBlock Error in read");
                                    continue;
                                }
                            }
                            log::error!("Error during write: {}", e);
                        }
                    }
                }
                msg = tx.recv() => {
                    let stats = funcs.get_stats();
                    match msg {
                        Some((ControlMessage::GetInterval, ch)) => {
                            let mbps_sent = stats.bytes_sent as f64 * 1e-6;
                            let mbps_recv = stats.bytes_recv as f64 * 1e-6;
                            println!("Called GetInterval from {:?}. Sent: {} ({} mbit/s), recv: {} ({} mbit/s).", setup.role, stats.bytes_sent, mbps_sent, stats.bytes_recv, mbps_recv);
                            let interval = TestInterval {
                                bytes_recv: stats.bytes_recv,
                                bytes_sent: stats.bytes_sent,
                                bits_per_sec: 0,
                                start: Duration::new(0,0),
                                end: Duration::new(0, 0)
                            };
                            ch.send(interval).unwrap();
                        },
                        Some((ControlMessage::StopTest, ch)) => {
                            println!("Called StopTest from {:?}", setup.role);
                            let mbps_sent = stats.bytes_sent as f64 * 1e-6;
                            let mbps_recv = stats.bytes_recv as f64 * 1e-6;
                            println!("Called GetInterval from {:?}. Sent: {} ({} mbit/s), recv: {} ({} mbit/s).", setup.role, stats.bytes_sent, mbps_sent, stats.bytes_recv, mbps_recv);
                            let interval = TestInterval {
                                bytes_recv: stats.bytes_recv,
                                bytes_sent: stats.bytes_sent,
                                bits_per_sec: 0,
                                start: Duration::new(0,0),
                                end: Duration::new(0, 0)
                            };
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
