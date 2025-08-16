//! 모니터링 모듈 - Prometheus 메트릭, 알람, 대시보드
//!
//! 시스템 전반의 성능과 상태를 모니터링합니다.

pub mod metrics;
pub mod metrics_init;

pub use metrics::{
    AlertAction,
    // metrics_handler, // TODO: warp crate 필요
    AlertManager,
    AlertRule,
    MetricsCollector,
};

pub use metrics_init::{healthcheck, initialize_metrics, is_metrics_initialized};

// 메트릭 재수출
pub use metrics::{
    GAME_ACTIVE_PLAYERS, GAME_ACTIVE_ROOMS, GAME_MESSAGES_PROCESSED, GRPC_REQUESTS_TOTAL,
    GRPC_REQUEST_DURATION, NETWORK_BYTES_RECEIVED, NETWORK_BYTES_SENT, REDIS_CACHE_HIT_RATE,
    REDIS_OPERATIONS_TOTAL, RUDP_PACKETS_RECEIVED, RUDP_PACKETS_SENT, RUDP_RETRANSMISSIONS,
    SECURITY_AUTHENTICATION_ATTEMPTS, SECURITY_AUTHENTICATION_FAILURES, SECURITY_THREAT_LEVEL,
    SYSTEM_CPU_USAGE, SYSTEM_DISK_USAGE, SYSTEM_MEMORY_USAGE, TCP_CONNECTIONS_ACTIVE,
    TCP_ERROR_COUNT, TCP_MESSAGES_PER_SECOND,
};
