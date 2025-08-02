use std::time::Duration;
use tokio::time::{sleep, timeout};
use anyhow::{anyhow, Result};
use rand::Rng;
use std::future::Future;

pub struct RetryOperation {
    pub retries: u8,
    pub delay_ms: u64,
    pub backoff: f64,
    pub jitter_ms: u64,
}

pub const RETRY_OPT: RetryOperation = RetryOperation {
    retries: 3,
    delay_ms: 100,
    backoff: 2.0,
    jitter_ms: 50,
};

impl RetryOperation {
    pub async fn execute<T, F, Fut>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut attempts = self.retries;
        let mut delay = self.delay_ms;

        while attempts > 0 {
            let op_future = operation();
            match timeout(Duration::from_millis(delay), op_future).await {
                Ok(Ok(val)) => return Ok(val),
                _ => {
                    attempts -= 1;
                    if attempts == 0 {
                        break;
                    }
                    // 지터 추가
                    let jitter = rand::thread_rng().gen_range(0..self.jitter_ms);
                    delay = (delay as f64 * self.backoff).round() as u64 + jitter;
                    sleep(Duration::from_millis(delay)).await;
                }
            }
        }
        Err(anyhow!("모든 재시도 실패"))
    }
}
