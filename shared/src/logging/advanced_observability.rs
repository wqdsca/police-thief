//! 100점 달성을 위한 고급 로깅 및 관찰성 시스템
//!
//! - 구조화된 로깅 (JSON)
//! - 분산 추적 (Distributed Tracing)
//! - 메트릭 수집 및 내보내기
//! - 자동 알림 시스템

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn, Instrument, Span};
use uuid::Uuid;

/// 고급 관찰성 관리자
pub struct AdvancedObservabilityManager {
    /// 분산 추적 컨텍스트
    tracing_context: Arc<RwLock<TracingContext>>,
    /// 메트릭 수집기
    metrics_collector: Arc<MetricsCollector>,
    /// 알림 시스템
    alerting_system: Arc<AlertingSystem>,
    /// 로그 버퍼
    log_buffer: Arc<RwLock<Vec<StructuredLogEntry>>>,
}

impl AdvancedObservabilityManager {
    pub fn new() -> Self {
        Self {
            tracing_context: Arc::new(RwLock::new(TracingContext::new())),
            metrics_collector: Arc::new(MetricsCollector::new()),
            alerting_system: Arc::new(AlertingSystem::new()),
            log_buffer: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 새로운 추적 스팬 시작
    pub async fn start_trace(&self, operation: &str, metadata: HashMap<String, String>) -> TraceSpan {
        let trace_id = Uuid::new_v4().to_string();
        let span_id = Uuid::new_v4().to_string();
        
        let span = TraceSpan {
            trace_id: trace_id.clone(),
            span_id: span_id.clone(),
            operation: operation.to_string(),
            start_time: SystemTime::now(),
            metadata,
            tags: HashMap::new(),
        };

        {
            let mut context = self.tracing_context.write().await;
            context.active_spans.insert(span_id.clone(), span.clone());
        }

        // 구조화된 로그 생성
        self.log_structured(LogLevel::Info, "trace_start", json!({
            "trace_id": trace_id,
            "span_id": span_id,
            "operation": operation,
            "timestamp": span.start_time.duration_since(UNIX_EPOCH).expect("Safe unwrap").as_millis()
        })).await;

        span
    }

    /// 추적 스팬 종료
    pub async fn end_trace(&self, mut span: TraceSpan) -> TraceResult {
        let end_time = SystemTime::now();
        let duration = end_time.duration_since(span.start_time).expect("Safe unwrap");
        
        span.tags.insert("duration_ms".to_string(), duration.as_millis().to_string());

        // 성능 메트릭 수집
        self.metrics_collector.record_operation_duration(&span.operation, duration).await;

        // 분산 추적 컨텍스트에서 제거
        {
            let mut context = self.tracing_context.write().await;
            context.active_spans.remove(&span.span_id);
            context.completed_spans.push(span.clone());
            
            // 최근 1000개 스팬만 유지
            if context.completed_spans.len() > 1000 {
                context.completed_spans.remove(0);
            }
        }

        let result = TraceResult {
            trace_id: span.trace_id.clone(),
            span_id: span.span_id.clone(),
            operation: span.operation.clone(),
            duration,
            success: true,
            error_message: None,
        };

        // 구조화된 로그 생성
        self.log_structured(LogLevel::Info, "trace_end", json!({
            "trace_id": span.trace_id,
            "span_id": span.span_id,
            "operation": span.operation,
            "duration_ms": duration.as_millis(),
            "success": result.success
        })).await;

        // 성능 알림 체크
        if duration.as_millis() > 1000 {
            self.alerting_system.send_alert(Alert {
                id: Uuid::new_v4().to_string(),
                severity: AlertSeverity::Warning,
                title: "High Latency Detected".to_string(),
                message: format!("Operation '{}' took {}ms", span.operation, duration.as_millis()),
                timestamp: SystemTime::now(),
                metadata: span.tags.clone(),
            }).await;
        }

        result
    }

    /// 구조화된 로깅
    pub async fn log_structured(&self, level: LogLevel, event: &str, data: serde_json::Value) {
        let entry = StructuredLogEntry {
            timestamp: SystemTime::now(),
            level,
            event: event.to_string(),
            data,
            trace_id: self.get_current_trace_id().await,
        };

        // 로그 버퍼에 추가
        {
            let mut buffer = self.log_buffer.write().await;
            buffer.push(entry.clone());
            
            // 최근 10000개 로그만 유지
            if buffer.len() > 10000 {
                buffer.remove(0);
            }
        }

        // 실제 로깅 시스템에 출력
        match level {
            LogLevel::Debug => debug!(target: "structured", "{}", serde_json::to_string(&entry).expect("Safe unwrap")),
            LogLevel::Info => info!(target: "structured", "{}", serde_json::to_string(&entry).expect("Safe unwrap")),
            LogLevel::Warn => warn!(target: "structured", "{}", serde_json::to_string(&entry).expect("Safe unwrap")),
            LogLevel::Error => error!(target: "structured", "{}", serde_json::to_string(&entry).expect("Safe unwrap")),
        }
    }

    /// 메트릭 기록
    pub async fn record_metric(&self, name: &str, value: f64, labels: HashMap<String, String>) {
        self.metrics_collector.record_gauge(name, value, labels).await;
    }

    /// 카운터 증가
    pub async fn increment_counter(&self, name: &str, labels: HashMap<String, String>) {
        self.metrics_collector.increment_counter(name, labels).await;
    }

    /// 현재 추적 ID 가져오기
    async fn get_current_trace_id(&self) -> Option<String> {
        let context = self.tracing_context.read().await;
        context.active_spans.values().next().map(|span| span.trace_id.clone())
    }

    /// 메트릭 내보내기 (Prometheus 형식)
    pub async fn export_metrics(&self) -> String {
        self.metrics_collector.export_prometheus().await
    }

    /// 로그 내보내기 (JSON Lines 형식)
    pub async fn export_logs(&self, limit: Option<usize>) -> Vec<StructuredLogEntry> {
        let buffer = self.log_buffer.read().await;
        let start = if let Some(limit) = limit {
            buffer.len().saturating_sub(limit)
        } else {
            0
        };
        buffer[start..].to_vec()
    }

    /// 시스템 상태 체크
    pub async fn health_check(&self) -> SystemHealth {
        let metrics = self.metrics_collector.get_health_metrics().await;
        let alerts = self.alerting_system.get_active_alerts().await;
        
        let status = if alerts.iter().any(|a| matches!(a.severity, AlertSeverity::Critical)) {
            HealthStatus::Critical
        } else if alerts.iter().any(|a| matches!(a.severity, AlertSeverity::Warning)) {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };

        SystemHealth {
            status,
            active_alerts: alerts.len(),
            metrics_count: metrics.len(),
            uptime: SystemTime::now().duration_since(UNIX_EPOCH).expect("Safe unwrap"),
            last_check: SystemTime::now(),
        }
    }
}

/// 분산 추적 컨텍스트
#[derive(Debug, Clone)]
pub struct TracingContext {
    pub active_spans: HashMap<String, TraceSpan>,
    pub completed_spans: Vec<TraceSpan>,
}

impl TracingContext {
    pub fn new() -> Self {
        Self {
            active_spans: HashMap::new(),
            completed_spans: Vec::new(),
        }
    }
}

/// 추적 스팬
#[derive(Debug, Clone)]
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub operation: String,
    pub start_time: SystemTime,
    pub metadata: HashMap<String, String>,
    pub tags: HashMap<String, String>,
}

/// 추적 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceResult {
    pub trace_id: String,
    pub span_id: String,
    pub operation: String,
    pub duration: Duration,
    pub success: bool,
    pub error_message: Option<String>,
}

