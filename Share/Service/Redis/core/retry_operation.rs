// retry_operation.rs
use std::time::Duration;
use redis::aio::ConnectionManager;
use tracing::{info, warn, error};
use rand::Rng;

use crate::share::comman::error::{AppError, AppResult};

pub type RedisConnection = ConnectionManager;

#[derive(Debug, Clone)]
pub struct RetryOptions {
    pub retries: u32,       // 추가 재시도 횟수
    pub delay_ms: u64,      // 최초 대기(ms)
    pub backoff_factor: u64, // 지수 배수
    pub jitter_ms: u64,     // 랜덤 지터(ms)
}

pub const RETRY_OPT: RetryOptions = RetryOptions { 
    retries: 2, 
    delay_ms: 50, 
    backoff_factor: 2,
    jitter_ms: 10,
};

/// 재시도 대상 오류인지 확인
fn is_retryable_error(err: &AppError) -> bool {
    match err {
        AppError::Redis { message, .. } => {
            // 네트워크 관련 오류만 재시도
            message.contains("Connection refused") ||
            message.contains("timeout") ||
            message.contains("network") ||
            message.contains("connection") ||
            message.contains("broken pipe") ||
            message.contains("reset by peer")
        }
        AppError::Network { .. } => true,
        AppError::System { .. } => true,
        _ => false, // 비즈니스 로직 오류는 재시도하지 않음
    }
}

/// 지터를 적용한 지연 시간 계산
fn calculate_delay_with_jitter(base_delay: u64, jitter_ms: u64) -> u64 {
    let mut rng = rand::thread_rng();
    let jitter = rng.gen_range(0..=jitter_ms);
    base_delay + jitter
}

pub async fn retry_operation<F, Fut, T>(
    mut op: F,
    mut opt: RetryOptions,
) -> AppResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = AppResult<T>>,
{
    let mut last_error = None;
    
    for attempt in 0..=opt.retries {
        match op().await {
            Ok(result) => {
                if attempt > 0 {
                    info!("재시도 성공 (시도 {}/{})", attempt + 1, opt.retries + 1);
                }
                return Ok(result);
            }
            Err(err) => {
                last_error = Some(err.clone());
                
                // 재시도 대상 오류인지 확인
                if !is_retryable_error(&err) {
                    error!("재시도 불가능한 오류 발생: {:?}", err);
                    return Err(err);
                }
                
                if attempt < opt.retries {
                    let delay_with_jitter = calculate_delay_with_jitter(opt.delay_ms, opt.jitter_ms);
                    warn!(
                        "재시도 대상 오류 발생, {}ms 후 재시도 (시도 {}/{}) - 오류: {:?}",
                        delay_with_jitter, attempt + 1, opt.retries + 1, err
                    );
                    
                    tokio::time::sleep(Duration::from_millis(delay_with_jitter)).await;
                    opt.delay_ms = opt.delay_ms.saturating_mul(opt.backoff_factor.max(1));
                } else {
                    error!("모든 재시도 실패 - 최종 오류: {:?}", err);
                }
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| AppError::business("모든 재시도 실패", Some("RETRY_FAILED"))))
}
