use crate::{EndCondition, NewTestMessage};
use owo_colors::OwoColorize;

use std::fmt;
use time::{Duration, OffsetDateTime};
use tokio::{
    sync::{
        mpsc,
        oneshot::{self, error::TryRecvError},
    },
    task::JoinHandle,
};
use tracing::{debug, error, info, warn};

#[derive(Debug)]
pub struct IntervalResult {
    bytes_received: u64,
    bytes_sent: u64,
    mbit_s_sent: f64,
    mbit_s_received: f64,
    start: OffsetDateTime,
    end: OffsetDateTime,
}

impl fmt::Display for IntervalResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "sent: {:.2} Mbit/s\treceived: {:.2} Mbit/s",
            self.mbit_s_sent, self.mbit_s_received
        )
    }
}

impl IntervalResult {
    fn print_for_display(&self, test_start_time: &OffsetDateTime) {
        println!(
            "{:.2}-{:.2} sec\t\t{:.2} Mbit/s\t\t{:.2} Mbit/s",
            (self.start - *test_start_time).as_seconds_f64(),
            (self.end - *test_start_time).as_seconds_f64(),
            self.mbit_s_sent,
            self.mbit_s_received
        )
    }
}

impl Default for IntervalResult {
    fn default() -> Self {
        Self {
            bytes_received: 0,
            bytes_sent: 0,
            mbit_s_received: 0.0,
            mbit_s_sent: 0.0,
            start: OffsetDateTime::now_utc(),
            end: OffsetDateTime::now_utc() + Duration::new(1, 0),
        }
    }
}

impl IntervalResult {
    pub(crate) fn add_bytes_sent(&mut self, sent: usize) {
        let sent: u64 = sent.try_into().unwrap();
        self.bytes_sent += sent;
    }

    pub(crate) fn add_bytes_received(&mut self, received: usize) {
        let received: u64 = received.try_into().unwrap();
        self.bytes_received += received;
    }

    pub(crate) fn prepare_to_send(&mut self) {
        self.mbit_s_received = (self.bytes_received * 8) as f64 * 1e-6;
        self.mbit_s_sent = (self.bytes_sent * 8) as f64 * 1e-6;
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

fn print_header(test_info: &NewTestMessage) {
    println!(
        "Running a {} test for {:?}...",
        test_info.protocol.green(),
        test_info.end_condition.green()
    );
    println!("{}", "Interval\t\tSent\t\tReceived".bold())
}

pub(crate) async fn run<T: Test>(test: T) {
    let test_info = test.test_info();
    print_header(test_info);

    let (send, recv) = tokio::sync::mpsc::channel(5);
    let test_start = OffsetDateTime::now_utc();
    let mut current_interval = test_start;
    let test_duration = match test_info.end_condition {
        EndCondition::Time(duration) => duration,
        _ => panic!("Only EndCondition::Time is currently implemented"),
    };
    const INTERVAL: Duration = Duration::new(1, 0);
    let mut intervals = Vec::new();
    let handle = test.start_test(recv);

    while OffsetDateTime::now_utc() - test_start < test_duration {
        if OffsetDateTime::now_utc() - current_interval > INTERVAL {
            current_interval = OffsetDateTime::now_utc();

            match get_interval_stats(&send).await {
                Ok(Some(res)) => {
                    info!("{res:?}");
                    res.print_for_display(&test_start);
                    intervals.push(res);
                }
                Ok(None) => warn!("didn't get interval results"),
                Err(e) => error!("join error: {e}"),
            }
        }
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
