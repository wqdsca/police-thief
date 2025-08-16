//! 채팅방 관리 핸들러
//!
//! 방 입장, 퇴장, 채팅 메시지 처리를 담당하는 핸들러입니다.
//! DashMap 기반 room_connection_service와 통합하여 고성능 채팅 시스템을 제공합니다.

use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::protocol::GameMessage;
use crate::service::room_connection_service::RoomConnectionService;

/// 채팅방 핸들러
///
/// 방 입장/퇴장, 채팅 메시지 처리를 담당합니다.
/// RoomConnectionService와 통합하여 방 기반 메시징을 제공합니다.
pub struct ChatRoomHandler {
    /// 방 기반 연결 관리 서비스
    room_service: Arc<RoomConnectionService>,
}

impl ChatRoomHandler {
    /// 새로운 채팅방 핸들러 생성
    pub fn new(room_service: Arc<RoomConnectionService>) -> Self {
        Self { room_service }
    }

    /// 방 입장 처리
    ///
    /// 사용자를 특정 방에 입장시키고, 기존 사용자들에게 알림을 전송합니다.
    ///
    /// # Arguments
    ///
    /// * `user_id` - 입장하는 사용자 ID
    /// * `room_id` - 입장할 방 ID  
    /// * `nickname` - 사용자 닉네임
    /// * `addr` - 사용자 연결 주소
    /// * `writer` - TCP 연결 writer
    ///
    /// # Returns
    ///
    /// 성공 시 방의 현재 사용자 수를 반환합니다.
    pub async fn handle_room_join(
        &self,
        user_id: u32,
        room_id: u32,
        nickname: String,
        addr: String,
        writer: Arc<Mutex<tokio::io::BufWriter<tokio::net::tcp::OwnedWriteHalf>>>,
    ) -> Result<u32> {
        info!("사용자 {} ({}) 방 {} 입장 시도", user_id, nickname, room_id);

        // 기존 방에 있다면 먼저 퇴장 처리
        if let Some(current_room) = self.room_service.get_user_room(user_id) {
            if current_room != room_id {
                warn!(
                    "사용자 {}가 방 {}에서 방 {}로 이동",
                    user_id, current_room, room_id
                );
                self.handle_room_leave(user_id, current_room).await?;
            }
        }

        // 방에 사용자 추가
        self.room_service
            .add_user_to_room(
                room_id,
                user_id,
                addr.clone(),
                nickname.clone(),
                writer.clone(),
            )
            .await?;

        let user_count = self.room_service.get_room_user_count(room_id);

        // 입장한 사용자에게 성공 메시지 전송
        let join_success = GameMessage::RoomJoinSuccess {
            room_id,
            user_count,
        };
        let room_users = self.room_service.get_room_users(room_id);

        if let Some(user_connection) = room_users.iter().find(|u| u.user_id == user_id) {
            if let Err(e) = user_connection.send_message(&join_success).await {
                error!("사용자 {}에게 입장 성공 메시지 전송 실패: {}", user_id, e);
            }
        }

        // 방의 다른 사용자들에게 새 사용자 입장 알림
        let user_joined = GameMessage::UserJoinedRoom {
            room_id,
            user_id,
            nickname: nickname.clone(),
            user_count,
        };

        // 본인 제외하고 알림 전송
        for connection in room_users.iter() {
            if connection.user_id != user_id {
                if let Err(e) = connection.send_message(&user_joined).await {
                    warn!(
                        "사용자 {}에게 입장 알림 전송 실패: {}",
                        connection.user_id, e
                    );
                }
            }
        }

        info!(
            "✅ 사용자 {} ({}) 방 {} 입장 완료, 현재 인원: {}",
            user_id, nickname, room_id, user_count
        );

        Ok(user_count)
    }

