use crate::NBError;
use std::cmp;
use time::OffsetDateTime;

#[derive(Debug)]
pub struct TokenBucket {
    rate: u64,
    max_token: u64,
    current_token: u64,
    last_refill: OffsetDateTime,
}

impl TokenBucket {
    pub fn new(rate: u64, max_token: u64) -> Self {
        TokenBucket {
            rate,
            max_token,
            current_token: max_token,
            last_refill: OffsetDateTime::now_utc(),
        }
    }

    fn update(&mut self) {
        let current = OffsetDateTime::now_utc();
        let diff = current - self.last_refill;
        let tokens_added = diff.as_seconds_f64() as u64 * self.rate;
        if tokens_added == 0 {
            return;
        }
        println!("diff = {} tokens_added = {}", diff, tokens_added);
        self.current_token = cmp::min(self.current_token + tokens_added, self.max_token);
        self.last_refill = current;
    }

    pub fn try_consume(&mut self, bytes: u64) -> Result<(), NBError> {
        self.update();
        if self.current_token < bytes {
            return Err(NBError::BucketEmpty);
        }

        self.current_token -= bytes;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::TokenBucket;
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    #[test]
    fn test_token_bucket() {
        let mut bucket = TokenBucket::new(50 * 1024, 100 * 1024);
        let start = Instant::now();
        let mut sent_this_sec = 0;
        let mut interval_start = start;
        while Instant::now() - start < Duration::from_secs(10) {
            sleep(Duration::from_millis(1));
            if bucket.try_consume(1500).is_ok() {
                sent_this_sec += 1500;
                if Instant::now() - interval_start >= Duration::from_secs(1) {
                    println!("Sent this sec: {}", sent_this_sec);
                    assert!(sent_this_sec > (50 * 1024) - 1500);
                    assert!(sent_this_sec < (100 * 1024) + 1500);
                    sent_this_sec = 0;
                    interval_start = Instant::now();
                }
            }
        }
    }
}
