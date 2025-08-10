//! 하트비트 서비스
//! 
//! 클라이언트 연결 상태 모니터링과 타임아웃 관리를 담당합니다.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, interval, Instant};
use tracing::{info, warn, debug};

use crate::service::ConnectionService;
use crate::tool::{SimpleUtils, error::{TcpServerError, ErrorHandler, ErrorSeverity}};
use crate::protocol::GameMessage;

/// 하트비트 서비스
pub struct HeartbeatService {
    connection_service: Arc<ConnectionService>,
    is_running: Arc<Mutex<bool>>,
    cleanup_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    heartbeat_interval_secs: u64,
    connection_timeout_secs: u64,
    heartbeat_stats: Arc<Mutex<HeartbeatStats>>,
}

/// 하트비트 통계
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HeartbeatStats {
    pub total_heartbeats: u64,
    pub timeout_cleanups: u64,
    #[serde(skip)]
    pub last_cleanup_time: Option<Instant>,
    /// 마지막 정리 시간 (Unix timestamp)
    pub last_cleanup_timestamp: Option<i64>,
    pub average_response_time_ms: f64,
    pub active_connections: u32,
}

impl HeartbeatService {
    /// 새로운 하트비트 서비스 생성
    pub fn new(
        connection_service: Arc<ConnectionService>,
        heartbeat_interval_secs: u64,
        connection_timeout_secs: u64,
    ) -> Self {
        Self {
            connection_service,
            is_running: Arc::new(Mutex::new(false)),
            cleanup_handle: Arc::new(Mutex::new(None)),
            heartbeat_interval_secs,
            connection_timeout_secs,
            heartbeat_stats: Arc::new(Mutex::new(HeartbeatStats::default())),
        }
    }
    
    /// 기본 설정으로 생성
    pub fn with_default_config(connection_service: Arc<ConnectionService>) -> Self {
        Self::new(connection_service, 600, 1800) // 600초(10분) 간격, 1800초(30분) 타임아웃
    }
    
    /// 하트비트 시스템 시작
    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.lock().await;
        
        if *is_running {
            warn!("하트비트 시스템이 이미 실행 중입니다");
            return Ok(());
        }
        
        *is_running = true;
        drop(is_running);
        
        info!("🔄 하트비트 시스템 시작 ({}초 간격, {}초 타임아웃)", 
              self.heartbeat_interval_secs, self.connection_timeout_secs);
        
        // 하트비트 정리 작업 시작
        let connection_service = self.connection_service.clone();
        let is_running_ref = self.is_running.clone();
        let stats_ref = self.heartbeat_stats.clone();
        let interval_secs = self.heartbeat_interval_secs;
        
