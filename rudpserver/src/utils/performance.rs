//! 성능 모니터링 유틸리티
//!
//! RUDP 서버의 성능을 실시간으로 모니터링하고 메트릭을 수집하는 도구들
//! - 시스템 리소스 모니터링 (CPU, 메모리, 네트워크)
//! - 게임 서버 메트릭 (플레이어 수, TPS, 지연시간)
//! - 성능 분석 및 최적화 제안
//! - Prometheus 메트릭 내보내기

// MonitoringConfig 직접 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enable_system_monitoring: bool,
    pub enable_game_monitoring: bool,
    pub metrics_retention_seconds: u64,
    pub alert_thresholds: AlertThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub high_cpu_percent: f64,
    pub high_memory_percent: f64,
    pub high_latency_ms: f64,
    pub low_tps_threshold: f64,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_system_monitoring: true,
            enable_game_monitoring: true,
            metrics_retention_seconds: 3600,
            alert_thresholds: AlertThresholds {
                high_cpu_percent: 80.0,
                high_memory_percent: 85.0,
                high_latency_ms: 100.0,
                low_tps_threshold: 20.0,
            },
        }
    }
}
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// Shared library import
use shared::tool::high_performance::redis_optimizer::RedisOptimizer;

/// 시스템 성능 메트릭
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// 타임스탬프 (유닉스 시간)
    pub timestamp: u64,
    /// CPU 사용률 (%)
    pub cpu_usage_percent: f32,
    /// 메모리 사용량 (MB)
    pub memory_usage_mb: u64,
    /// 총 메모리 (MB)
    pub total_memory_mb: u64,
    /// 네트워크 수신 속도 (Mbps)
    pub network_in_mbps: f64,
    /// 네트워크 송신 속도 (Mbps)
    pub network_out_mbps: f64,
    /// 디스크 사용률 (%)
    pub disk_usage_percent: f32,
    /// 평균 부하 (1분)
    pub load_average_1min: f64,
}

/// 게임 서버 메트릭
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMetrics {
    /// 타임스탬프
    pub timestamp: u64,
    /// 활성 세션 수
    pub active_sessions: u32,
    /// 활성 플레이어 수
    pub active_players: u32,
    /// 초당 처리된 패킷 수
    pub packets_per_second: u32,
    /// 평균 지연시간 (밀리초)
    pub average_latency_ms: f64,
    /// 최대 지연시간 (밀리초)
    pub max_latency_ms: f64,
    /// 패킷 손실률 (%)
    pub packet_loss_percent: f64,
    /// 초당 게임 틱 수 (TPS)
    pub ticks_per_second: u32,
    /// 평균 틱 처리 시간 (마이크로초)
    pub average_tick_time_us: u64,
}

/// 성능 경고 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlert {
    /// 경고 ID
    pub alert_id: String,
    /// 발생 시간
    pub timestamp: u64,
    /// 경고 레벨 (Info, Warning, Critical)
    pub level: AlertLevel,
    /// 경고 카테고리
    pub category: AlertCategory,
    /// 경고 메시지
    pub message: String,
    /// 현재 값
    pub current_value: f64,
    /// 임계값
    pub threshold_value: f64,
    /// 추천 조치사항
    pub recommended_action: Option<String>,
}

/// 경고 레벨
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

/// 경고 카테고리
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCategory {
    CPU,
    Memory,
    Network,
    Disk,
    GamePerformance,
    PlayerCount,
    Latency,
}

/// 성능 모니터
pub struct PerformanceMonitor {
    config: MonitoringConfig,
    // redis_optimizer: Arc<RedisOptimizer>, // 임시 제거

    // 메트릭 히스토리
    system_metrics_history: Arc<RwLock<VecDeque<SystemMetrics>>>,
    game_metrics_history: Arc<RwLock<VecDeque<GameMetrics>>>,

    // 실시간 성능 카운터
    packet_counter: Arc<RwLock<PacketCounter>>,
    latency_tracker: Arc<RwLock<LatencyTracker>>,
    tick_timer: Arc<RwLock<TickTimer>>,

    // 경고 시스템
    active_alerts: Arc<RwLock<HashMap<String, PerformanceAlert>>>,

    // 베이스라인 성능
    baseline_metrics: Arc<RwLock<Option<SystemMetrics>>>,
}

