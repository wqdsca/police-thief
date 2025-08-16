//! 연결 관리 모듈
//!
//! 데이터베이스 연결 풀링 및 생명주기 처리

use crate::service::db::core::config::{DbServiceConfig, PoolConfig};
use crate::service::db::core::types::ConnectionStats;
use crate::tool::error::AppError;
use sqlx::mysql::MySqlPool;
use sqlx::MySql;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// 데이터베이스 작업을 위한 연결 관리자
pub struct ConnectionManager {
    /// 연결 풀
    pool: Arc<MySqlPool>,
    
    /// 풀 설정
    config: PoolConfig,
    
    /// 연결 통계
    stats: Arc<ConnectionStatistics>,
    
    /// 상태 확인 간격
    health_check_interval: Duration,
}

/// 내부 통계 추적기
struct ConnectionStatistics {
    active_connections: AtomicU32,    // 활성 연결 수
    total_connections: AtomicU32,     // 총 연결 수
    connection_errors: AtomicU64,     // 연결 오류 수
    total_queries: AtomicU64,         // 총 쿼리 수
    last_health_check: RwLock<Instant>,  // 마지막 상태 확인 시간
}

impl ConnectionManager {
    /// 새 연결 관리자 생성
    pub async fn new(config: &DbServiceConfig) -> Result<Self, AppError> {
        let pool = config.db_config.get_pool().clone();
        
        Ok(Self {
            pool: Arc::new(pool),
            config: config.pool_config.clone(),
            stats: Arc::new(ConnectionStatistics {
                active_connections: AtomicU32::new(0),
                total_connections: AtomicU32::new(0),
                connection_errors: AtomicU64::new(0),
                total_queries: AtomicU64::new(0),
                last_health_check: RwLock::new(Instant::now()),
            }),
            health_check_interval: Duration::from_secs(30),
        })
    }
    
    /// 연결 풀 참조 가져오기
    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }
    
    /// Clone the connection manager
    pub fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            config: self.config.clone(),
            stats: self.stats.clone(),
            health_check_interval: self.health_check_interval,
        }
    }
    
    /// Get connection with retry logic
    pub async fn get_connection(&self) -> Result<sqlx::pool::PoolConnection<MySql>, AppError> {
        let mut attempts = 0;
        let max_attempts = if self.config.enable_retry { 
            self.config.max_retries 
        } else { 
            1 
        };
        
        loop {
            attempts += 1;
            
            match self.pool.acquire().await {
                Ok(conn) => {
                    self.stats.active_connections.fetch_add(1, Ordering::Relaxed);
                    self.stats.total_connections.fetch_add(1, Ordering::Relaxed);
                    debug!("Connection acquired (attempt {}/{})", attempts, max_attempts);
                    return Ok(conn);
                }
                Err(e) => {
                    self.stats.connection_errors.fetch_add(1, Ordering::Relaxed);
                    
                    if attempts >= max_attempts {
                        error!("Failed to acquire connection after {} attempts: {}", attempts, e);
                        return Err(AppError::DatabaseConnection(format!(
                            "Connection pool exhausted: {}", e
                        )));
                    }
                    
                    warn!("Connection attempt {} failed, retrying: {}", attempts, e);
                    tokio::time::sleep(self.config.retry_delay * attempts).await;
                }
            }
        }
    }
    
    /// Release connection back to pool
    pub fn release_connection(&self) {
        self.stats.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
    
    /// Perform health check
    pub async fn health_check(&self) -> Result<bool, AppError> {
        match sqlx::query("SELECT 1 as health")
            .fetch_one(&*self.pool)
            .await
        {
            Ok(_) => {
                *self.stats.last_health_check.write().await = Instant::now();
                debug!("Database health check passed");
                Ok(true)
            }
            Err(e) => {
                error!("Database health check failed: {}", e);
                self.stats.connection_errors.fetch_add(1, Ordering::Relaxed);
                Err(AppError::DatabaseConnection(format!(
                    "Health check failed: {}", e
                )))
            }
        }
    }
    
    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        let _pool_options = self.pool.options();
        
        ConnectionStats {
            active_connections: self.stats.active_connections.load(Ordering::Relaxed),
            idle_connections: self.pool.size() - self.stats.active_connections.load(Ordering::Relaxed),
            total_connections: self.stats.total_connections.load(Ordering::Relaxed),
            max_connections: self.config.max_connections,
            connection_errors: self.stats.connection_errors.load(Ordering::Relaxed),
            total_queries: self.stats.total_queries.load(Ordering::Relaxed),
        }
    }
    
    /// Increment query counter
    pub fn record_query(&self) {
        self.stats.total_queries.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Test connection with timeout
    pub async fn test_connection(&self, timeout: Duration) -> Result<bool, AppError> {
        match tokio::time::timeout(timeout, self.health_check()).await {
            Ok(result) => result,
            Err(_) => {
                error!("Connection test timed out after {:?}", timeout);
                Err(AppError::Timeout("Connection test timed out".to_string()))
            }
        }
    }
    
    /// Gracefully close all connections
    pub async fn close(&self) {
        info!("Closing database connection pool");
        self.pool.close().await;
    }
    
    /// Check if pool is healthy
    pub async fn is_healthy(&self) -> bool {
        let stats = self.get_stats().await;
        let last_check = *self.stats.last_health_check.read().await;
        
        // Check various health indicators
        let connection_health = stats.active_connections < self.config.max_connections;
        let error_health = stats.connection_errors < self.config.max_connections as u64 * 10;
        let recent_check = last_check.elapsed() < self.health_check_interval * 2;
        
        connection_health && error_health && recent_check
    }
    
    /// Warm up connection pool
    pub async fn warmup(&self) -> Result<(), AppError> {
        info!("Warming up connection pool");
        
        let warmup_count = self.config.min_connections;
        let mut handles = Vec::new();
        
        for i in 0..warmup_count {
            let pool = self.pool.clone();
            handles.push(tokio::spawn(async move {
                match pool.acquire().await {
                    Ok(_conn) => {
                        debug!("Warmed up connection {}", i);
                        Ok(())
                    }
                    Err(e) => {
                        warn!("Failed to warm up connection {}: {}", i, e);
                        Err(e)
                    }
                }
            }));
        }
        
        // Wait for all warmup connections
        for handle in handles {
            handle.await.map_err(|e| {
                AppError::DatabaseConnection(format!("Warmup failed: {}", e))
            })??;
        }
        
        info!("Connection pool warmup complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_connection_manager_creation() {
        // Test configuration
        let config = DbServiceConfig::new(DbConfig::default());
        
        // Manager creation test would require actual database
        // This is a placeholder for unit testing
    }
}