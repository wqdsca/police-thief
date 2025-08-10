//! 성능 모니터링 및 프로파일링 도구
//! 
//! 시스템 전반의 성능을 실시간으로 모니터링하고 분석하는 통합 도구입니다.
//! CPU, 메모리, 네트워크, 레이턴시 등 다양한 메트릭을 수집하고 분석합니다.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::collections::{HashMap, VecDeque};
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::info;
use serde::{Serialize, Deserialize};
use sysinfo::{System, SystemExt, ProcessExt, CpuExt};

/// 메트릭 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricType {
    CpuUsage,
    MemoryUsage,
    NetworkThroughput,
    MessageLatency,
    ConnectionCount,
    RequestRate,
    ErrorRate,
    CacheHitRate,
    DiskIO,
    ThreadCount,
}

/// 메트릭 샘플
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    pub timestamp: u64,
    pub value: f64,
    pub metric_type: MetricType,
    pub labels: HashMap<String, String>,
}

impl MetricSample {
    pub fn new(metric_type: MetricType, value: f64) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            value,
            metric_type,
            labels: HashMap::new(),
        }
    }
    
    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }
}

/// 성능 모니터링 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMonitorConfig {
    /// 샘플링 간격 (밀리초)
    pub sampling_interval_ms: u64,
    /// 메트릭 보관 기간 (초)
    pub retention_period_secs: u64,
    /// 최대 샘플 수
    pub max_samples_per_metric: usize,
    /// CPU 프로파일링 활성화
    pub enable_cpu_profiling: bool,
    /// 메모리 프로파일링 활성화
    pub enable_memory_profiling: bool,
    /// 네트워크 프로파일링 활성화
    pub enable_network_profiling: bool,
    /// 경고 임계값
    pub alert_thresholds: HashMap<MetricType, f64>,
    /// 자동 보고서 생성 간격 (초)
    pub report_interval_secs: u64,
}

impl Default for PerformanceMonitorConfig {
    fn default() -> Self {
        let mut thresholds = HashMap::new();
        thresholds.insert(MetricType::CpuUsage, 80.0);
        thresholds.insert(MetricType::MemoryUsage, 90.0);
        thresholds.insert(MetricType::ErrorRate, 5.0);
        thresholds.insert(MetricType::MessageLatency, 100.0); // ms
        
        Self {
            sampling_interval_ms: 1000,
            retention_period_secs: 3600,
            max_samples_per_metric: 3600,
            enable_cpu_profiling: true,
            enable_memory_profiling: true,
            enable_network_profiling: true,
            alert_thresholds: thresholds,
            report_interval_secs: 60,
        }
    }
}

/// 히스토그램 (레이턴시 분포 추적)
#[derive(Debug)]
pub struct Histogram {
    buckets: Vec<(f64, AtomicU64)>, // (상한값, 카운트)
    sum: AtomicU64,
    count: AtomicU64,
}

impl Histogram {
    pub fn new(bucket_bounds: Vec<f64>) -> Self {
        let buckets = bucket_bounds
            .into_iter()
            .map(|bound| (bound, AtomicU64::new(0)))
            .collect();
        
        Self {
            buckets,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }
    
    pub fn observe(&self, value: f64) {
        // 적절한 버킷 찾기
        for (bound, counter) in &self.buckets {
            if value <= *bound {
                counter.fetch_add(1, Ordering::Relaxed);
                break;
            }
        }
        
        self.sum.fetch_add((value * 1000.0) as u64, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn percentile(&self, p: f64) -> f64 {
        let total = self.count.load(Ordering::Relaxed) as f64;
        if total == 0.0 {
            return 0.0;
        }
        
        let target = (total * p / 100.0) as u64;
        let mut cumulative = 0u64;
        
        for (bound, counter) in &self.buckets {
            cumulative += counter.load(Ordering::Relaxed);
            if cumulative >= target {
                return *bound;
            }
        }
        
        self.buckets.last().map(|(b, _)| *b).unwrap_or(0.0)
    }
    
    pub fn mean(&self) -> f64 {
        let count = self.count.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }
        
        let sum = self.sum.load(Ordering::Relaxed) as f64 / 1000.0;
        sum / count as f64
    }
}

/// 실시간 메트릭 수집기
pub struct MetricsCollector {
    /// 메트릭 버퍼
    metrics: Arc<RwLock<HashMap<MetricType, VecDeque<MetricSample>>>>,
    /// 히스토그램 (레이턴시 추적)
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
    /// 카운터
    counters: Arc<RwLock<HashMap<String, AtomicU64>>>,
    /// 게이지
    gauges: Arc<RwLock<HashMap<String, AtomicU64>>>,
    /// 시스템 정보
    system: Arc<Mutex<System>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            system: Arc::new(Mutex::new(system)),
        }
    }
    
