//! 실시간 메트릭 수집 시스템
//! 
//! - 성능 메트릭 실시간 수집
//! - Prometheus 형식 메트릭
//! - 시스템 리소스 모니터링
//! - 자동화된 알림 시스템

use anyhow::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{info, warn};

/// 타입 별칭들
type TimeSeriesMap = DashMap<String, Vec<(u64, MetricValue)>>;

/// 메트릭 데이터 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram { sum: f64, count: u64, buckets: Vec<f64> },
    Summary { sum: f64, count: u64, quantiles: Vec<(f64, f64)> },
}

/// 메트릭 엔트리
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEntry {
    pub name: String,
    pub value: MetricValue,
    pub labels: std::collections::HashMap<String, String>,
    pub timestamp: u64,
    pub help: String,
}

/// 메트릭 수집기 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// 메트릭 수집 간격 (초)
    pub collection_interval_secs: u64,
    /// 메트릭 보관 기간 (초)
    pub retention_period_secs: u64,
    /// 시스템 메트릭 수집 활성화
    pub enable_system_metrics: bool,
    /// 성능 메트릭 수집 활성화
    pub enable_performance_metrics: bool,
    /// 네트워크 메트릭 수집 활성화
    pub enable_network_metrics: bool,
    /// 메트릭 압축 활성화
    pub enable_compression: bool,
    /// 알림 임계값 설정
    pub alert_thresholds: AlertThresholds,
}

/// 알림 임계값 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// CPU 사용률 임계값 (%)
    pub cpu_usage_threshold: f64,
    /// 메모리 사용률 임계값 (%)
    pub memory_usage_threshold: f64,
    /// 응답시간 임계값 (ms)
    pub response_time_threshold: f64,
    /// 에러율 임계값 (%)
    pub error_rate_threshold: f64,
    /// 연결 수 임계값
    pub connection_count_threshold: usize,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            collection_interval_secs: 10,
            retention_period_secs: 3600, // 1시간
            enable_system_metrics: true,
            enable_performance_metrics: true,
            enable_network_metrics: true,
            enable_compression: true,
            alert_thresholds: AlertThresholds {
                cpu_usage_threshold: 80.0,
                memory_usage_threshold: 85.0,
                response_time_threshold: 1000.0, // 1초
                error_rate_threshold: 5.0,
                connection_count_threshold: 1000,
            },
        }
    }
}

/// 시스템 리소스 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemResourceStats {
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_total_bytes: u64,
    pub memory_usage_percent: f64,
    pub disk_usage_bytes: u64,
    pub disk_total_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub open_file_descriptors: usize,
    pub thread_count: usize,
}

/// 실시간 메트릭 수집기
pub struct MetricsCollector {
    config: MetricsConfig,
    /// 메트릭 저장소 (이름 -> 메트릭)
    metrics: Arc<DashMap<String, MetricEntry>>,
    /// 시계열 데이터 (이름 -> 시간별 값들)
    time_series: Arc<RwLock<TimeSeriesMap>>,
    /// 성능 카운터들
    request_counter: AtomicU64,
    error_counter: AtomicU64,
    response_time_sum: AtomicU64, // 나노초 단위
    response_time_count: AtomicU64,
    active_connections: AtomicUsize,
    /// 시작 시간
    start_time: Instant,
}

impl MetricsCollector {
    /// 새 메트릭 수집기 생성
    pub fn new(config: MetricsConfig) -> Self {
        let collector = Self {
            config,
            metrics: Arc::new(DashMap::new()),
            time_series: Arc::new(RwLock::new(DashMap::new())),
            request_counter: AtomicU64::new(0),
            error_counter: AtomicU64::new(0),
            response_time_sum: AtomicU64::new(0),
            response_time_count: AtomicU64::new(0),
            active_connections: AtomicUsize::new(0),
            start_time: Instant::now(),
        };
        
        // 수집 작업 시작
        collector.start_collection_task();
        
        collector
    }
    
    /// 기본 설정으로 메트릭 수집기 생성
    pub fn with_default_config() -> Self {
        Self::new(MetricsConfig::default())
    }
    
