//! Server Performance Benchmarks
//!
//! Specialized benchmark suite for measuring and validating server performance:
//! - Individual server performance benchmarks
//! - Comparative analysis across servers
//! - Optimization effectiveness measurement
//! - Performance regression detection

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use shared::tool::high_performance::*;

/// Benchmark configuration for consistent testing
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub message_sizes: Vec<usize>,
    pub connection_counts: Vec<usize>,
    pub test_iterations: usize,
    pub warmup_iterations: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            message_sizes: vec![64, 256, 512, 1024, 2048, 4096],
            connection_counts: vec![1, 10, 50, 100, 250, 500],
            test_iterations: 1000,
            warmup_iterations: 100,
        }
    }
}

/// Benchmark the gRPC server optimized controller
fn benchmark_grpc_server_optimized(c: &mut Criterion) {
    let rt = Runtime::new().expect("Safe unwrap");
    let config = BenchmarkConfig::default();
    
    // Initialize optimized components
    let stats = Arc::new(AtomicStats::new());
    let metrics = Arc::new(MetricsCollector::new());
    let compression_config = MessageCompressionConfig {
        algorithm: CompressionAlgorithm::Adaptive,
        compression_threshold: 256,
        compression_level: 3,
        enable_batching: false,
        batch_size: 1,
        batch_timeout_ms: 0,
        max_batch_bytes: 0,
        enable_compression_cache: true,
        cache_ttl_secs: 300,
    };
    let compression = Arc::new(MessageCompression::new(compression_config));
    let safe_ops = Arc::new(SafePrimitives::new());

    let mut group = c.benchmark_group("grpc_server_optimized");
    
    for message_size in &config.message_sizes {
        group.throughput(Throughput::Bytes(*message_size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("login_request_optimized", message_size),
            message_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let start = Instant::now();
                    
                    // Simulate optimized login request processing
                    let login_data = generate_test_login_data(size);
                    
                    // JWT verification with metrics
                    stats.increment_counter("jwt_verifications");
                    simulate_jwt_verification().await;
                    
                    // Enhanced validation with safe operations
                    let _valid = safe_ops.safe_string_check(&login_data.login_type, 1, 20);
                    let _valid_token = safe_ops.safe_string_check(&login_data.login_token, 1, 1000);
                    
                    // Business logic processing
                    simulate_user_service_call().await;
                    
                    // Record metrics
                    let duration = start.elapsed();
                    metrics.record_operation_duration("login_processing", duration);
                    stats.increment_counter("successful_logins");
                    
                    black_box(duration)
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("register_request_optimized", message_size),
            message_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let start = Instant::now();
                    
                    // Simulate optimized register request processing
                    let register_data = generate_test_register_data(size);
                    
                    // Enhanced validation
                    let _valid_type = safe_ops.safe_string_check(&register_data.login_type, 1, 20);
                    let _valid_token = safe_ops.safe_string_check(&register_data.login_token, 1, 1000);
                    let _valid_nick = safe_ops.safe_string_check(&register_data.nick_name, 1, 20);
                    
                    // Business logic processing
                    simulate_user_registration().await;
                    
                    // Record metrics
                    let duration = start.elapsed();
                    metrics.record_operation_duration("register_processing", duration);
                    stats.increment_counter("successful_registers");
                    
                    black_box(duration)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark the TCP server performance manager
fn benchmark_tcp_server_performance_manager(c: &mut Criterion) {
    let rt = Runtime::new().expect("Safe unwrap");
    let config = BenchmarkConfig::default();
    
    let mut group = c.benchmark_group("tcp_server_performance_manager");
    group.sample_size(200); // More samples for TCP server
    
    for message_size in &config.message_sizes {
        group.throughput(Throughput::Bytes(*message_size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("message_processing_optimized", message_size),
            message_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    // Initialize performance manager
                    let stats = Arc::new(AtomicStats::new());
                    let simd_optimizer = Arc::new(SIMDOptimizer::new());
                    let memory_pool = Arc::new(
                        EnhancedMemoryPool::with_capacity(1000, 100).await.expect("Safe unwrap")
                    );
                    let metrics = Arc::new(MetricsCollector::new());
                    
                    let start = Instant::now();
                    let test_data = generate_test_message_data(size);
                    
                    // SIMD-accelerated message processing
                    let processed = simd_optimizer.process_data_parallel(&test_data).await.expect("Safe unwrap");
                    
                    // Memory pool allocation for connection object
                    let _connection_obj = memory_pool.allocate::<String>().await.expect("Safe unwrap");
                    
                    // Simulate message handling
                    simulate_tcp_message_handling(&processed).await;
                    
                    // Record metrics
                    let duration = start.elapsed();
                    metrics.record_operation_duration("message_processing", duration);
                    stats.increment_counter("messages_processed");
                    
                    black_box(duration)
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("high_priority_task_scheduling", message_size),
            message_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    // Initialize task scheduler
                    let task_scheduler = Arc::new(
                        AsyncTaskScheduler::new(2, num_cpus::get(), 2).await.expect("Safe unwrap")
                    );
                    
                    let start = Instant::now();
                    
                    // Schedule high-priority task
                    let task_data = generate_test_message_data(size);
                    task_scheduler.schedule_task(
                        move || async move {
                            simulate_high_priority_processing(&task_data).await;
                            Ok(())
                        },
                        TaskPriority::High
                    ).await.expect("Safe unwrap");
                    
                    let duration = start.elapsed();
                    black_box(duration)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark the QUIC server performance suite
fn benchmark_quic_server_performance_suite(c: &mut Criterion) {
    let rt = Runtime::new().expect("Safe unwrap");
    let config = BenchmarkConfig::default();
    
    let mut group = c.benchmark_group("quic_server_performance_suite");
    group.sample_size(150); // Balanced samples for QUIC
    
    for message_size in &config.message_sizes {
        group.throughput(Throughput::Bytes(*message_size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("packet_processing_optimized", message_size),
            message_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    // Initialize QUIC performance components
                    let stats = Arc::new(AtomicStats::new());
                    let simd_optimizer = Arc::new(SIMDOptimizer::new());
                    let memory_pool = Arc::new(
                        EnhancedMemoryPool::with_capacity(5000, 500).await.expect("Safe unwrap")
                    );
                    let compression_config = MessageCompressionConfig {
                        algorithm: CompressionAlgorithm::LZ4,
                        compression_threshold: 512,
                        compression_level: 1,
                        enable_batching: true,
                        batch_size: 10,
                        batch_timeout_ms: 2,
                        max_batch_bytes: 32 * 1024,
                        enable_compression_cache: true,
                        cache_ttl_secs: 300,
                    };
                    let compression = Arc::new(MessageCompression::new(compression_config));
                    let metrics = Arc::new(MetricsCollector::new());
                    
                    let start = Instant::now();
                    let packet_data = generate_test_packet_data(size);
                    
                    // SIMD-accelerated packet parsing
                    let parsed_data = simd_optimizer.process_data_parallel(&packet_data).await.expect("Safe unwrap");
                    
                    // Compress if beneficial
                    let final_data = if parsed_data.len() > 512 {
                        compression.compress_data(&parsed_data).await.expect("Safe unwrap")
                    } else {
                        parsed_data
                    };
                    
                    // Allocate stream resources
                    let _stream_buffer = memory_pool.allocate::<Vec<u8>>().await.expect("Safe unwrap");
                    
                    // Record metrics
                    let duration = start.elapsed();
                    metrics.record_operation_duration("packet_processing", duration);
                    stats.increment_counter("packets_processed");
                    
                    black_box((final_data, duration))
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("stream_multiplexing", message_size),
            message_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    // Initialize parallel processor
                    let parallel_processor = Arc::new(
                        ParallelProcessor::new(num_cpus::get() * 2, true).await.expect("Safe unwrap")
                    );
                    let stats = Arc::new(AtomicStats::new());
                    let metrics = Arc::new(MetricsCollector::new());
                    
                    let start = Instant::now();
                    
                    // Create multiple streams
                    let streams: Vec<(u64, Vec<u8>)> = (0..10)
                        .map(|i| (i, generate_test_stream_data(size)))
                        .collect();
                    
                    // Process streams in parallel
                    let results = parallel_processor.process_items_parallel(
                        streams,
                        |item| async move {
                            let (stream_id, data) = item;
                            simulate_stream_processing(&data).await;
                            Ok((stream_id, data.len()))
                        }
                    ).await.expect("Safe unwrap");
                    
                    // Record metrics
                    let duration = start.elapsed();
                    metrics.record_operation_duration("stream_multiplexing", duration);
                    stats.add_value("concurrent_streams_processed", results.len() as u64);
                    
                    black_box((results, duration))
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark optimization effectiveness comparison
fn benchmark_optimization_effectiveness(c: &mut Criterion) {
    let rt = Runtime::new().expect("Safe unwrap");
    
    let mut group = c.benchmark_group("optimization_effectiveness");
    
    // Compare SIMD vs non-SIMD processing
    group.bench_function("simd_vs_standard_processing", |b| {
        b.to_async(&rt).iter(|| async {
            let data = generate_test_message_data(1024);
            
            // Standard processing
            let start_standard = Instant::now();
            let _standard_result = simulate_standard_processing(&data).await;
            let standard_duration = start_standard.elapsed();
            
            // SIMD processing
            let simd_optimizer = SIMDOptimizer::new();
            let start_simd = Instant::now();
            let _simd_result = simd_optimizer.process_data_parallel(&data).await.expect("Safe unwrap");
            let simd_duration = start_simd.elapsed();
            
            let speedup = standard_duration.as_nanos() as f64 / simd_duration.as_nanos() as f64;
            black_box((standard_duration, simd_duration, speedup))
        });
    });
    
    // Compare with/without memory pooling
    group.bench_function("memory_pool_vs_standard_allocation", |b| {
        b.to_async(&rt).iter(|| async {
            // Standard allocation
            let start_standard = Instant::now();
            for _ in 0..100 {
                let _obj = String::with_capacity(1024);
            }
            let standard_duration = start_standard.elapsed();
            
            // Memory pool allocation
            let memory_pool = EnhancedMemoryPool::with_capacity(200, 50).await.expect("Safe unwrap");
            let start_pool = Instant::now();
            for _ in 0..100 {
                let _obj = memory_pool.allocate::<String>().await.expect("Safe unwrap");
            }
            let pool_duration = start_pool.elapsed();
            
            let efficiency = standard_duration.as_nanos() as f64 / pool_duration.as_nanos() as f64;
            black_box((standard_duration, pool_duration, efficiency))
        });
    });
    
    // Compare compression effectiveness
    group.bench_function("compression_vs_no_compression", |b| {
        b.to_async(&rt).iter(|| async {
            let large_data = "x".repeat(2048).into_bytes();
            
            // No compression
            let start_no_compression = Instant::now();
            let _uncompressed_size = large_data.len();
            let no_compression_duration = start_no_compression.elapsed();
            
            // With compression
            let compression_config = MessageCompressionConfig {
                algorithm: CompressionAlgorithm::LZ4,
                compression_threshold: 100,
                compression_level: 1,
                enable_batching: false,
                batch_size: 1,
                batch_timeout_ms: 0,
                max_batch_bytes: 0,
                enable_compression_cache: true,
                cache_ttl_secs: 300,
            };
            let compression = MessageCompression::new(compression_config);
            
            let start_compression = Instant::now();
            let compressed = compression.compress_data(&large_data).await.expect("Safe unwrap");
            let compression_duration = start_compression.elapsed();
            
            let compression_ratio = large_data.len() as f64 / compressed.len() as f64;
            black_box((no_compression_duration, compression_duration, compression_ratio))
        });
    });
    
    group.finish();
}

/// Benchmark cross-server performance comparison
fn benchmark_cross_server_comparison(c: &mut Criterion) {
    let rt = Runtime::new().expect("Safe unwrap");
    
    let mut group = c.benchmark_group("cross_server_comparison");
    group.sample_size(100);
    
    let message_size = 1024;
    
    group.bench_function("grpc_vs_tcp_vs_quic_processing", |b| {
        b.to_async(&rt).iter(|| async {
            let test_data = generate_test_message_data(message_size);
            
            // gRPC-style processing
            let start_grpc = Instant::now();
            simulate_grpc_processing(&test_data).await;
            let grpc_duration = start_grpc.elapsed();
            
            // TCP-style processing
            let start_tcp = Instant::now();
            simulate_tcp_processing(&test_data).await;
            let tcp_duration = start_tcp.elapsed();
            
            // QUIC-style processing
            let start_quic = Instant::now();
            simulate_quic_processing(&test_data).await;
            let quic_duration = start_quic.elapsed();
            
            black_box((grpc_duration, tcp_duration, quic_duration))
        });
    });
    
    group.finish();
}

// Helper functions for test data generation and simulation

#[derive(Debug)]
struct TestLoginData {
    login_type: String,
    login_token: String,
}

#[derive(Debug)]
struct TestRegisterData {
    login_type: String,
    login_token: String,
    nick_name: String,
}

fn generate_test_login_data(size: usize) -> TestLoginData {
    TestLoginData {
        login_type: "test".to_string(),
        login_token: "x".repeat(size.min(1000)),
    }
}

fn generate_test_register_data(size: usize) -> TestRegisterData {
    TestRegisterData {
        login_type: "test".to_string(),
        login_token: "x".repeat(size.min(1000)),
        nick_name: "testuser".to_string(),
    }
}

fn generate_test_message_data(size: usize) -> Vec<u8> {
    vec![42u8; size]
}

fn generate_test_packet_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

fn generate_test_stream_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| ((i * 7) % 256) as u8).collect()
}

async fn simulate_jwt_verification() {
    tokio::time::sleep(Duration::from_micros(25)).await;
}

async fn simulate_user_service_call() {
    tokio::time::sleep(Duration::from_micros(50)).await;
}

async fn simulate_user_registration() {
    tokio::time::sleep(Duration::from_micros(75)).await;
}

async fn simulate_tcp_message_handling(_data: &[u8]) {
    tokio::time::sleep(Duration::from_micros(10)).await;
}

async fn simulate_high_priority_processing(_data: &[u8]) {
    tokio::time::sleep(Duration::from_micros(5)).await;
}

async fn simulate_stream_processing(_data: &[u8]) {
    tokio::time::sleep(Duration::from_micros(8)).await;
}

async fn simulate_standard_processing(data: &[u8]) -> Vec<u8> {
    tokio::time::sleep(Duration::from_micros(data.len() / 100)).await;
    data.to_vec()
}

async fn simulate_grpc_processing(_data: &[u8]) {
    tokio::time::sleep(Duration::from_micros(75)).await;
}

async fn simulate_tcp_processing(_data: &[u8]) {
    tokio::time::sleep(Duration::from_micros(10)).await;
}

async fn simulate_quic_processing(_data: &[u8]) {
    tokio::time::sleep(Duration::from_micros(15)).await;
}

criterion_group!(
    benches,
    benchmark_grpc_server_optimized,
    benchmark_tcp_server_performance_manager,
    benchmark_quic_server_performance_suite,
    benchmark_optimization_effectiveness,
    benchmark_cross_server_comparison
);
criterion_main!(benches);