//! TCP 서버 메인 서비스
//! 
//! TCP 서버의 생명주기와 전반적인 관리를 담당합니다.

use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{info, error, warn};
use shared::config::redis_config::RedisConfig;

use crate::service::{ConnectionService, HeartbeatService};
use crate::tool::SimpleUtils;

/// TCP 서버 설정
#[derive(Debug, Clone)]
pub struct TcpServerConfig {
    pub bind_address: String,
    pub max_connections: u32,
    pub heartbeat_interval_secs: u64,
    pub connection_timeout_secs: u64,
    pub enable_compression: bool,
    pub enable_logging: bool,
}

impl Default for TcpServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:8080".to_string(),
            max_connections: 1000,
            heartbeat_interval_secs: 10,
            connection_timeout_secs: 30,
            enable_compression: false,
            enable_logging: true,
        }
    }
}

/// TCP 게임 서버 서비스
pub struct TcpGameService {
    config: TcpServerConfig,
    connection_service: Arc<ConnectionService>,
    heartbeat_service: Arc<HeartbeatService>,
    redis_config: Arc<Mutex<Option<RedisConfig>>>,
    is_running: Arc<Mutex<bool>>,
}

impl TcpGameService {
    /// 새로운 TCP 게임 서비스 생성
    pub fn new(config: TcpServerConfig) -> Self {
        let connection_service = Arc::new(ConnectionService::new(config.max_connections));
        let heartbeat_service = Arc::new(HeartbeatService::new(
            connection_service.clone(),
            config.heartbeat_interval_secs,
            config.connection_timeout_secs,
        ));
        
        Self {
            config,
            connection_service,
            heartbeat_service,
            redis_config: Arc::new(Mutex::new(None)),
            is_running: Arc::new(Mutex::new(false)),
        }
    }
    
    /// 기본 설정으로 서비스 생성
    pub fn with_default_config() -> Self {
        Self::new(TcpServerConfig::default())
    }
    
    /// 사용자 정의 설정으로 서비스 생성
    pub fn with_config(config: TcpServerConfig) -> Self {
        Self::new(config)
    }
    
    /// 서버 시작
    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.lock().await;
        
        if *is_running {
            warn!("TCP 서버가 이미 실행 중입니다");
            return Ok(());
        }
        
        info!("🚀 TCP 게임 서버 시작 중... ({})", self.config.bind_address);
        
        // 바인드 주소 사용
        let bind_addr = &self.config.bind_address;
        
        // Redis 연결 설정
        if let Ok(redis_config) = RedisConfig::new().await {
            *self.redis_config.lock().await = Some(redis_config);
            info!("✅ Redis 연결 완료");
        } else {
            warn!("⚠️ Redis 연결 실패 - Redis 없이 실행");
        }
        
        // TCP 리스너 시작
        let listener = TcpListener::bind(bind_addr)
            .await
            .context("TCP 리스너 바인드 실패")?;
        
        info!("✅ TCP 서버가 {}에서 실행 중입니다", bind_addr);
        
        // 서버 상태 설정
        *is_running = true;
        drop(is_running);
        
        // 하트비트 시스템 시작
        self.heartbeat_service.start().await
            .context("하트비트 시스템 시작 실패")?;
        
        // 클라이언트 연결 처리 루프
        while *self.is_running.lock().await {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("새 클라이언트 연결: {}", addr);
                    let connection_service = self.connection_service.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = connection_service.handle_new_connection(stream, addr.to_string()).await {
                            error!("클라이언트 처리 오류: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("클라이언트 연결 수락 실패: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
        
        Ok(())
    }
    
    /// 서버 중지
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.lock().await;
        
        if !*is_running {
            warn!("TCP 서버가 이미 중지되어 있습니다");
            return Ok(());
        }
        
        info!("🛑 TCP 게임 서버 중지 중...");
        
        *is_running = false;
        drop(is_running);
        
        // 하트비트 시스템 중지
        self.heartbeat_service.stop().await
            .context("하트비트 시스템 중지 실패")?;
        
        // 모든 연결 종료
        self.connection_service.close_all_connections().await;
        
        info!("✅ TCP 게임 서버가 성공적으로 중지되었습니다");
        Ok(())
    }
    
    /// 서버 실행 상태 확인
    pub async fn is_running(&self) -> bool {
        *self.is_running.lock().await
    }
    
    /// 현재 연결 수 조회
    pub async fn get_connection_count(&self) -> usize {
        self.connection_service.get_connection_count().await
    }
    
    /// 서버 통계 조회
    pub async fn get_server_stats(&self) -> ServerStats {
        let connection_count = self.connection_service.get_connection_count().await;
        let heartbeat_running = self.heartbeat_service.is_running().await;
        let uptime_secs = self.connection_service.get_uptime_seconds().await;
        
        ServerStats {
            is_running: self.is_running().await,
            connection_count,
            heartbeat_running,
            uptime_seconds: uptime_secs,
            max_connections: self.config.max_connections,
            bind_address: self.config.bind_address.clone(),
        }
    }
    
    /// Redis 연결 상태 확인
    pub async fn is_redis_connected(&self) -> bool {
        self.redis_config.lock().await.is_some()
    }
    
    /// 설정 조회
    pub fn get_config(&self) -> &TcpServerConfig {
        &self.config
    }
}

/// 서버 통계 정보
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerStats {
    pub is_running: bool,
    pub connection_count: usize,
    pub heartbeat_running: bool,
    pub uptime_seconds: u64,
    pub max_connections: u32,
    pub bind_address: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tcp_server_config() {
        let config = TcpServerConfig::default();
        assert_eq!(config.bind_address, "127.0.0.1:8080");
        assert_eq!(config.max_connections, 1000);
        assert_eq!(config.heartbeat_interval_secs, 10);
    }
    
    #[test]
    fn test_custom_config() {
        let config = TcpServerConfig {
            bind_address: "0.0.0.0:9999".to_string(),
            max_connections: 500,
            heartbeat_interval_secs: 5,
            connection_timeout_secs: 15,
            enable_compression: true,
            enable_logging: false,
        };
        
        let service = TcpGameService::with_config(config.clone());
        assert_eq!(service.get_config().bind_address, "0.0.0.0:9999");
        assert_eq!(service.get_config().max_connections, 500);
    }
    
    #[tokio::test]
    async fn test_service_lifecycle() {
        let service = TcpGameService::with_default_config();
        
        // 초기 상태
        assert!(!service.is_running().await);
        assert_eq!(service.get_connection_count().await, 0);
        
        // 중지 상태에서 중지 시도 (경고만)
        assert!(service.stop().await.is_ok());
        
        // 통계 조회
        let stats = service.get_server_stats().await;
        assert!(!stats.is_running);
        assert_eq!(stats.connection_count, 0);
    }
}