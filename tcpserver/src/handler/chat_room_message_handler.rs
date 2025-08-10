//! 채팅방 메시지 라우팅 핸들러
//! 
//! 새로운 채팅방 시스템의 메시지들을 처리하는 통합 라우팅 시스템입니다.
//! ChatRoomHandler와 RoomConnectionService를 사용하여 방 입장/퇴장/채팅을 처리합니다.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use tokio::io::{BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tracing::{info, error, warn, debug};

use crate::protocol::GameMessage;
use crate::service::room_connection_service::RoomConnectionService;
use crate::handler::chat_room_handler::ChatRoomHandler;

/// 채팅방 메시지 라우팅 핸들러
/// 
/// 모든 채팅방 관련 메시지를 적절한 핸들러로 라우팅합니다.
/// 방 입장/퇴장, 채팅 메시지, 연결 해제를 처리합니다.
pub struct ChatRoomMessageHandler {
    /// 방 기반 연결 관리 서비스
    room_service: Arc<RoomConnectionService>,
    /// 채팅방 핸들러
    chat_handler: Arc<ChatRoomHandler>,
}

impl ChatRoomMessageHandler {
    /// 새로운 채팅방 메시지 핸들러 생성
    pub fn new(room_service: Arc<RoomConnectionService>) -> Self {
        let chat_handler = Arc::new(ChatRoomHandler::new(room_service.clone()));
        
        Self {
            room_service,
            chat_handler,
        }
    }

    /// 클라이언트 연결과 첫 Connect 메시지 처리
    /// 
    /// 새로운 클라이언트가 연결되고 Connect 메시지를 보낸 후 메시지 루프를 시작합니다.
    /// 
    /// # Arguments
    /// 
    /// * `reader` - TCP 스트림 읽기 핸들
    /// * `writer` - TCP 스트림 쓰기 핸들
    /// * `addr` - 클라이언트 주소
    /// * `connect_message` - 초기 Connect 메시지
    pub async fn handle_client_connection(
        &self,
        reader: BufReader<OwnedReadHalf>,
        writer: BufWriter<OwnedWriteHalf>,
        addr: String,
        connect_message: GameMessage,
    ) -> Result<()> {
        // Connect 메시지 처리
        let (user_id, room_id) = match connect_message {
            GameMessage::Connect { room_id, user_id } => {
                info!("새 클라이언트 연결: user_id={}, room_id={}, addr={}", user_id, room_id, addr);
                (user_id, room_id)
            }
            _ => {
                error!("첫 메시지는 Connect 메시지여야 합니다");
                return Err(anyhow!("잘못된 초기 메시지"));
            }
        };

        // 연결 확인 메시지 전송
        let writer_arc = Arc::new(tokio::sync::Mutex::new(writer));
        let ack_message = GameMessage::ConnectionAck { user_id };
        {
            let mut writer_guard = writer_arc.lock().await;
            if let Err(e) = ack_message.write_to_stream(&mut *writer_guard).await {
                error!("연결 확인 메시지 전송 실패: {}", e);
                return Err(e);
            }
        }

        // 메시지 처리 루프 시작
        self.message_loop(reader, writer_arc, user_id, addr).await
    }

    /// 메시지 처리 루프
    /// 
    /// 클라이언트로부터 메시지를 지속적으로 읽고 적절한 핸들러로 라우팅합니다.
    /// 
    /// # Arguments
    /// 
    /// * `reader` - TCP 스트림 읽기 핸들
    /// * `writer` - TCP 스트림 쓰기 핸들 (Arc<Mutex>로 래핑됨)
    /// * `user_id` - 연결된 사용자 ID
    /// * `addr` - 클라이언트 주소
    async fn message_loop(
        &self,
        mut reader: BufReader<OwnedReadHalf>,
        writer: Arc<tokio::sync::Mutex<BufWriter<OwnedWriteHalf>>>,
        user_id: u32,
        addr: String,
    ) -> Result<()> {
        info!("사용자 {} 메시지 루프 시작", user_id);

        loop {
            // 메시지 읽기
            let message = match GameMessage::read_from_stream(&mut reader).await {
                Ok(msg) => msg,
                Err(e) => {
                    warn!("사용자 {} 메시지 읽기 실패: {}", user_id, e);
                    break;
                }
            };

            debug!("사용자 {}로부터 메시지 수신: {:?}", user_id, message);

            // 메시지 타입별 처리
            if let Err(e) = self.route_message(user_id, &addr, writer.clone(), message).await {
                error!("사용자 {} 메시지 처리 실패: {}", user_id, e);
                
                // 에러 응답 전송
                let error_msg = GameMessage::Error {
                    code: 500,
                    message: format!("메시지 처리 실패: {}", e),
                };
                
                let mut writer_guard = writer.lock().await;
                if let Err(write_err) = error_msg.write_to_stream(&mut *writer_guard).await {
                    error!("에러 응답 전송 실패: {}", write_err);
                    break;
                }
            }
        }

        // 연결 해제 처리
        info!("사용자 {} 연결 해제 처리", user_id);
        if let Err(e) = self.chat_handler.handle_user_disconnect(user_id).await {
            error!("사용자 {} 연결 해제 처리 실패: {}", user_id, e);
        }

        Ok(())
    }

