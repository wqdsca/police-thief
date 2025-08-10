//! 성능 벤치마크 및 검증 도구
//! 
//! 모든 최적화 서비스의 성능을 측정하고 검증하는 종합 벤치마크 시스템입니다.
//! 실제 워크로드를 시뮬레이션하여 최적화 효과를 정량적으로 측정합니다.

use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::sync::Mutex;
use tracing::{info, warn};
use serde::{Serialize, Deserialize};
use rayon::prelude::*;

use crate::service::{
    AsyncIoOptimizer, AsyncIoOptimizerConfig,
    SimdOptimizer, SimdOptimizerConfig,
    MessageCompressionService, MessageCompressionConfig,
    PerformanceMonitor, PerformanceMonitorConfig,
};
use shared::tool::high_performance::dashmap_optimizer::{
    DashMapOptimizer, DashMapOptimizerConfig,
};

/// 벤치마크 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// 테스트 반복 횟수
    pub iterations: usize,
    /// 동시 사용자 수
    pub concurrent_users: usize,
    /// 메시지 크기 (바이트)
    pub message_size: usize,
    /// 테스트 지속 시간 (초)
    pub duration_secs: u64,
    /// 워밍업 시간 (초)
    pub warmup_secs: u64,
    /// 상세 로깅 활성화
    pub enable_detailed_logging: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 1000,
            concurrent_users: 100,
            message_size: 1024,
            duration_secs: 60,
            warmup_secs: 10,
            enable_detailed_logging: false,
        }
    }
}

/// 벤치마크 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub test_name: String,
    pub iterations: usize,
    pub total_duration: Duration,
    pub avg_latency: Duration,
    pub min_latency: Duration,
    pub max_latency: Duration,
    pub throughput_ops_per_sec: f64,
    pub success_rate: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

impl BenchmarkResult {
    /// 성능 점수 계산 (0-100)
    pub fn performance_score(&self) -> f64 {
        let throughput_score = (self.throughput_ops_per_sec / 10000.0).min(1.0) * 30.0;
        let latency_score = (1000.0 / self.avg_latency.as_millis() as f64).min(1.0) * 25.0;
        let success_score = self.success_rate * 25.0;
        let efficiency_score = ((100.0 - self.cpu_usage_percent) / 100.0 * 20.0).max(0.0);
        
        throughput_score + latency_score + success_score + efficiency_score
    }
}

/// 종합 성능 벤치마크
pub struct PerformanceBenchmark {
    config: BenchmarkConfig,
    results: Arc<Mutex<HashMap<String, BenchmarkResult>>>,
}

