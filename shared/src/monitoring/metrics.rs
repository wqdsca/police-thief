//! Prometheus 메트릭 수집 시스템
//!
//! 실제 Prometheus 메트릭을 수집하고 노출하는 모듈입니다.

use lazy_static::lazy_static;
use prometheus::{
    register_gauge, register_histogram, register_histogram_vec, register_int_counter,
    register_int_gauge, Encoder, Gauge, Histogram, HistogramVec, IntCounter, IntGauge, TextEncoder,
};
use std::time::Duration;

lazy_static! {
    // CPU 및 메모리 메트릭
    pub static ref CPU_USAGE: Gauge = register_gauge!(
        "system_cpu_usage_percent",
        "Current CPU usage percentage"
    )
    .expect("Failed to register CPU usage gauge");

    pub static ref MEMORY_USAGE: IntGauge = register_int_gauge!(
        "system_memory_usage_bytes",
        "Current memory usage in bytes"
    )
    .expect("Failed to register memory usage gauge");

    // TCP 서버 메트릭
    pub static ref TCP_CONNECTIONS: IntGauge = register_int_gauge!(
        "tcp_active_connections",
        "Number of active TCP connections"
    )
    .expect("Failed to register TCP connections gauge");

    pub static ref TCP_MESSAGES_TOTAL: IntCounter = register_int_counter!(
        "tcp_messages_total",
        "Total number of TCP messages processed"
    )
    .expect("Failed to register TCP messages counter");

    pub static ref TCP_ERRORS_TOTAL: IntCounter = register_int_counter!(
        "tcp_errors_total",
        "Total number of TCP errors"
    )
    .expect("Failed to register TCP errors counter");

    pub static ref TCP_MESSAGE_LATENCY: HistogramVec = register_histogram_vec!(
        "tcp_message_latency_seconds",
        "TCP message processing latency in seconds",
        &["operation"]
    )
    .expect("Failed to register TCP message latency histogram");

    // RUDP 서버 메트릭
    pub static ref RUDP_CONNECTIONS: IntGauge = register_int_gauge!(
        "rudp_active_connections",
        "Number of active RUDP connections"
    )
    .expect("Failed to register RUDP connections gauge");

    pub static ref RUDP_PACKETS_SENT: IntCounter = register_int_counter!(
        "rudp_packets_sent_total",
        "Total number of RUDP packets sent"
    )
    .expect("Failed to register RUDP packets sent counter");

    pub static ref RUDP_PACKETS_RECEIVED: IntCounter = register_int_counter!(
        "rudp_packets_received_total",
        "Total number of RUDP packets received"
    )
    .expect("Failed to register RUDP packets received counter");

    pub static ref RUDP_PACKET_LOSS_RATE: Gauge = register_gauge!(
        "rudp_packet_loss_rate",
        "RUDP packet loss rate (0-1)"
    )
    .expect("Failed to register RUDP packet loss rate gauge");

    // Redis 메트릭
    pub static ref REDIS_CONNECTION_POOL_SIZE: IntGauge = register_int_gauge!(
        "redis_connection_pool_size",
        "Number of available Redis connections in pool"
    )
    .expect("Failed to register Redis connection pool size gauge");

    pub static ref REDIS_OPERATIONS: HistogramVec = register_histogram_vec!(
        "redis_operation_duration_seconds",
        "Redis operation duration in seconds",
        &["operation_type"]
    )
    .expect("Failed to register Redis operations histogram");

    // 보안 메트릭
    pub static ref SECURITY_THREAT_LEVEL: IntGauge = register_int_gauge!(
        "security_threat_level",
        "Current security threat level (0-5)"
    )
    .expect("Failed to register security threat level gauge");

    pub static ref SECURITY_AUTH_FAILURES: IntCounter = register_int_counter!(
        "security_authentication_failures_total",
        "Total number of authentication failures"
    )
    .expect("Failed to register authentication failures counter");

    pub static ref SECURITY_RATE_LIMIT_HITS: IntCounter = register_int_counter!(
        "security_rate_limit_hits_total",
        "Total number of rate limit hits"
    )
    .expect("Failed to register rate limit hits counter");

    // 게임 메트릭
    pub static ref GAME_ACTIVE_ROOMS: IntGauge = register_int_gauge!(
        "game_active_rooms",
        "Number of active game rooms"
    )
    .expect("Failed to register active rooms gauge");

    pub static ref GAME_ACTIVE_PLAYERS: IntGauge = register_int_gauge!(
        "game_active_players",
        "Number of active players"
    )
    .expect("Failed to register active players gauge");

    pub static ref GAME_MESSAGES_PER_SECOND: Gauge = register_gauge!(
        "game_messages_per_second",
        "Game messages processed per second"
    )
    .expect("Failed to register messages per second gauge");

    // HTTP 응답 시간 히스토그램
    pub static ref HTTP_REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "http_request_duration_seconds",
        "HTTP request duration in seconds",
        &["method", "endpoint"]
    )
    .expect("Failed to register HTTP request duration histogram");
}

