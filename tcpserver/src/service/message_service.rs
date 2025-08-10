//! 메시지 서비스
//! 
//! 게임 메시지 라우팅, 검증, 변환을 담당합니다.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use tracing::{info, error, warn, debug};
use std::collections::HashMap;

use crate::protocol::GameMessage;
use crate::service::ConnectionService;
use crate::tool::SimpleUtils;

/// 메시지 핸들러 타입
pub type MessageHandler = Box<dyn Fn(u32, &GameMessage) -> Result<Option<GameMessage>> + Send + Sync>;

/// 메시지 서비스
pub struct MessageService {
    connection_service: Arc<ConnectionService>,
    message_handlers: Arc<Mutex<HashMap<String, MessageHandler>>>,
    message_stats: Arc<Mutex<MessageStats>>,
    broadcast_rx: Arc<Mutex<Option<broadcast::Receiver<(Option<u32>, GameMessage)>>>>,
    is_processing: Arc<Mutex<bool>>,
}

/// 메시지 통계
#[derive(Debug, Clone, Default)]
pub struct MessageStats {
    pub total_messages: u64,
    pub heartbeat_messages: u64,
    pub chat_messages: u64,
    pub error_messages: u64,
    pub broadcast_messages: u64,
    pub failed_messages: u64,
    pub average_processing_time_ms: f64,
}

impl MessageService {
    /// 새로운 메시지 서비스 생성
    pub fn new(connection_service: Arc<ConnectionService>) -> Self {
        let broadcast_rx = connection_service.subscribe_broadcast();
        
        Self {
            connection_service,
            message_handlers: Arc::new(Mutex::new(HashMap::new())),
            message_stats: Arc::new(Mutex::new(MessageStats::default())),
            broadcast_rx: Arc::new(Mutex::new(Some(broadcast_rx))),
            is_processing: Arc::new(Mutex::new(false)),
        }
    }
    
    /// 메시지 핸들러 등록
    pub async fn register_handler<F>(&self, message_type: &str, handler: F) 
    where 
        F: Fn(u32, &GameMessage) -> Result<Option<GameMessage>> + Send + Sync + 'static
    {
        let mut handlers = self.message_handlers.lock().await;
        handlers.insert(message_type.to_string(), Box::new(handler));
        
        info!("메시지 핸들러 등록: {}", message_type);
    }
    
    /// 메시지 처리 시작
    pub async fn start_processing(&self) -> Result<()> {
        let mut is_processing = self.is_processing.lock().await;
        
        if *is_processing {
            warn!("메시지 처리가 이미 실행 중입니다");
            return Ok(());
        }
        
        *is_processing = true;
        drop(is_processing);
        
        // 브로드캐스트 수신자 가져오기
        let mut rx_option = self.broadcast_rx.lock().await;
        let mut rx = match rx_option.take() {
            Some(receiver) => receiver,
            None => {
                return Err(anyhow!("브로드캐스트 수신자가 이미 사용 중입니다"));
            }
        };
        drop(rx_option);
        
        info!("🔄 메시지 처리 시작");
        
        let handlers_ref = self.message_handlers.clone();
        let stats_ref = self.message_stats.clone();
        let connection_service = self.connection_service.clone();
        let is_processing_ref = self.is_processing.clone();
        
        tokio::spawn(async move {
            while *is_processing_ref.lock().await {
                match rx.recv().await {
                    Ok((client_id, message)) => {
                        let start_time = std::time::Instant::now();
                        
                        debug!("메시지 수신: {:?} from client {:?}", message, client_id);
                        
                        // 메시지 타입별 처리
                        let message_type = Self::get_message_type(&message);
                        let processed = Self::process_message(
                            &handlers_ref,
                            &connection_service,
                            client_id,
                            &message,
                            &message_type
                        ).await;
                        
                        // 통계 업데이트
                        let processing_time = start_time.elapsed().as_millis() as f64;
                        Self::update_message_stats(&stats_ref, &message_type, processing_time, processed.is_ok()).await;
                        
                        if let Err(e) = processed {
                            error!("메시지 처리 실패: {}", e);
                        }
                    }
                    Err(e) => {
                        debug!("브로드캐스트 수신 종료: {}", e);
                        break;
                    }
                }
            }
            
            info!("메시지 처리 루프 종료");
        });
        
        Ok(())
    }
    
    /// 메시지 처리 중지
    pub async fn stop_processing(&self) -> Result<()> {
        let mut is_processing = self.is_processing.lock().await;
        
        if !*is_processing {
            warn!("메시지 처리가 이미 중지되어 있습니다");
            return Ok(());
        }
        
        *is_processing = false;
        
        info!("🛑 메시지 처리 중지 완료");
        Ok(())
    }
    
