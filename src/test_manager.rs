use crate::NewTestMessage;

use std::fmt;
use time::{Duration, OffsetDateTime};
use tokio::{
    sync::{
        mpsc,
        oneshot::{self, error::TryRecvError},
    },
    task::JoinHandle,
};
use tracing::{debug, error, info, log::warn};

#[derive(Debug)]
pub struct IntervalResult {
    bytes_received: usize,
    bytes_sent: usize,
    start: OffsetDateTime,
    end: OffsetDateTime,
}

impl fmt::Display for IntervalResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "---Interval Results---\nMbit/s sent: {}\nMbit/s received: {}",
            (self.bytes_sent * 8) as f64 * 1e-6,
            (self.bytes_sent * 8) as f64 * 1e-6
        )
    }
}

impl Default for IntervalResult {
    fn default() -> Self {
        Self {
            bytes_received: 0,
            bytes_sent: 0,
            start: OffsetDateTime::now_utc(),
            end: OffsetDateTime::now_utc() + Duration::new(1, 0),
        }
    }
}

impl IntervalResult {
    pub(crate) fn add_bytes_sent(&mut self, sent: usize) {
        self.bytes_sent += sent;
    }

    pub(crate) fn add_bytes_received(&mut self, received: usize) {
        self.bytes_received += received;
    }

    pub(crate) fn update_end_time(&mut self) {
        self.end = OffsetDateTime::now_utc();
    }
}

#[derive(Debug)]
pub(crate) enum TestControlMessage {
    Done,
    GetIntervalResult(oneshot::Sender<IntervalResult>),
}

pub(crate) trait Test {
    fn start_test(
        self,
        comm_channel: tokio::sync::mpsc::Receiver<TestControlMessage>,
    ) -> JoinHandle<()>;
    fn test_info(&self) -> &NewTestMessage;
}

pub(crate) async fn run<T: Test>(test: T) {
    let (send, recv) = tokio::sync::mpsc::channel(5);
    let test_start = OffsetDateTime::now_utc();
    let mut current_interval = test_start;
    let test_duration = test.test_info().duration;
    const INTERVAL: Duration = Duration::new(1, 0);

    let handle = test.start_test(recv);
    while OffsetDateTime::now_utc() - test_start < test_duration {
        if OffsetDateTime::now_utc() - current_interval > INTERVAL {
            current_interval = OffsetDateTime::now_utc();

            match get_interval_stats(&send).await {
                Ok(Some(res)) => info!("{res}"),
                Ok(None) => warn!("didn't get interval results"),
                Err(e) => error!("join error: {e}"),
            }
        }
        //tokio::time::sleep(StdDuration::from_millis(100)).await;
    }

    debug!("done");
    if !handle.is_finished() {
        send.send(TestControlMessage::Done).await.unwrap();
    }

    handle.await.unwrap();
}

fn get_interval_stats(
    send: &mpsc::Sender<TestControlMessage>,
) -> JoinHandle<Option<IntervalResult>> {
    let (interval_send, mut interval_recv) = tokio::sync::oneshot::channel();
    send.try_send(TestControlMessage::GetIntervalResult(interval_send))
        .expect("failed to send");

    tokio::spawn(async move {
        loop {
            match interval_recv.try_recv() {
                Ok(res) => {
                    return Some(res);
                }
                Err(TryRecvError::Closed) => {
                    error!("channel closed without value");
                    return None;
                }
                Err(TryRecvError::Empty) => continue,
            }
        }
    })
}
