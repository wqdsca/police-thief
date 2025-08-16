//! 간단한 서비스 구현 (컴파일 안정화용)

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::service::{ConnectionService, HeartbeatService};

/// 간단한 TCP 서비스
pub struct SimpleTcpService {
    connection_service: Arc<ConnectionService>,
    heartbeat_service: HeartbeatService,
    is_running: Arc<Mutex<bool>>,
}

impl SimpleTcpService {
    /// 새로운 서비스 생성
    pub fn new() -> Self {
        let connection_service = Arc::new(ConnectionService::new(100));
        let heartbeat_service = HeartbeatService::with_default_config(connection_service.clone());

        Self {
            connection_service,
            heartbeat_service,
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// 서버 시작
    pub async fn start(&self, bind_addr: &str) -> Result<()> {
        let mut is_running = self.is_running.lock().await;

        if *is_running {
            warn!("서버가 이미 실행 중입니다");
            return Ok(());
        }

        *is_running = true;

        info!("🚀 간단한 TCP 서버 시작: {}", bind_addr);

        // 하트비트 시작
        self.heartbeat_service.start().await?;

        info!("✅ 서버 시작 완료");
        Ok(())
    }

    /// 서버 중지
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.lock().await;

        if !*is_running {
            warn!("서버가 이미 중지되어 있습니다");
            return Ok(());
        }

        *is_running = false;

        info!("🛑 서버 중지 중...");

        // 하트비트 중지
        self.heartbeat_service.stop().await?;

        // 연결 정리
        self.connection_service.close_all_connections().await;

        info!("✅ 서버 중지 완료");
        Ok(())
    }

    /// 실행 상태 확인
    pub async fn is_running(&self) -> bool {
        *self.is_running.lock().await
    }

    /// 서버 상태 조회
    pub async fn get_status(&self) -> String {
        if *self.is_running.lock().await {
            "running".to_string()
        } else {
            "ready".to_string()
        }
    }
}

impl Clone for SimpleTcpService {
    fn clone(&self) -> Self {
        let connection_service = Arc::new(ConnectionService::new(100));
        let heartbeat_service = HeartbeatService::with_default_config(connection_service.clone());

        Self {
            connection_service,
            heartbeat_service,
            is_running: self.is_running.clone(),
        }
    }
}

impl Default for SimpleTcpService {
    fn default() -> Self {
        Self::new()
    }
}