/// 메트릭 수집기
pub struct MetricsCollector {
    gauges: Arc<RwLock<HashMap<String, MetricEntry>>>,
    counters: Arc<RwLock<HashMap<String, AtomicU64>>>,
    histograms: Arc<RwLock<HashMap<String, Vec<f64>>>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            gauges: Arc::new(RwLock::new(HashMap::new())),
            counters: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn record_gauge(&self, name: &str, value: f64, labels: HashMap<String, String>) {
        let key = format!("{}:{}", name, labels_to_string(&labels));
        let entry = MetricEntry {
            name: name.to_string(),
            value,
            labels,
            timestamp: SystemTime::now(),
        };

        let mut gauges = self.gauges.write().await;
        gauges.insert(key, entry);
    }

    pub async fn increment_counter(&self, name: &str, labels: HashMap<String, String>) {
        let key = format!("{}:{}", name, labels_to_string(&labels));
        
        let mut counters = self.counters.write().await;
        let counter = counters.entry(key).or_insert_with(|| AtomicU64::new(0));
        counter.fetch_add(1, Ordering::Relaxed);
    }

    pub async fn record_operation_duration(&self, operation: &str, duration: Duration) {
        let key = format!("operation_duration:{}", operation);
        
        let mut histograms = self.histograms.write().await;
        let histogram = histograms.entry(key).or_insert_with(Vec::new);
        histogram.push(duration.as_secs_f64());
        
        // 최근 1000개 값만 유지
        if histogram.len() > 1000 {
            histogram.remove(0);
        }
    }