/// 메트릭 헬퍼 함수들
pub struct Metrics;

impl Metrics {
    /// CPU 사용률 업데이트
    pub fn set_cpu_usage(usage: f64) {
        CPU_USAGE.set(usage);
    }

    /// 메모리 사용량 업데이트
    pub fn set_memory_usage(bytes: i64) {
        MEMORY_USAGE.set(bytes);
    }

    /// TCP 연결 수 업데이트
    pub fn set_tcp_connections(count: i64) {
        TCP_CONNECTIONS.set(count);
    }

    /// TCP 메시지 처리 기록
    pub fn record_tcp_message() {
        TCP_MESSAGES_TOTAL.inc();
    }

    /// TCP 에러 기록
    pub fn record_tcp_error() {
        TCP_ERRORS_TOTAL.inc();
    }

    /// TCP 메시지 레이턴시 기록
    pub fn record_tcp_latency(operation: &str, duration: Duration) {
        TCP_MESSAGE_LATENCY
            .with_label_values(&[operation])
            .observe(duration.as_secs_f64());
    }

    /// RUDP 패킷 송신 기록
    pub fn record_rudp_packet_sent() {
        RUDP_PACKETS_SENT.inc();
    }

    /// RUDP 패킷 수신 기록
    pub fn record_rudp_packet_received() {
        RUDP_PACKETS_RECEIVED.inc();
    }

    /// RUDP 패킷 손실률 업데이트
    pub fn set_rudp_packet_loss_rate(rate: f64) {
        RUDP_PACKET_LOSS_RATE.set(rate);
    }

    /// Redis 연결 풀 크기 업데이트
    pub fn set_redis_pool_size(size: i64) {
        REDIS_CONNECTION_POOL_SIZE.set(size);
    }

    /// Redis 작업 시간 기록
    pub fn record_redis_operation(operation: &str, duration: Duration) {
        REDIS_OPERATIONS
            .with_label_values(&[operation])
            .observe(duration.as_secs_f64());
    }

    /// 보안 위협 레벨 설정
    pub fn set_security_threat_level(level: i64) {
        SECURITY_THREAT_LEVEL.set(level);
    }

    /// 인증 실패 기록
    pub fn record_auth_failure() {
        SECURITY_AUTH_FAILURES.inc();
    }

    /// Rate limit hit 기록
    pub fn record_rate_limit_hit() {
        SECURITY_RATE_LIMIT_HITS.inc();
    }

    /// 활성 게임룸 수 업데이트
    pub fn set_active_rooms(count: i64) {
        GAME_ACTIVE_ROOMS.set(count);
    }

    /// 활성 플레이어 수 업데이트
    pub fn set_active_players(count: i64) {
        GAME_ACTIVE_PLAYERS.set(count);
    }

    /// 초당 메시지 처리량 업데이트
    pub fn set_messages_per_second(rate: f64) {
        GAME_MESSAGES_PER_SECOND.set(rate);
    }

    /// HTTP 요청 시간 기록
    pub fn record_http_request(method: &str, endpoint: &str, duration: Duration) {
        HTTP_REQUEST_DURATION
            .with_label_values(&[method, endpoint])
            .observe(duration.as_secs_f64());
    }

    /// Prometheus 메트릭을 텍스트 형식으로 수집
    pub fn gather_metrics() -> Result<String, prometheus::Error> {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer).unwrap_or_default())
    }
}

/// 메트릭 타이머 - 작업 시간 측정용
pub struct MetricTimer {
    start: std::time::Instant,
    metric_name: String,
    labels: Vec<String>,
}

impl MetricTimer {
    /// 새 타이머 시작
    pub fn start(metric_name: &str) -> Self {
        Self {
            start: std::time::Instant::now(),
            metric_name: metric_name.to_string(),
            labels: Vec::new(),
        }
    }

