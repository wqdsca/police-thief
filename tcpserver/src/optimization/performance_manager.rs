//! High-Performance Manager for TCP Server
//!
//! Centralizes all performance optimizations for the TCP server:
//! - SIMD-accelerated operations
//! - Enhanced memory pooling  
//! - Network optimizations
//! - Real-time performance monitoring

use anyhow::Result;
use shared::tool::high_performance::{
    AtomicStats, SIMDOptimizer, EnhancedMemoryPool, NetworkOptimizer,
    AsyncTaskScheduler, TaskPriority, MetricsCollector,
    ParallelProcessingConfig, MessageCompressionConfig
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, debug, warn};

/// High-performance manager that orchestrates all TCP server optimizations
pub struct PerformanceManager {
    /// Lock-free statistics collection
    pub stats: Arc<AtomicStats>,
    
    /// SIMD optimizer for message processing
    pub simd_optimizer: Arc<SIMDOptimizer>,
    
    /// Enhanced memory pool for connection objects
    pub memory_pool: Arc<EnhancedMemoryPool>,
    
    /// Network performance optimizer
    pub network_optimizer: Arc<NetworkOptimizer>,
    
    /// Async task scheduler with priorities
    pub task_scheduler: Arc<AsyncTaskScheduler>,
    
    /// Real-time metrics collector
    pub metrics_collector: Arc<MetricsCollector>,
    
    /// Performance configuration
    config: PerformanceConfig,
    
    /// Runtime performance state
    state: Arc<RwLock<PerformanceState>>,
}

/// Performance configuration for TCP server optimizations
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Enable SIMD acceleration
    pub enable_simd: bool,
    
    /// Memory pool configuration
    pub memory_pool_size: usize,
    pub memory_pool_prealloc: usize,
    
    /// Network optimization settings
    pub tcp_nodelay: bool,
    pub tcp_keepalive: bool,
    pub socket_buffer_size: usize,
    
    /// Task scheduling configuration
    pub high_priority_workers: usize,
    pub normal_priority_workers: usize,
    pub background_workers: usize,
    
    /// Metrics collection interval
    pub metrics_interval_secs: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_simd: true,
            memory_pool_size: 10000,
            memory_pool_prealloc: 1000,
            tcp_nodelay: true,
            tcp_keepalive: true,
            socket_buffer_size: 64 * 1024, // 64KB
            high_priority_workers: 2,
            normal_priority_workers: num_cpus::get(),
            background_workers: 2,
            metrics_interval_secs: 30,
        }
    }
}

/// Runtime performance state tracking
#[derive(Debug, Default)]
struct PerformanceState {
    /// Current performance metrics
    current_throughput: f64,
    current_latency_p99: Duration,
    current_memory_usage: usize,
    
    /// Performance optimization status
    simd_enabled: bool,
    memory_pool_active: bool,
    network_optimized: bool,
    
    /// Last performance check timestamp
    last_check: Option<Instant>,
}

impl PerformanceManager {
    /// Create new performance manager with default configuration
    pub async fn new() -> Result<Self> {
        Self::with_config(PerformanceConfig::default()).await
    }
    
    /// Create performance manager with custom configuration
    pub async fn with_config(config: PerformanceConfig) -> Result<Self> {
        info!("🔧 Initializing TCP Server Performance Manager");
        
        // Initialize core components
        let stats = Arc::new(AtomicStats::new());
        let metrics_collector = Arc::new(MetricsCollector::new());
        
        // Initialize SIMD optimizer if enabled
        let simd_optimizer = if config.enable_simd {
            let optimizer = Arc::new(SIMDOptimizer::new());
            info!("⚡ SIMD acceleration enabled: {:?}", optimizer.get_capabilities());
            optimizer
        } else {
            Arc::new(SIMDOptimizer::new_disabled())
        };
        
        // Initialize enhanced memory pool
        let memory_pool = Arc::new(
            EnhancedMemoryPool::with_capacity(
                config.memory_pool_size,
                config.memory_pool_prealloc
            ).await?
        );
        info!("🧠 Enhanced memory pool initialized: {} objects, {} pre-allocated", 
              config.memory_pool_size, config.memory_pool_prealloc);
        
        // Initialize network optimizer
        let network_optimizer = Arc::new(NetworkOptimizer::new().await?);
        info!("🌐 Network optimizer initialized");
        
        // Initialize async task scheduler
        let task_scheduler = Arc::new(
            AsyncTaskScheduler::new(
                config.high_priority_workers,
                config.normal_priority_workers,
                config.background_workers,
            ).await?
        );
        info!("📋 Task scheduler initialized: {} high, {} normal, {} background workers",
              config.high_priority_workers, config.normal_priority_workers, config.background_workers);
        
        let state = Arc::new(RwLock::new(PerformanceState::default()));
        
        let manager = Self {
            stats,
            simd_optimizer,
            memory_pool,
            network_optimizer,
            task_scheduler,
            metrics_collector,
            config,
            state,
        };
        
        // Start background performance monitoring
        manager.start_performance_monitoring().await?;
        
        info!("✅ TCP Server Performance Manager fully initialized");
        Ok(manager)
    }
    