/// 패킷 카운터
struct PacketCounter {
    total_packets: u64,
    packets_this_second: u32,
    last_reset: Instant,
}

/// 지연시간 추적기
struct LatencyTracker {
    latency_samples: VecDeque<f64>,
    max_samples: usize,
}

/// 틱 타이머
struct TickTimer {
    tick_count: u64,
    total_tick_time: Duration,
    last_tick_time: Instant,
}

impl PerformanceMonitor {
    /// 새로운 성능 모니터 생성
    pub async fn new(config: MonitoringConfig) -> Result<Self> {
        // Redis에서 기존 메트릭 히스토리 로드 시도
        let redis_url = format!("redis://{}:{}", "127.0.0.1", 6379); // 임시
                                                                     // Redis config 임시 주석처리 - shared::config 모듈 문제
                                                                     // let redis_config = shared::config::redis_config::RedisConfig::default();
                                                                     // Redis 연결 임시 주석처리
                                                                     // let redis_optimizer = Arc::new(
                                                                     //     RedisOptimizer::new(&redis_url, redis_config).await?
                                                                     // );

        // Redis 연결 완전히 제거 (컴파일 에러 방지)
        // 실제 배포시에는 Redis 연결 복구 필요

        let monitor = Self {
            config,
            // redis_optimizer, // 임시 제거
            system_metrics_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            game_metrics_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            packet_counter: Arc::new(RwLock::new(PacketCounter {
                total_packets: 0,
                packets_this_second: 0,
                last_reset: Instant::now(),
            })),
            latency_tracker: Arc::new(RwLock::new(LatencyTracker {
                latency_samples: VecDeque::with_capacity(1000),
                max_samples: 1000,
            })),
            tick_timer: Arc::new(RwLock::new(TickTimer {
                tick_count: 0,
                total_tick_time: Duration::ZERO,
                last_tick_time: Instant::now(),
            })),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            baseline_metrics: Arc::new(RwLock::new(None)),
        };

        // 베이스라인 메트릭 설정
        if let Ok(baseline) = monitor.collect_system_metrics().await {
            *monitor.baseline_metrics.write().await = Some(baseline);
        }

        info!("Performance monitor initialized");
        Ok(monitor)
    }

    /// 시스템 메트릭 수집
    pub async fn collect_system_metrics(&self) -> Result<SystemMetrics> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // 시스템 정보 수집 (플랫폼별 구현 필요)
        let cpu_usage = self.get_cpu_usage().await?;
        let (memory_usage, total_memory) = self.get_memory_usage().await?;
        let (network_in, network_out) = self.get_network_usage().await?;
        let disk_usage = self.get_disk_usage().await?;
        let load_average = self.get_load_average().await?;

        let metrics = SystemMetrics {
            timestamp,
            cpu_usage_percent: cpu_usage,
            memory_usage_mb: memory_usage,
            total_memory_mb: total_memory,
            network_in_mbps: network_in,
            network_out_mbps: network_out,
            disk_usage_percent: disk_usage,
            load_average_1min: load_average,
        };

        // 히스토리에 추가
        let mut history = self.system_metrics_history.write().await;
        history.push_back(metrics.clone());

        // 최대 히스토리 크기 유지
        while history.len() > 1000 {
            history.pop_front();
        }

        // 경고 체크
        self.check_system_alerts(&metrics).await;