    /// 메시지 타입 문자열 반환
    fn get_message_type(message: &GameMessage) -> String {
        match message {
            GameMessage::HeartBeat => "heartbeat".to_string(),
            GameMessage::HeartBeatResponse { .. } => "heartbeat_response".to_string(),
            GameMessage::ConnectionAck { .. } => "connection_ack".to_string(),
            GameMessage::Error { .. } => "error".to_string(),
            GameMessage::RoomJoin { .. } => "room_join".to_string(),
            GameMessage::RoomLeave { .. } => "room_leave".to_string(),
            GameMessage::RoomJoinSuccess { .. } => "room_join_success".to_string(),
            GameMessage::RoomLeaveSuccess { .. } => "room_leave_success".to_string(),
            GameMessage::UserJoinedRoom { .. } => "user_joined_room".to_string(),
            GameMessage::UserLeftRoom { .. } => "user_left_room".to_string(),
            GameMessage::ChatMessage { .. } => "chat".to_string(),
            GameMessage::FriendAdd { .. } => "friend_add".to_string(),
            GameMessage::FriendRemove { .. } => "friend_remove".to_string(),
            GameMessage::Connect { .. } => "connect".to_string(),
            GameMessage::ChatResponse { .. } => "chat_response".to_string(),
            GameMessage::UserInfo { .. } => "user_info".to_string(),
            GameMessage::SystemMessage { .. } => "system_message".to_string(),
        }
    }
    
    /// 메시지 처리 로직
    async fn process_message(
        handlers: &Arc<Mutex<HashMap<String, MessageHandler>>>,
        connection_service: &Arc<ConnectionService>,
        client_id: Option<u32>,
        message: &GameMessage,
        message_type: &str,
    ) -> Result<()> {
        // 등록된 핸들러 확인
        let handlers_lock = handlers.lock().await;
        
        if let Some(handler) = handlers_lock.get(message_type) {
            if let Some(cid) = client_id {
                if let Ok(Some(response)) = handler(cid, message) {
                    connection_service.send_to_user(cid, &response).await?;
                }
            }
        } else {
            // 기본 메시지 처리
            match message {
                GameMessage::HeartBeat => {
                    if let Some(cid) = client_id {
                        let response = GameMessage::HeartBeatResponse { 
                            timestamp: SimpleUtils::current_timestamp() 
                        };
                        connection_service.send_to_user(cid, &response).await?;
                    }
                }
                GameMessage::Error { code, message } => {
                    warn!("클라이언트 {:?}에서 에러 수신: {} - {}", client_id, code, message);
                }
                _ => {
                    debug!("처리되지 않은 메시지: {:?}", message);
                }
            }
        }
        
        Ok(())
    }
    
    /// 메시지 통계 업데이트
    async fn update_message_stats(
        stats_ref: &Arc<Mutex<MessageStats>>,
        message_type: &str,
        processing_time_ms: f64,
        success: bool,
    ) {
        if let Ok(mut stats) = stats_ref.try_lock() {
            stats.total_messages += 1;
            
            match message_type {
                "heartbeat" => stats.heartbeat_messages += 1,
                "chat" => stats.chat_messages += 1,
                "error" => stats.error_messages += 1,
                _ => {}
            }
            
            if !success {
                stats.failed_messages += 1;
            }
            
            // 평균 처리 시간 업데이트
            if stats.average_processing_time_ms == 0.0 {
                stats.average_processing_time_ms = processing_time_ms;
            } else {
                stats.average_processing_time_ms = 
                    (stats.average_processing_time_ms * 0.9) + (processing_time_ms * 0.1);
            }
        }
    }
    
    /// 에러 메시지 전송
    pub async fn send_error(&self, client_id: u32, error_code: u16, error_message: &str) -> Result<()> {
        let error_msg = GameMessage::Error {
            code: error_code,
            message: error_message.to_string(),
        };
        
        self.connection_service.send_to_user(client_id, &error_msg).await?;
        
        Self::update_message_stats(
            &self.message_stats,
            "error",
            0.0,
            true
        ).await;
        
        Ok(())
    }
    
    /// 브로드캐스트 메시지 전송
    pub async fn broadcast(&self, message: &GameMessage) -> Result<usize> {
        let count = self.connection_service.broadcast_message(message).await?;
        
        Self::update_message_stats(
            &self.message_stats,
            "broadcast",
            0.0,
            true
        ).await;
        
        Ok(count)
    }
    
    /// 메시지 통계 조회
    pub async fn get_message_stats(&self) -> MessageStats {
        self.message_stats.lock().await.clone()
    }
    
    /// 메시지 처리 상태 확인
    pub async fn is_processing(&self) -> bool {
        *self.is_processing.lock().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_message_service() {
        let connection_service = Arc::new(ConnectionService::new(100));
        let message_service = MessageService::new(connection_service);
        
        // 초기 상태
        assert!(!message_service.is_processing().await);
        
        let stats = message_service.get_message_stats().await;
        assert_eq!(stats.total_messages, 0);
    }
    
    #[tokio::test]
    async fn test_message_handler_registration() {
        let connection_service = Arc::new(ConnectionService::new(100));
        let message_service = MessageService::new(connection_service);
        
        // 테스트 핸들러 등록
        message_service.register_handler("test", |client_id, message| {
            println!("테스트 핸들러: {} - {:?}", client_id, message);
            Ok(None)
        }).await;
        
        // 핸들러가 등록되었는지 확인
        let handlers = message_service.message_handlers.lock().await;
        assert!(handlers.contains_key("test"));
    }
}