        let handle = tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(interval_secs));
            
            while *is_running_ref.lock().await {
                cleanup_interval.tick().await;
                
                let start_time = Instant::now();
                
                // 타임아웃된 연결 정리
                let cleanup_count = connection_service.cleanup_timeout_connections().await;
                let current_connections = connection_service.get_connection_count().await;
                
                // 통계 업데이트
                if let Ok(mut stats) = stats_ref.try_lock() {
                    if cleanup_count > 0 {
                        stats.timeout_cleanups += cleanup_count as u64;
                        stats.last_cleanup_time = Some(start_time);
                        stats.last_cleanup_timestamp = Some(chrono::Utc::now().timestamp());
                    }
                    stats.active_connections = current_connections as u32;
                    
                    // 평균 응답 시간 업데이트 (단순화된 계산)
                    let cleanup_time_ms = start_time.elapsed().as_millis() as f64;
                    if stats.average_response_time_ms == 0.0 {
                        stats.average_response_time_ms = cleanup_time_ms;
                    } else {
                        stats.average_response_time_ms = (stats.average_response_time_ms * 0.9) + (cleanup_time_ms * 0.1);
                    }
                }
                
                if cleanup_count > 0 {
                    info!("하트비트 타임아웃 연결 정리: {}개 (활성: {}개)", cleanup_count, current_connections);
                } else if current_connections > 0 {
                    debug!("하트비트 체크 완료 - 활성 연결: {}개", current_connections);
                }
            }
            
            info!("하트비트 정리 작업 종료");
        });
        
        // 핸들 저장
        *self.cleanup_handle.lock().await = Some(handle);
        
        Ok(())
    }
    
    /// 하트비트 시스템 중지
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.lock().await;
        
        if !*is_running {
            warn!("하트비트 시스템이 이미 중지되어 있습니다");
            return Ok(());
        }
        
        *is_running = false;
        drop(is_running);
        
        info!("🛑 하트비트 시스템 중지 중...");
        
        // 정리 작업 핸들 종료
        let mut handle_option = self.cleanup_handle.lock().await;
        if let Some(handle) = handle_option.take() {
            handle.abort();
            debug!("하트비트 정리 작업 핸들 종료됨");
        }
        
        info!("✅ 하트비트 시스템 중지 완료");
        Ok(())
    }
    
    /// 하트비트 시스템 실행 상태 확인
    pub async fn is_running(&self) -> bool {
        *self.is_running.lock().await
    }
    
    /// 수동으로 타임아웃된 연결 정리
    pub async fn cleanup_now(&self) -> Result<usize> {
        let start_time = Instant::now();
        let cleanup_count = self.connection_service.cleanup_timeout_connections().await;
        
        // 통계 업데이트
        if let Ok(mut stats) = self.heartbeat_stats.try_lock() {
            if cleanup_count > 0 {
                stats.timeout_cleanups += cleanup_count as u64;
                stats.last_cleanup_time = Some(start_time);
                stats.last_cleanup_timestamp = Some(chrono::Utc::now().timestamp());
            }
        }
        
        if cleanup_count > 0 {
            info!("수동 하트비트 정리: {}개 연결 해제", cleanup_count);
        } else {
            debug!("정리할 타임아웃 연결이 없습니다");
        }
        
        Ok(cleanup_count)
    }
    
    /// 하트비트 처리 (클라이언트에서 받은 하트비트)
    pub async fn handle_heartbeat(&self, client_id: u32) -> Result<()> {
        // 하트비트 응답 전송
        let response = GameMessage::HeartBeatResponse { 
            timestamp: SimpleUtils::current_timestamp() 
        };
        
        if let Err(e) = self.connection_service.send_to_user(client_id, &response).await {
            let tcp_error = TcpServerError::heartbeat_error(Some(client_id), "send_response", &e.to_string());
            ErrorHandler::handle_error(tcp_error.clone(), ErrorSeverity::Error, "HeartbeatService", "handle_heartbeat");
            return Err(anyhow::anyhow!(tcp_error));
        }
        
        // 통계 업데이트
        if let Ok(mut stats) = self.heartbeat_stats.try_lock() {
            stats.total_heartbeats += 1;
        }
        
        debug!("클라이언트 {} 하트비트 처리 완료", client_id);
        Ok(())
    }
    
    /// 하트비트 통계 조회
    pub async fn get_heartbeat_stats(&self) -> HeartbeatStats {
        self.heartbeat_stats.lock().await.clone()
    }
    
    /// 현재 활성 연결 수 조회
    pub async fn get_active_connections(&self) -> usize {
        self.connection_service.get_connection_count().await
    }
    
    /// 하트비트 설정 조회
    pub fn get_config(&self) -> (u64, u64) {
        (self.heartbeat_interval_secs, self.connection_timeout_secs)
    }
    
    /// 연결 건강성 평가
    pub async fn evaluate_connection_health(&self) -> ConnectionHealth {
        let stats = self.get_heartbeat_stats().await;
        let connection_count = self.get_active_connections().await;
        
        let timeout_rate = if stats.total_heartbeats > 0 {
            stats.timeout_cleanups as f64 / stats.total_heartbeats as f64
        } else {
            0.0
        };
        
        let health_score = match timeout_rate {
            r if r < 0.01 => HealthScore::Excellent,
            r if r < 0.05 => HealthScore::Good,
            r if r < 0.10 => HealthScore::Fair,
            r if r < 0.20 => HealthScore::Poor,
            _ => HealthScore::Critical,
        };
        
        ConnectionHealth {
            score: health_score,
            active_connections: connection_count,
            timeout_rate,
            average_response_ms: stats.average_response_time_ms,
            total_heartbeats: stats.total_heartbeats,
        }
    }
}

/// 연결 건강성 점수
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HealthScore {
    Excellent, // < 1% 타임아웃
    Good,      // < 5% 타임아웃  
    Fair,      // < 10% 타임아웃
    Poor,      // < 20% 타임아웃
    Critical,  // >= 20% 타임아웃
}

/// 연결 건강성 정보
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectionHealth {
    pub score: HealthScore,
    pub active_connections: usize,
    pub timeout_rate: f64,
    pub average_response_ms: f64,
    pub total_heartbeats: u64,
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_heartbeat_service_lifecycle() {
        let connection_service = Arc::new(ConnectionService::new(100));
        let heartbeat_service = HeartbeatService::new(connection_service, 1, 3); // 빠른 테스트용
        
        // 초기 상태
        assert!(!heartbeat_service.is_running().await);
        
        // 시작 테스트
        assert!(heartbeat_service.start().await.is_ok());
        assert!(heartbeat_service.is_running().await);
        
        // 잠시 대기 후 통계 확인
        tokio::time::sleep(Duration::from_millis(100)).await;
        let stats = heartbeat_service.get_heartbeat_stats().await;
        assert_eq!(stats.active_connections, 0);
        
        // 중지 테스트
        assert!(heartbeat_service.stop().await.is_ok());
        assert!(!heartbeat_service.is_running().await);
    }
    
    #[tokio::test]
    async fn test_heartbeat_config() {
        let connection_service = Arc::new(ConnectionService::new(100));
        let heartbeat_service = HeartbeatService::new(connection_service, 5, 15);
        
        let (interval, timeout) = heartbeat_service.get_config();
        assert_eq!(interval, 5);
        assert_eq!(timeout, 15);
    }
    
    #[tokio::test]
    async fn test_connection_health() {
        let connection_service = Arc::new(ConnectionService::new(100));
        let heartbeat_service = HeartbeatService::with_default_config(connection_service);
        
        let health = heartbeat_service.evaluate_connection_health().await;
        assert_eq!(health.score, HealthScore::Excellent); // 초기 상태
        assert_eq!(health.active_connections, 0);
    }
}