    /// 메시지 라우팅
    /// 
    /// 받은 메시지를 타입에 따라 적절한 핸들러로 라우팅합니다.
    /// 
    /// # Arguments
    /// 
    /// * `user_id` - 메시지를 보낸 사용자 ID
    /// * `addr` - 클라이언트 주소
    /// * `writer` - TCP 스트림 쓰기 핸들
    /// * `message` - 처리할 메시지
    async fn route_message(
        &self,
        user_id: u32,
        addr: &str,
        writer: Arc<tokio::sync::Mutex<BufWriter<OwnedWriteHalf>>>,
        message: GameMessage,
    ) -> Result<()> {
        match message {
            // 하트비트 처리
            GameMessage::HeartBeat => {
                self.handle_heartbeat(user_id, writer).await
            }

            // 방 입장 처리
            GameMessage::RoomJoin { user_id: msg_user_id, room_id, nickname } => {
                if msg_user_id != user_id {
                    return Err(anyhow!("사용자 ID 불일치"));
                }
                self.handle_room_join(user_id, room_id, nickname, addr.to_string(), writer).await
            }

            // 방 퇴장 처리
            GameMessage::RoomLeave { user_id: msg_user_id, room_id } => {
                if msg_user_id != user_id {
                    return Err(anyhow!("사용자 ID 불일치"));
                }
                self.handle_room_leave(user_id, room_id).await
            }

            // 채팅 메시지 처리
            GameMessage::ChatMessage { user_id: msg_user_id, room_id, message } => {
                if msg_user_id != user_id {
                    return Err(anyhow!("사용자 ID 불일치"));
                }
                self.handle_chat_message(user_id, room_id, message).await
            }

            // 기타 메시지는 로그만 남김
            _ => {
                debug!("처리되지 않은 메시지: {:?}", message);
                Ok(())
            }
        }
    }

    /// 하트비트 메시지 처리
    /// 
    /// 클라이언트로부터 하트비트를 받고 응답을 전송합니다.
    /// 
    /// # Arguments
    /// 
    /// * `user_id` - 하트비트를 보낸 사용자 ID
    /// * `writer` - 응답을 전송할 TCP 스트림 쓰기 핸들
    async fn handle_heartbeat(
        &self,
        user_id: u32,
        writer: Arc<tokio::sync::Mutex<BufWriter<OwnedWriteHalf>>>,
    ) -> Result<()> {
        debug!("하트비트 수신: 사용자 {}", user_id);

        // 하트비트 응답 생성
        let response = GameMessage::HeartBeatResponse {
            timestamp: chrono::Utc::now().timestamp(),
        };

        // 응답 전송
        let mut writer_guard = writer.lock().await;
        response.write_to_stream(&mut *writer_guard).await?;

        debug!("하트비트 응답 전송 완료: 사용자 {}", user_id);
        Ok(())
    }

    /// 방 입장 처리
    /// 
    /// 사용자를 특정 방에 입장시키고 관련 알림을 전송합니다.
    /// 
    /// # Arguments
    /// 
    /// * `user_id` - 입장하는 사용자 ID
    /// * `room_id` - 입장할 방 ID
    /// * `nickname` - 사용자 닉네임
    /// * `addr` - 사용자 주소
    /// * `writer` - TCP 스트림 쓰기 핸들
    async fn handle_room_join(
        &self,
        user_id: u32,
        room_id: u32,
        nickname: String,
        addr: String,
        writer: Arc<tokio::sync::Mutex<BufWriter<OwnedWriteHalf>>>,
    ) -> Result<()> {
        info!("방 입장 처리: 사용자 {} -> 방 {} ({})", user_id, room_id, nickname);

        // 채팅방 핸들러를 통해 방 입장 처리
        match self.chat_handler.handle_room_join(user_id, room_id, nickname, addr, writer).await {
            Ok(user_count) => {
                info!("✅ 사용자 {} 방 {} 입장 성공, 현재 인원: {}", user_id, room_id, user_count);
                Ok(())
            }
            Err(e) => {
                error!("사용자 {} 방 {} 입장 실패: {}", user_id, room_id, e);
                Err(e)
            }
        }
    }

