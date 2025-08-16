//! 락-프리 원자적 통계 시스템
//!
//! 고성능 TCP 서버를 위한 락-프리 통계 수집 및 모니터링 시스템입니다.
//! AtomicU64를 사용하여 락 없이 멀티스레드 환경에서 안전하게 통계를 수집합니다.
//!
//! ## 주요 기능
//! - 락-프리 통계 수집으로 성능 오버헤드 최소화
//! - 실시간 통계 조회 및 모니터링
//! - 메트릭별 세부 분류 및 집계
//! - 성능 임계값 기반 알림 시스템

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// 원자적 통계 데이터 구조체
///
/// 모든 필드는 AtomicU64로 구성되어 락 없이 멀티스레드 환경에서 안전합니다.
pub struct AtomicStats {
    // 연결 관련 통계
    pub total_connections: AtomicU64,
    pub active_connections: AtomicU64,
    pub peak_connections: AtomicU64,
    pub failed_connections: AtomicU64,

    // 메시지 통계
    pub total_messages: AtomicU64,
    pub heartbeat_messages: AtomicU64,
    pub chat_messages: AtomicU64,
    pub room_messages: AtomicU64,
    pub system_messages: AtomicU64,
    pub error_messages: AtomicU64,

    // 성능 통계 (마이크로초 단위)
    pub total_processing_time_us: AtomicU64,
    pub max_processing_time_us: AtomicU64,
    pub broadcast_time_us: AtomicU64,
    pub serialization_time_us: AtomicU64,

    // 방 관련 통계
    pub total_rooms: AtomicU64,
    pub active_rooms: AtomicU64,
    pub peak_rooms: AtomicU64,
    pub room_joins: AtomicU64,
    pub room_leaves: AtomicU64,

    // 대역폭 통계 (바이트)
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub total_bandwidth: AtomicU64,

    // 에러 통계
    pub connection_timeouts: AtomicU64,
    pub protocol_errors: AtomicU64,
    pub serialization_errors: AtomicU64,
    pub broadcast_errors: AtomicU64,

    // 시스템 시간
    pub start_time: SystemTime,
    pub last_reset_time: AtomicU64, // Unix timestamp
}

/// 통계 스냅샷 (조회용 불변 데이터)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSnapshot {
    // 연결 통계
    pub total_connections: u64,
    pub active_connections: u64,
    pub peak_connections: u64,
    pub failed_connections: u64,
    pub connection_success_rate: f64,

    // 메시지 통계
    pub total_messages: u64,
    pub heartbeat_messages: u64,
    pub chat_messages: u64,
    pub room_messages: u64,
    pub system_messages: u64,
    pub error_messages: u64,
    pub messages_per_second: f64,

    // 성능 통계
    pub avg_processing_time_ms: f64,
    pub max_processing_time_ms: f64,
    pub avg_broadcast_time_ms: f64,
    pub avg_serialization_time_ms: f64,

    // 방 통계
    pub total_rooms: u64,
    pub active_rooms: u64,
    pub peak_rooms: u64,
    pub room_joins: u64,
    pub room_leaves: u64,
    pub avg_users_per_room: f64,

    // 대역폭 통계
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub total_bandwidth: u64,
    pub bandwidth_mbps: f64,

    // 에러 통계
    pub connection_timeouts: u64,
    pub protocol_errors: u64,
    pub serialization_errors: u64,
    pub broadcast_errors: u64,
    pub total_errors: u64,
    pub error_rate: f64,

    // 시간 정보
    pub uptime_seconds: u64,
    pub snapshot_timestamp: u64,
}

