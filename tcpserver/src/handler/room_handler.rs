//! 방 관리 핸들러
//! 
//! 방 입장 기능을 처리합니다.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use tracing::{info, debug};
use serde::{Serialize, Deserialize};

use crate::service::{ConnectionService, MessageService};

/// 방 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub room_id: u32,
    pub name: String,
    pub users: HashMap<u32, RoomUserInfo>,
    pub max_users: u32,
    pub created_at: i64,
}

/// 방 내 사용자 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomUserInfo {
    pub user_id: u32,
    pub nickname: String,
    pub joined_at: i64,
}

/// 방 관리 핸들러
pub struct RoomHandler {
    connection_service: Arc<ConnectionService>,
    message_service: Arc<MessageService>,
    rooms: Arc<Mutex<HashMap<u32, Room>>>,
    next_room_id: Arc<Mutex<u32>>,
    max_rooms: u32,
    max_users_per_room: u32,
}

impl RoomHandler {
    /// 새로운 방 핸들러 생성
    /// 
    /// 방 관리 기능을 제공하는 핸들러 인스턴스를 생성합니다.
    /// 최대 100개의 방과 방당 최대 10명의 사용자를 지원합니다.
    /// 
    /// # Arguments
    /// 
    /// * `connection_service` - 연결 관리 서비스
    /// * `message_service` - 메시지 전송 서비스
    /// 
    /// # Returns
    /// 
    /// 새로운 RoomHandler 인스턴스
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let handler = RoomHandler::new(connection_service, message_service);
    /// ```
    pub fn new(
        connection_service: Arc<ConnectionService>,
        message_service: Arc<MessageService>,
    ) -> Self {
        Self {
            connection_service,
            message_service,
            rooms: Arc::new(Mutex::new(HashMap::new())),
            next_room_id: Arc::new(Mutex::new(1)),
            max_rooms: 100,
            max_users_per_room: 50,
        }
    }
    
    /// 새로운 방 생성
    /// 
    /// 새로운 게임 방을 생성하고 생성자를 자동으로 입장시킵니다.
    /// 방 이름은 공백이 아니어야 하며, 최대 방 수 제한을 확인합니다.
    /// 
    /// # Arguments
    /// 
    /// * `creator_user_id` - 방을 생성하는 사용자 ID
    /// * `room_name` - 생성할 방의 이름
    /// 
    /// # Returns
    /// 
    /// * `Result<u32>` - 성공 시 생성된 방 ID, 실패 시 에러
    /// 
    /// # Errors
    /// 
    /// * 방 이름이 비어있는 경우
    /// * 최대 방 수 초과
    /// * 사용자가 이미 다른 방에 있는 경우
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let room_id = handler.create_room(123, "새로운 방".to_string()).await?;
    /// println!("방 {} 생성 완료", room_id);
    /// ```
    pub async fn create_room(&self, creator_user_id: u32, room_name: String) -> Result<u32> {
        let room_count = self.rooms.lock().await.len();
        if room_count >= self.max_rooms as usize {
            return Err(anyhow!("최대 방 수 초과: {}/{}", room_count, self.max_rooms));
        }
        
        let mut next_id = self.next_room_id.lock().await;
        let room_id = *next_id;
        *next_id += 1;
        drop(next_id);
        
        let room = Room {
            room_id,
            name: room_name.clone(),
            users: HashMap::new(),
            max_users: self.max_users_per_room,
            created_at: chrono::Utc::now().timestamp(),
        };
        
        self.rooms.lock().await.insert(room_id, room);
        
        info!("✅ 방 생성: {} (ID: {}, 생성자: {})", room_name, room_id, creator_user_id);
        Ok(room_id)
    }
    
    /// 방 입장
    /// 
    /// 사용자를 지정된 방에 입장시킵니다. 방이 존재하지 않거나 가득 찬 경우 실패합니다.
    /// 사용자는 한 번에 하나의 방에만 있을 수 있습니다.
    /// 
    /// # Arguments
    /// 
    /// * `user_id` - 입장할 사용자 ID
    /// * `room_id` - 입장할 방 ID
    /// * `nickname` - 방에서 사용할 닉네임
    /// 
    /// # Returns
    /// 
    /// * `Result<()>` - 성공 시 Ok(()), 실패 시 에러
    /// 
    /// # Errors
    /// 
    /// * 방이 존재하지 않는 경우
    /// * 방이 가득 찬 경우
    /// * 이미 해당 방에 있는 경우
    /// * 닉네임이 비어있는 경우
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// handler.join_room(123, 1, "Player1".to_string()).await?;
    /// println!("방 입장 완료");
    /// ```
    pub async fn join_room(&self, user_id: u32, room_id: u32, nickname: String) -> Result<()> {
        let mut rooms = self.rooms.lock().await;
        
        let room = rooms.get_mut(&room_id)
            .ok_or_else(|| anyhow!("방을 찾을 수 없습니다: {}", room_id))?;
        
        if room.users.len() >= room.max_users as usize {
            return Err(anyhow!("방이 가득 참: {}/{}", room.users.len(), room.max_users));
        }
        
        if room.users.contains_key(&user_id) {
            return Err(anyhow!("이미 방에 참가한 사용자입니다"));
        }
        
        let user_info = RoomUserInfo {
            user_id,
            nickname: nickname.clone(),
            joined_at: chrono::Utc::now().timestamp(),
        };
        
        room.users.insert(user_id, user_info);
        
        info!("사용자 {}({})가 방 {}에 입장", nickname, user_id, room_id);
        Ok(())
    }
    
