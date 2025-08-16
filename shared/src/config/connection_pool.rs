use crate::config::redis_config::{RedisConfig, RedisConnection};
use redis::RedisError;
use std::sync::OnceLock;
use tokio::sync::RwLock;

/// Global Redis connection pool singleton
static REDIS_POOL: OnceLock<RwLock<Option<RedisConfig>>> = OnceLock::new();

/// Redis connection pool manager
pub struct ConnectionPool;

impl ConnectionPool {
    /// Initialize the global Redis connection pool
    pub async fn init() -> Result<(), RedisError> {
        let pool = REDIS_POOL.get_or_init(|| RwLock::new(None));
        let mut pool_guard = pool.write().await;

        if pool_guard.is_none() {
            let redis_config = RedisConfig::new().await?;
            *pool_guard = Some(redis_config);
        }

        Ok(())
    }

    /// Get a Redis connection from the pool
    pub async fn get_connection() -> Result<RedisConnection, RedisError> {
        // Ensure pool is initialized
        Self::init().await?;

        let pool = REDIS_POOL.get().expect("Redis pool not initialized");
        let pool_guard = pool.read().await;

        match pool_guard.as_ref() {
            Some(redis_config) => Ok(redis_config.get_connection()),
            None => {
                // This shouldn't happen after init(), but handle it gracefully
                drop(pool_guard);
                Self::init().await?;
                let new_guard = pool.read().await;
                match new_guard.as_ref() {
                    Some(config) => Ok(config.get_connection()),
                    None => Err(RedisError::from((
                        redis::ErrorKind::IoError,
                        "Config not available",
                    ))),
                }
            }
        }
    }

    /// Get Redis configuration from the pool
    pub async fn get_config() -> Result<RedisConfig, RedisError> {
        // Ensure pool is initialized
        Self::init().await?;

        let pool = REDIS_POOL.get().expect("Redis pool not initialized");
        let pool_guard = pool.read().await;

        match pool_guard.as_ref() {
            Some(redis_config) => Ok(redis_config.clone()),
            None => {
                // This shouldn't happen after init(), but handle it gracefully
                drop(pool_guard);
                Self::init().await?;
                let new_guard = pool.read().await;
                match new_guard.as_ref() {
                    Some(config) => Ok(config.clone()),
                    None => Err(RedisError::from((
                        redis::ErrorKind::IoError,
                        "Config not available",
                    ))),
                }
            }
        }
    }
}