/// 성능 알림 임계값 설정
#[derive(Debug, Clone)]
pub struct PerformanceThresholds {
    pub max_processing_time_ms: f64,
    pub max_broadcast_time_ms: f64,
    pub max_error_rate: f64,
    pub max_memory_usage_mb: u64,
    pub min_messages_per_second: f64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            max_processing_time_ms: 10.0,   // 10ms 이상 시 경고
            max_broadcast_time_ms: 5.0,     // 5ms 이상 시 경고
            max_error_rate: 0.05,           // 5% 이상 에러 시 경고
            max_memory_usage_mb: 512,       // 512MB 이상 시 경고
            min_messages_per_second: 100.0, // 100 msg/s 이하 시 경고
        }
    }
}

impl AtomicStats {
    /// 새로운 원자적 통계 인스턴스 생성
    pub fn new() -> Self {
        let now = SystemTime::now();
        let now_timestamp = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

        Self {
            total_connections: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
            peak_connections: AtomicU64::new(0),
            failed_connections: AtomicU64::new(0),

            total_messages: AtomicU64::new(0),
            heartbeat_messages: AtomicU64::new(0),
            chat_messages: AtomicU64::new(0),
            room_messages: AtomicU64::new(0),
            system_messages: AtomicU64::new(0),
            error_messages: AtomicU64::new(0),

            total_processing_time_us: AtomicU64::new(0),
            max_processing_time_us: AtomicU64::new(0),
            broadcast_time_us: AtomicU64::new(0),
            serialization_time_us: AtomicU64::new(0),

            total_rooms: AtomicU64::new(0),
            active_rooms: AtomicU64::new(0),
            peak_rooms: AtomicU64::new(0),
            room_joins: AtomicU64::new(0),
            room_leaves: AtomicU64::new(0),

            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            total_bandwidth: AtomicU64::new(0),

            connection_timeouts: AtomicU64::new(0),
            protocol_errors: AtomicU64::new(0),
            serialization_errors: AtomicU64::new(0),
            broadcast_errors: AtomicU64::new(0),

            start_time: now,
            last_reset_time: AtomicU64::new(now_timestamp),
        }
    }

    // === 연결 통계 메서드 ===

    /// 새 연결 등록
    pub fn record_connection(&self) {
        self.total_connections.fetch_add(1, Ordering::Relaxed);
        let current_active = self.active_connections.fetch_add(1, Ordering::Relaxed) + 1;

        // 최대 동시 연결 수 업데이트
        self.update_peak_connections(current_active);

        debug!(
            "새 연결 등록: 총 {}, 활성 {}",
            self.total_connections.load(Ordering::Relaxed),
            current_active
        );
    }

    /// 연결 해제 기록
    pub fn record_disconnection(&self) {
        let current_active = self.active_connections.fetch_sub(1, Ordering::Relaxed);
        debug!("연결 해제: 활성 연결 {}", current_active.saturating_sub(1));
    }

    /// 연결 실패 기록
    pub fn record_connection_failure(&self) {
        self.failed_connections.fetch_add(1, Ordering::Relaxed);
        warn!(
            "연결 실패 기록: 총 {}",
            self.failed_connections.load(Ordering::Relaxed)
        );
    }