    /// 메트릭 기록
    pub async fn record_metric(&self, metric_type: MetricType, value: f64, max_samples: usize) {
        let sample = MetricSample::new(metric_type, value);
        
        let mut metrics = self.metrics.write().await;
        let samples = metrics.entry(metric_type).or_insert_with(VecDeque::new);
        
        samples.push_back(sample);
        
        // 최대 샘플 수 제한
        while samples.len() > max_samples {
            samples.pop_front();
        }
    }
    
    /// 히스토그램에 값 기록
    pub async fn observe_histogram(&self, name: &str, value: f64) {
        let histograms = self.histograms.read().await;
        if let Some(histogram) = histograms.get(name) {
            histogram.observe(value);
        }
    }
    
    /// 카운터 증가
    pub async fn increment_counter(&self, name: &str, value: u64) {
        let counters = self.counters.read().await;
        if let Some(counter) = counters.get(name) {
            counter.fetch_add(value, Ordering::Relaxed);
        }
    }
    
    /// 게이지 설정
    pub async fn set_gauge(&self, name: &str, value: u64) {
        let gauges = self.gauges.read().await;
        if let Some(gauge) = gauges.get(name) {
            gauge.store(value, Ordering::Relaxed);
        }
    }
    
    /// CPU 사용률 수집
    pub async fn collect_cpu_usage(&self) -> f64 {
        let mut system = self.system.lock().await;
        system.refresh_cpu();
        
        let cpu_usage = system.global_cpu_info().cpu_usage() as f64;
        cpu_usage
    }
    
    /// 메모리 사용률 수집
    pub async fn collect_memory_usage(&self) -> f64 {
        let mut system = self.system.lock().await;
        system.refresh_memory();
        
        let total_memory = system.total_memory();
        let used_memory = system.used_memory();
        
        if total_memory > 0 {
            (used_memory as f64 / total_memory as f64) * 100.0
        } else {
            0.0
        }
    }
    
    /// 프로세스 정보 수집
    pub async fn collect_process_info(&self) -> ProcessInfo {
        let mut system = self.system.lock().await;
        system.refresh_processes();
        
        let pid = std::process::id();
        
        if let Some(process) = system.process(sysinfo::Pid::from(pid as usize)) {
            ProcessInfo {
                cpu_usage: process.cpu_usage() as f64,
                memory_usage: process.memory() as f64 / 1024.0 / 1024.0, // MB
                virtual_memory: process.virtual_memory() as f64 / 1024.0 / 1024.0, // MB
                thread_count: 0, // 스레드 수는 별도 계산 필요
                open_files: 0, // 열린 파일 수는 별도 계산 필요
            }
        } else {
            ProcessInfo::default()
        }
    }
    
    /// 메트릭 조회
    pub async fn get_metrics(&self, metric_type: MetricType) -> Vec<MetricSample> {
        let metrics = self.metrics.read().await;
        metrics.get(&metric_type)
            .map(|samples| samples.iter().cloned().collect())
            .unwrap_or_default()
    }
    
    /// 최근 메트릭 조회
    pub async fn get_latest_metric(&self, metric_type: MetricType) -> Option<f64> {
        let metrics = self.metrics.read().await;
        metrics.get(&metric_type)
            .and_then(|samples| samples.back())
            .map(|sample| sample.value)
    }
}

/// 프로세스 정보
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub cpu_usage: f64,
    pub memory_usage: f64, // MB
    pub virtual_memory: f64, // MB
    pub thread_count: usize,
    pub open_files: usize,
}

