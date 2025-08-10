//! 방 내 사용자 관리
//!
//! Redis의 RoomUserList를 통해 방별 사용자 정보를 관리합니다.
//! shared 라이브러리의 5개 Redis 키만을 사용한 최적화된 구조입니다.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use shared::service::redis::core::redis_get_key::{get_key, KeyType};
use shared::tool::high_performance::redis_optimizer::RedisOptimizer;

use crate::game::messages::{PlayerId, PlayerState, Position};

/// 방 내 사용자 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomUserInfo {
    /// 플레이어 ID
    pub player_id: PlayerId,
    /// 플레이어 이름
    pub player_name: String,
    /// 현재 위치
    pub position: Position,
    /// 현재 체력
    pub health: u32,
    /// 최대 체력
    pub max_health: u32,
    /// 플레이어 레벨
    pub level: u32,
    /// 마지막 업데이트 시간
    pub last_updated: u64,
    /// 연결 상태
    pub is_connected: bool,
    /// 추가 게임 상태 정보
    pub game_data: HashMap<String, String>,
}

impl Default for RoomUserInfo {
    fn default() -> Self {
        Self {
            player_id: 0,
            player_name: String::new(),
            position: Position::default(),
            health: 100,
            max_health: 100,
            level: 1,
            last_updated: crate::utils::current_timestamp_ms(),
            is_connected: true,
            game_data: HashMap::new(),
        }
    }
}

/// 방 사용자 관리자
///
/// Redis의 5개 키를 활용하여 방별 사용자 정보를 효율적으로 관리합니다.
/// - User: 개별 사용자 기본 정보
/// - RoomInfo: 방 메타데이터
/// - RoomUserList: 방별 사용자 목록 (핵심)
/// - RoomListByTime: 방 생성 시간순 인덱스
/// - RoomId: 방 ID 재활용 관리
pub struct RoomUserManager {
    /// Redis 최적화기
    redis_optimizer: Arc<RedisOptimizer>,

    /// 현재 활성화된 방들의 캐시
    /// Key: room_id, Value: 방 내 사용자 정보들
    room_cache: Arc<RwLock<HashMap<u16, HashMap<PlayerId, RoomUserInfo>>>>,

    /// 사용자-방 매핑 캐시
    /// Key: player_id, Value: room_id
    player_room_mapping: Arc<RwLock<HashMap<PlayerId, u16>>>,
}