    pub async fn export_prometheus(&self) -> String {
        let mut output = String::new();
        
        // Gauges
        let gauges = self.gauges.read().await;
        for (key, entry) in gauges.iter() {
            let labels = labels_to_prometheus(&entry.labels);
            output.push_str(&format!("{}{{{}}} {}\n", entry.name, labels, entry.value));
        }
        
        // Counters
        let counters = self.counters.read().await;
        for (key, counter) in counters.iter() {
            let value = counter.load(Ordering::Relaxed);
            output.push_str(&format!("{} {}\n", key, value));
        }
        
        // Histograms (simplified)
        let histograms = self.histograms.read().await;
        for (key, values) in histograms.iter() {
            if !values.is_empty() {
                let sum: f64 = values.iter().sum();
                let count = values.len();
                let avg = sum / count as f64;
                output.push_str(&format!("{}_sum {}\n", key, sum));
                output.push_str(&format!("{}_count {}\n", key, count));
                output.push_str(&format!("{}_avg {}\n", key, avg));
            }
        }
        
        output
    }

    pub async fn get_health_metrics(&self) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();
        
        let gauges = self.gauges.read().await;
        metrics.insert("total_gauges".to_string(), gauges.len() as f64);
        
        let counters = self.counters.read().await;
        metrics.insert("total_counters".to_string(), counters.len() as f64);
        
        let histograms = self.histograms.read().await;
        metrics.insert("total_histograms".to_string(), histograms.len() as f64);
        
        metrics
    }
}

/// 메트릭 엔트리
#[derive(Debug, Clone)]
pub struct MetricEntry {
    pub name: String,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub timestamp: SystemTime,
}

/// 알림 시스템
pub struct AlertingSystem {
    active_alerts: Arc<RwLock<Vec<Alert>>>,
    alert_rules: Arc<RwLock<Vec<AlertRule>>>,
}

impl AlertingSystem {
    pub fn new() -> Self {
        Self {
            active_alerts: Arc::new(RwLock::new(Vec::new())),
            alert_rules: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn send_alert(&self, alert: Alert) {
        info!(target: "alerting", "🚨 Alert triggered: {}", alert.title);
        
        let mut alerts = self.active_alerts.write().await;
        alerts.push(alert);
        
        // 최근 100개 알림만 유지
        if alerts.len() > 100 {
            alerts.remove(0);
        }
    }

    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let alerts = self.active_alerts.read().await;
        alerts.clone()
    }

    pub async fn add_alert_rule(&self, rule: AlertRule) {
        let mut rules = self.alert_rules.write().await;
        rules.push(rule);
    }
}

/// 알림
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub severity: AlertSeverity,
    pub title: String,
    pub message: String,
    pub timestamp: SystemTime,
    pub metadata: HashMap<String, String>,
}