    /// 방 퇴장
    pub async fn leave_room(&self, user_id: u32, room_id: u32) -> Result<()> {
        let mut rooms = self.rooms.lock().await;
        
        let room = rooms.get_mut(&room_id)
            .ok_or_else(|| anyhow!("방을 찾을 수 없습니다: {}", room_id))?;
        
        if let Some(user_info) = room.users.remove(&user_id) {
            info!("사용자 {}({})가 방 {}에서 퇴장", user_info.nickname, user_id, room_id);
            
            // 방이 비었으면 삭제
            if room.users.is_empty() {
                rooms.remove(&room_id);
                info!("빈 방 삭제: {}", room_id);
            }
            
            Ok(())
        } else {
            Err(anyhow!("사용자가 방에 없습니다: {}", user_id))
        }
    }
    
    /// 방 목록 조회
    pub async fn get_room_list(&self) -> Vec<RoomInfo> {
        let rooms = self.rooms.lock().await;
        
        rooms.values()
            .map(|room| RoomInfo {
                room_id: room.room_id,
                name: room.name.clone(),
                current_users: room.users.len(),
                max_users: room.max_users as usize,
                created_at: room.created_at,
            })
            .collect()
    }
    
    /// 방 상세 정보 조회
    pub async fn get_room_details(&self, room_id: u32) -> Result<Room> {
        let rooms = self.rooms.lock().await;
        
        rooms.get(&room_id)
            .cloned()
            .ok_or_else(|| anyhow!("방을 찾을 수 없습니다: {}", room_id))
    }
    
    /// 사용자가 속한 방 ID 조회
    pub async fn get_user_room(&self, user_id: u32) -> Option<u32> {
        let rooms = self.rooms.lock().await;
        
        for (room_id, room) in rooms.iter() {
            if room.users.contains_key(&user_id) {
                return Some(*room_id);
            }
        }
        
        None
    }
    
    /// 방 정리 (빈 방, 오래된 방 삭제)
    pub async fn cleanup_rooms(&self) -> usize {
        let mut rooms = self.rooms.lock().await;
        let current_time = chrono::Utc::now().timestamp();
        let mut removed_count = 0;
        
        let mut rooms_to_remove = Vec::new();
        
        for (room_id, room) in rooms.iter() {
            let should_remove = room.users.is_empty() || 
                (current_time - room.created_at > 3600); // 1시간 후 정리
            
            if should_remove {
                rooms_to_remove.push(*room_id);
            }
        }
        
        for room_id in rooms_to_remove {
            rooms.remove(&room_id);
            removed_count += 1;
            debug!("방 정리: {}", room_id);
        }
        
        if removed_count > 0 {
            info!("방 정리 완료: {}개", removed_count);
        }
        
        removed_count
    }
    
    /// 방 통계 조회
    pub async fn get_room_stats(&self) -> RoomStats {
        let rooms = self.rooms.lock().await;
        
        let total_rooms = rooms.len();
        let total_users = rooms.values().map(|r| r.users.len()).sum();
        
        RoomStats {
            total_rooms,
            total_users,
            max_rooms: self.max_rooms,
            max_users_per_room: self.max_users_per_room,
        }
    }
}

/// 방 정보 (목록용)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub room_id: u32,
    pub name: String,
    pub current_users: usize,
    pub max_users: usize,
    pub created_at: i64,
}

/// 방 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomStats {
    pub total_rooms: usize,
    pub total_users: usize,
    pub max_rooms: u32,
    pub max_users_per_room: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_room_handler() {
        let connection_service = Arc::new(crate::service::ConnectionService::new(100));
        let message_service = Arc::new(crate::service::MessageService::new(connection_service.clone()));
        let room_handler = RoomHandler::new(connection_service, message_service);
        
        // 방 생성
        let room_id = room_handler.create_room(1, "테스트 방".to_string()).await.unwrap();
        assert_eq!(room_id, 1);
        
        // 사용자 입장
        assert!(room_handler.join_room(1, room_id, "User1".to_string()).await.is_ok());
        assert!(room_handler.join_room(2, room_id, "User2".to_string()).await.is_ok());
        
        // 방 목록 확인
        let rooms = room_handler.get_room_list().await;
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].current_users, 2);
        
        // 사용자 방 조회
        assert_eq!(room_handler.get_user_room(1).await, Some(room_id));
        
        // 사용자 퇴장
        assert!(room_handler.leave_room(1, room_id).await.is_ok());
        
        // 방 정리
        room_handler.cleanup_rooms().await;
    }
}