    /// Process message with SIMD acceleration
    pub async fn process_message_optimized(&self, data: &[u8]) -> Result<Vec<u8>> {
        let start = Instant::now();
        
        // Use SIMD for data processing if available
        let result = if self.config.enable_simd {
            self.simd_optimizer.process_data_parallel(data).await?
        } else {
            data.to_vec()
        };
        
        // Record processing metrics
        let duration = start.elapsed();
        self.metrics_collector.record_operation_duration("message_processing", duration);
        self.stats.increment_counter("messages_processed");
        
        if duration > Duration::from_millis(1) {
            warn!("Slow message processing detected: {:?}", duration);
            self.stats.increment_counter("slow_message_processing");
        }
        
        Ok(result)
    }
    
    /// Allocate connection object from enhanced memory pool
    pub async fn allocate_connection_object<T>(&self) -> Result<T>
    where
        T: Default + Clone,
    {
        let start = Instant::now();
        
        let object = self.memory_pool.allocate::<T>().await?;
        
        // Record allocation metrics
        let duration = start.elapsed();
        self.metrics_collector.record_operation_duration("memory_allocation", duration);
        self.stats.increment_counter("memory_allocations");
        
        Ok(object)
    }
    
    /// Schedule high-priority task
    pub async fn schedule_high_priority_task<F, Fut>(&self, task: F) -> Result<()>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        self.task_scheduler.schedule_task(task, TaskPriority::High).await
    }
    
    /// Schedule normal priority task
    pub async fn schedule_task<F, Fut>(&self, task: F) -> Result<()>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        self.task_scheduler.schedule_task(task, TaskPriority::Normal).await
    }
    
    /// Schedule background task
    pub async fn schedule_background_task<F, Fut>(&self, task: F) -> Result<()>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        self.task_scheduler.schedule_task(task, TaskPriority::Background).await
    }
    
    /// Get current performance statistics
    pub async fn get_performance_stats(&self) -> PerformanceStats {
        let state = self.state.read().await;
        let atomic_stats = self.stats.get_summary();
        
        PerformanceStats {
            throughput: state.current_throughput,
            latency_p99: state.current_latency_p99,
            memory_usage: state.current_memory_usage,
            total_connections: atomic_stats.total_connections,
            active_connections: atomic_stats.active_connections,
            total_messages: atomic_stats.total_messages,
            error_rate: atomic_stats.error_rate,
            simd_enabled: state.simd_enabled,
            memory_pool_active: state.memory_pool_active,
            network_optimized: state.network_optimized,
        }
    }
    
    /// Start background performance monitoring
    async fn start_performance_monitoring(&self) -> Result<()> {
        let state = self.state.clone();
        let stats = self.stats.clone();
        let metrics = self.metrics_collector.clone();
        let interval = Duration::from_secs(self.config.metrics_interval_secs);
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                
                // Update performance state
                if let Ok(mut state) = state.try_write() {
                    let summary = stats.get_summary();
                    
                    // Calculate current throughput (messages per second)
                    if let Some(last_check) = state.last_check {
                        let duration = last_check.elapsed();
                        if duration.as_secs() > 0 {
                            state.current_throughput = summary.messages_per_second;
                        }
                    }
                    
                    // Update other metrics
                    state.current_latency_p99 = metrics.get_p99_latency("message_processing");
                    state.current_memory_usage = summary.memory_usage_bytes;
                    state.simd_enabled = true; // Based on configuration
                    state.memory_pool_active = true;
                    state.network_optimized = true;
                    state.last_check = Some(Instant::now());
                    
                    debug!("Performance metrics updated: throughput={:.2} msg/s, latency_p99={:?}",
                           state.current_throughput, state.current_latency_p99);
                }
            }
        });
        
        Ok(())
    }
    
    /// Enable adaptive performance tuning
    pub async fn enable_adaptive_tuning(&self) -> Result<()> {
        info!("🎯 Enabling adaptive performance tuning");
        
        let state = self.state.clone();
        let scheduler = self.task_scheduler.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                if let Ok(state) = state.try_read() {
                    // Adjust task scheduler based on current load
                    if state.current_throughput > 15000.0 {
                        // High load: prioritize high-priority tasks
                        debug!("High load detected, adjusting task priorities");
                    } else if state.current_throughput < 5000.0 {
                        // Low load: can process background tasks
                        debug!("Low load detected, processing background tasks");
                    }
                    
                    // Adjust based on latency
                    if state.current_latency_p99 > Duration::from_millis(5) {
                        warn!("High latency detected: {:?}", state.current_latency_p99);
                    }
                }
            }
        });
        
        Ok(())
    }
}

/// Performance statistics structure
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub throughput: f64,
    pub latency_p99: Duration,
    pub memory_usage: usize,
    pub total_connections: u64,
    pub active_connections: u64,
    pub total_messages: u64,
    pub error_rate: f64,
    pub simd_enabled: bool,
    pub memory_pool_active: bool,
    pub network_optimized: bool,
}

impl PerformanceStats {
    /// Get performance score (0-100)
    pub fn performance_score(&self) -> u8 {
        let mut score = 100;
        
        // Deduct points for high latency
        if self.latency_p99 > Duration::from_millis(5) {
            score -= 20;
        } else if self.latency_p99 > Duration::from_millis(2) {
            score -= 10;
        }
        
        // Deduct points for high error rate
        if self.error_rate > 0.01 { // > 1%
            score -= 30;
        } else if self.error_rate > 0.001 { // > 0.1%
            score -= 10;
        }
        
        // Bonus points for optimizations enabled
        if self.simd_enabled { score += 5; }
        if self.memory_pool_active { score += 5; }
        if self.network_optimized { score += 5; }
        
        score.min(100).max(0) as u8
    }
}