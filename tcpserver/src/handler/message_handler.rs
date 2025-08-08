//! 메시지 핸들러
//! 
//! 채팅 메시지 및 서버 메시지별 처리 로직을 정의합니다.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use tracing::{info, error, warn, debug};

use crate::protocol::GameMessage;
use crate::service::{ConnectionService, HeartbeatService, MessageService};
// Removed circular dependency - handlers should be injected or use events

/// 채팅 메시지 기록
#[derive(Debug, Clone)]
pub struct ChatRecord {
    pub user_id: u32,
    pub room_id: u32,
    pub content: String,
    pub timestamp: i64,
}

/// 메시지 핸들러
/// 
/// 순환 의존성을 피하기 위해 다른 핸들러들에 대한 직접 참조를 제거했습니다.
/// 대신 이벤트 시스템이나 의존성 주입을 사용해야 합니다.
pub struct ServerMessageHandler {
    connection_service: Arc<ConnectionService>,
    heartbeat_service: Arc<HeartbeatService>,
    message_service: Arc<MessageService>,
    /// 채팅 기록: room_id -> Vec<ChatRecord>
    chat_history: Arc<Mutex<HashMap<u32, Vec<ChatRecord>>>>,
}

impl ServerMessageHandler {
    /// 새로운 메시지 핸들러 생성
    /// 
    /// 메시지 처리를 위한 핸들러 인스턴스를 생성합니다.
    /// 
    /// # Arguments
    /// 
    /// * `connection_service` - 연결 관리 서비스
    /// * `heartbeat_service` - 하트비트 관리 서비스  
    /// * `message_service` - 메시지 전송 서비스
    /// 
    /// # Returns
    /// 
    /// 새로운 ServerMessageHandler 인스턴스
    pub fn new(
        connection_service: Arc<ConnectionService>,
        heartbeat_service: Arc<HeartbeatService>,
        message_service: Arc<MessageService>,
    ) -> Self {
        Self {
            connection_service,
            heartbeat_service,
            message_service,
            chat_history: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// 모든 메시지 핸들러 등록
    /// 
    /// 시스템에서 사용하는 모든 메시지 타입에 대한 핸들러를 등록합니다.
    /// 하트비트, 연결 관리, 에러 처리 및 4대 핵심 기능 핸들러를 등록합니다.
    /// 
    /// # Returns
    /// 
    /// * `Result<()>` - 등록 성공 시 Ok(()), 실패 시 에러
    /// 
    /// # Errors
    /// 
    /// * 핸들러 등록 실패 시
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let handler = ServerMessageHandler::new(/* services */);
    /// handler.register_all_handlers().await?;
    /// ```
    pub async fn register_all_handlers(&self) -> Result<()> {
        info!("메시지 핸들러 등록 시작");
        
        // 기본 메시지 핸들러
        self.register_heartbeat_handler().await?;
        self.register_connection_handlers().await?;
        self.register_error_handler().await?;
        
        // 4대 기능 핸들러 (간소화 - 순환 의존성 해결 후 복원 필요)
        // TODO: 이벤트 시스템이나 의존성 주입으로 핸들러 등록 개선
        self.register_simplified_handlers().await?;
        
        info!("✅ 모든 메시지 핸들러 등록 완료");
        Ok(())
    }
    
    /// 하트비트 핸들러 등록
    /// 
    /// 클라이언트로부터 받은 하트비트 메시지를 처리하고 응답하는 핸들러를 등록합니다.
    /// HeartBeat 메시지를 받으면 현재 서버 시간을 포함한 HeartBeatResponse로 응답합니다.
    /// 
    /// # Returns
    /// 
    /// * `Result<()>` - 등록 성공 시 Ok(()), 실패 시 에러
    /// 
    /// # 처리 메시지
    /// 
    /// * `GameMessage::HeartBeat` → `GameMessage::HeartBeatResponse`
    async fn register_heartbeat_handler(&self) -> Result<()> {
        let heartbeat_service = self.heartbeat_service.clone();
        
        self.message_service.register_handler("heartbeat", move |user_id, message| {
            match message {
                GameMessage::HeartBeat => {
                    debug!("하트비트 수신: 사용자 {}", user_id);
                    
                    // 하트비트 응답 생성
                    let response = GameMessage::HeartBeatResponse {
                        timestamp: chrono::Utc::now().timestamp(),
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
        
        self.message_service.register_handler("connection_ack", move |user_id, message| {
            match message {
                GameMessage::ConnectionAck { user_id: ack_id } => {
                    info!("연결 확인 응답: 사용자 {} (ack: {})", user_id, ack_id);
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
        self.message_service.register_handler("error", move |user_id, message| {
            match message {
                GameMessage::Error { code, message } => {
                    error!("사용자 {}에서 에러 수신: {} - {}", user_id, code, message);
                    
                    // 심각한 에러인 경우 연결 종료 권장
                    if *code >= 500 {
                        warn!("심각한 에러로 인한 연결 종료 권장: 사용자 {}", user_id);
                    }
                    
                    Ok(None)
                }
                _ => Ok(None)
            }
        }).await;
        
        debug!("에러 핸들러 등록 완료");
        Ok(())
    }
    
    /// 간소화된 핸들러 등록 (순환 의존성 해결을 위한 임시 구현)
    async fn register_simplified_handlers(&self) -> Result<()> {
        // 방 입장 핸들러 (간소화)
        self.message_service.register_handler("room_join", move |user_id, message| {
            match message {
                GameMessage::RoomJoin { user_id: msg_user_id, room_id, nickname: _ } => {
                    if *msg_user_id != user_id {
                        return Ok(Some(GameMessage::Error {
                            code: 400,
                            message: "사용자 ID 불일치".to_string(),
                        }));
                    }
                    
                    // 임시 구현 - 실제로는 방 핸들러를 사용해야 함
                    info!("방 입장 요청 수신: 사용자 {} -> 방 {}", msg_user_id, room_id);
                    Ok(Some(GameMessage::ConnectionAck { user_id: *msg_user_id }))
                }
                _ => Ok(None)
            }
        }).await;
        
        // 채팅 핸들러 (간소화)
        // 동기 핸들러로 변경 - async 블록 제거
        self.message_service.register_handler("chat", move |user_id, message| {
            match message {
                GameMessage::ChatMessage { user_id: msg_user_id, room_id, content, timestamp } => {
                    if *msg_user_id != user_id {
                        return Ok(Some(GameMessage::Error {
                            code: 400,
                            message: "사용자 ID 불일치".to_string(),
                        }));
                    }
                    
                    // 채팅 기록은 이벤트 시스템이나 별도 처리로 이동 필요
                    // 현재는 로그만 남김
                    info!("채팅 메시지 수신: 사용자 {} -> 방 {}: {}", msg_user_id, room_id, content);
                    Ok(None)
                }
                _ => Ok(None)
            }
        }).await;
        
        // 친구 추가 핸들러 (간소화)
        self.message_service.register_handler("friend_add", move |user_id, message| {
            match message {
                GameMessage::FriendAdd { user_id: msg_user_id, friend_user_id, nickname: _ } => {
                    if *msg_user_id != user_id {
                        return Ok(Some(GameMessage::Error {
                            code: 400,
                            message: "사용자 ID 불일치".to_string(),
                        }));
                    }
                    
                    // 임시 구현 - 실제로는 친구 핸들러 사용 필요
                    info!("친구 추가 요청 수신: {} -> {}", msg_user_id, friend_user_id);
                    Ok(Some(GameMessage::ConnectionAck { user_id: *msg_user_id }))
                }
                _ => Ok(None)
            }
        }).await;
        
        // 친구 삭제 핸들러 (간소화)
        self.message_service.register_handler("friend_remove", move |user_id, message| {
            match message {
                GameMessage::FriendRemove { user_id: msg_user_id, friend_user_id } => {
                    if *msg_user_id != user_id {
                        return Ok(Some(GameMessage::Error {
                            code: 400,
                            message: "사용자 ID 불일치".to_string(),
                        }));
                    }
                    
                    // 임시 구현 - 실제로는 친구 핸들러 사용 필요
                    info!("친구 삭제 요청 수신: {} -> {}", msg_user_id, friend_user_id);
                    Ok(Some(GameMessage::ConnectionAck { user_id: *msg_user_id }))
                }
                _ => Ok(None)
            }
        }).await;
        
        debug!("간소화된 핸들러 등록 완료");
        Ok(())
    }
    
    /// 채팅 기록 조회
    /// 
    /// 특정 방의 채팅 기록을 조회합니다. 기록이 없는 경우 빈 벡터를 반환합니다.
    /// 
    /// # Arguments
    /// 
    /// * `room_id` - 조회할 방 ID
    /// 
    /// # Returns
    /// 
    /// * `Vec<ChatRecord>` - 해당 방의 채팅 기록 목록
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let history = handler.get_chat_history(1).await;
    /// println!("방 1의 채팅 기록: {} 개", history.len());
    /// ```
    pub async fn get_chat_history(&self, room_id: u32) -> Vec<ChatRecord> {
        let history = self.chat_history.lock().await;
        history.get(&room_id).cloned().unwrap_or_else(Vec::new)
    }
    
    /// 채팅 기록 정리
    pub async fn cleanup_chat_history(&self, room_id: u32) {
        let mut history = self.chat_history.lock().await;
        history.remove(&room_id);
        debug!("방 {} 채팅 기록 정리 완료", room_id);
    }
    
    /// 메시지 검증
    /// 
    /// 클라이언트로부터 받은 메시지의 유효성을 검증합니다.
    /// 사용자 ID 일치, 필드 값 검증, 메시지 타입별 특수 검증을 수행합니다.
    /// 
    /// # Arguments
    /// 
    /// * `user_id` - 메시지를 전송한 사용자 ID
    /// * `message` - 검증할 게임 메시지
    /// 
    /// # Returns
    /// 
    /// * `Result<()>` - 검증 통과 시 Ok(()), 실패 시 에러 메시지
    /// 
    /// # Errors
    /// 
    /// * 사용자 ID 불일치
    /// * 필수 필드 누락 또는 잘못된 값
    /// * 메시지 타입별 비즈니스 규칙 위반
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let message = GameMessage::ChatMessage { /* ... */ };
    /// match handler.validate_message(user_id, &message) {
    ///     Ok(()) => println!("메시지 검증 통과"),
    ///     Err(e) => println!("메시지 검증 실패: {}", e),
    /// }
    /// ```
    pub fn validate_message(&self, user_id: u32, message: &GameMessage) -> Result<()> {
        match message {
            GameMessage::HeartBeat => Ok(()),
            GameMessage::HeartBeatResponse { timestamp } => {
                let current_time = chrono::Utc::now().timestamp();
                let time_diff = (current_time - timestamp).abs();
                
                if time_diff > 60 {
                    return Err(anyhow!("하트비트 응답 시간이 너무 오래됨: {}초", time_diff));
                }
                
                Ok(())
            }
            GameMessage::ConnectionAck { user_id: ack_id } => {
                if *ack_id != user_id {
                    return Err(anyhow!("사용자 ID 불일치: {} != {}", ack_id, user_id));
                }
                Ok(())
            }
            GameMessage::Error { code, message: _ } => {
                if *code == 0 {
                    return Err(anyhow!("에러 코드는 0이 될 수 없습니다"));
                }
                Ok(())
            }
            GameMessage::RoomJoin { user_id: msg_user_id, room_id: _, nickname } => {
                if *msg_user_id != user_id {
                    return Err(anyhow!("사용자 ID 불일치"));
                }
                if nickname.is_empty() {
                    return Err(anyhow!("닉네임이 비어있습니다"));
                }
                Ok(())
            }
            GameMessage::ChatMessage { user_id: msg_user_id, room_id: _, content, timestamp: _ } => {
                if *msg_user_id != user_id {
                    return Err(anyhow!("사용자 ID 불일치"));
                }
                if content.is_empty() {
                    return Err(anyhow!("채팅 내용이 비어있습니다"));
                }
                if content.len() > 1000 {
                    return Err(anyhow!("채팅 내용이 너무 깁니다"));
                }
                Ok(())
            }
            GameMessage::FriendAdd { user_id: msg_user_id, friend_user_id, nickname } => {
                if *msg_user_id != user_id {
                    return Err(anyhow!("사용자 ID 불일치"));
                }
                if *msg_user_id == *friend_user_id {
                    return Err(anyhow!("자기 자신을 친구로 추가할 수 없습니다"));
                }
                if nickname.is_empty() {
                    return Err(anyhow!("친구 닉네임이 비어있습니다"));
                }
                Ok(())
            }
            GameMessage::FriendRemove { user_id: msg_user_id, friend_user_id } => {
                if *msg_user_id != user_id {
                    return Err(anyhow!("사용자 ID 불일치"));
                }
                if *msg_user_id == *friend_user_id {
                    return Err(anyhow!("자기 자신을 친구에서 삭제할 수 없습니다"));
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_message_handler() {
        let connection_service = Arc::new(crate::service::ConnectionService::new(100));
        let message_service = Arc::new(crate::service::MessageService::new(connection_service.clone()));
        let heartbeat_service = Arc::new(crate::service::HeartbeatService::with_default_config(connection_service.clone()));
        
        let handler = ServerMessageHandler::new(
            connection_service,
            heartbeat_service,
            message_service,
        );
        
        // 핸들러 등록 테스트
        assert!(handler.register_all_handlers().await.is_ok());
        
        // 채팅 메시지 검증 테스트
        let chat_msg = GameMessage::ChatMessage {
            user_id: 1,
            room_id: 1,
            content: "안녕하세요!".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        assert!(handler.validate_message(1, &chat_msg).is_ok());
    }
}