    /// 카운터 메트릭 증가
    pub fn increment_counter(&self, name: &str, labels: std::collections::HashMap<String, String>) {
        let current_value = if let Some(entry) = self.metrics.get(name) {
            if let MetricValue::Counter(count) = entry.value {
                count + 1
            } else {
                1
            }
        } else {
            1
        };
        
        let metric = MetricEntry {
            name: name.to_string(),
            value: MetricValue::Counter(current_value),
            labels,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            help: format!("Counter for {}", name),
        };
        
        self.metrics.insert(name.to_string(), metric);
    }
    
    /// 게이지 메트릭 설정
    pub fn set_gauge(&self, name: &str, value: f64, labels: std::collections::HashMap<String, String>) {
        let metric = MetricEntry {
            name: name.to_string(),
            value: MetricValue::Gauge(value),
            labels,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            help: format!("Gauge for {}", name),
        };
        
        self.metrics.insert(name.to_string(), metric);
        
        // 시계열 데이터 저장
        tokio::spawn({
            let time_series = self.time_series.clone();
            let name = name.to_string();
            let value = MetricValue::Gauge(value);
            async move {
                let time_series = time_series.write().await;
                let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                
                let mut series = time_series.entry(name).or_insert_with(Vec::new);
                series.push((timestamp, value));
                
                // 오래된 데이터 정리 (1시간 초과)
                series.retain(|(ts, _)| timestamp - ts <= 3600);
            }
        });
    }
    
    /// 히스토그램 메트릭 관찰
    pub fn observe_histogram(&self, name: &str, value: f64, buckets: Vec<f64>, labels: std::collections::HashMap<String, String>) {
        let (sum, count) = if let Some(entry) = self.metrics.get(name) {
            if let MetricValue::Histogram { sum, count, .. } = entry.value {
                (sum + value, count + 1)
            } else {
                (value, 1)
            }
        } else {
            (value, 1)
        };
        
        let metric = MetricEntry {
            name: name.to_string(),
            value: MetricValue::Histogram { sum, count, buckets },
            labels,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            help: format!("Histogram for {}", name),
        };
        
        self.metrics.insert(name.to_string(), metric);
    }
    
