use std::time::Duration;
use tokio::time::sleep;
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

        while attempts > 0 {
            match operation().await {
                Ok(val) => return Ok(val),
                Err(e) => {
                    attempts -= 1;
                    if attempts == 0 {
                        return Err(e);
                    }
                    // 지터 추가
                    let jitter = rand::thread_rng().gen_range(0..self.jitter_ms);
                    let delay = (self.delay_ms as f64 * self.backoff).round() as u64 + jitter;
                    sleep(Duration::from_millis(delay)).await;
                }
            }
        }
        Err(anyhow!("모든 재시도 실패"))
    }
}