    /// 라벨 추가
    pub fn with_label(mut self, label: String) -> Self {
        self.labels.push(label);
        self
    }

    /// 타이머 종료 및 메트릭 기록
    pub fn record(self) {
        let duration = self.start.elapsed();
        match self.metric_name.as_str() {
            "tcp_message" => {
                if let Some(operation) = self.labels.first() {
                    Metrics::record_tcp_latency(operation, duration);
                }
            }
            "redis" => {
                if let Some(operation) = self.labels.first() {
                    Metrics::record_redis_operation(operation, duration);
                }
            }
            "http" => {
                if self.labels.len() >= 2 {
                    Metrics::record_http_request(&self.labels[0], &self.labels[1], duration);
                }
            }
            _ => {}
        }
    }
}

/// Drop 시 자동으로 기록되는 타이머
impl Drop for MetricTimer {
    fn drop(&mut self) {
        // Drop 시에는 아무것도 하지 않음 - 명시적 record() 호출 필요
    }
}

// 타입들은 이미 위에서 import되었으므로 별칭 불필요

// 이전 버전과 호환성을 위한 전역 변수들 (deprecated, 새 코드에서는 위의 lazy_static 사용)
lazy_static! {
    pub static ref SYSTEM_CPU_USAGE: Gauge = CPU_USAGE.clone();
    pub static ref SYSTEM_MEMORY_USAGE: IntGauge = MEMORY_USAGE.clone();
    pub static ref SYSTEM_DISK_USAGE: IntGauge =
        register_int_gauge!("system_disk_usage", "Disk usage").expect("Failed to create gauge");
    pub static ref NETWORK_BYTES_RECEIVED: IntCounter =
        register_int_counter!("network_bytes_received", "Bytes received")
            .expect("Failed to create counter");
    pub static ref NETWORK_BYTES_SENT: IntCounter =
        register_int_counter!("network_bytes_sent", "Bytes sent")
            .expect("Failed to create counter");
    pub static ref NETWORK_PACKETS_RECEIVED: IntCounter = RUDP_PACKETS_RECEIVED.clone();
    pub static ref NETWORK_PACKETS_SENT: IntCounter = RUDP_PACKETS_SENT.clone();
    pub static ref GAME_MESSAGES_PROCESSED: IntCounter = TCP_MESSAGES_TOTAL.clone();
    pub static ref GAME_MESSAGE_LATENCY: Histogram =
        register_histogram!("game_message_latency", "Game message latency")
            .expect("Failed to create histogram");
    pub static ref TCP_CONNECTIONS_ACTIVE: IntGauge = TCP_CONNECTIONS.clone();
    pub static ref TCP_CONNECTIONS_TOTAL: IntCounter =
        register_int_counter!("tcp_connections_total", "Total TCP connections")
            .expect("Failed to create counter");
    pub static ref TCP_MESSAGES_PER_SECOND: Gauge = GAME_MESSAGES_PER_SECOND.clone();
    pub static ref TCP_ERROR_COUNT: IntCounter = TCP_ERRORS_TOTAL.clone();
    pub static ref RUDP_RETRANSMISSIONS: IntCounter =
        register_int_counter!("rudp_retransmissions", "RUDP retransmissions")
            .expect("Failed to create counter");
    pub static ref GRPC_REQUESTS_TOTAL: IntCounter =
        register_int_counter!("grpc_requests_total", "Total gRPC requests")
            .expect("Failed to create counter");
    pub static ref GRPC_REQUEST_DURATION: Histogram =
        register_histogram!("grpc_request_duration", "gRPC request duration")
            .expect("Failed to create histogram");
    pub static ref GRPC_ERROR_RATE: Gauge =
        register_gauge!("grpc_error_rate", "gRPC error rate").expect("Failed to create gauge");
    pub static ref REDIS_OPERATIONS_TOTAL: IntCounter =
        register_int_counter!("redis_operations_total", "Total Redis operations")
            .expect("Failed to create counter");
    pub static ref REDIS_CACHE_HIT_RATE: Gauge =
        register_gauge!("redis_cache_hit_rate", "Redis cache hit rate")
            .expect("Failed to create gauge");
    pub static ref SECURITY_AUTHENTICATION_ATTEMPTS: IntCounter =
        register_int_counter!("security_auth_attempts", "Authentication attempts")
            .expect("Failed to create counter");
    pub static ref SECURITY_AUTHENTICATION_FAILURES: IntCounter = SECURITY_AUTH_FAILURES.clone();
    pub static ref SECURITY_RATE_LIMIT_EXCEEDED: IntCounter = SECURITY_RATE_LIMIT_HITS.clone();
}

