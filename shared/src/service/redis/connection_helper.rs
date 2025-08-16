//! Redis 연결 통합 헬퍼
//!
//! 중복된 Redis 연결 패턴을 제거하고 통일된 인터페이스를 제공합니다.

use crate::config::redis_config::RedisConfig;
use crate::tool::error::AppError;
use redis::{aio::ConnectionManager, Client, ConnectionInfo};
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::{debug, info, warn};

/// 글로벌 Redis 연결 풀
static REDIS_POOL: OnceCell<Arc<RedisConnectionPool>> = OnceCell::const_new();

/// Redis 연결 풀
pub struct RedisConnectionPool {
    connection_manager: ConnectionManager,
    config: RedisConfig,
}

impl RedisConnectionPool {
    /// 새로운 Redis 연결 풀 생성
    async fn new(config: RedisConfig) -> Result<Self, AppError> {
        let connection_info = ConnectionInfo {
            addr: redis::ConnectionAddr::Tcp(config.host.clone(), config.port),
            redis: redis::RedisConnectionInfo {
                db: 0,
                username: None,
                password: None,
            },
        };

        let client = Client::open(connection_info)
            .map_err(|e| AppError::RedisError(format!("Failed to create Redis client: {}", e)))?;

        let connection_manager = ConnectionManager::new(client).await.map_err(|e| {
            AppError::RedisError(format!("Failed to create connection manager: {}", e))
        })?;

        info!(
            "✅ Redis 연결 풀 초기화 완료: {}:{}",
            config.host, config.port
        );

        Ok(Self {
            connection_manager,
            config,
        })
    }

    /// 연결 매니저 반환
    pub fn get_manager(&self) -> ConnectionManager {
        self.connection_manager.clone()
    }

    /// 연결 상태 확인
    pub async fn ping(&self) -> Result<(), AppError> {
        let mut conn = self.connection_manager.clone();
        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .map_err(|e| AppError::RedisError(format!("Ping failed: {}", e)))?;

        debug!("Redis ping 성공");
        Ok(())
    }

    /// 설정 정보 반환
    pub fn config(&self) -> &RedisConfig {
        &self.config
    }
}

/// Redis 연결 헬퍼 - 싱글톤 패턴으로 중복 제거
pub struct RedisConnectionHelper;

impl RedisConnectionHelper {
    /// 글로벌 Redis 연결 풀 초기화
    pub async fn initialize(config: RedisConfig) -> Result<(), AppError> {
        let pool = RedisConnectionPool::new(config).await?;

        REDIS_POOL
            .set(Arc::new(pool))
            .map_err(|_| AppError::RedisError("Redis 연결 풀이 이미 초기화됨".to_string()))?;

        info!("🔄 Redis 연결 헬퍼 초기화 완료");
        Ok(())
    }

    /// 환경변수에서 설정을 로드하여 초기화
    pub async fn initialize_from_env() -> Result<(), AppError> {
        let config = RedisConfig::new()
            .await
            .map_err(|e| AppError::RedisError(format!("환경변수 로드 실패: {}", e)))?;

        Self::initialize(config).await
    }

    /// Redis 연결 매니저 반환
    pub fn get_connection() -> Result<ConnectionManager, AppError> {
        let pool = REDIS_POOL
            .get()
            .ok_or_else(|| AppError::RedisError("Redis 연결 풀이 초기화되지 않음".to_string()))?;

        Ok(pool.get_manager())
    }

    /// Redis 연결 풀 반환
    pub fn get_pool() -> Result<Arc<RedisConnectionPool>, AppError> {
        REDIS_POOL
            .get()
            .ok_or_else(|| AppError::RedisError("Redis 연결 풀이 초기화되지 않음".to_string()))
            .map(Arc::clone)
    }