/// 성능 프로파일러
pub struct PerformanceProfiler {
    /// 함수별 실행 시간
    function_times: Arc<RwLock<HashMap<String, Vec<Duration>>>>,
    /// 호출 스택
    call_stack: Arc<Mutex<Vec<(String, Instant)>>>,
    /// 플레임 그래프 데이터
    flame_graph: Arc<RwLock<HashMap<String, FlameGraphNode>>>,
}

impl PerformanceProfiler {
    pub fn new() -> Self {
        Self {
            function_times: Arc::new(RwLock::new(HashMap::new())),
            call_stack: Arc::new(Mutex::new(Vec::new())),
            flame_graph: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 함수 시작 추적
    pub async fn start_timing(&self, function_name: String) {
        let mut stack = self.call_stack.lock().await;
        stack.push((function_name, Instant::now()));
    }
    
    /// 함수 종료 추적
    pub async fn end_timing(&self, function_name: &str) {
        let mut stack = self.call_stack.lock().await;
        
        if let Some(pos) = stack.iter().rposition(|(name, _)| name == function_name) {
            let (_, start_time) = stack.remove(pos);
            let duration = start_time.elapsed();
            
            let mut times = self.function_times.write().await;
            times.entry(function_name.to_string())
                .or_insert_with(Vec::new)
                .push(duration);
        }
    }
    
    /// 프로파일링 결과 조회
    pub async fn get_profile_results(&self) -> HashMap<String, ProfileResult> {
        let times = self.function_times.read().await;
        let mut results = HashMap::new();
        
        for (function_name, durations) in times.iter() {
            if durations.is_empty() {
                continue;
            }
            
            let total_time: Duration = durations.iter().sum();
            let avg_time = total_time / durations.len() as u32;
            let max_time = durations.iter().max().cloned().unwrap_or_default();
            let min_time = durations.iter().min().cloned().unwrap_or_default();
            
            results.insert(function_name.clone(), ProfileResult {
                function_name: function_name.clone(),
                call_count: durations.len(),
                total_time,
                avg_time,
                max_time,
                min_time,
            });
        }
        
        results
    }
}

/// 프로파일링 결과
#[derive(Debug, Clone)]
pub struct ProfileResult {
    pub function_name: String,
    pub call_count: usize,
    pub total_time: Duration,
    pub avg_time: Duration,
    pub max_time: Duration,
    pub min_time: Duration,
}

/// 플레임 그래프 노드
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlameGraphNode {
    pub name: String,
    pub value: u64, // 마이크로초
    pub children: Vec<FlameGraphNode>,
}

/// 통합 성능 모니터
pub struct PerformanceMonitor {
    config: PerformanceMonitorConfig,
    collector: Arc<MetricsCollector>,
    profiler: Arc<PerformanceProfiler>,
    alerts: Arc<Mutex<Vec<PerformanceAlert>>>,
    monitoring_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    reporting_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl PerformanceMonitor {
    /// 새 성능 모니터 생성
    pub async fn new(config: PerformanceMonitorConfig) -> Self {
        let collector = Arc::new(MetricsCollector::new());
        
        // 히스토그램 초기화
        let mut histograms = collector.histograms.write().await;
        histograms.insert(
            "message_latency".to_string(),
            Histogram::new(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]),
        );
        histograms.insert(
            "request_duration".to_string(),
            Histogram::new(vec![10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0]),
        );
        drop(histograms);
        
        // 카운터 초기화
        let mut counters = collector.counters.write().await;
        counters.insert("total_requests".to_string(), AtomicU64::new(0));
        counters.insert("total_errors".to_string(), AtomicU64::new(0));
        counters.insert("total_messages".to_string(), AtomicU64::new(0));
        drop(counters);
        
        // 게이지 초기화
        let mut gauges = collector.gauges.write().await;
        gauges.insert("active_connections".to_string(), AtomicU64::new(0));
        gauges.insert("memory_usage_mb".to_string(), AtomicU64::new(0));
        drop(gauges);
        
        let monitor = Self {
            config,
            collector,
            profiler: Arc::new(PerformanceProfiler::new()),
            alerts: Arc::new(Mutex::new(Vec::new())),
            monitoring_handle: Arc::new(Mutex::new(None)),
            reporting_handle: Arc::new(Mutex::new(None)),
        };
        
        monitor.start_monitoring().await;
        monitor
    }
    
    /// 모니터링 시작
    async fn start_monitoring(&self) {
        // 메트릭 수집 태스크
        let collector = self.collector.clone();
        let config = self.config.clone();
        let alerts = self.alerts.clone();
        
        let monitoring_handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.sampling_interval_ms));
            
            loop {
                interval.tick().await;
                
                // CPU 사용률 수집
                if config.enable_cpu_profiling {
                    let cpu_usage = collector.collect_cpu_usage().await;
                    collector.record_metric(
                        MetricType::CpuUsage,
                        cpu_usage,
                        config.max_samples_per_metric,
                    ).await;
                    
                    // 경고 확인
                    if let Some(&threshold) = config.alert_thresholds.get(&MetricType::CpuUsage) {
                        if cpu_usage > threshold {
                            let alert = PerformanceAlert {
                                timestamp: Instant::now(),
                                metric_type: MetricType::CpuUsage,
                                current_value: cpu_usage,
                                threshold,
                                message: format!("CPU 사용률이 {}%를 초과했습니다", threshold),
                            };
                            alerts.lock().await.push(alert);
                        }
                    }
                }
                
                // 메모리 사용률 수집
                if config.enable_memory_profiling {
                    let memory_usage = collector.collect_memory_usage().await;
                    collector.record_metric(
                        MetricType::MemoryUsage,
                        memory_usage,
                        config.max_samples_per_metric,
                    ).await;
                    
                    // 경고 확인
                    if let Some(&threshold) = config.alert_thresholds.get(&MetricType::MemoryUsage) {
                        if memory_usage > threshold {
                            let alert = PerformanceAlert {
                                timestamp: Instant::now(),
                                metric_type: MetricType::MemoryUsage,
                                current_value: memory_usage,
                                threshold,
                                message: format!("메모리 사용률이 {}%를 초과했습니다", threshold),
                            };
                            alerts.lock().await.push(alert);
                        }
                    }
                }
            }
        });
        
        *self.monitoring_handle.lock().await = Some(monitoring_handle);
        
        // 보고서 생성 태스크
        let collector = self.collector.clone();
        let config = self.config.clone();
        
        let reporting_handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.report_interval_secs));
            
            loop {
                interval.tick().await;
                
                // 성능 보고서 생성 및 로깅
                let cpu = collector.get_latest_metric(MetricType::CpuUsage).await.unwrap_or(0.0);
                let memory = collector.get_latest_metric(MetricType::MemoryUsage).await.unwrap_or(0.0);
                
                info!(
                    "📊 성능 보고서: CPU={:.1}%, 메모리={:.1}%",
                    cpu, memory
                );
            }
        });
        
        *self.reporting_handle.lock().await = Some(reporting_handle);
    }
    
    /// 레이턴시 기록
    pub async fn record_latency(&self, name: &str, latency: Duration) {
        let latency_ms = latency.as_secs_f64() * 1000.0;
        self.collector.observe_histogram(name, latency_ms).await;
        
        if name == "message_latency" {
            self.collector.record_metric(
                MetricType::MessageLatency,
                latency_ms,
                self.config.max_samples_per_metric,
            ).await;
        }
    }
    
    /// 요청 카운트 증가
    pub async fn increment_request_count(&self) {
        self.collector.increment_counter("total_requests", 1).await;
    }
    
    /// 에러 카운트 증가
    pub async fn increment_error_count(&self) {
        self.collector.increment_counter("total_errors", 1).await;
    }
    
    /// 연결 수 설정
    pub async fn set_connection_count(&self, count: usize) {
        self.collector.set_gauge("active_connections", count as u64).await;
        self.collector.record_metric(
            MetricType::ConnectionCount,
            count as f64,
            self.config.max_samples_per_metric,
        ).await;
    }
    
    /// 종합 성능 보고서 생성
    pub async fn generate_report(&self) -> PerformanceReport {
        let process_info = self.collector.collect_process_info().await;
        let profile_results = self.profiler.get_profile_results().await;
        let alerts = self.alerts.lock().await.clone();
        
        let histograms = self.collector.histograms.read().await;
        let latency_p50 = histograms.get("message_latency")
            .map(|h| h.percentile(50.0))
            .unwrap_or(0.0);
        let latency_p95 = histograms.get("message_latency")
            .map(|h| h.percentile(95.0))
            .unwrap_or(0.0);
        let latency_p99 = histograms.get("message_latency")
            .map(|h| h.percentile(99.0))
            .unwrap_or(0.0);
        
        let counters = self.collector.counters.read().await;
        let total_requests = counters.get("total_requests")
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0);
        let total_errors = counters.get("total_errors")
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0);
        
        PerformanceReport {
            timestamp: SystemTime::now(),
            cpu_usage: self.collector.get_latest_metric(MetricType::CpuUsage).await.unwrap_or(0.0),
            memory_usage: self.collector.get_latest_metric(MetricType::MemoryUsage).await.unwrap_or(0.0),
            process_info,
            latency_p50,
            latency_p95,
            latency_p99,
            total_requests,
            total_errors,
            error_rate: if total_requests > 0 {
                (total_errors as f64 / total_requests as f64) * 100.0
            } else {
                0.0
            },
            profile_results,
            alerts,
        }
    }
    
    /// 메트릭 수집기 접근
    pub fn collector(&self) -> Arc<MetricsCollector> {
        self.collector.clone()
    }
    
    /// 프로파일러 접근
    pub fn profiler(&self) -> Arc<PerformanceProfiler> {
        self.profiler.clone()
    }
}