    /// 방 퇴장 처리
    ///
    /// 사용자를 방에서 퇴장시키고, 기존 사용자들에게 알림을 전송합니다.
    /// 방이 비어있게 되면 자동으로 정리합니다.
    ///
    /// # Arguments
    ///
    /// * `user_id` - 퇴장하는 사용자 ID
    /// * `room_id` - 퇴장할 방 ID
    ///
    /// # Returns
    ///
    /// 성공 시 방이 삭제되었는지 여부를 반환합니다.
    pub async fn handle_room_leave(&self, user_id: u32, room_id: u32) -> Result<bool> {
        info!("사용자 {} 방 {} 퇴장 시도", user_id, room_id);

        // 사용자 정보 가져오기 (퇴장 알림용)
        let room_users = self.room_service.get_room_users(room_id);
        let leaving_user = room_users.iter().find(|u| u.user_id == user_id).cloned();

        if leaving_user.is_none() {
            return Err(anyhow!("사용자 {}가 방 {}에 없습니다", user_id, room_id));
        }

        let leaving_user = leaving_user.expect("User should exist");

        // 방에서 사용자 제거
        self.room_service
            .remove_user_from_room(room_id, user_id)
            .await?;

        let new_user_count = self.room_service.get_room_user_count(room_id);

        // 퇴장한 사용자에게 퇴장 성공 메시지 전송
        let leave_success = GameMessage::RoomLeaveSuccess {
            room_id,
            user_count: new_user_count,
        };
        if let Err(e) = leaving_user.send_message(&leave_success).await {
            warn!("사용자 {}에게 퇴장 성공 메시지 전송 실패: {}", user_id, e);
        }
        let room_deleted = new_user_count == 0;

        if room_deleted {
            info!("🗑️ 방 {}이 비어서 자동 삭제되었습니다", room_id);
        } else {
            // 방의 다른 사용자들에게 퇴장 알림
            let user_left = GameMessage::UserLeftRoom {
                room_id,
                user_id,
                nickname: leaving_user.nickname.clone(),
                user_count: new_user_count,
            };

            let remaining_users = self.room_service.get_room_users(room_id);
            for connection in remaining_users.iter() {
                if let Err(e) = connection.send_message(&user_left).await {
                    warn!(
                        "사용자 {}에게 퇴장 알림 전송 실패: {}",
                        connection.user_id, e
                    );
                }
            }

            info!(
                "✅ 사용자 {} ({}) 방 {} 퇴장 완료, 남은 인원: {}",
                user_id, leaving_user.nickname, room_id, new_user_count
            );
        }

        Ok(room_deleted)
    }

    /// 채팅 메시지 처리
    ///
    /// 채팅 메시지를 방의 모든 사용자에게 브로드캐스트합니다.
    ///
    /// # Arguments
    ///
    /// * `user_id` - 메시지 전송자 ID
    /// * `room_id` - 채팅이 발생하는 방 ID
    /// * `content` - 채팅 메시지 내용
    ///
    /// # Returns
    ///
    /// 성공 시 메시지를 받은 사용자 수를 반환합니다.
    pub async fn handle_chat_message(
        &self,
        user_id: u32,
        room_id: u32,
        content: String,
    ) -> Result<usize> {
        debug!("사용자 {} 방 {} 채팅: {}", user_id, room_id, content);

        // 메시지 전송자가 방에 있는지 확인
        if self.room_service.get_user_room(user_id) != Some(room_id) {
            return Err(anyhow!("사용자 {}가 방 {}에 없습니다", user_id, room_id));
        }

        // 채팅 메시지 생성
        let chat_message = GameMessage::ChatMessage {
            user_id,
            room_id,
            message: content.clone(),
        };

        // 방의 모든 사용자에게 메시지 브로드캐스트
        let sent_count = self
            .room_service
            .send_to_room(room_id, &chat_message)
            .await?;

        info!(
            "💬 채팅 메시지 전송 완료: 사용자 {} → 방 {} ({}명 수신)",
            user_id, room_id, sent_count
        );

        Ok(sent_count)
    }