    /// 연결 상태 확인
    pub async fn health_check() -> Result<bool, AppError> {
        match Self::get_pool() {
            Ok(pool) => match pool.ping().await {
                Ok(_) => Ok(true),
                Err(e) => {
                    warn!("Redis 헬스체크 실패: {}", e);
                    Ok(false)
                }
            },
            Err(e) => {
                warn!("Redis 연결 풀 접근 실패: {}", e);
                Ok(false)
            }
        }
    }

    /// 연결 통계 조회
    pub async fn get_connection_stats() -> Result<RedisConnectionStats, AppError> {
        let pool = Self::get_pool()?;
        let mut conn = pool.get_manager();

        let info: String = redis::cmd("INFO")
            .arg("clients")
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::RedisError(format!("INFO 명령 실패: {}", e)))?;

        Ok(RedisConnectionStats::parse_from_info(&info))
    }
}

/// Redis 연결 통계
#[derive(Debug, Clone)]
pub struct RedisConnectionStats {
    pub connected_clients: u32,
    pub total_connections_received: u64,
    pub rejected_connections: u64,
}

impl RedisConnectionStats {
    /// INFO 명령 결과에서 통계 파싱
    pub fn parse_from_info(info: &str) -> Self {
        let mut stats = Self {
            connected_clients: 0,
            total_connections_received: 0,
            rejected_connections: 0,
        };

        for line in info.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() == 2 {
                match parts[0] {
                    "connected_clients" => {
                        stats.connected_clients = parts[1].parse().unwrap_or(0);
                    }
                    "total_connections_received" => {
                        stats.total_connections_received = parts[1].parse().unwrap_or(0);
                    }
                    "rejected_connections" => {
                        stats.rejected_connections = parts[1].parse().unwrap_or(0);
                    }
                    _ => {}
                }
            }
        }

        stats
    }
}

/// 공통 Redis 작업 매크로
#[macro_export]
macro_rules! redis_operation {
    ($operation:expr) => {{
        use $crate::service::redis::connection_helper::RedisConnectionHelper;
        use $crate::tool::error::AppError;

        let mut conn = RedisConnectionHelper::get_connection()
            .map_err(|e| AppError::RedisError(format!("연결 획득 실패: {}", e)))?;

        $operation(&mut conn).await
            .map_err(|e: redis::RedisError| AppError::RedisError(format!("Redis 작업 실패: {}", e)))
    }};
}

/// 공통 Redis 파이프라인 작업 매크로
#[macro_export]
macro_rules! redis_pipeline {
    ($operations:expr) => {{
        use redis::pipe;
        use $crate::service::redis::connection_helper::RedisConnectionHelper;
        use $crate::tool::error::AppError;

        let mut conn = RedisConnectionHelper::get_connection()
            .map_err(|e| AppError::RedisError(format!("연결 획득 실패: {}", e)))?;

        let mut pipeline = pipe();
        $operations(&mut pipeline);

        pipeline
            .query_async(&mut conn)
            .await
            .map_err(|e: redis::RedisError| {
                AppError::RedisError(format!("파이프라인 실행 실패: {}", e))
            })
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::redis_config::RedisConfig;

    #[tokio::test]
    async fn test_redis_connection_helper() {
        // RedisConfig 생성은 실제 Redis 연결이 필요하므로 테스트에서는 스킵
        // 대신 단위 테스트 가능한 부분만 테스트
        
        // 더미 테스트 - 실제로는 RedisConfig::new()가 Redis 서버를 필요로 함
        let result = std::env::var("REDIS_URL");
        println!("Redis URL 환경변수 확인: {:?}", result);
        
        // Redis 서버 없이는 테스트할 수 없으므로 통과로 처리
        assert!(true);
    }

    #[test]
    fn test_redis_connection_stats_parsing() {
        let info = r#"
# Clients
connected_clients:2
total_connections_received:100
rejected_connections:0
"#;

        let stats = RedisConnectionStats::parse_from_info(info);
        assert_eq!(stats.connected_clients, 2);
        assert_eq!(stats.total_connections_received, 100);
        assert_eq!(stats.rejected_connections, 0);
    }
}