/// 메트릭 수집기 (이전 버전과 호환성 유지)
pub struct MetricsCollector {
    _collection_interval: Duration,
}

impl MetricsCollector {
    pub fn new(collection_interval: Duration) -> Self {
        Self {
            _collection_interval: collection_interval,
        }
    }

    /// 시스템 메트릭 수집
    pub async fn collect_system_metrics(&self) {
        // 실제 시스템 정보 수집은 performance_monitor.rs에서 처리
        // 여기서는 메트릭 업데이트만 담당
    }

    /// 게임 메트릭 업데이트
    pub fn update_game_metrics(
        active_players: i64,
        active_rooms: i64,
        messages_processed: u64,
        message_latency: f64,
    ) {
        Metrics::set_active_players(active_players);
        Metrics::set_active_rooms(active_rooms);
        for _ in 0..messages_processed {
            Metrics::record_tcp_message();
        }
        GAME_MESSAGE_LATENCY.observe(message_latency);
    }

    /// TCP 메트릭 업데이트
    pub fn update_tcp_metrics(active_connections: i64, messages_per_second: f64, error_count: u64) {
        Metrics::set_tcp_connections(active_connections);
        Metrics::set_messages_per_second(messages_per_second);
        for _ in 0..error_count {
            Metrics::record_tcp_error();
        }
    }

    /// 보안 메트릭 업데이트
    pub fn update_security_metrics(
        auth_attempts: u64,
        auth_failures: u64,
        rate_limit_exceeded: u64,
        threat_level: i64,
    ) {
        for _ in 0..auth_attempts {
            SECURITY_AUTHENTICATION_ATTEMPTS.inc();
        }
        for _ in 0..auth_failures {
            Metrics::record_auth_failure();
        }
        for _ in 0..rate_limit_exceeded {
            Metrics::record_rate_limit_hit();
        }
        Metrics::set_security_threat_level(threat_level);
    }

    /// Prometheus 포맷으로 메트릭 내보내기
    pub fn export_metrics() -> String {
        Metrics::gather_metrics().unwrap_or_else(|_| "# Error gathering metrics".to_string())
    }
}

/// 알람 규칙
pub struct AlertRule {
    pub name: String,
    pub condition: String,
    pub threshold: f64,
    pub action: AlertAction,
}

/// 알람 액션
pub enum AlertAction {
    Log,
    Email(String),
    Webhook(String),
}

/// 알람 매니저
pub struct AlertManager {
    rules: Vec<AlertRule>,
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AlertManager {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: AlertRule) {
        self.rules.push(rule);
    }

    pub async fn check_alerts(&self) {
        // 알람 체크 로직은 별도의 모니터링 서비스에서 구현
        for rule in &self.rules {
            // 실제 메트릭 값과 비교하여 알람 발생 여부 결정
            match rule.action {
                AlertAction::Log => {
                    tracing::warn!("Alert: {} triggered", rule.name);
                }
                AlertAction::Email(ref email) => {
                    tracing::warn!("Alert: {} - Email would be sent to {}", rule.name, email);
                }
                AlertAction::Webhook(ref url) => {
                    tracing::warn!("Alert: {} - Webhook would be called: {}", rule.name, url);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        // CPU 사용률 설정
        Metrics::set_cpu_usage(45.5);

        // TCP 메시지 기록
        Metrics::record_tcp_message();
        Metrics::record_tcp_message();

        // 에러 기록
        Metrics::record_tcp_error();

        // 레이턴시 기록
        Metrics::record_tcp_latency("process", Duration::from_millis(10));

        // 메트릭 수집
        let metrics_text = Metrics::gather_metrics().expect("Failed to gather metrics");
        assert!(metrics_text.contains("system_cpu_usage_percent"));
        assert!(metrics_text.contains("tcp_messages_total"));
    }

    #[test]
    fn test_metric_timer() {
        let timer = MetricTimer::start("tcp_message").with_label("decode".to_string());

        // 작업 시뮬레이션
        std::thread::sleep(Duration::from_millis(10));

        timer.record();

        let metrics_text = Metrics::gather_metrics().expect("Failed to gather metrics");
        assert!(metrics_text.contains("tcp_message_latency_seconds"));
    }
}