impl RoomUserManager {
    /// 새로운 방 사용자 관리자 생성
    pub async fn new(redis_optimizer: Arc<RedisOptimizer>) -> Result<Self> {
        info!("RoomUserManager 초기화 시작 - Redis 5개 키 기반");

        Ok(Self {
            redis_optimizer,
            room_cache: Arc::new(RwLock::new(HashMap::new())),
            player_room_mapping: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 플레이어를 방에 추가
    ///
    /// # Arguments
    /// * `room_id` - 방 ID
    /// * `user_info` - 사용자 정보
    pub async fn add_user_to_room(&self, room_id: u16, mut user_info: RoomUserInfo) -> Result<()> {
        user_info.last_updated = crate::utils::current_timestamp_ms();

        // 1. Redis RoomUserList에 저장
        let room_key = get_key(&KeyType::RoomUserList, &room_id);
        let field = format!("player:{}", user_info.player_id);
        let value = serde_json::to_string(&user_info)?;

        self.redis_optimizer
            .hset(&room_key, &field, value.as_bytes())
            .await?;

        // 2. 로컬 캐시 업데이트
        {
            let mut cache = self.room_cache.write().await;
            let room_users = cache.entry(room_id).or_insert_with(HashMap::new);
            room_users.insert(user_info.player_id, user_info.clone());
        }

        // 3. 플레이어-방 매핑 업데이트
        {
            let mut mapping = self.player_room_mapping.write().await;
            mapping.insert(user_info.player_id, room_id);
        }

        debug!(
            player_id = %user_info.player_id,
            room_id = %room_id,
            "플레이어를 방에 추가"
        );

        Ok(())
    }

    /// 플레이어 정보 업데이트
    ///
    /// # Arguments
    /// * `room_id` - 방 ID
    /// * `player_id` - 플레이어 ID
    /// * `update_fn` - 업데이트 함수
    pub async fn update_user_in_room<F>(
        &self,
        room_id: u16,
        player_id: PlayerId,
        update_fn: F,
    ) -> Result<()>
    where
        F: FnOnce(&mut RoomUserInfo),
    {
        // 1. 로컬 캐시에서 업데이트
        let updated_info = {
            let mut cache = self.room_cache.write().await;
            if let Some(room_users) = cache.get_mut(&room_id) {
                if let Some(user_info) = room_users.get_mut(&player_id) {
                    update_fn(user_info);
                    user_info.last_updated = crate::utils::current_timestamp_ms();
                    user_info.clone()
                } else {
                    return Err(anyhow::anyhow!(
                        "Player {} not found in room {}",
                        player_id,
                        room_id
                    ));
                }
            } else {
                return Err(anyhow::anyhow!("Room {} not found", room_id));
            }
        };

        // 2. Redis에 저장
        let room_key = get_key(&KeyType::RoomUserList, &room_id);
        let field = format!("player:{}", player_id);
        let value = serde_json::to_string(&updated_info)?;

        self.redis_optimizer
            .hset(&room_key, &field, value.as_bytes())
            .await?;

        debug!(
            player_id = %player_id,
            room_id = %room_id,
            "플레이어 정보 업데이트"
        );

        Ok(())
    }

    /// 플레이어를 방에서 제거
    ///
    /// # Arguments
    /// * `room_id` - 방 ID
    /// * `player_id` - 플레이어 ID
    pub async fn remove_user_from_room(&self, room_id: u16, player_id: PlayerId) -> Result<()> {
        // 1. Redis에서 제거
        let room_key = get_key(&KeyType::RoomUserList, &room_id);
        let field = format!("player:{}", player_id);

        self.redis_optimizer.hdel(&room_key, &field).await?;

        // 2. 로컬 캐시에서 제거
        {
            let mut cache = self.room_cache.write().await;
            if let Some(room_users) = cache.get_mut(&room_id) {
                room_users.remove(&player_id);

                // 방이 비어있으면 캐시에서 제거
                if room_users.is_empty() {
                    cache.remove(&room_id);
                }
            }
        }

        // 3. 플레이어-방 매핑에서 제거
        {
            let mut mapping = self.player_room_mapping.write().await;
            mapping.remove(&player_id);
        }

        info!(
            player_id = %player_id,
            room_id = %room_id,
            "플레이어를 방에서 제거"
        );

        Ok(())
    }

    /// 방 내 모든 사용자 조회
    ///
    /// # Arguments
    /// * `room_id` - 방 ID
    ///
    /// # Returns
    /// 방 내 사용자 정보 목록
    pub async fn get_room_users(&self, room_id: u16) -> Result<Vec<RoomUserInfo>> {
        // 1. 로컬 캐시 확인
        {
            let cache = self.room_cache.read().await;
            if let Some(room_users) = cache.get(&room_id) {
                return Ok(room_users.values().cloned().collect());
            }
        }

        // 2. Redis에서 로드
        let room_key = get_key(&KeyType::RoomUserList, &room_id);
        let user_data: Vec<(String, String)> = match self.redis_optimizer.hgetall(&room_key).await {
            Ok(data) => data,
            Err(_) => Vec::new(), // Redis 오류시 빈 벡터 반환
        };

        let mut users = Vec::new();
        for (field, value) in user_data {
            if field.starts_with("player:") {
                match serde_json::from_str::<RoomUserInfo>(&value) {
                    Ok(user_info) => users.push(user_info),
                    Err(e) => error!(
                        field = %field,
                        error = %e,
                        "사용자 정보 역직렬화 실패"
                    ),
                }
            }
        }

        // 3. 로컬 캐시에 저장
        {
            let mut cache = self.room_cache.write().await;
            let room_users: HashMap<PlayerId, RoomUserInfo> = users
                .iter()
                .map(|user| (user.player_id, user.clone()))
                .collect();
            cache.insert(room_id, room_users);
        }

        Ok(users)
    }

    /// 특정 플레이어의 방 내 정보 조회
    ///
    /// # Arguments
    /// * `room_id` - 방 ID
    /// * `player_id` - 플레이어 ID
    ///
    /// # Returns
    /// 플레이어 정보 (없으면 None)
    pub async fn get_user_in_room(
        &self,
        room_id: u16,
        player_id: PlayerId,
    ) -> Result<Option<RoomUserInfo>> {
        // 1. 로컬 캐시 확인
        {
            let cache = self.room_cache.read().await;
            if let Some(room_users) = cache.get(&room_id) {
                if let Some(user_info) = room_users.get(&player_id) {
                    return Ok(Some(user_info.clone()));
                }
            }
        }

        // 2. Redis에서 로드
        let room_key = get_key(&KeyType::RoomUserList, &room_id);
        let field = format!("player:{}", player_id);

        match self
            .redis_optimizer
            .hget(&room_key, &field)
            .await
            .ok()
            .flatten()
        {
            Some(value) => {
                let value_str = String::from_utf8(value).unwrap_or_default();
                let user_info: RoomUserInfo = serde_json::from_str(&value_str)?;

                // 로컬 캐시에 저장
                {
                    let mut cache = self.room_cache.write().await;
                    let room_users = cache.entry(room_id).or_insert_with(HashMap::new);
                    room_users.insert(player_id, user_info.clone());
                }

                Ok(Some(user_info))
            }
            None => Ok(None),
        }
    }

    /// 플레이어가 속한 방 ID 조회
    ///
    /// # Arguments
    /// * `player_id` - 플레이어 ID
    ///
    /// # Returns
    /// 방 ID (없으면 None)
    pub async fn get_player_room(&self, player_id: PlayerId) -> Option<u16> {
        let mapping = self.player_room_mapping.read().await;
        mapping.get(&player_id).copied()
    }

    /// 방 내 연결된 사용자 수 조회
    ///
    /// # Arguments
    /// * `room_id` - 방 ID
    ///
    /// # Returns
    /// 연결된 사용자 수
    pub async fn get_connected_user_count(&self, room_id: u16) -> Result<usize> {
        let users = self.get_room_users(room_id).await?;
        Ok(users.iter().filter(|u| u.is_connected).count())
    }

    /// 방 내 모든 연결된 사용자들에게 브로드캐스트할 플레이어 ID 목록 반환
    ///
    /// # Arguments
    /// * `room_id` - 방 ID
    /// * `exclude_player_id` - 제외할 플레이어 ID (보통 자신)
    ///
    /// # Returns
    /// 브로드캐스트 대상 플레이어 ID 목록
    pub async fn get_broadcast_targets(
        &self,
        room_id: u16,
        exclude_player_id: Option<PlayerId>,
    ) -> Result<Vec<PlayerId>> {
        let users = self.get_room_users(room_id).await?;

        let targets: Vec<PlayerId> = users
            .iter()
            .filter(|u| u.is_connected)
            .filter(|u| {
                if let Some(exclude_id) = exclude_player_id {
                    u.player_id != exclude_id
                } else {
                    true
                }
            })
            .map(|u| u.player_id)
            .collect();

        Ok(targets)
    }

    /// 비활성 사용자 정리 (주기적 호출용)
    ///
    /// # Arguments
    /// * `timeout_ms` - 타임아웃 시간 (밀리초)
    ///
    /// # Returns
    /// 정리된 사용자 수
    pub async fn cleanup_inactive_users(&self, timeout_ms: u64) -> Result<usize> {
        let current_time = crate::utils::current_timestamp_ms();
        let mut cleaned_count = 0;

        let room_ids: Vec<u16> = {
            let cache = self.room_cache.read().await;
            cache.keys().copied().collect()
        };

        for room_id in room_ids {
            let inactive_players: Vec<PlayerId> = {
                let cache = self.room_cache.read().await;
                if let Some(room_users) = cache.get(&room_id) {
                    room_users
                        .values()
                        .filter(|u| current_time - u.last_updated > timeout_ms)
                        .map(|u| u.player_id)
                        .collect()
                } else {
                    Vec::new()
                }
            };

            for player_id in inactive_players {
                if let Err(e) = self.remove_user_from_room(room_id, player_id).await {
                    error!(
                        player_id = %player_id,
                        room_id = %room_id,
                        error = %e,
                        "비활성 사용자 정리 실패"
                    );
                } else {
                    cleaned_count += 1;
                }
            }
        }

        if cleaned_count > 0 {
            info!(cleaned = %cleaned_count, "비활성 사용자 정리 완료");
        }

        Ok(cleaned_count)
    }

    /// 전체 통계 조회
    pub async fn get_statistics(&self) -> HashMap<String, u64> {
        let cache = self.room_cache.read().await;

        let total_rooms = cache.len() as u64;
        let total_users = cache.values().map(|room| room.len() as u64).sum::<u64>();
        let connected_users = cache
            .values()
            .map(|room| room.values().filter(|u| u.is_connected).count() as u64)
            .sum::<u64>();

        HashMap::from([
            ("total_rooms".to_string(), total_rooms),
            ("total_users".to_string(), total_users),
            ("connected_users".to_string(), connected_users),
        ])
    }
}