    /// 사용자 연결 해제 처리
    ///
    /// 사용자가 연결을 끊었을 때 자동으로 모든 방에서 퇴장 처리합니다.
    ///
    /// # Arguments
    ///
    /// * `user_id` - 연결 해제된 사용자 ID
    ///
    /// # Returns
    ///
    /// 성공 시 정리된 방의 개수를 반환합니다.
    pub async fn handle_user_disconnect(&self, user_id: u32) -> Result<usize> {
        info!("사용자 {} 연결 해제 처리", user_id);

        let mut cleaned_rooms = 0;

        // 사용자가 속한 방 찾기
        if let Some(room_id) = self.room_service.get_user_room(user_id) {
            // 방에서 퇴장 처리
            match self.handle_room_leave(user_id, room_id).await {
                Ok(room_deleted) => {
                    if room_deleted {
                        cleaned_rooms += 1;
                    }
                }
                Err(e) => {
                    error!("사용자 {} 자동 퇴장 처리 실패: {}", user_id, e);
                }
            }
        }

        info!(
            "✅ 사용자 {} 연결 해제 처리 완료, 정리된 방: {}",
            user_id, cleaned_rooms
        );
        Ok(cleaned_rooms)
    }

    /// 방 상태 조회
    ///
    /// 특정 방의 현재 상태 정보를 반환합니다.
    ///
    /// # Arguments
    ///
    /// * `room_id` - 조회할 방 ID
    ///
    /// # Returns
    ///
    /// (사용자 수, 사용자 목록)을 반환합니다.
    pub fn get_room_status(&self, room_id: u32) -> (u32, Vec<(u32, String)>) {
        let user_count = self.room_service.get_room_user_count(room_id);
        let users = self
            .room_service
            .get_room_users(room_id)
            .into_iter()
            .map(|u| (u.user_id, u.nickname))
            .collect();

        (user_count, users)
    }

    /// 전체 방 목록 조회
    ///
    /// 현재 활성화된 모든 방의 정보를 반환합니다.
    ///
    /// # Returns
    ///
    /// (방 ID, 사용자 수) 목록을 반환합니다.
    pub fn get_all_rooms_status(&self) -> Vec<(u32, u32)> {
        self.room_service
            .get_all_rooms()
            .into_iter()
            .map(|room| (room.room_id, room.user_count))
            .collect()
    }

    /// 빈 방 정리
    ///
    /// 사용자가 없는 방들을 자동으로 정리합니다.
    ///
    /// # Returns
    ///
    /// 정리된 방의 개수를 반환합니다.
    pub async fn cleanup_empty_rooms(&self) -> usize {
        let cleaned = self.room_service.cleanup_empty_rooms().await;
        if cleaned > 0 {
            info!("🧹 빈 방 {}개 정리 완료", cleaned);
        }
        cleaned
    }
}

mod tests {

    #[tokio::test]
    async fn test_chat_room_handler_creation() {
        let room_service = Arc::new(RoomConnectionService::new("test_server".to_string()));
        let handler = ChatRoomHandler::new(room_service);

        // 핸들러가 정상적으로 생성되는지 확인
        assert_eq!(handler.get_all_rooms_status().len(), 0);
    }

    #[tokio::test]
    async fn test_room_status() {
        let room_service = Arc::new(RoomConnectionService::new("test_server".to_string()));
        let handler = ChatRoomHandler::new(room_service);

        // 빈 방 상태 확인
        let (user_count, users) = handler.get_room_status(999);
        assert_eq!(user_count, 0);
        assert!(users.is_empty());
    }

    #[tokio::test]
    async fn test_cleanup_empty_rooms() {
        let room_service = Arc::new(RoomConnectionService::new("test_server".to_string()));
        let handler = ChatRoomHandler::new(room_service);

        // 빈 방 정리 테스트
        let cleaned = handler.cleanup_empty_rooms().await;
        assert_eq!(cleaned, 0); // 초기에는 방이 없으므로 0
    }
}
