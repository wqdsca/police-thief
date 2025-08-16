//! 친구 관리 핸들러
//!
//! 친구 추가/삭제 기능을 처리합니다.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::service::{ConnectionService, MessageService};

/// 친구 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Friend {
    pub user_id: u32,
    pub nickname: String,
    pub added_at: i64,
}

/// 친구 관리 핸들러
pub struct FriendHandler {
    connection_service: Arc<ConnectionService>,
    message_service: Arc<MessageService>,
    /// user_id -> Set<friend_user_id>
    friendships: Arc<Mutex<HashMap<u32, HashSet<u32>>>>,
    /// user_id -> user_nickname
    user_nicknames: Arc<Mutex<HashMap<u32, String>>>,
    /// friendship details: (user_id, friend_id) -> Friend
    friend_details: Arc<Mutex<HashMap<(u32, u32), Friend>>>,
}

impl FriendHandler {
    /// 새로운 친구 핸들러 생성
    pub fn new(
        connection_service: Arc<ConnectionService>,
        message_service: Arc<MessageService>,
    ) -> Self {
        Self {
            connection_service,
            message_service,
            friendships: Arc::new(Mutex::new(HashMap::new())),
            user_nicknames: Arc::new(Mutex::new(HashMap::new())),
            friend_details: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 사용자 닉네임 등록/업데이트
    pub async fn register_user(&self, user_id: u32, nickname: String) {
        let mut nicknames = self.user_nicknames.lock().await;
        nicknames.insert(user_id, nickname);
    }

    /// 친구 추가
    pub async fn add_friend(
        &self,
        user_id: u32,
        friend_user_id: u32,
        friend_nickname: String,
    ) -> Result<()> {
        if user_id == friend_user_id {
            return Err(anyhow!("자기 자신을 친구로 추가할 수 없습니다"));
        }

        let mut friendships = self.friendships.lock().await;
        let mut friend_details = self.friend_details.lock().await;

        // 사용자의 친구 목록 가져오기 (없으면 새로 생성)
        let user_friends = friendships.entry(user_id).or_insert_with(HashSet::new);

        // 이미 친구인지 확인
        if user_friends.contains(&friend_user_id) {
            return Err(anyhow!("이미 친구로 등록된 사용자입니다"));
        }

        // 친구 추가
        user_friends.insert(friend_user_id);

        // 친구 상세 정보 저장
        let friend_info = Friend {
            user_id: friend_user_id,
            nickname: friend_nickname.clone(),
            added_at: chrono::Utc::now().timestamp(),
        };
        friend_details.insert((user_id, friend_user_id), friend_info);

        // 사용자 닉네임도 업데이트
        let mut nicknames = self.user_nicknames.lock().await;
        nicknames.insert(friend_user_id, friend_nickname.clone());
        drop(nicknames);

        info!(
            "친구 추가 완료: {} -> {} ({})",
            user_id, friend_user_id, friend_nickname
        );
        Ok(())
    }

    /// 친구 삭제
    pub async fn remove_friend(&self, user_id: u32, friend_user_id: u32) -> Result<()> {
        let mut friendships = self.friendships.lock().await;
        let mut friend_details = self.friend_details.lock().await;

        // 사용자의 친구 목록 가져오기
        let user_friends = friendships
            .get_mut(&user_id)
            .ok_or_else(|| anyhow!("사용자의 친구 목록을 찾을 수 없습니다"))?;

        // 친구 관계 확인
        if !user_friends.contains(&friend_user_id) {
            return Err(anyhow!("친구 관계가 아닙니다"));
        }

        // 친구 삭제
        user_friends.remove(&friend_user_id);
        friend_details.remove(&(user_id, friend_user_id));

        // 빈 친구 목록 정리
        if user_friends.is_empty() {
            friendships.remove(&user_id);
        }

        info!("친구 삭제 완료: {} -> {}", user_id, friend_user_id);
        Ok(())
    }

    /// 친구 목록 조회
    pub async fn get_friend_list(&self, user_id: u32) -> Vec<Friend> {
        let friendships = self.friendships.lock().await;
        let friend_details = self.friend_details.lock().await;

        if let Some(friend_ids) = friendships.get(&user_id) {
            friend_ids
                .iter()
                .filter_map(|&friend_id| friend_details.get(&(user_id, friend_id)).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// 친구 관계 확인
    pub async fn is_friend(&self, user_id: u32, friend_user_id: u32) -> bool {
        let friendships = self.friendships.lock().await;

        if let Some(user_friends) = friendships.get(&user_id) {
            user_friends.contains(&friend_user_id)
        } else {
            false
        }
    }

    /// 상호 친구 관계 확인
    pub async fn are_mutual_friends(&self, user1_id: u32, user2_id: u32) -> bool {
        let friendships = self.friendships.lock().await;

        let user1_has_user2 = friendships
            .get(&user1_id)
            .map(|friends| friends.contains(&user2_id))
            .unwrap_or(false);

        let user2_has_user1 = friendships
            .get(&user2_id)
            .map(|friends| friends.contains(&user1_id))
            .unwrap_or(false);

        user1_has_user2 && user2_has_user1
    }

    /// 친구 수 조회
    pub async fn get_friend_count(&self, user_id: u32) -> usize {
        let friendships = self.friendships.lock().await;

        friendships
            .get(&user_id)
            .map(|friends| friends.len())
            .unwrap_or(0)
    }

    /// 공통 친구 찾기
    pub async fn get_mutual_friends(&self, user1_id: u32, user2_id: u32) -> Vec<u32> {
        let friendships = self.friendships.lock().await;

        let user1_friends = friendships.get(&user1_id);
        let user2_friends = friendships.get(&user2_id);

        match (user1_friends, user2_friends) {
            (Some(friends1), Some(friends2)) => friends1.intersection(friends2).cloned().collect(),
            _ => Vec::new(),
        }
    }

    /// 사용자 연결 해제 시 정리
    pub async fn cleanup_user(&self, user_id: u32) {
        let mut friendships = self.friendships.lock().await;
        let mut friend_details = self.friend_details.lock().await;
        let mut nicknames = self.user_nicknames.lock().await;

        // 사용자의 친구 목록 제거
        if let Some(user_friends) = friendships.remove(&user_id) {
            // 친구 상세 정보도 제거
            for friend_id in user_friends {
                friend_details.remove(&(user_id, friend_id));
            }
        }

        // 다른 사용자들의 친구 목록에서도 제거
        let mut to_remove = Vec::new();
        for (other_user_id, friends) in friendships.iter_mut() {
            if friends.remove(&user_id) {
                friend_details.remove(&(*other_user_id, user_id));

                // 빈 친구 목록 정리
                if friends.is_empty() {
                    to_remove.push(*other_user_id);
                }
            }
        }

        // 빈 친구 목록들 제거
        for user_id_to_remove in to_remove {
            friendships.remove(&user_id_to_remove);
        }

        // 닉네임 정보 제거
        nicknames.remove(&user_id);

        debug!("사용자 {} 친구 관계 정리 완료", user_id);
    }

    /// 친구 관계 통계
    pub async fn get_friend_stats(&self) -> FriendStats {
        let friendships = self.friendships.lock().await;
        let nicknames = self.user_nicknames.lock().await;

        let total_users = nicknames.len();
        let total_friendships = friendships.values().map(|friends| friends.len()).sum();
        let users_with_friends = friendships.len();

        let max_friends = friendships
            .values()
            .map(|friends| friends.len())
            .max()
            .unwrap_or(0);

        let avg_friends = if users_with_friends > 0 {
            total_friendships as f64 / users_with_friends as f64
        } else {
            0.0
        };

        FriendStats {
            total_users,
            total_friendships,
            users_with_friends,
            max_friends,
            avg_friends,
        }
    }
}

/// 친구 관계 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendStats {
    pub total_users: usize,
    pub total_friendships: usize,
    pub users_with_friends: usize,
    pub max_friends: usize,
    pub avg_friends: f64,
}

mod tests {

    #[tokio::test]
    async fn test_friend_handler() {
        let connection_service = Arc::new(crate::service::ConnectionService::new(100));
        let message_service = Arc::new(crate::service::MessageService::new(
            connection_service.clone(),
        ));
        let friend_handler = FriendHandler::new(connection_service, message_service);

        // 사용자 등록
        friend_handler.register_user(1, "User1".to_string()).await;
        friend_handler.register_user(2, "User2".to_string()).await;

        // 친구 추가
        assert!(friend_handler
            .add_friend(1, 2, "User2".to_string())
            .await
            .is_ok());

        // 친구 관계 확인
        assert!(friend_handler.is_friend(1, 2).await);
        assert!(!friend_handler.is_friend(2, 1).await); // 단방향

        // 친구 목록 조회
        let friends = friend_handler.get_friend_list(1).await;
        assert_eq!(friends.len(), 1);
        assert_eq!(friends[0].user_id, 2);

        // 친구 삭제
        assert!(friend_handler.remove_friend(1, 2).await.is_ok());
        assert!(!friend_handler.is_friend(1, 2).await);

        // 통계 조회
        let stats = friend_handler.get_friend_stats().await;
        assert_eq!(stats.total_users, 2);
    }

    #[tokio::test]
    async fn test_mutual_friends() {
        let connection_service = Arc::new(crate::service::ConnectionService::new(100));
        let message_service = Arc::new(crate::service::MessageService::new(
            connection_service.clone(),
        ));
        let friend_handler = FriendHandler::new(connection_service, message_service);

        // 상호 친구 추가
        friend_handler
            .add_friend(1, 2, "User2".to_string())
            .await
            .expect("Async test assertion");
        friend_handler
            .add_friend(2, 1, "User1".to_string())
            .await
            .expect("Operation failed");

        // 상호 친구 확인
        assert!(friend_handler.are_mutual_friends(1, 2).await);
        assert!(friend_handler.are_mutual_friends(2, 1).await);
    }
}
