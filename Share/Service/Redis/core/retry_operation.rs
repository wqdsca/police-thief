// common.rs
use std::time::Duration;
use redis::aio::ConnectionManager;
use tracing::{info, warn};

use crate::Share::Comman::error::{AppError, AppResult};

pub type RedisConnection = ConnectionManager;

#[derive(Debug, Clone)]
pub struct RetryOptions {
    pub retries: u32,       // 추가 재시도 횟수
    pub delay_ms: u64,      // 최초 대기(ms)
    pub backoff_factor: u64 // 지수 배수
}

pub const RETRY_OPT: RetryOptions = RetryOptions { retries: 2, delay_ms: 50, backoff_factor: 2 };

pub async fn retry_operation<F, Fut, T>(
    mut op: F,
    mut opt: RetryOptions,
) -> AppResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = AppResult<T>>,
{
    let mut last = None;
    for i in 0..=opt.retries {
        match op().await {
            Ok(v) => {
                if i > 0 { info!("재시도 성공({}회차)", i + 1); }
                return Ok(v);
            }
            Err(e) => {
                last = Some(e);
                if i < opt.retries {
                    warn!("작업 실패, {}ms 후 재시도 (시도 {}/{})", opt.delay_ms, i + 1, opt.retries + 1);
                    tokio::time::sleep(Duration::from_millis(opt.delay_ms)).await;
                    opt.delay_ms = opt.delay_ms.saturating_mul(opt.backoff_factor.max(1));
                }
            }
        }
    }
    Err(last.unwrap_or_else(|| AppError::business("모든 재시도 실패", Some("RETRY_FAILED"))))
}
