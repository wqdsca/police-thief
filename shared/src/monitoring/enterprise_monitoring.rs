//! Enterprise Monitoring and Observability System
//!
//! 엔터프라이즈급 모니터링 시스템
//! OpenTelemetry, Prometheus, Jaeger 통합

use anyhow::Result;
use opentelemetry::{
    global,
    metrics::{Counter, Histogram, Meter, ObservableGauge},
    trace::{Span, Tracer},
    Context, KeyValue,
};
use opentelemetry_prometheus::PrometheusExporter;
use prometheus::{Encoder, TextEncoder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, Registry};

/// 엔터프라이즈 모니터링 시스템
pub struct EnterpriseMonitor {
    /// OpenTelemetry Meter
    meter: Meter,
    
    /// Prometheus Exporter
    prometheus_exporter: PrometheusExporter,
    
    /// 메트릭 컬렉터들
    request_counter: Counter<u64>,
    request_duration: Histogram<f64>,
    active_connections: ObservableGauge<u64>,
    error_counter: Counter<u64>,
    
    /// 성능 임계값
    thresholds: PerformanceThresholds,
    
    /// 실시간 메트릭
    real_time_metrics: Arc<RwLock<RealTimeMetrics>>,
    
    /// 알림 시스템
    alert_manager: AlertManager,
}

/// 성능 임계값 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// 최대 응답 시간 (밀리초)
    pub max_response_time_ms: u64,
    
    /// 최대 에러율 (퍼센트)
    pub max_error_rate: f64,
    
    /// 최소 처리량 (msg/sec)
    pub min_throughput: u64,
    
    /// 최대 메모리 사용량 (MB)
    pub max_memory_mb: u64,
    
    /// 최대 CPU 사용률 (퍼센트)
    pub max_cpu_percent: f64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            max_response_time_ms: 2,  // 2ms (현재 <1ms 달성 중)
            max_error_rate: 1.0,       // 1%
            min_throughput: 10_000,    // 10,000 msg/sec (현재 12,991 달성 중)
            max_memory_mb: 100,        // 100MB
            max_cpu_percent: 80.0,     // 80%
        }
    }
}

/// 실시간 메트릭
#[derive(Debug, Default)]
struct RealTimeMetrics {
    /// 현재 처리량
    current_throughput: u64,
    
    /// 평균 응답 시간
    avg_response_time: f64,
    
    /// 현재 에러율
    current_error_rate: f64,
    
    /// 활성 연결 수
    active_connections: u64,
    
    /// 마지막 업데이트 시간
    last_update: Option<Instant>,
}

/// 알림 관리자
struct AlertManager {
    /// 알림 채널
    alert_tx: tokio::sync::mpsc::UnboundedSender<Alert>,
    
    /// 알림 설정
    config: AlertConfig,
}

/// 알림 설정
#[derive(Debug, Clone)]
struct AlertConfig {
    /// Slack Webhook URL
    slack_webhook: Option<String>,
    
    /// Email 설정
    email_config: Option<EmailConfig>,
    
    /// PagerDuty 설정
    pagerduty_config: Option<PagerDutyConfig>,
}

/// 알림 타입
#[derive(Debug, Clone, Serialize)]
pub enum Alert {
    /// 성능 저하
    PerformanceDegradation {
        metric: String,
        current: f64,
        threshold: f64,
        severity: AlertSeverity,
    },
    
    /// 에러율 초과
    HighErrorRate {
        current_rate: f64,
        threshold: f64,
        affected_endpoints: Vec<String>,
    },
    
    /// 시스템 리소스 경고
    ResourceAlert {
        resource_type: ResourceType,
        usage: f64,
        threshold: f64,
    },
    
    /// 보안 이벤트
    SecurityEvent {
        event_type: String,
        source_ip: String,
        details: String,
    },
}

/// 알림 심각도
#[derive(Debug, Clone, Serialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// 리소스 타입
#[derive(Debug, Clone, Serialize)]
pub enum ResourceType {
    Cpu,
    Memory,
    Disk,
    Network,
}

