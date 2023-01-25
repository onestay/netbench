use std::mem;

use libc::c_int;
use tokio::net::TcpStream;
use tracing::{debug, error, trace};

use crate::{
    test_manager::{IntervalResult, Test, TestControlMessage},
    NewTestMessage, Role,
};

#[cfg(target_os = "linux")]
#[derive(Debug, Default, PartialEq, PartialOrd)]
pub(crate) struct TcpInfo {
    state: u8,
    ca_state: u8,
    retransmits: u8,
    probes: u8,
    backoff: u8,
    options: u8,
    snd_rcv_wscale: u8,
    delivery_rate_app_limited_fastopen_client_fail: u8,

    rto: u32,
    ato: u32,
    snd_mss: u32,
    rcv_mss: u32,

    unacked: u32,
    sacked: u32,
    lost: u32,
    retrans: u32,
    fackets: u32,

    last_data_sent: u32,
    last_ack_sent: u32,
    last_data_recv: u32,
    last_ack_recv: u32,

    pmtu: u32,
    rcv_ssthresh: u32,
    rtt: u32,
    rttvar: u32,
    snd_ssthresh: u32,
    snd_cwnd: u32,
    advmss: u32,
    reodering: u32,

    rcv_rtt: u32,
    rcv_space: u32,

    total_retrans: u32,

    pacing_rate: u64,
    max_pacing_rate: u64,
    bytes_acked: u64,
    bytes_received: u64,
    segs_out: u32,
    segs_in: u32,

    notsent_bytes: u32,
    min_rtt: u32,
    data_segs_in: u32,
    data_segs_out: u32,

    delivery_rate: u64,

    busy_time: u64,
    rwnd_limited: u64,
    sndbuf_limited: u64,

    delivered: u32,
    delivered_ce: u32,

    bytes_sent: u64,
    bytes_retrans: u64,
    dsack_dups: u32,
    reord_seen: u32,

    rcv_ooopack: u32,

    snd_wnd: u32,
}

pub(crate) struct TCPTest {
    socket: TcpStream,
    test_info: NewTestMessage,
    role: Role,
}

#[cfg(target_os = "linux")]
fn get_tcp_info(sockfd: c_int) -> Option<TcpInfo> {
    let mut tcp_info = TcpInfo::default();
    let mut tcp_info_size = mem::size_of::<TcpInfo>() as libc::socklen_t;
    unsafe {
        let ret = libc::getsockopt(
            sockfd,
            libc::SOL_TCP,
            libc::TCP_INFO,
            &mut tcp_info as *mut _ as *mut libc::c_void,
            &mut tcp_info_size,
        );

        if ret != 0 {
            return None;
        }
    }

    if tcp_info != TcpInfo::default() {
        Some(tcp_info)
    } else {
        None
    }
}

impl Test for TCPTest {
    fn start_test(
        self,
        mut comm_channel: tokio::sync::mpsc::Receiver<crate::test_manager::TestControlMessage>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = IntervalResult::default();
            let mut read_buf = [0; 4096];
            let send_buf = [0xAB; 4096];
            let mut is_done = false;
            let (mut n_send, mut n_read, mut n_chan) = (0, 0, 0);
            loop {
                tokio::select! {
                    _ = self.socket.readable(), if matches!(self.role, Role::Server) => {
                        n_read += 1;
                        match self.socket.try_read(&mut read_buf) {
                            Ok(n) => {
                                if n == 0 {
                                    trace!("read 0");
                                    break;
                                }
                                interval.add_bytes_received(n);
                            },
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                continue;
                            }
                            Err(e) => {
                                error!("{e}");
                                break;
                            }

                        }
                    }
                    _ = self.socket.writable(), if matches!(self.role, Role::Client) => {
                        n_send += 1;
                        //trace!("select write");
                        if is_done {
                            trace!("done from send");
                            break;
                        }
                        match self.socket.try_write(&send_buf) {
                            Ok(n) => {
                                //trace!("sent bytes");
                                interval.add_bytes_sent(n);
                            },
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                trace!("got would block on send");
                                continue;
                            }
                            Err(e) => {
                                error!("{e}");
                                break;
                            }
                        }
                    }

                    msg = comm_channel.recv(), if !is_done => {
                        trace!("chan selected ({msg:?})");
                        n_chan += 1;
                        if let Some(msg) = msg {
                            match msg {
                                TestControlMessage::GetIntervalResult(chan) => {
                                    //info!("{:?}", get_tcp_info(self.socket.as_raw_fd()));
                                    let mut interval_to_send = std::mem::take(&mut interval);
                                    interval_to_send.update_end_time();
                                    if chan.send(interval_to_send).is_err() {
                                        error!("failed to send interval results");
                                    }

                                }

                                TestControlMessage::Done => {
                                    debug!("done");
                                    is_done = true;
                                }
                            }
                        }
                    }

                    else => {
                        trace!("else branch");
                        break;
                    }
                }
            }
            trace!("{n_send} {n_read} {n_chan}")
        })
    }

    fn test_info(&self) -> &NewTestMessage {
        &self.test_info
    }
}

impl TCPTest {
    pub(crate) fn new(msg: NewTestMessage, role: Role, socket: TcpStream) -> Self {
        TCPTest {
            socket,
            test_info: msg,
            role,
        }
    }
}