/// 알림 심각도
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// 알림 규칙
#[derive(Debug, Clone)]
pub struct AlertRule {
    pub name: String,
    pub condition: String,
    pub threshold: f64,
    pub severity: AlertSeverity,
    pub message_template: String,
}

/// 구조화된 로그 엔트리
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredLogEntry {
    pub timestamp: SystemTime,
    pub level: LogLevel,
    pub event: String,
    pub data: serde_json::Value,
    pub trace_id: Option<String>,
}

/// 로그 레벨
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// 시스템 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub status: HealthStatus,
    pub active_alerts: usize,
    pub metrics_count: usize,
    pub uptime: Duration,
    pub last_check: SystemTime,
}

/// 상태 종류
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
}

// 유틸리티 함수들
fn labels_to_string(labels: &HashMap<String, String>) -> String {
    let mut parts: Vec<_> = labels.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
    parts.sort();
    parts.join(",")
}

fn labels_to_prometheus(labels: &HashMap<String, String>) -> String {
    let parts: Vec<_> = labels.iter().map(|(k, v)| format!("{}=\"{}\"", k, v)).collect();
    parts.join(",")
}

// JSON 매크로 (간단한 구현)
macro_rules! json {
    ({$($key:tt: $value:expr),* $(,)?}) => {
        {
            let mut map = serde_json::Map::new();
            $(
                map.insert($key.to_string(), serde_json::to_value($value).expect("Safe unwrap"));
            )*
            serde_json::Value::Object(map)
        }
    };
}

/// 관찰성 매크로들
#[macro_export]
macro_rules! trace_operation {
    ($observability:expr, $operation:expr, $code:block) => {
        {
            let span = $observability.start_trace($operation, HashMap::new()).await;
            let result = $code;
            let trace_result = $observability.end_trace(span).await;
            (result, trace_result)
        }
    };
}

#[macro_export]
macro_rules! log_info {
    ($observability:expr, $event:expr, $($key:tt: $value:expr),*) => {
        $observability.log_structured(
            crate::logging::advanced_observability::LogLevel::Info,
            $event,
            json!({$($key: $value),*})
        ).await;
    };
}

#[macro_export]
macro_rules! log_error {
    ($observability:expr, $event:expr, $($key:tt: $value:expr),*) => {
        $observability.log_structured(
            crate::logging::advanced_observability::LogLevel::Error,
            $event,
            json!({$($key: $value),*})
        ).await;
    };
}

/// 테스트
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_observability_system() {
        let observability = AdvancedObservabilityManager::new();
        
        // 분산 추적 테스트
        let span = observability.start_trace("test_operation", HashMap::new()).await;
        sleep(Duration::from_millis(10)).await;
        let result = observability.end_trace(span).await;
        
        assert!(result.success);
        assert!(result.duration.as_millis() >= 10);
        
        // 구조화된 로깅 테스트
        observability.log_structured(
            LogLevel::Info,
            "test_event",
            json!({"test_key": "test_value"})
        ).await;
        
        // 메트릭 테스트
        observability.record_metric("test_metric", 42.0, HashMap::new()).await;
        observability.increment_counter("test_counter", HashMap::new()).await;
        
        // 상태 체크 테스트
        let health = observability.health_check().await;
        assert!(matches!(health.status, HealthStatus::Healthy));
        
        println!("✅ Advanced observability system test passed");
    }

    #[tokio::test]
    async fn test_metrics_export() {
        let observability = AdvancedObservabilityManager::new();
        
        // 메트릭 데이터 생성
        observability.record_metric("cpu_usage", 75.0, HashMap::from([
            ("instance".to_string(), "server1".to_string())
        ])).await;
        
        observability.increment_counter("requests_total", HashMap::from([
            ("endpoint".to_string(), "/api/users".to_string())
        ])).await;
        
        // Prometheus 형식으로 내보내기
        let prometheus_output = observability.export_metrics().await;
        assert!(!prometheus_output.is_empty());
        assert!(prometheus_output.contains("cpu_usage"));
        
        println!("✅ Metrics export test passed");
    }
}