use crate::{
    should_recv, should_send, Direction, EndCondition, NBytes, NBytesDisplay, NewTestMessage, Role,
};

use std::{fmt, io};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream};
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
    bytes_received: NBytes,
    bytes_sent: NBytes,
    start: OffsetDateTime,
    end: OffsetDateTime,
}

impl IntervalResult {
    fn format_bytes_received(&self) -> NBytesDisplay {
        self.bytes_received.format_as_bytes()
    }

    fn format_bytes_send(&self) -> NBytesDisplay {
        self.bytes_sent.format_as_bytes()
    }

    fn format_bits_per_second_received(&self) -> NBytesDisplay {
        self.bytes_received.format_as_bits()
    }

    fn format_bits_per_second_send(&self) -> NBytesDisplay {
        self.bytes_sent.format_as_bits()
    }
}

const SUFFIXES_SI: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

impl IntervalResult {
    fn print_for_display<P: io::Write + termcolor::WriteColor>(
        &self,
        test_start_time: &OffsetDateTime,
        role: Role,
        direction: Direction,
        printer: &mut P,
    ) -> io::Result<()> {
        printer.reset()?;
        // Write the time interval
        write!(
            printer,
            "{:.2}-{:.2}",
            (self.start - *test_start_time).as_seconds_f64(),
            (self.end - *test_start_time).as_seconds_f64(),
        )?;
        let mut print_seperator = false;
        // Write the time interval unit
        printer.set_color(ColorSpec::new().set_bold(true))?;
        write!(printer, " sec")?;
        // Write the separator
        printer.reset()?;
        write!(printer, "{:^3}", "|")?;

        if should_send(direction, role) {
            let bits_sent = self.format_bits_per_second_send();
            write!(printer, "{:<4.2}", bits_sent.n)?;
            printer.set_color(ColorSpec::new().set_bold(true))?;
            write!(printer, " {}/s", bits_sent.unit)?;
            printer.reset()?;
            print_seperator = true;
        }

        if print_seperator {
            write!(printer, "{:^3}", "|")?;
        }

        if should_recv(direction, role) {
            let bits_recv = self.format_bits_per_second_received();
            write!(printer, "{:<4.2}", bits_recv.n)?;
            printer.set_color(ColorSpec::new().set_bold(true))?;
            write!(printer, " {}/s", bits_recv.unit)?;
        }

        writeln!(printer)?;
        printer.flush()?;
        Ok(())
    }
}

impl Default for IntervalResult {
    fn default() -> Self {
        Self {
            bytes_received: NBytes::default(),
            bytes_sent: NBytes::default(),
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
    match test_info.end_condition {
        EndCondition::Bytes(bytes) => {
            println!(
                "Running a {} test for {} bytes...",
                test_info.protocol, bytes
            );
        }
        EndCondition::Time(duration) => {
            println!(
                "Running a {} test for {:.0} seconds...",
                test_info.protocol,
                duration.as_seconds_f64()
            );
        }
    }
}

pub(crate) async fn run<T: Test>(test: T, role: Role) {
    let test_info = test.test_info();
    let direction = test_info.direction;
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
                    let stdout = StandardStream::stdout(ColorChoice::Always);
                    let mut stdout = stdout.lock();
                    res.print_for_display(&test_start, role, direction, &mut stdout)
                        .unwrap();
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
