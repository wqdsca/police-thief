//! 메시지 핸들러
//! 
//! 게임 메시지별 처리 로직을 정의합니다.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use tracing::{info, error, warn, debug};

use crate::protocol::GameMessage;
use crate::service::{ConnectionService, HeartbeatService, MessageService};
use crate::tool::{DataUtils, HexUtils};

/// 메시지 핸들러
pub struct GameMessageHandler {
    connection_service: Arc<ConnectionService>,
    heartbeat_service: Arc<HeartbeatService>,
    message_service: Arc<MessageService>,
}

impl GameMessageHandler {
    /// 새로운 메시지 핸들러 생성
    pub fn new(
        connection_service: Arc<ConnectionService>,
        heartbeat_service: Arc<HeartbeatService>,
        message_service: Arc<MessageService>,
    ) -> Self {
        Self {
            connection_service,
            heartbeat_service,
            message_service,
        }
    }
    
    /// 모든 메시지 핸들러 등록
    pub async fn register_all_handlers(&self) -> Result<()> {
        info!("메시지 핸들러 등록 시작");
        
        // 하트비트 핸들러
        self.register_heartbeat_handler().await?;
        
        // 연결 관련 핸들러
        self.register_connection_handlers().await?;
        
        // 에러 핸들러
        self.register_error_handler().await?;
        
        info!("✅ 모든 메시지 핸들러 등록 완료");
        Ok(())
    }
    
    /// 하트비트 핸들러 등록
    async fn register_heartbeat_handler(&self) -> Result<()> {
        let heartbeat_service = self.heartbeat_service.clone();
        
        self.message_service.register_handler("heartbeat", move |client_id, message| {
            match message {
                GameMessage::HeartBeat => {
                    debug!("하트비트 수신: 클라이언트 {}", client_id);
                    
                    // 하트비트 응답 생성
                    let response = GameMessage::HeartBeatResponse {
                        timestamp: DataUtils::current_timestamp(),
                    };
                    
                    Ok(Some(response))
                }
                _ => Ok(None)
            }
        }).await;
        
        debug!("하트비트 핸들러 등록 완료");
        Ok(())
    }
    
    /// 연결 관련 핸들러 등록
    async fn register_connection_handlers(&self) -> Result<()> {
        let connection_service = self.connection_service.clone();
        
        self.message_service.register_handler("connection_ack", move |client_id, message| {
            match message {
                GameMessage::ConnectionAck { client_id: ack_id } => {
                    info!("연결 확인 응답: 클라이언트 {} (ack: {})", client_id, ack_id);
                    Ok(None) // 응답 불필요
                }
                _ => Ok(None)
            }
        }).await;
        
        debug!("연결 핸들러 등록 완료");
        Ok(())
    }
    
    /// 에러 핸들러 등록
    async fn register_error_handler(&self) -> Result<()> {
        self.message_service.register_handler("error", move |client_id, message| {
            match message {
                GameMessage::Error { code, message } => {
                    error!("클라이언트 {}에서 에러 수신: {} - {}", client_id, code, message);
                    
                    // 심각한 에러인 경우 연결 종료 권장
                    if *code >= 500 {
                        warn!("심각한 에러로 인한 연결 종료 권장: 클라이언트 {}", client_id);
                    }
                    
                    Ok(None)
                }
                _ => Ok(None)
            }
        }).await;
        
        debug!("에러 핸들러 등록 완료");
        Ok(())
    }
    
    /// 사용자 정의 핸들러 등록
    pub async fn register_custom_handler<F>(&self, message_type: &str, handler: F) -> Result<()>
    where 
        F: Fn(u32, &GameMessage) -> Result<Option<GameMessage>> + Send + Sync + 'static
    {
        self.message_service.register_handler(message_type, handler).await;
        info!("사용자 정의 핸들러 등록: {}", message_type);
        Ok(())
    }
    