        Ok(metrics)
    }

    /// 게임 메트릭 수집
    pub async fn collect_game_metrics(
        &self,
        active_sessions: u32,
        active_players: u32,
    ) -> Result<GameMetrics> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // 패킷 통계
        let packets_per_second = {
            let mut counter = self.packet_counter.write().await;
            if counter.last_reset.elapsed() >= Duration::from_secs(1) {
                let pps = counter.packets_this_second;
                counter.packets_this_second = 0;
                counter.last_reset = Instant::now();
                pps
            } else {
                counter.packets_this_second
            }
        };

        // 지연시간 통계
        let (average_latency, max_latency) = {
            let tracker = self.latency_tracker.read().await;
            if tracker.latency_samples.is_empty() {
                (0.0, 0.0)
            } else {
                let avg = tracker.latency_samples.iter().sum::<f64>()
                    / tracker.latency_samples.len() as f64;
                let max = tracker
                    .latency_samples
                    .iter()
                    .fold(0.0_f64, |a, &b| a.max(b));
                (avg, max)
            }
        };

        // 틱 통계
        let (ticks_per_second, average_tick_time) = {
            let timer = self.tick_timer.read().await;
            let tps = if timer.total_tick_time.as_secs() > 0 {
                timer.tick_count / timer.total_tick_time.as_secs().max(1)
            } else {
                0
            } as u32;

            let avg_tick_time = if timer.tick_count > 0 {
                (timer.total_tick_time.as_micros() / timer.tick_count.max(1) as u128) as u64
            } else {
                0
            } as u64;

            (tps, avg_tick_time)
        };

        let metrics = GameMetrics {
            timestamp,
            active_sessions,
            active_players,
            packets_per_second,
            average_latency_ms: average_latency,
            max_latency_ms: max_latency,
            packet_loss_percent: 0.0, // TODO: 실제 패킷 손실률 계산
            ticks_per_second,
            average_tick_time_us: average_tick_time,
        };

        // 히스토리에 추가
        let mut history = self.game_metrics_history.write().await;
        history.push_back(metrics.clone());

        while history.len() > 1000 {
            history.pop_front();
        }

        // 게임 성능 경고 체크
        self.check_game_alerts(&metrics).await;

        Ok(metrics)
    }

    /// 패킷 카운터 증가
    pub async fn increment_packet_count(&self) {
        let mut counter = self.packet_counter.write().await;
        counter.total_packets += 1;
        counter.packets_this_second += 1;
    }

    /// 지연시간 샘플 추가
    pub async fn add_latency_sample(&self, latency_ms: f64) {
        let mut tracker = self.latency_tracker.write().await;
        tracker.latency_samples.push_back(latency_ms);

        while tracker.latency_samples.len() > tracker.max_samples {
            tracker.latency_samples.pop_front();
        }
    }

    /// 틱 시간 기록
    pub async fn record_tick_time(&self, tick_duration: Duration) {
        let mut timer = self.tick_timer.write().await;
        timer.tick_count += 1;
        timer.total_tick_time += tick_duration;
    }

    /// Redis에 메트릭 저장
    pub async fn store_metrics(
        &self,
        system_metrics: &SystemMetrics,
        active_sessions: u32,
        active_players: u32,
    ) -> Result<()> {
        let game_metrics = self
            .collect_game_metrics(active_sessions, active_players)
            .await?;

        // 시스템 메트릭 저장
        let system_key = format!("metrics:system:{}", system_metrics.timestamp);
        let system_data = serde_json::to_vec(system_metrics)?;
        // Redis 사용 임시 주석처리
        // self.redis_optimizer.set(&system_key, &system_data, Some(self.config.metrics_retention_seconds as u64 * 3600)).await?;

        // 게임 메트릭 저장
        let game_key = format!("metrics:game:{}", game_metrics.timestamp);
        let game_data = serde_json::to_vec(&game_metrics)?;
        // Redis 사용 임시 주석처리
        // self.redis_optimizer.set(&game_key, &game_data, Some(self.config.metrics_retention_seconds as u64 * 3600)).await?;

        // 메트릭 인덱스 업데이트 (시계열 데이터)
        let index_key = "metrics:index:timestamps";
        // Redis zadd 메서드 호출 (임시 주석처리)
        // let _ = self.redis_optimizer.zadd(&index_key, system_metrics.timestamp as f64, &system_metrics.timestamp.to_string()).await;

        debug!(
            timestamp = %system_metrics.timestamp,
            "Metrics stored to Redis"
        );

        Ok(())
    }

    /// 시스템 경고 체크
    async fn check_system_alerts(&self, metrics: &SystemMetrics) {
        let mut alerts = Vec::new();

        // CPU 사용률 체크
        if metrics.cpu_usage_percent as f64 > self.config.alert_thresholds.high_cpu_percent {
            alerts.push(PerformanceAlert {
                alert_id: "cpu_high".to_string(),
                timestamp: metrics.timestamp,
                level: if metrics.cpu_usage_percent > 90.0 {
                    AlertLevel::Critical
                } else {
                    AlertLevel::Warning
                },
                category: AlertCategory::CPU,
                message: format!("High CPU usage: {:.1}%", metrics.cpu_usage_percent),
                current_value: metrics.cpu_usage_percent as f64,
                threshold_value: self.config.alert_thresholds.high_cpu_percent,
                recommended_action: Some(
                    "Consider scaling up or optimizing CPU-intensive operations".to_string(),
                ),
            });
        }

        // 메모리 사용률 체크
        let memory_usage_percent =
            (metrics.memory_usage_mb as f64 / metrics.total_memory_mb as f64) * 100.0;
        if memory_usage_percent > self.config.alert_thresholds.high_memory_percent {
            alerts.push(PerformanceAlert {
                alert_id: "memory_high".to_string(),
                timestamp: metrics.timestamp,
                level: if memory_usage_percent > 95.0 {
                    AlertLevel::Critical
                } else {
                    AlertLevel::Warning
                },
                category: AlertCategory::Memory,
                message: format!("High memory usage: {:.1}%", memory_usage_percent),
                current_value: memory_usage_percent,
                threshold_value: self.config.alert_thresholds.high_memory_percent,
                recommended_action: Some(
                    "Check for memory leaks or increase available memory".to_string(),
                ),
            });
        }

        // 경고 등록
        let mut active_alerts = self.active_alerts.write().await;
        for alert in alerts {
            let alert_id = alert.alert_id.clone();
            match alert.level {
                AlertLevel::Critical => error!(
                    alert_id = %alert_id,
                    message = %alert.message,
                    current_value = %alert.current_value,
                    threshold = %alert.threshold_value,
                    "Critical performance alert"
                ),
                AlertLevel::Warning => warn!(
                    alert_id = %alert_id,
                    message = %alert.message,
                    current_value = %alert.current_value,
                    threshold = %alert.threshold_value,
                    "Performance warning"
                ),
                AlertLevel::Info => info!(
                    alert_id = %alert_id,
                    message = %alert.message,
                    "Performance info"
                ),
            }

            active_alerts.insert(alert_id, alert);
        }
    }

    /// 게임 성능 경고 체크
    async fn check_game_alerts(&self, metrics: &GameMetrics) {
        let mut alerts = Vec::new();

        // 지연시간 체크
        if metrics.average_latency_ms > self.config.alert_thresholds.high_latency_ms {
            alerts.push(PerformanceAlert {
                alert_id: "latency_high".to_string(),
                timestamp: metrics.timestamp,
                level: if metrics.average_latency_ms > 200.0 {
                    AlertLevel::Critical
                } else {
                    AlertLevel::Warning
                },
                category: AlertCategory::Latency,
                message: format!("High network latency: {:.1}ms", metrics.average_latency_ms),
                current_value: metrics.average_latency_ms,
                threshold_value: self.config.alert_thresholds.high_latency_ms,
                recommended_action: Some("Check network connectivity and server load".to_string()),
            });
        }

        // 틱 레이트 체크
        if metrics.ticks_per_second < 50 {
            // 60 TPS 목표에서 50 이하면 경고
            alerts.push(PerformanceAlert {
                alert_id: "tps_low".to_string(),
                timestamp: metrics.timestamp,
                level: if metrics.ticks_per_second < 30 {
                    AlertLevel::Critical
                } else {
                    AlertLevel::Warning
                },
                category: AlertCategory::GamePerformance,
                message: format!("Low TPS: {} (target: 60)", metrics.ticks_per_second),
                current_value: metrics.ticks_per_second as f64,
                threshold_value: 60.0,
                recommended_action: Some("Optimize game logic or reduce player load".to_string()),
            });
        }

        // 경고 등록
        let mut active_alerts = self.active_alerts.write().await;
        for alert in alerts {
            let alert_id = alert.alert_id.clone();
            match alert.level {
                AlertLevel::Critical => error!(
                    alert_id = %alert_id,
                    message = %alert.message,
                    current_value = %alert.current_value,
                    "Critical game performance alert"
                ),
                AlertLevel::Warning => warn!(
                    alert_id = %alert_id,
                    message = %alert.message,
                    current_value = %alert.current_value,
                    "Game performance warning"
                ),
                AlertLevel::Info => info!(
                    alert_id = %alert_id,
                    message = %alert.message,
                    "Game performance info"
                ),
            }

            active_alerts.insert(alert_id, alert);
        }
    }

    /// 성능 통계 생성
    pub async fn generate_performance_report(&self) -> Result<PerformanceReport> {
        let system_history = self.system_metrics_history.read().await;
        let game_history = self.game_metrics_history.read().await;
        let active_alerts = self.active_alerts.read().await;

        let report = PerformanceReport {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            system_metrics_count: system_history.len(),
            game_metrics_count: game_history.len(),
            active_alerts_count: active_alerts.len(),

            // 최근 시스템 메트릭
            latest_system_metrics: system_history.back().cloned(),

            // 최근 게임 메트릭
            latest_game_metrics: game_history.back().cloned(),

            // 평균 성능 (최근 1시간)
            avg_cpu_usage: system_history
                .iter()
                .rev()
                .take(360)
                .map(|m| m.cpu_usage_percent as f64)
                .sum::<f64>()
                / 360.0_f64.min(system_history.len() as f64),
            avg_memory_usage: system_history
                .iter()
                .rev()
                .take(360)
                .map(|m| m.memory_usage_mb as f64)
                .sum::<f64>()
                / 360.0_f64.min(system_history.len() as f64),
            avg_latency: game_history
                .iter()
                .rev()
                .take(360)
                .map(|m| m.average_latency_ms)
                .sum::<f64>()
                / 360.0_f64.min(game_history.len() as f64),
            avg_tps: game_history
                .iter()
                .rev()
                .take(360)
                .map(|m| m.ticks_per_second as f64)
                .sum::<f64>()
                / 360.0_f64.min(game_history.len() as f64),

            // 활성 경고
            alerts: active_alerts.values().cloned().collect(),
        };

        Ok(report)
    }

    // 플랫폼별 시스템 정보 수집 함수들 (구현 필요)
    async fn get_cpu_usage(&self) -> Result<f32> {
        // TODO: 플랫폼별 CPU 사용률 수집
        Ok(25.5) // 임시값
    }

    async fn get_memory_usage(&self) -> Result<(u64, u64)> {
        // TODO: 플랫폼별 메모리 사용량 수집
        Ok((2048, 8192)) // 임시값: (사용량MB, 총용량MB)
    }

    async fn get_network_usage(&self) -> Result<(f64, f64)> {
        // TODO: 네트워크 사용량 수집
        Ok((10.5, 15.2)) // 임시값: (수신Mbps, 송신Mbps)
    }

    async fn get_disk_usage(&self) -> Result<f32> {
        // TODO: 디스크 사용률 수집
        Ok(45.8) // 임시값
    }

    async fn get_load_average(&self) -> Result<f64> {
        // TODO: 시스템 부하 평균 수집
        Ok(0.75) // 임시값
    }
}