    /// 최대 동시 연결 수 업데이트
    fn update_peak_connections(&self, current: u64) {
        let mut current_peak = self.peak_connections.load(Ordering::Relaxed);
        while current > current_peak {
            match self.peak_connections.compare_exchange_weak(
                current_peak,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    info!("새로운 최대 동시 연결 기록: {}", current);
                    break;
                }
                Err(actual) => current_peak = actual,
            }
        }
    }

    // === 메시지 통계 메서드 ===

    /// 메시지 처리 기록
    pub fn record_message_processing(&self, message_type: &str, processing_time: Duration) {
        let processing_us = processing_time.as_micros() as u64;

        // 총 메시지 수와 처리 시간 업데이트
        self.total_messages.fetch_add(1, Ordering::Relaxed);
        self.total_processing_time_us
            .fetch_add(processing_us, Ordering::Relaxed);

        // 최대 처리 시간 업데이트
        self.update_max_processing_time(processing_us);

        // 메시지 타입별 카운터 업데이트
        match message_type {
            "heartbeat" => self.heartbeat_messages.fetch_add(1, Ordering::Relaxed),
            "chat" => self.chat_messages.fetch_add(1, Ordering::Relaxed),
            "room_join" | "room_leave" => self.room_messages.fetch_add(1, Ordering::Relaxed),
            "system" => self.system_messages.fetch_add(1, Ordering::Relaxed),
            "error" => self.error_messages.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };

        debug!("메시지 처리 기록: {} ({}μs)", message_type, processing_us);
    }

    /// 최대 처리 시간 업데이트
    fn update_max_processing_time(&self, current_us: u64) {
        let mut current_max = self.max_processing_time_us.load(Ordering::Relaxed);
        while current_us > current_max {
            match self.max_processing_time_us.compare_exchange_weak(
                current_max,
                current_us,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    if current_us > 10_000 {
                        // 10ms 이상이면 경고
                        warn!("긴 처리 시간 감지: {:.2}ms", current_us as f64 / 1000.0);
                    }
                    break;
                }
                Err(actual) => current_max = actual,
            }
        }
    }

    // === 브로드캐스트 성능 기록 ===

    /// 브로드캐스트 처리 시간 기록
    pub fn record_broadcast_time(&self, duration: Duration) {
        let broadcast_us = duration.as_micros() as u64;
        self.broadcast_time_us
            .fetch_add(broadcast_us, Ordering::Relaxed);

        if broadcast_us > 5_000 {
            // 5ms 이상이면 경고
            warn!(
                "긴 브로드캐스트 시간: {:.2}ms",
                broadcast_us as f64 / 1000.0
            );
        }
    }

    /// 직렬화 처리 시간 기록
    pub fn record_serialization_time(&self, duration: Duration) {
        let serialization_us = duration.as_micros() as u64;
        self.serialization_time_us
            .fetch_add(serialization_us, Ordering::Relaxed);
    }

    // === 방 관련 통계 ===

    /// 방 생성 기록
    pub fn record_room_created(&self) {
        self.total_rooms.fetch_add(1, Ordering::Relaxed);
        let current_active = self.active_rooms.fetch_add(1, Ordering::Relaxed) + 1;
        self.update_peak_rooms(current_active);
    }

    /// 방 삭제 기록
    pub fn record_room_deleted(&self) {
        self.active_rooms.fetch_sub(1, Ordering::Relaxed);
    }

    /// 방 입장 기록
    pub fn record_room_join(&self) {
        self.room_joins.fetch_add(1, Ordering::Relaxed);
    }

    /// 방 퇴장 기록
    pub fn record_room_leave(&self) {
        self.room_leaves.fetch_add(1, Ordering::Relaxed);
    }

    /// 최대 방 수 업데이트
    fn update_peak_rooms(&self, current: u64) {
        let mut current_peak = self.peak_rooms.load(Ordering::Relaxed);
        while current > current_peak {
            match self.peak_rooms.compare_exchange_weak(
                current_peak,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_peak = actual,
            }
        }
    }

    // === 대역폭 통계 ===

    /// 송신 데이터 기록
    pub fn record_bytes_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
        self.total_bandwidth.fetch_add(bytes, Ordering::Relaxed);
    }

    /// 수신 데이터 기록
    pub fn record_bytes_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
        self.total_bandwidth.fetch_add(bytes, Ordering::Relaxed);
    }

    // === 에러 통계 ===

    /// 연결 타임아웃 기록
    pub fn record_connection_timeout(&self) {
        self.connection_timeouts.fetch_add(1, Ordering::Relaxed);
    }

    /// 프로토콜 에러 기록
    pub fn record_protocol_error(&self) {
        self.protocol_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// 직렬화 에러 기록
    pub fn record_serialization_error(&self) {
        self.serialization_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// 브로드캐스트 에러 기록
    pub fn record_broadcast_error(&self) {
        self.broadcast_errors.fetch_add(1, Ordering::Relaxed);
    }

    // === 통계 조회 ===

    /// 현재 통계 스냅샷 생성
    pub fn get_snapshot(&self) -> StatsSnapshot {
        let uptime = self.start_time.elapsed().unwrap_or_default().as_secs();

        let total_messages = self.total_messages.load(Ordering::Relaxed);
        let total_connections = self.total_connections.load(Ordering::Relaxed);
        let failed_connections = self.failed_connections.load(Ordering::Relaxed);
        let active_rooms = self.active_rooms.load(Ordering::Relaxed);
        let active_connections = self.active_connections.load(Ordering::Relaxed);

        let total_processing_us = self.total_processing_time_us.load(Ordering::Relaxed);
        let broadcast_time_us = self.broadcast_time_us.load(Ordering::Relaxed);
        let serialization_time_us = self.serialization_time_us.load(Ordering::Relaxed);

        let total_errors = self.connection_timeouts.load(Ordering::Relaxed)
            + self.protocol_errors.load(Ordering::Relaxed)
            + self.serialization_errors.load(Ordering::Relaxed)
            + self.broadcast_errors.load(Ordering::Relaxed);

        let total_bandwidth = self.total_bandwidth.load(Ordering::Relaxed);

        StatsSnapshot {
            // 연결 통계
            total_connections,
            active_connections,
            peak_connections: self.peak_connections.load(Ordering::Relaxed),
            failed_connections,
            connection_success_rate: if total_connections > 0 {
                (total_connections - failed_connections) as f64 / total_connections as f64
            } else {
                1.0
            },

            // 메시지 통계
            total_messages,
            heartbeat_messages: self.heartbeat_messages.load(Ordering::Relaxed),
            chat_messages: self.chat_messages.load(Ordering::Relaxed),
            room_messages: self.room_messages.load(Ordering::Relaxed),
            system_messages: self.system_messages.load(Ordering::Relaxed),
            error_messages: self.error_messages.load(Ordering::Relaxed),
            messages_per_second: if uptime > 0 {
                total_messages as f64 / uptime as f64
            } else {
                0.0
            },

            // 성능 통계
            avg_processing_time_ms: if total_messages > 0 {
                total_processing_us as f64 / total_messages as f64 / 1000.0
            } else {
                0.0
            },
            max_processing_time_ms: self.max_processing_time_us.load(Ordering::Relaxed) as f64
                / 1000.0,
            avg_broadcast_time_ms: if total_messages > 0 {
                broadcast_time_us as f64 / total_messages as f64 / 1000.0
            } else {
                0.0
            },
            avg_serialization_time_ms: if total_messages > 0 {
                serialization_time_us as f64 / total_messages as f64 / 1000.0
            } else {
                0.0
            },

            // 방 통계
            total_rooms: self.total_rooms.load(Ordering::Relaxed),
            active_rooms,
            peak_rooms: self.peak_rooms.load(Ordering::Relaxed),
            room_joins: self.room_joins.load(Ordering::Relaxed),
            room_leaves: self.room_leaves.load(Ordering::Relaxed),
            avg_users_per_room: if active_rooms > 0 {
                active_connections as f64 / active_rooms as f64
            } else {
                0.0
            },

            // 대역폭 통계
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            total_bandwidth,
            bandwidth_mbps: if uptime > 0 {
                (total_bandwidth as f64 * 8.0) / (uptime as f64 * 1_000_000.0)
            } else {
                0.0
            },

            // 에러 통계
            connection_timeouts: self.connection_timeouts.load(Ordering::Relaxed),
            protocol_errors: self.protocol_errors.load(Ordering::Relaxed),
            serialization_errors: self.serialization_errors.load(Ordering::Relaxed),
            broadcast_errors: self.broadcast_errors.load(Ordering::Relaxed),
            total_errors,
            error_rate: if total_messages > 0 {
                total_errors as f64 / total_messages as f64
            } else {
                0.0
            },

            // 시간 정보
            uptime_seconds: uptime,
            snapshot_timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// 통계 초기화
    pub fn reset(&self) {
        // 누적 통계는 유지, 현재 상태만 초기화
        self.active_connections.store(0, Ordering::Relaxed);
        self.active_rooms.store(0, Ordering::Relaxed);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_reset_time.store(now, Ordering::Relaxed);

        info!("통계 시스템 초기화 완료");
    }

    /// 성능 임계값 확인 및 알림
    pub fn check_performance_thresholds(&self, thresholds: &PerformanceThresholds) -> Vec<String> {
        let snapshot = self.get_snapshot();
        let mut alerts = Vec::new();

        // 처리 시간 확인
        if snapshot.avg_processing_time_ms > thresholds.max_processing_time_ms {
            alerts.push(format!(
                "평균 처리 시간 임계값 초과: {:.2}ms > {:.2}ms",
                snapshot.avg_processing_time_ms, thresholds.max_processing_time_ms
            ));
        }

        // 브로드캐스트 시간 확인
        if snapshot.avg_broadcast_time_ms > thresholds.max_broadcast_time_ms {
            alerts.push(format!(
                "평균 브로드캐스트 시간 임계값 초과: {:.2}ms > {:.2}ms",
                snapshot.avg_broadcast_time_ms, thresholds.max_broadcast_time_ms
            ));
        }

        // 에러율 확인
        if snapshot.error_rate > thresholds.max_error_rate {
            alerts.push(format!(
                "에러율 임계값 초과: {:.2}% > {:.2}%",
                snapshot.error_rate * 100.0,
                thresholds.max_error_rate * 100.0
            ));
        }

        // 메시지 처리율 확인
        if snapshot.messages_per_second < thresholds.min_messages_per_second {
            alerts.push(format!(
                "메시지 처리율 저하: {:.2} msg/s < {:.2} msg/s",
                snapshot.messages_per_second, thresholds.min_messages_per_second
            ));
        }

        if !alerts.is_empty() {
            for alert in &alerts {
                warn!("성능 알림: {}", alert);
            }
        }

        alerts
    }
}

impl Default for AtomicStats {
    fn default() -> Self {
        Self::new()
    }
}

/// 통계 모니터링 서비스
pub struct StatsMonitor {
    stats: std::sync::Arc<AtomicStats>,
    thresholds: PerformanceThresholds,
    monitoring_enabled: std::sync::atomic::AtomicBool,
}

impl StatsMonitor {
    /// 새로운 통계 모니터 생성
    pub fn new(stats: std::sync::Arc<AtomicStats>) -> Self {
        Self {
            stats,
            thresholds: PerformanceThresholds::default(),
            monitoring_enabled: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// 임계값 설정 업데이트
    pub fn update_thresholds(&mut self, thresholds: PerformanceThresholds) {
        self.thresholds = thresholds;
        info!("성능 임계값 업데이트 완료");
    }

    /// 주기적 모니터링 시작
    pub async fn start_periodic_monitoring(&self, interval: Duration) -> Result<()> {
        let stats = self.stats.clone();
        let thresholds = self.thresholds.clone();
        let enabled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            while enabled.load(Ordering::Relaxed) {
                interval_timer.tick().await;

                let alerts = stats.check_performance_thresholds(&thresholds);
                if !alerts.is_empty() {
                    // 알림 처리 (로깅, 이메일, 슬랙 등)
                    for alert in alerts {
                        warn!("⚠️  {}", alert);
                    }
                }

                // 주기적 통계 로그
                let snapshot = stats.get_snapshot();
                info!(
                    "📊 서버 통계 - 활성연결: {}, 총메시지: {}, 평균처리: {:.2}ms, 에러율: {:.2}%",
                    snapshot.active_connections,
                    snapshot.total_messages,
                    snapshot.avg_processing_time_ms,
                    snapshot.error_rate * 100.0
                );
            }
        });

        info!("통계 모니터링 시작 (간격: {:?})", interval);
        Ok(())
    }

    /// 모니터링 중지
    pub fn stop_monitoring(&self) {
        self.monitoring_enabled.store(false, Ordering::Relaxed);
        info!("통계 모니터링 중지");
    }
}

mod tests {

    #[test]
    fn test_atomic_stats_creation() {
        let stats = AtomicStats::new();
        let snapshot = stats.get_snapshot();

        assert_eq!(snapshot.total_connections, 0);
        assert_eq!(snapshot.total_messages, 0);
        assert_eq!(snapshot.active_connections, 0);
    }

    #[test]
    fn test_connection_tracking() {
        let stats = AtomicStats::new();

        // 연결 등록
        stats.record_connection();
        stats.record_connection();

        let snapshot = stats.get_snapshot();
        assert_eq!(snapshot.total_connections, 2);
        assert_eq!(snapshot.active_connections, 2);
        assert_eq!(snapshot.peak_connections, 2);

        // 연결 해제
        stats.record_disconnection();

        let snapshot = stats.get_snapshot();
        assert_eq!(snapshot.total_connections, 2);
        assert_eq!(snapshot.active_connections, 1);
        assert_eq!(snapshot.peak_connections, 2); // 최대값은 유지
    }

    #[test]
    fn test_message_processing() {
        let stats = AtomicStats::new();
        let processing_time = Duration::from_millis(5);

        stats.record_message_processing("chat", processing_time);
        stats.record_message_processing("heartbeat", processing_time);

        let snapshot = stats.get_snapshot();
        assert_eq!(snapshot.total_messages, 2);
        assert_eq!(snapshot.chat_messages, 1);
        assert_eq!(snapshot.heartbeat_messages, 1);
        assert!(snapshot.avg_processing_time_ms > 0.0);
    }

    #[test]
    fn test_performance_thresholds() {
        let stats = AtomicStats::new();
        let thresholds = PerformanceThresholds::default();

        // 긴 처리 시간으로 임계값 테스트
        let long_time = Duration::from_millis(20); // 기본 임계값 10ms 초과
        stats.record_message_processing("test", long_time);

        let alerts = stats.check_performance_thresholds(&thresholds);
        assert!(!alerts.is_empty(), "임계값 초과 알림이 발생해야 함");
    }

    #[tokio::test]
    async fn test_stats_monitor() {
        let stats = std::sync::Arc::new(AtomicStats::new());
        let monitor = StatsMonitor::new(stats.clone());

        // 모니터링 시작
        let result = monitor
            .start_periodic_monitoring(Duration::from_millis(100))
            .await;
        assert!(result.is_ok());

        // 짧은 대기 후 모니터링 중지
        tokio::time::sleep(Duration::from_millis(150)).await;
        monitor.stop_monitoring();
    }

    #[test]
    fn test_bandwidth_tracking() {
        let stats = AtomicStats::new();

        stats.record_bytes_sent(1000);
        stats.record_bytes_received(500);

        let snapshot = stats.get_snapshot();
        assert_eq!(snapshot.bytes_sent, 1000);
        assert_eq!(snapshot.bytes_received, 500);
        assert_eq!(snapshot.total_bandwidth, 1500);
    }

    #[test]
    fn test_room_statistics() {
        let stats = AtomicStats::new();

        stats.record_room_created();
        stats.record_room_created();
        stats.record_room_join();
        stats.record_room_leave();

        let snapshot = stats.get_snapshot();
        assert_eq!(snapshot.total_rooms, 2);
        assert_eq!(snapshot.active_rooms, 2);
        assert_eq!(snapshot.room_joins, 1);
        assert_eq!(snapshot.room_leaves, 1);
    }
}