    /// 메시지 검증
    pub fn validate_message(&self, client_id: u32, message: &GameMessage) -> Result<()> {
        match message {
            GameMessage::HeartBeat => {
                // 하트비트는 항상 유효
                Ok(())
            }
            GameMessage::HeartBeatResponse { timestamp } => {
                let current_time = DataUtils::current_timestamp();
                let time_diff = (current_time - timestamp).abs();
                
                if time_diff > 60 {
                    return Err(anyhow!("하트비트 응답 시간이 너무 오래됨: {}초", time_diff));
                }
                
                Ok(())
            }
            GameMessage::ConnectionAck { client_id: ack_id } => {
                if *ack_id != client_id {
                    return Err(anyhow!("연결 ID 불일치: {} != {}", ack_id, client_id));
                }
                Ok(())
            }
            GameMessage::Error { code, message: _ } => {
                if *code == 0 {
                    return Err(anyhow!("에러 코드는 0이 될 수 없습니다"));
                }
                Ok(())
            }
        }
    }
    
    /// 메시지를 16진수로 덤프 (디버깅용)
    pub fn dump_message_hex(&self, message: &GameMessage) -> Result<String> {
        let bytes = message.to_bytes()?;
        Ok(HexUtils::bytes_to_hex_debug(&bytes))
    }
    
    /// 메시지 크기 계산
    pub fn calculate_message_size(&self, message: &GameMessage) -> Result<usize> {
        let bytes = message.to_bytes()?;
        Ok(bytes.len())
    }
    
    /// 통계 기반 메시지 분석
    pub async fn analyze_message_patterns(&self) -> MessageAnalysis {
        let stats = self.message_service.get_message_stats().await;
        
        let total = stats.total_messages;
        let heartbeat_ratio = if total > 0 { stats.heartbeat_messages as f64 / total as f64 } else { 0.0 };
        let error_ratio = if total > 0 { stats.error_messages as f64 / total as f64 } else { 0.0 };
        
        MessageAnalysis {
            total_messages: total,
            heartbeat_percentage: heartbeat_ratio * 100.0,
            error_percentage: error_ratio * 100.0,
            average_processing_ms: stats.average_processing_time_ms,
            health_status: if error_ratio < 0.05 { "건강함".to_string() } else { "주의".to_string() },
        }
    }
}

/// 메시지 분석 결과
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessageAnalysis {
    pub total_messages: u64,
    pub heartbeat_percentage: f64,
    pub error_percentage: f64,
    pub average_processing_ms: f64,
    pub health_status: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::ConnectionService;
    
    #[tokio::test]
    async fn test_message_handler_creation() {
        let connection_service = Arc::new(ConnectionService::new(100));
        let heartbeat_service = Arc::new(HeartbeatService::with_default_config(connection_service.clone()));
        let message_service = Arc::new(MessageService::new(connection_service.clone()));
        
        let handler = GameMessageHandler::new(
            connection_service,
            heartbeat_service,
            message_service,
        );
        
        // 핸들러 등록 테스트
        assert!(handler.register_all_handlers().await.is_ok());
    }
    
    #[test]
    fn test_message_validation() {
        let connection_service = Arc::new(ConnectionService::new(100));
        let heartbeat_service = Arc::new(HeartbeatService::with_default_config(connection_service.clone()));
        let message_service = Arc::new(MessageService::new(connection_service.clone()));
        
        let handler = GameMessageHandler::new(
            connection_service,
            heartbeat_service,
            message_service,
        );
        
        // 유효한 메시지
        let heartbeat = GameMessage::HeartBeat;
        assert!(handler.validate_message(1, &heartbeat).is_ok());
        
        // 잘못된 연결 ACK
        let wrong_ack = GameMessage::ConnectionAck { client_id: 999 };
        assert!(handler.validate_message(1, &wrong_ack).is_err());
        
        // 잘못된 에러 코드
        let invalid_error = GameMessage::Error { code: 0, message: "test".to_string() };
        assert!(handler.validate_message(1, &invalid_error).is_err());
    }
    
    #[test]
    fn test_message_size_calculation() {
        let connection_service = Arc::new(ConnectionService::new(100));
        let heartbeat_service = Arc::new(HeartbeatService::with_default_config(connection_service.clone()));
        let message_service = Arc::new(MessageService::new(connection_service.clone()));
        
        let handler = GameMessageHandler::new(
            connection_service,
            heartbeat_service,
            message_service,
        );
        
        let heartbeat = GameMessage::HeartBeat;
        let size = handler.calculate_message_size(&heartbeat).unwrap();
        assert!(size > 4); // 최소 길이 헤더 포함
        
        println!("하트비트 메시지 크기: {} 바이트", size);
    }
}