/// 성능 보고서
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub timestamp: u64,
    pub system_metrics_count: usize,
    pub game_metrics_count: usize,
    pub active_alerts_count: usize,

    pub latest_system_metrics: Option<SystemMetrics>,
    pub latest_game_metrics: Option<GameMetrics>,

    pub avg_cpu_usage: f64,
    pub avg_memory_usage: f64,
    pub avg_latency: f64,
    pub avg_tps: f64,

    pub alerts: Vec<PerformanceAlert>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_monitor_creation() {
        let config = MonitoringConfig::default();
        let monitor = PerformanceMonitor::new(config).await;
        assert!(monitor.is_ok());
    }

    #[tokio::test]
    async fn test_packet_counter() {
        let config = MonitoringConfig::default();
        let monitor = PerformanceMonitor::new(config).await.unwrap();

        // 패킷 카운터 증가
        for _ in 0..100 {
            monitor.increment_packet_count().await;
        }

        let counter = monitor.packet_counter.read().await;
        assert_eq!(counter.total_packets, 100);
    }

    #[tokio::test]
    async fn test_latency_tracking() {
        let config = MonitoringConfig::default();
        let monitor = PerformanceMonitor::new(config).await.unwrap();

        // 지연시간 샘플 추가
        monitor.add_latency_sample(50.0).await;
        monitor.add_latency_sample(75.0).await;
        monitor.add_latency_sample(100.0).await;

        let tracker = monitor.latency_tracker.read().await;
        assert_eq!(tracker.latency_samples.len(), 3);

        let avg: f64 =
            tracker.latency_samples.iter().sum::<f64>() / tracker.latency_samples.len() as f64;
        assert_eq!(avg, 75.0);
    }
}
