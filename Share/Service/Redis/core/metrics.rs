// metrics.rs
use metrics::{counter, histogram, gauge};
use std::time::Instant;

/// Redis 작업 메트릭
pub struct RedisMetrics {
    operation: String,
    start_time: Instant,
}

impl RedisMetrics {
    pub fn new(operation: &str) -> Self {
        counter!("redis_operations_total", 1, "operation" => operation.to_string());
        Self {
            operation: operation.to_string(),
            start_time: Instant::now(),
        }
    }

    pub fn success(self) {
        counter!("redis_operations_success_total", 1, "operation" => self.operation);
        histogram!("redis_operation_duration_seconds", self.start_time.elapsed().as_secs_f64(), "operation" => self.operation, "status" => "success");
    }

    pub fn failure(self, error_type: &str) {
        counter!("redis_operations_failure_total", 1, "operation" => self.operation.clone(), "error_type" => error_type.to_string());
        histogram!("redis_operation_duration_seconds", self.start_time.elapsed().as_secs_f64(), "operation" => self.operation, "status" => "failure");
    }

    pub fn retry(operation: &str, attempt: u32) {
        counter!("redis_retries_total", 1, "operation" => operation.to_string(), "attempt" => attempt.to_string());
    }

    pub fn connection_pool_size(size: usize) {
        gauge!("redis_connection_pool_size", size as f64);
    }

    pub fn cache_hit(operation: &str) {
        counter!("redis_cache_hits_total", 1, "operation" => operation.to_string());
    }

    pub fn cache_miss(operation: &str) {
        counter!("redis_cache_misses_total", 1, "operation" => operation.to_string());
    }
}

/// 메트릭 매크로
#[macro_export]
macro_rules! redis_metrics {
    ($operation:expr) => {{
        let metrics = RedisMetrics::new($operation);
        let _guard = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // 성공 시 메트릭 업데이트
            metrics.success();
        }));
        metrics
    }};
} 