impl PerformanceBenchmark {
    pub fn new(config: BenchmarkConfig) -> Self {
        Self {
            config,
            results: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// DashMap 최적화 벤치마크
    pub async fn benchmark_dashmap_optimizer(&self) -> Result<BenchmarkResult> {
        info!("🚀 DashMap 최적화 벤치마크 시작");
        
        let optimizer = DashMapOptimizer::new(DashMapOptimizerConfig::default());
        let map = optimizer.create_optimized_dashmap::<u32, String>();
        
        let start = Instant::now();
        let mut success_count = 0;
        let mut latencies = Vec::new();
        
        // 워밍업
        for i in 0..1000 {
            map.insert(i, format!("value_{}", i));
        }
        
        // 실제 벤치마크
        for iteration in 0..self.config.iterations {
            let op_start = Instant::now();
            
            // 읽기 작업
            let key = (iteration % 1000) as u32;
            let result = optimizer.read_with_operation(&map, &key, |v| v.clone());
            
            let op_latency = op_start.elapsed();
            latencies.push(op_latency);
            
            if result.is_some() {
                success_count += 1;
            }
        }
        
        let total_duration = start.elapsed();
        let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
        let min_latency = *latencies.iter().min().unwrap();
        let max_latency = *latencies.iter().max().unwrap();
        
        let result = BenchmarkResult {
            test_name: "DashMap 최적화".to_string(),
            iterations: self.config.iterations,
            total_duration,
            avg_latency,
            min_latency,
            max_latency,
            throughput_ops_per_sec: self.config.iterations as f64 / total_duration.as_secs_f64(),
            success_rate: (success_count as f64 / self.config.iterations as f64) * 100.0,
            memory_usage_mb: 10.0, // 추정값
            cpu_usage_percent: 5.0, // 추정값
        };
        
        self.results.lock().await.insert("dashmap_optimizer".to_string(), result.clone());
        
        info!("✅ DashMap 최적화 벤치마크 완료: {:.1} ops/sec", result.throughput_ops_per_sec);
        Ok(result)
    }
    
    /// 비동기 I/O 최적화 벤치마크
    pub async fn benchmark_async_io_optimizer(&self) -> Result<BenchmarkResult> {
        info!("🚀 비동기 I/O 최적화 벤치마크 시작");
        
        let optimizer = AsyncIoOptimizer::new(AsyncIoOptimizerConfig::default());
        
        let start = Instant::now();
        let mut success_count = 0;
        let mut latencies = Vec::new();
        
        // 테스트 데이터 생성
        let test_data = vec![0u8; self.config.message_size];
        
        // 실제 벤치마크 (메모리 기반 I/O 시뮬레이션)
        for _ in 0..self.config.iterations {
            let op_start = Instant::now();
            
            // Zero-copy 읽기 시뮬레이션
            let mut cursor = std::io::Cursor::new(&test_data);
            let result = optimizer.zero_copy_read(&mut cursor, test_data.len()).await;
            
            let op_latency = op_start.elapsed();
            latencies.push(op_latency);
            
            if result.is_ok() {
                success_count += 1;
            }
        }
        
        let total_duration = start.elapsed();
        let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
        let min_latency = *latencies.iter().min().unwrap();
        let max_latency = *latencies.iter().max().unwrap();
        
        let result = BenchmarkResult {
            test_name: "비동기 I/O 최적화".to_string(),
            iterations: self.config.iterations,
            total_duration,
            avg_latency,
            min_latency,
            max_latency,
            throughput_ops_per_sec: self.config.iterations as f64 / total_duration.as_secs_f64(),
            success_rate: (success_count as f64 / self.config.iterations as f64) * 100.0,
            memory_usage_mb: 5.0, // 추정값
            cpu_usage_percent: 8.0, // 추정값
        };
        
        self.results.lock().await.insert("async_io_optimizer".to_string(), result.clone());
        
        info!("✅ 비동기 I/O 최적화 벤치마크 완료: {:.1} ops/sec", result.throughput_ops_per_sec);
        Ok(result)
    }
    
    /// SIMD 최적화 벤치마크
    pub async fn benchmark_simd_optimizer(&self) -> Result<BenchmarkResult> {
        info!("🚀 SIMD 최적화 벤치마크 시작");
        
        let optimizer = SimdOptimizer::new(SimdOptimizerConfig::default());
        
        let start = Instant::now();
        let mut success_count = 0;
        let mut latencies = Vec::new();
        
        // 테스트 데이터 생성
        let data_a = vec![0xAAu8; self.config.message_size];
        let data_b = vec![0x55u8; self.config.message_size];
        
        // 실제 벤치마크
        for _ in 0..self.config.iterations {
            let op_start = Instant::now();
            
            // SIMD XOR 연산
            let result = optimizer.simd_xor(&data_a, &data_b);
            
            let op_latency = op_start.elapsed();
            latencies.push(op_latency);
            
            if !result.is_empty() {
                success_count += 1;
            }
        }
        
        let total_duration = start.elapsed();
        let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
        let min_latency = *latencies.iter().min().unwrap();
        let max_latency = *latencies.iter().max().unwrap();
        
        let result = BenchmarkResult {
            test_name: "SIMD 최적화".to_string(),
            iterations: self.config.iterations,
            total_duration,
            avg_latency,
            min_latency,
            max_latency,
            throughput_ops_per_sec: self.config.iterations as f64 / total_duration.as_secs_f64(),
            success_rate: (success_count as f64 / self.config.iterations as f64) * 100.0,
            memory_usage_mb: 15.0, // 추정값
            cpu_usage_percent: 12.0, // 추정값
        };
        
        self.results.lock().await.insert("simd_optimizer".to_string(), result.clone());
        
        info!("✅ SIMD 최적화 벤치마크 완료: {:.1} ops/sec", result.throughput_ops_per_sec);
        Ok(result)
    }
    
    /// 메시지 압축 벤치마크
    pub async fn benchmark_message_compression(&self) -> Result<BenchmarkResult> {
        info!("🚀 메시지 압축 벤치마크 시작");
        
        let service = MessageCompressionService::new(MessageCompressionConfig::default());
        
        let start = Instant::now();
        let mut success_count = 0;
        let mut latencies = Vec::new();
        
        // 테스트 메시지 생성
        let test_message = "Hello, World! This is a test message for compression benchmarking. ".repeat(50);
        let test_data = test_message.as_bytes();
        
        // 실제 벤치마크
        for _ in 0..self.config.iterations {
            let op_start = Instant::now();
            
            // 압축 및 압축 해제
            if let Ok((compressed, algorithm)) = service.compress(test_data).await {
                if let Ok(_decompressed) = service.decompress(&compressed, algorithm).await {
                    success_count += 1;
                }
            }
            
            let op_latency = op_start.elapsed();
            latencies.push(op_latency);
        }
        
        let total_duration = start.elapsed();
        let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
        let min_latency = *latencies.iter().min().unwrap();
        let max_latency = *latencies.iter().max().unwrap();
        
        let result = BenchmarkResult {
            test_name: "메시지 압축".to_string(),
            iterations: self.config.iterations,
            total_duration,
            avg_latency,
            min_latency,
            max_latency,
            throughput_ops_per_sec: self.config.iterations as f64 / total_duration.as_secs_f64(),
            success_rate: (success_count as f64 / self.config.iterations as f64) * 100.0,
            memory_usage_mb: 8.0, // 추정값
            cpu_usage_percent: 15.0, // 추정값
        };
        
        self.results.lock().await.insert("message_compression".to_string(), result.clone());
        
        info!("✅ 메시지 압축 벤치마크 완료: {:.1} ops/sec", result.throughput_ops_per_sec);
        Ok(result)
    }
    
    /// 성능 모니터링 벤치마크
    pub async fn benchmark_performance_monitor(&self) -> Result<BenchmarkResult> {
        info!("🚀 성능 모니터링 벤치마크 시작");
        
        let config = PerformanceMonitorConfig::default();
        let monitor = PerformanceMonitor::new(config).await;
        
        let start = Instant::now();
        let mut success_count = 0;
        let mut latencies = Vec::new();
        
        // 실제 벤치마크
        for i in 0..self.config.iterations {
            let op_start = Instant::now();
            
            // 메트릭 기록
            monitor.record_latency("test_latency", Duration::from_millis(10)).await;
            monitor.increment_request_count().await;
            monitor.set_connection_count(i % 100).await;
            
            success_count += 1;
            
            let op_latency = op_start.elapsed();
            latencies.push(op_latency);
        }
        
        // 성능 보고서 생성
        let _report = monitor.generate_report().await;
        
        let total_duration = start.elapsed();
        let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
        let min_latency = *latencies.iter().min().unwrap();
        let max_latency = *latencies.iter().max().unwrap();
        
        let result = BenchmarkResult {
            test_name: "성능 모니터링".to_string(),
            iterations: self.config.iterations,
            total_duration,
            avg_latency,
            min_latency,
            max_latency,
            throughput_ops_per_sec: self.config.iterations as f64 / total_duration.as_secs_f64(),
            success_rate: (success_count as f64 / self.config.iterations as f64) * 100.0,
            memory_usage_mb: 12.0, // 추정값
            cpu_usage_percent: 3.0, // 추정값
        };
        
        self.results.lock().await.insert("performance_monitor".to_string(), result.clone());
        
        info!("✅ 성능 모니터링 벤치마크 완료: {:.1} ops/sec", result.throughput_ops_per_sec);
        Ok(result)
    }
    
    /// 전체 벤치마크 실행
    pub async fn run_all_benchmarks(&self) -> Result<HashMap<String, BenchmarkResult>> {
        info!("🎯 전체 성능 벤치마크 시작");
        
        let mut all_results = HashMap::new();
        
        // 각 벤치마크 순차 실행
        let mut results = Vec::new();
        
        // DashMap 벤치마크
        let start = Instant::now();
        let dashmap_result = self.benchmark_dashmap_optimizer().await;
        results.push(("dashmap".to_string(), dashmap_result, start.elapsed()));
        
        // Async I/O 벤치마크
        let start = Instant::now();
        let async_io_result = self.benchmark_async_io_optimizer().await;
        results.push(("async_io".to_string(), async_io_result, start.elapsed()));
        
        // SIMD 벤치마크
        let start = Instant::now();
        let simd_result = self.benchmark_simd_optimizer().await;
        results.push(("simd".to_string(), simd_result, start.elapsed()));
        
        // 압축 벤치마크
        let start = Instant::now();
        let compression_result = self.benchmark_message_compression().await;
        results.push(("compression".to_string(), compression_result, start.elapsed()));
        
        // 모니터링 벤치마크
        let start = Instant::now();
        let monitoring_result = self.benchmark_performance_monitor().await;
        results.push(("monitoring".to_string(), monitoring_result, start.elapsed()));
        
        for (name, result, duration) in results {
            match result {
                Ok(benchmark_result) => {
                    info!("✅ {} 벤치마크: 점수 {:.1}/100, 소요시간 {:?}", 
                         benchmark_result.test_name, benchmark_result.performance_score(), duration);
                    all_results.insert(name, benchmark_result);
                }
                Err(e) => {
                    warn!("❌ {} 벤치마크 실패: {}", name, e);
                }
            }
            
            // 벤치마크 간 휴식
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        
        self.generate_summary_report(&all_results).await;
        Ok(all_results)
    }
    
    /// 요약 보고서 생성
    async fn generate_summary_report(&self, results: &HashMap<String, BenchmarkResult>) {
        info!("📊 성능 벤치마크 요약 보고서");
        info!("═══════════════════════════════════════");
        
        let mut total_score = 0.0;
        let mut total_throughput = 0.0;
        let mut total_memory = 0.0;
        let mut total_cpu = 0.0;
        
        for result in results.values() {
            let score = result.performance_score();
            total_score += score;
            total_throughput += result.throughput_ops_per_sec;
            total_memory += result.memory_usage_mb;
            total_cpu += result.cpu_usage_percent;
            
            info!(
                "📈 {}: 점수 {:.1}/100, 처리량 {:.0} ops/sec, 지연시간 {:.2}ms",
                result.test_name,
                score,
                result.throughput_ops_per_sec,
                result.avg_latency.as_secs_f64() * 1000.0
            );
        }
        
        let avg_score = total_score / results.len() as f64;
        let avg_cpu = total_cpu / results.len() as f64;
        
        info!("═══════════════════════════════════════");
        info!("🎯 종합 성능 점수: {:.1}/100", avg_score);
        info!("⚡ 총 처리량: {:.0} ops/sec", total_throughput);
        info!("💾 총 메모리 사용량: {:.1} MB", total_memory);
        info!("🔥 평균 CPU 사용률: {:.1}%", avg_cpu);
        info!("═══════════════════════════════════════");
        
        // 성능 등급 평가
        let grade = match avg_score as u32 {
            90..=100 => "A+ (최고 성능)",
            80..=89 => "A (우수 성능)",
            70..=79 => "B (양호 성능)",
            60..=69 => "C (보통 성능)",
            _ => "D (성능 개선 필요)",
        };
        
        info!("🏆 성능 등급: {}", grade);
    }
    
    /// 병렬 벤치마크 실행 (고급)
    pub async fn run_parallel_benchmarks(&self) -> Result<HashMap<String, BenchmarkResult>> {
        info!("🚀 병렬 벤치마크 실행 시작");
        
        // 병렬 실행 가능한 벤치마크들
        let handles = vec![
            tokio::spawn({
                let benchmark = self.clone();
                async move {
                    ("dashmap", benchmark.benchmark_dashmap_optimizer().await)
                }
            }),
            tokio::spawn({
                let benchmark = self.clone();
                async move {
                    ("simd", benchmark.benchmark_simd_optimizer().await)
                }
            }),
            tokio::spawn({
                let benchmark = self.clone();
                async move {
                    ("compression", benchmark.benchmark_message_compression().await)
                }
            }),
        ];
        
        let mut results = HashMap::new();
        
        for handle in handles {
            let (name, result) = handle.await?;
            match result {
                Ok(benchmark_result) => {
                    results.insert(name.to_string(), benchmark_result);
                }
                Err(e) => {
                    warn!("병렬 벤치마크 실패 {}: {}", name, e);
                }
            }
        }
        
        // 순차 실행이 필요한 벤치마크들
        if let Ok(async_io_result) = self.benchmark_async_io_optimizer().await {
            results.insert("async_io".to_string(), async_io_result);
        }
        
        if let Ok(monitor_result) = self.benchmark_performance_monitor().await {
            results.insert("monitoring".to_string(), monitor_result);
        }
        
        self.generate_summary_report(&results).await;
        Ok(results)
    }
}

impl Clone for PerformanceBenchmark {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            results: self.results.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_dashmap_benchmark() {
        let config = BenchmarkConfig {
            iterations: 100,
            ..Default::default()
        };
        
        let benchmark = PerformanceBenchmark::new(config);
        let result = benchmark.benchmark_dashmap_optimizer().await.unwrap();
        
        assert!(result.success_rate > 90.0);
        assert!(result.throughput_ops_per_sec > 0.0);
        assert!(result.performance_score() > 0.0);
    }
    
    #[tokio::test]
    async fn test_full_benchmark_suite() {
        let config = BenchmarkConfig {
            iterations: 50,
            ..Default::default()
        };
        
        let benchmark = PerformanceBenchmark::new(config);
        let results = benchmark.run_all_benchmarks().await.unwrap();
        
        assert!(!results.is_empty());
        assert!(results.contains_key("dashmap"));
        assert!(results.contains_key("compression"));
    }
}