    /// 요청 메트릭 기록
    pub fn record_request(&self, response_time: Duration, is_error: bool) {
        self.request_counter.fetch_add(1, Ordering::Relaxed);
        self.response_time_sum.fetch_add(response_time.as_nanos() as u64, Ordering::Relaxed);
        self.response_time_count.fetch_add(1, Ordering::Relaxed);
        
        if is_error {
            self.error_counter.fetch_add(1, Ordering::Relaxed);
        }
        
        // 자동 메트릭 업데이트
        self.increment_counter("requests_total", std::collections::HashMap::new());
        
        if is_error {
            self.increment_counter("errors_total", std::collections::HashMap::new());
        }
        
        // 응답시간 히스토그램
        let buckets = vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0];
        self.observe_histogram("response_time_seconds", response_time.as_secs_f64(), buckets, std::collections::HashMap::new());
    }
    
    /// 연결 수 업데이트
    pub fn set_active_connections(&self, count: usize) {
        self.active_connections.store(count, Ordering::Relaxed);
        self.set_gauge("active_connections", count as f64, std::collections::HashMap::new());
    }
    
    /// 시스템 리소스 통계 수집
    pub async fn collect_system_metrics(&self) -> Result<SystemResourceStats> {
        // 실제 구현에서는 sysinfo 크레이트 사용
        // 여기서는 Mock 데이터 반환
        let stats = SystemResourceStats {
            cpu_usage_percent: 15.5,
            memory_usage_bytes: 256_000_000, // 256MB
            memory_total_bytes: 8_589_934_592, // 8GB
            memory_usage_percent: 3.0,
            disk_usage_bytes: 1_073_741_824, // 1GB
            disk_total_bytes: 107_374_182_400, // 100GB
            network_rx_bytes: 1_048_576, // 1MB
            network_tx_bytes: 524_288, // 512KB
            open_file_descriptors: 128,
            thread_count: 16,
        };
        
        // 시스템 메트릭을 게이지로 저장
        self.set_gauge("cpu_usage_percent", stats.cpu_usage_percent, std::collections::HashMap::new());
        self.set_gauge("memory_usage_percent", stats.memory_usage_percent, std::collections::HashMap::new());
        self.set_gauge("memory_usage_bytes", stats.memory_usage_bytes as f64, std::collections::HashMap::new());
        self.set_gauge("open_file_descriptors", stats.open_file_descriptors as f64, std::collections::HashMap::new());
        self.set_gauge("thread_count", stats.thread_count as f64, std::collections::HashMap::new());
        
        // 알림 확인
        self.check_alerts(&stats).await;
        
        Ok(stats)
    }
    
    /// 알림 임계값 확인
    async fn check_alerts(&self, stats: &SystemResourceStats) {
        let thresholds = &self.config.alert_thresholds;
        
        if stats.cpu_usage_percent > thresholds.cpu_usage_threshold {
            warn!(
                "HIGH CPU USAGE ALERT: {:.1}% (threshold: {:.1}%)",
                stats.cpu_usage_percent,
                thresholds.cpu_usage_threshold
            );
        }
        
        if stats.memory_usage_percent > thresholds.memory_usage_threshold {
            warn!(
                "HIGH MEMORY USAGE ALERT: {:.1}% (threshold: {:.1}%)",
                stats.memory_usage_percent,
                thresholds.memory_usage_threshold
            );
        }
        
        let error_rate = self.calculate_error_rate();
        if error_rate > thresholds.error_rate_threshold {
            warn!(
                "HIGH ERROR RATE ALERT: {:.1}% (threshold: {:.1}%)",
                error_rate,
                thresholds.error_rate_threshold
            );
        }
        
        let avg_response_time = self.calculate_avg_response_time_ms();
        if avg_response_time > thresholds.response_time_threshold {
            warn!(
                "HIGH RESPONSE TIME ALERT: {:.1}ms (threshold: {:.1}ms)",
                avg_response_time,
                thresholds.response_time_threshold
            );
        }
        
        let active_conn = self.active_connections.load(Ordering::Relaxed);
        if active_conn > thresholds.connection_count_threshold {
            warn!(
                "HIGH CONNECTION COUNT ALERT: {} (threshold: {})",
                active_conn,
                thresholds.connection_count_threshold
            );
        }
    }
    
    /// 에러율 계산
    pub fn calculate_error_rate(&self) -> f64 {
        let total_requests = self.request_counter.load(Ordering::Relaxed);
        let total_errors = self.error_counter.load(Ordering::Relaxed);
        
        if total_requests == 0 {
            0.0
        } else {
            (total_errors as f64 / total_requests as f64) * 100.0
        }
    }
    
    /// 평균 응답시간 계산 (밀리초)
    pub fn calculate_avg_response_time_ms(&self) -> f64 {
        let total_time = self.response_time_sum.load(Ordering::Relaxed);
        let count = self.response_time_count.load(Ordering::Relaxed);
        
        if count == 0 {
            0.0
        } else {
            (total_time as f64 / count as f64) / 1_000_000.0 // 나노초 -> 밀리초
        }
    }
    
    /// 모든 메트릭 가져오기
    pub fn get_all_metrics(&self) -> Vec<MetricEntry> {
        self.metrics.iter().map(|entry| entry.value().clone()).collect()
    }
    
    /// 특정 메트릭 가져오기
    pub fn get_metric(&self, name: &str) -> Option<MetricEntry> {
        self.metrics.get(name).map(|entry| entry.value().clone())
    }
    
    /// Prometheus 형식으로 메트릭 내보내기
    pub fn export_prometheus_format(&self) -> String {
        let mut output = String::new();
        
        for entry in self.metrics.iter() {
            let metric = entry.value();
            
            // HELP 라인
            output.push_str(&format!("# HELP {} {}\n", metric.name, metric.help));
            
            // TYPE 라인
            let metric_type = match metric.value {
                MetricValue::Counter(_) => "counter",
                MetricValue::Gauge(_) => "gauge",
                MetricValue::Histogram { .. } => "histogram",
                MetricValue::Summary { .. } => "summary",
            };
            output.push_str(&format!("# TYPE {} {}\n", metric.name, metric_type));
            
            // 값 라인
            let labels_str = if metric.labels.is_empty() {
                String::new()
            } else {
                let labels: Vec<String> = metric.labels
                    .iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                    .collect();
                format!("{{{}}}", labels.join(","))
            };
            
            match &metric.value {
                MetricValue::Counter(value) => {
                    output.push_str(&format!("{}{} {}\n", metric.name, labels_str, value));
                }
                MetricValue::Gauge(value) => {
                    output.push_str(&format!("{}{} {}\n", metric.name, labels_str, value));
                }
                MetricValue::Histogram { sum, count, .. } => {
                    output.push_str(&format!("{}_sum{} {}\n", metric.name, labels_str, sum));
                    output.push_str(&format!("{}_count{} {}\n", metric.name, labels_str, count));
                }
                MetricValue::Summary { sum, count, .. } => {
                    output.push_str(&format!("{}_sum{} {}\n", metric.name, labels_str, sum));
                    output.push_str(&format!("{}_count{} {}\n", metric.name, labels_str, count));
                }
            }
            
            output.push('\n');
        }
        
        output
    }
    
    /// 수집 작업 시작
    fn start_collection_task(&self) {
        let config = self.config.clone();
        let metrics_ref = Arc::downgrade(&self.metrics);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.collection_interval_secs));
            
            loop {
                interval.tick().await;
                
                if let Some(_metrics) = metrics_ref.upgrade() {
                    // 업타임 업데이트
                    if let Some(_collector) = metrics_ref.upgrade() {
                        // 실제 구현에서는 collector 참조를 통해 업데이트
                        info!("메트릭 수집 작업 실행");
                    }
                } else {
                    break; // 메트릭 수집기가 드롭된 경우 종료
                }
            }
        });
    }
    
    /// 성능 요약 생성
    pub fn generate_performance_summary(&self) -> String {
        let uptime_secs = self.start_time.elapsed().as_secs();
        let total_requests = self.request_counter.load(Ordering::Relaxed);
        let total_errors = self.error_counter.load(Ordering::Relaxed);
        let active_conn = self.active_connections.load(Ordering::Relaxed);
        let error_rate = self.calculate_error_rate();
        let avg_response_time = self.calculate_avg_response_time_ms();
        
        format!(
            "📊 성능 요약\n\
            ═══════════════════════════════════════\n\
            🕐 업타임: {}초\n\
            📈 총 요청: {}\n\
            ❌ 총 에러: {} ({:.2}%)\n\
            🔗 활성 연결: {}\n\
            ⚡ 평균 응답시간: {:.2}ms\n\
            💾 메트릭 수: {}\n\
            ═══════════════════════════════════════",
            uptime_secs,
            total_requests,
            total_errors,
            error_rate,
            active_conn,
            avg_response_time,
            self.metrics.len()
        )
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::with_default_config()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_metrics_collection() {
        let collector = MetricsCollector::with_default_config();
        
        // 카운터 테스트
        collector.increment_counter("test_counter", std::collections::HashMap::new());
        let metric = collector.get_metric("test_counter").unwrap();
        assert!(matches!(metric.value, MetricValue::Counter(1)));
        
        // 게이지 테스트
        collector.set_gauge("test_gauge", 42.5, std::collections::HashMap::new());
        let metric = collector.get_metric("test_gauge").unwrap();
        assert!(matches!(metric.value, MetricValue::Gauge(val) if (val - 42.5).abs() < f64::EPSILON));
        
        // 요청 기록 테스트
        collector.record_request(Duration::from_millis(100), false);
        collector.record_request(Duration::from_millis(200), true);
        
        assert_eq!(collector.calculate_error_rate(), 50.0);
        assert!(collector.calculate_avg_response_time_ms() > 0.0);
    }
    
    #[tokio::test]
    async fn test_prometheus_export() {
        let collector = MetricsCollector::with_default_config();
        
        collector.increment_counter("test_requests_total", std::collections::HashMap::new());
        collector.set_gauge("test_memory_usage_bytes", 1024.0, std::collections::HashMap::new());
        
        let prometheus_output = collector.export_prometheus_format();
        
        assert!(prometheus_output.contains("# HELP test_requests_total"));
        assert!(prometheus_output.contains("# TYPE test_requests_total counter"));
        assert!(prometheus_output.contains("test_requests_total 1"));
        assert!(prometheus_output.contains("test_memory_usage_bytes 1024"));
    }
    
    #[test]
    fn test_performance_summary() {
        let collector = MetricsCollector::with_default_config();
        
        collector.record_request(Duration::from_millis(100), false);
        collector.set_active_connections(50);
        
        let summary = collector.generate_performance_summary();
        assert!(summary.contains("성능 요약"));
        assert!(summary.contains("활성 연결: 50"));
    }
}