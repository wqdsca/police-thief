//! 데이터베이스 서비스 설정 모듈
//!
//! 빌더 패턴으로 데이터베이스 서비스 설정을 관리

use crate::config::db::DbConfig;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 데이터베이스 서비스 설정
#[derive(Debug, Clone)]
pub struct DbServiceConfig {
    /// 기본 데이터베이스 설정
    pub db_config: DbConfig,
    
    /// 쿼리 실행 설정
    pub query_config: QueryConfig,
    
    /// 연결 풀 설정
    pub pool_config: PoolConfig,
    
    /// 로깅 및 모니터링 설정
    pub monitoring_config: MonitoringConfig,
    
    /// 성능 설정
    pub performance_config: PerformanceConfig,
}

/// 쿼리 실행 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfig {
    /// 쿼리 로깅 활성화
    pub enable_query_logging: bool,
    
    /// 느린 쿼리 로깅 (임계값: ms)
    pub slow_query_threshold_ms: u64,
    
    /// 기본 쿼리 타임아웃
    pub default_timeout: Duration,
    
    /// 최대 결과 집합 크기
    pub max_result_size: usize,
    
    /// 쿼리 플랜 분석 활성화
    pub enable_query_plan: bool,
}

/// 연결 풀 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// 풀의 최소 연결 수
    pub min_connections: u32,
    
    /// 풀의 최대 연결 수
    pub max_connections: u32,
    
    /// 연결 타임아웃
    pub connect_timeout: Duration,
    
    /// 유휴 연결 타임아웃
    pub idle_timeout: Duration,
    
    /// 최대 연결 수명
    pub max_lifetime: Duration,
    
    /// 연결 재시도 활성화
    pub enable_retry: bool,
    
    /// 최대 재시도 횟수
    pub max_retries: u32,
    
    /// 재시도 지연 시간
    pub retry_delay: Duration,
}

/// 모니터링 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// 성능 메트릭 수집 활성화
    pub enable_metrics: bool,
    
    /// Metrics collection interval
    pub metrics_interval: Duration,
    
    /// Enable query tracing
    pub enable_tracing: bool,
    
    /// Enable connection pool monitoring
    pub enable_pool_monitoring: bool,
    
    /// Alert on connection errors
    pub alert_on_errors: bool,
    
    /// Error threshold for alerts
    pub error_threshold: u32,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable query result caching
    pub enable_query_cache: bool,
    
    /// Query cache size (number of entries)
    pub query_cache_size: usize,
    
    /// Query cache TTL
    pub query_cache_ttl: Duration,
    
    /// Enable prepared statements
    pub use_prepared_statements: bool,
    
    /// Prepared statement cache size
    pub prepared_cache_size: usize,
    
    /// Enable batch optimization
    pub optimize_batch_operations: bool,
    
    /// Batch size for bulk operations
    pub default_batch_size: usize,
}

impl DbServiceConfig {
    /// Create new configuration from environment
    pub async fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let db_config = DbConfig::new().await?;
        Ok(Self::new(db_config))
    }
    
    /// Create new configuration with defaults
    pub fn new(db_config: DbConfig) -> Self {
        Self {
            db_config,
            query_config: QueryConfig::default(),
            pool_config: PoolConfig::default(),
            monitoring_config: MonitoringConfig::default(),
            performance_config: PerformanceConfig::default(),
        }
    }
    
    /// Builder method for query configuration
    pub fn with_query_config(mut self, config: QueryConfig) -> Self {
        self.query_config = config;
        self
    }
    
    /// Builder method for pool configuration
    pub fn with_pool_config(mut self, config: PoolConfig) -> Self {
        self.pool_config = config;
        self
    }
    
    /// Builder method for monitoring configuration
    pub fn with_monitoring(mut self, config: MonitoringConfig) -> Self {
        self.monitoring_config = config;
        self
    }
    
    /// Builder method for performance configuration
    pub fn with_performance(mut self, config: PerformanceConfig) -> Self {
        self.performance_config = config;
        self
    }
    
    /// Enable all optimizations
    pub fn optimized(mut self) -> Self {
        self.performance_config.enable_query_cache = true;
        self.performance_config.use_prepared_statements = true;
        self.performance_config.optimize_batch_operations = true;
        self.pool_config.enable_retry = true;
        self
    }
    
    /// Enable all monitoring features
    pub fn with_full_monitoring(mut self) -> Self {
        self.monitoring_config.enable_metrics = true;
        self.monitoring_config.enable_tracing = true;
        self.monitoring_config.enable_pool_monitoring = true;
        self.monitoring_config.alert_on_errors = true;
        self.query_config.enable_query_logging = true;
        self.query_config.enable_query_plan = true;
        self
    }
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            enable_query_logging: true,
            slow_query_threshold_ms: 1000,
            default_timeout: Duration::from_secs(30),
            max_result_size: 10000,
            enable_query_plan: false,
        }
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 5,
            max_connections: 100,
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(3600),
            enable_retry: true,
            max_retries: 3,
            retry_delay: Duration::from_millis(100),
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_metrics: false,
            metrics_interval: Duration::from_secs(60),
            enable_tracing: false,
            enable_pool_monitoring: false,
            alert_on_errors: false,
            error_threshold: 10,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_query_cache: false,
            query_cache_size: 1000,
            query_cache_ttl: Duration::from_secs(300),
            use_prepared_statements: true,
            prepared_cache_size: 100,
            optimize_batch_operations: true,
            default_batch_size: 1000,
        }
    }
}