impl EnterpriseMonitor {
    /// 새로운 엔터프라이즈 모니터 생성
    pub fn new(config: MonitorConfig) -> Result<Self> {
        // OpenTelemetry 초기화
        let meter = global::meter("police-thief");
        
        // Prometheus Exporter 생성
        let prometheus_exporter = opentelemetry_prometheus::exporter()
            .with_registry(prometheus::Registry::new())
            .build()?;
            
        // 메트릭 생성
        let request_counter = meter
            .u64_counter("requests_total")
            .with_description("Total number of requests")
            .init();
            
        let request_duration = meter
            .f64_histogram("request_duration_seconds")
            .with_description("Request duration in seconds")
            .init();
            
        let active_connections = meter
            .u64_observable_gauge("active_connections")
            .with_description("Number of active connections")
            .init();
            
        let error_counter = meter
            .u64_counter("errors_total")
            .with_description("Total number of errors")
            .init();
            
        // 알림 채널 생성
        let (alert_tx, mut alert_rx) = tokio::sync::mpsc::unbounded_channel();
        
        // 알림 처리 태스크 시작
        tokio::spawn(async move {
            while let Some(alert) = alert_rx.recv().await {
                Self::handle_alert(alert).await;
            }
        });
        
        Ok(Self {
            meter,
            prometheus_exporter,
            request_counter,
            request_duration,
            active_connections,
            error_counter,
            thresholds: config.thresholds,
            real_time_metrics: Arc::new(RwLock::new(RealTimeMetrics::default())),
            alert_manager: AlertManager {
                alert_tx,
                config: config.alert_config,
            },
        })
    }
    
    /// 요청 기록
    pub async fn record_request(&self, endpoint: &str, duration: Duration, success: bool) {
        let labels = vec![
            KeyValue::new("endpoint", endpoint.to_string()),
            KeyValue::new("status", if success { "success" } else { "failure" }),
        ];
        
        // 카운터 증가
        self.request_counter.add(1, &labels);
        
        // 지속 시간 기록
        self.request_duration.record(duration.as_secs_f64(), &labels);
        
        // 에러 카운터
        if !success {
            self.error_counter.add(1, &labels);
        }
        
        // 실시간 메트릭 업데이트
        self.update_real_time_metrics(duration, success).await;
        
        // 임계값 확인
        if duration.as_millis() as u64 > self.thresholds.max_response_time_ms {
            self.send_alert(Alert::PerformanceDegradation {
                metric: "response_time".to_string(),
                current: duration.as_millis() as f64,
                threshold: self.thresholds.max_response_time_ms as f64,
                severity: AlertSeverity::Warning,
            });
        }
    }
    
    /// 실시간 메트릭 업데이트
    async fn update_real_time_metrics(&self, duration: Duration, success: bool) {
        let mut metrics = self.real_time_metrics.write().await;
        
        let now = Instant::now();
        if let Some(last_update) = metrics.last_update {
            let elapsed = now.duration_since(last_update).as_secs_f64();
            if elapsed > 0.0 {
                // 처리량 계산 (지수 이동 평균)
                let alpha = 0.1; // 평활 계수
                let instant_throughput = 1.0 / elapsed;
                metrics.current_throughput = 
                    ((1.0 - alpha) * metrics.current_throughput as f64 + 
                     alpha * instant_throughput) as u64;
            }
        }
        
        // 응답 시간 업데이트 (지수 이동 평균)
        let alpha = 0.1;
        metrics.avg_response_time = 
            (1.0 - alpha) * metrics.avg_response_time + 
            alpha * duration.as_millis() as f64;
        
        // 에러율 업데이트
        if !success {
            metrics.current_error_rate = 
                (1.0 - alpha) * metrics.current_error_rate + alpha * 100.0;
        } else {
            metrics.current_error_rate = 
                (1.0 - alpha) * metrics.current_error_rate;
        }
        
        metrics.last_update = Some(now);
        
        // 처리량 임계값 확인
        if metrics.current_throughput < self.thresholds.min_throughput {
            self.send_alert(Alert::PerformanceDegradation {
                metric: "throughput".to_string(),
                current: metrics.current_throughput as f64,
                threshold: self.thresholds.min_throughput as f64,
                severity: AlertSeverity::Critical,
            });
        }
        
        // 에러율 임계값 확인
        if metrics.current_error_rate > self.thresholds.max_error_rate {
            self.send_alert(Alert::HighErrorRate {
                current_rate: metrics.current_error_rate,
                threshold: self.thresholds.max_error_rate,
                affected_endpoints: vec![], // TODO: 엔드포인트별 추적
            });
        }
    }
    