/// 성능 경고
#[derive(Debug, Clone)]
pub struct PerformanceAlert {
    pub timestamp: Instant,
    pub metric_type: MetricType,
    pub current_value: f64,
    pub threshold: f64,
    pub message: String,
}

/// 종합 성능 보고서
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub timestamp: SystemTime,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub process_info: ProcessInfo,
    pub latency_p50: f64,
    pub latency_p95: f64,
    pub latency_p99: f64,
    pub total_requests: u64,
    pub total_errors: u64,
    pub error_rate: f64,
    pub profile_results: HashMap<String, ProfileResult>,
    pub alerts: Vec<PerformanceAlert>,
}

impl PerformanceReport {
    /// 성능 점수 계산 (0-100)
    pub fn performance_score(&self) -> f64 {
        let cpu_score = ((100.0 - self.cpu_usage) / 100.0 * 25.0).max(0.0);
        let memory_score = ((100.0 - self.memory_usage) / 100.0 * 25.0).max(0.0);
        let latency_score = (100.0 / self.latency_p95.max(1.0)).min(1.0) * 25.0;
        let error_score = ((100.0 - self.error_rate) / 100.0 * 25.0).max(0.0);
        
        cpu_score + memory_score + latency_score + error_score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_metrics_collector() {
        let collector = MetricsCollector::new();
        
        // CPU 사용률 수집
        let cpu_usage = collector.collect_cpu_usage().await;
        assert!(cpu_usage >= 0.0 && cpu_usage <= 100.0);
        
        // 메모리 사용률 수집
        let memory_usage = collector.collect_memory_usage().await;
        assert!(memory_usage >= 0.0 && memory_usage <= 100.0);
    }
    
    #[test]
    fn test_histogram() {
        let histogram = Histogram::new(vec![10.0, 50.0, 100.0, 500.0]);
        
        histogram.observe(5.0);
        histogram.observe(25.0);
        histogram.observe(75.0);
        histogram.observe(200.0);
        
        assert!(histogram.mean() > 0.0);
        assert!(histogram.percentile(50.0) > 0.0);
    }
    
    #[tokio::test]
    async fn test_performance_monitor() {
        let config = PerformanceMonitorConfig::default();
        let monitor = PerformanceMonitor::new(config).await;
        
        // 레이턴시 기록
        monitor.record_latency("test", Duration::from_millis(10)).await;
        
        // 카운터 증가
        monitor.increment_request_count().await;
        
        // 보고서 생성
        let report = monitor.generate_report().await;
        assert!(report.performance_score() >= 0.0);
    }
}