    /// 방 퇴장 처리
    /// 
    /// 사용자를 방에서 퇴장시키고 필요한 경우 방을 정리합니다.
    /// 
    /// # Arguments
    /// 
    /// * `user_id` - 퇴장하는 사용자 ID
    /// * `room_id` - 퇴장할 방 ID
    async fn handle_room_leave(&self, user_id: u32, room_id: u32) -> Result<()> {
        info!("방 퇴장 처리: 사용자 {} -> 방 {}", user_id, room_id);

        // 채팅방 핸들러를 통해 방 퇴장 처리
        match self.chat_handler.handle_room_leave(user_id, room_id).await {
            Ok(room_deleted) => {
                if room_deleted {
                    info!("✅ 사용자 {} 방 {} 퇴장 완료 - 빈 방 자동 삭제", user_id, room_id);
                } else {
                    info!("✅ 사용자 {} 방 {} 퇴장 완료", user_id, room_id);
                }
                Ok(())
            }
            Err(e) => {
                error!("사용자 {} 방 {} 퇴장 실패: {}", user_id, room_id, e);
                Err(e)
            }
        }
    }

    /// 채팅 메시지 처리
    /// 
    /// 채팅 메시지를 방의 모든 사용자에게 브로드캐스트합니다.
    /// 
    /// # Arguments
    /// 
    /// * `user_id` - 메시지를 보낸 사용자 ID
    /// * `room_id` - 채팅이 발생하는 방 ID
    /// * `content` - 채팅 메시지 내용
    async fn handle_chat_message(&self, user_id: u32, room_id: u32, content: String) -> Result<()> {
        debug!("채팅 메시지 처리: 사용자 {} -> 방 {}: {}", user_id, room_id, content);

        // 채팅 메시지 내용 검증
        if content.is_empty() {
            return Err(anyhow!("채팅 내용이 비어있습니다"));
        }
        
        if content.len() > 1000 {
            return Err(anyhow!("채팅 내용이 너무 깁니다 (최대 1000자)"));
        }

        // 채팅방 핸들러를 통해 메시지 브로드캐스트
        match self.chat_handler.handle_chat_message(user_id, room_id, content.clone()).await {
            Ok(sent_count) => {
                info!("✅ 채팅 메시지 전송 완료: 사용자 {} -> 방 {} ({}명 수신)", user_id, room_id, sent_count);
                Ok(())
            }
            Err(e) => {
                error!("채팅 메시지 전송 실패: 사용자 {} -> 방 {}: {}", user_id, room_id, e);
                Err(e)
            }
        }
    }

    /// 방 상태 조회
    /// 
    /// 특정 방의 현재 상태를 조회합니다.
    /// 
    /// # Arguments
    /// 
    /// * `room_id` - 조회할 방 ID
    /// 
    /// # Returns
    /// 
    /// (사용자 수, 사용자 목록)
    pub fn get_room_status(&self, room_id: u32) -> (u32, Vec<(u32, String)>) {
        self.chat_handler.get_room_status(room_id)
    }

    /// 전체 방 목록 조회
    /// 
    /// 현재 활성화된 모든 방의 정보를 조회합니다.
    /// 
    /// # Returns
    /// 
    /// (방 ID, 사용자 수) 목록
    pub fn get_all_rooms_status(&self) -> Vec<(u32, u32)> {
        self.chat_handler.get_all_rooms_status()
    }

    /// 빈 방 정리
    /// 
    /// 사용자가 없는 방들을 자동으로 정리합니다.
    /// 
    /// # Returns
    /// 
    /// 정리된 방의 개수
    pub async fn cleanup_empty_rooms(&self) -> usize {
        self.chat_handler.cleanup_empty_rooms().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufWriter;
    use tokio::net::TcpStream;

    #[tokio::test]
    async fn test_chat_room_message_handler_creation() {
        let room_service = Arc::new(RoomConnectionService::new("test_server".to_string()));
        let handler = ChatRoomMessageHandler::new(room_service);

        // 핸들러가 정상적으로 생성되는지 확인
        assert_eq!(handler.get_all_rooms_status().len(), 0);
    }

    #[tokio::test]
    async fn test_room_status() {
        let room_service = Arc::new(RoomConnectionService::new("test_server".to_string()));
        let handler = ChatRoomMessageHandler::new(room_service);

        // 빈 방 상태 확인
        let (user_count, users) = handler.get_room_status(999);
        assert_eq!(user_count, 0);
        assert!(users.is_empty());
    }

    #[tokio::test]
    async fn test_cleanup_empty_rooms() {
        let room_service = Arc::new(RoomConnectionService::new("test_server".to_string()));
        let handler = ChatRoomMessageHandler::new(room_service);

        // 빈 방 정리 테스트
        let cleaned = handler.cleanup_empty_rooms().await;
        assert_eq!(cleaned, 0); // 초기에는 방이 없으므로 0
    }
}