    /// 알림 전송
    fn send_alert(&self, alert: Alert) {
        if let Err(e) = self.alert_manager.alert_tx.send(alert) {
            error!("Failed to send alert: {}", e);
        }
    }
    
    /// 알림 처리
    async fn handle_alert(alert: Alert) {
        match &alert {
            Alert::PerformanceDegradation { severity, .. } => {
                match severity {
                    AlertSeverity::Emergency => {
                        error!("EMERGENCY: {:?}", alert);
                        // PagerDuty 호출
                    }
                    AlertSeverity::Critical => {
                        error!("CRITICAL: {:?}", alert);
                        // Slack + Email
                    }
                    AlertSeverity::Warning => {
                        warn!("WARNING: {:?}", alert);
                        // Slack만
                    }
                    AlertSeverity::Info => {
                        info!("INFO: {:?}", alert);
                        // 로그만
                    }
                }
            }
            _ => {
                warn!("Alert: {:?}", alert);
            }
        }
    }
    
    /// Prometheus 메트릭 내보내기
    pub fn export_metrics(&self) -> Result<String> {
        let mut buffer = Vec::new();
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
    
    /// 대시보드용 실시간 메트릭 가져오기
    pub async fn get_dashboard_metrics(&self) -> DashboardMetrics {
        let metrics = self.real_time_metrics.read().await;
        
        DashboardMetrics {
            throughput: metrics.current_throughput,
            avg_response_time_ms: metrics.avg_response_time,
            error_rate: metrics.current_error_rate,
            active_connections: metrics.active_connections,
            health_score: self.calculate_health_score(&metrics),
        }
    }
    
    /// 시스템 건강 점수 계산 (0-100)
    fn calculate_health_score(&self, metrics: &RealTimeMetrics) -> f64 {
        let mut score = 100.0;
        
        // 처리량 점수 (40점)
        if metrics.current_throughput < self.thresholds.min_throughput {
            let ratio = metrics.current_throughput as f64 / self.thresholds.min_throughput as f64;
            score -= 40.0 * (1.0 - ratio);
        }
        
        // 응답 시간 점수 (30점)
        if metrics.avg_response_time > self.thresholds.max_response_time_ms as f64 {
            let ratio = self.thresholds.max_response_time_ms as f64 / metrics.avg_response_time;
            score -= 30.0 * (1.0 - ratio);
        }
        
        // 에러율 점수 (30점)
        if metrics.current_error_rate > self.thresholds.max_error_rate {
            let ratio = self.thresholds.max_error_rate / metrics.current_error_rate;
            score -= 30.0 * (1.0 - ratio);
        }
        
        score.max(0.0).min(100.0)
    }
}

/// 모니터 설정
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub thresholds: PerformanceThresholds,
    pub alert_config: AlertConfig,
}

/// Email 설정
#[derive(Debug, Clone)]
struct EmailConfig {
    smtp_server: String,
    from_address: String,
    to_addresses: Vec<String>,
}

/// PagerDuty 설정
#[derive(Debug, Clone)]
struct PagerDutyConfig {
    api_key: String,
    service_id: String,
}

/// 대시보드 메트릭
#[derive(Debug, Serialize)]
pub struct DashboardMetrics {
    pub throughput: u64,
    pub avg_response_time_ms: f64,
    pub error_rate: f64,
    pub active_connections: u64,
    pub health_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_monitoring_system() {
        let config = MonitorConfig {
            thresholds: PerformanceThresholds::default(),
            alert_config: AlertConfig {
                slack_webhook: None,
                email_config: None,
                pagerduty_config: None,
            },
        };
        
        let monitor = EnterpriseMonitor::new(config).unwrap();
        
        // 성공 요청 시뮬레이션
        for _ in 0..100 {
            monitor.record_request(
                "/api/test",
                Duration::from_millis(1),
                true
            ).await;
        }
        
        // 실패 요청 시뮬레이션
        for _ in 0..5 {
            monitor.record_request(
                "/api/test",
                Duration::from_millis(10),
                false
            ).await;
        }
        
        // 대시보드 메트릭 확인
        let metrics = monitor.get_dashboard_metrics().await;
        assert!(metrics.health_score > 90.0);
    }
}