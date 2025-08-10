//! 방 기반 연결 관리 서비스
//! 
//! DashMap을 사용하여 방(room_id) 기반으로 사용자 연결을 관리합니다.
//! Redis 백업을 통해 데이터 영속성과 서버 간 상태 공유를 지원합니다.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{Duration, Instant};
use tracing::{info, error, warn, debug};
use chrono;
use dashmap::DashMap;
use serde::{Serialize, Deserialize};

use crate::protocol::GameMessage;
use crate::service::atomic_stats::AtomicStats;
use shared::config::redis_config::RedisConfig;
use redis::AsyncCommands;

/// 사용자 연결 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomUserConnection {
    pub user_id: u32,
    pub room_id: u32,
    pub addr: String,
    pub nickname: String,
    pub connected_at: i64,
    pub last_heartbeat: i64,
    #[serde(skip)]
    pub writer: Option<Arc<Mutex<tokio::io::BufWriter<tokio::net::tcp::OwnedWriteHalf>>>>,
}

impl RoomUserConnection {
    pub fn new(user_id: u32, room_id: u32, addr: String, nickname: String, writer: Arc<Mutex<tokio::io::BufWriter<tokio::net::tcp::OwnedWriteHalf>>>) -> Self {
        let now = chrono::Utc::now().timestamp();
        
        Self {
            user_id,
            room_id,
            addr,
            nickname,
            connected_at: now,
            last_heartbeat: now,
            writer: Some(writer),
        }
    }
    
    /// 메시지 전송
    pub async fn send_message(&self, message: &GameMessage) -> Result<()> {
        if let Some(writer) = &self.writer {
            let mut writer_guard = writer.lock().await;
            message.write_to_stream(&mut *writer_guard).await
        } else {
            Err(anyhow!("Writer not available for user {}", self.user_id))
        }
    }
    
    /// 하트비트 업데이트
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = chrono::Utc::now().timestamp();
    }
    
    /// 하트비트 타임아웃 체크
    pub fn is_heartbeat_timeout(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        (now - self.last_heartbeat) > 1800 // 30분
    }
}

/// 방 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub room_id: u32,
    pub user_count: u32,
    pub created_at: i64,
    pub last_activity: i64,
}

impl RoomInfo {
    pub fn new(room_id: u32) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            room_id,
            user_count: 0,
            created_at: now,
            last_activity: now,
        }
    }
    
    pub fn update_activity(&mut self) {
        self.last_activity = chrono::Utc::now().timestamp();
    }
}

/// 연결 통계
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoomConnectionStats {
    pub total_rooms: u32,
    pub total_users: u32,
    pub total_connections: u64,
    pub total_messages_sent: u64,
    pub failed_messages: u64,
    pub redis_sync_count: u64,
    pub redis_sync_failures: u64,
}

/// 방 기반 연결 관리 서비스
pub struct RoomConnectionService {
    /// 방별 사용자 연결 정보: room_id -> HashMap<user_id, RoomUserConnection>
    room_connections: Arc<DashMap<u32, HashMap<u32, RoomUserConnection>>>,
    
    /// 사용자 -> 방 매핑: user_id -> room_id
    user_room_map: Arc<DashMap<u32, u32>>,
    
    /// 방 정보: room_id -> RoomInfo
    room_info: Arc<DashMap<u32, RoomInfo>>,
    
    /// 브로드캐스트 채널
    broadcast_tx: broadcast::Sender<(Option<u32>, GameMessage)>,
    
    /// Redis 설정 (Phase 2 백업용)
    redis_config: Option<Arc<RedisConfig>>,
    
    /// 서버 ID
    server_id: String,
    
    /// 통계 (기존 유지용)
    stats: Arc<Mutex<RoomConnectionStats>>,
    
    /// 원자적 통계 시스템
    atomic_stats: Arc<AtomicStats>,
    
    /// 서버 시작 시간
    server_start_time: Instant,
    
    /// Redis 동기화 태스크 핸들
    sync_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl RoomConnectionService {
    /// 새로운 방 기반 연결 서비스 생성
    pub fn new(server_id: String) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        
        Self {
            room_connections: Arc::new(DashMap::new()),
            user_room_map: Arc::new(DashMap::new()),
            room_info: Arc::new(DashMap::new()),
            broadcast_tx,
            redis_config: None,
            server_id,
            stats: Arc::new(Mutex::new(RoomConnectionStats::default())),
            atomic_stats: Arc::new(AtomicStats::new()),
            server_start_time: Instant::now(),
            sync_handle: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Redis 백업 설정 추가 (Phase 2)
    pub async fn with_redis_backup(mut self) -> Result<Self> {
        match RedisConfig::new().await {
            Ok(config) => {
                self.redis_config = Some(Arc::new(config));
                info!("Redis 백업 활성화됨");
                self.start_redis_sync().await?;
                Ok(self)
            }
            Err(e) => {
                warn!("Redis 백업 비활성화: {}", e);
                Ok(self)
            }
        }
    }
    
    /// 사용자를 방에 추가
    pub async fn add_user_to_room(
        &self, 
        room_id: u32, 
        user_id: u32, 
        addr: String, 
        nickname: String,
        writer: Arc<Mutex<tokio::io::BufWriter<tokio::net::tcp::OwnedWriteHalf>>>
    ) -> Result<()> {
        debug!("사용자 {} 방 {}에 추가: {}", user_id, room_id, addr);
        
        // 기존 연결이 있으면 제거
        if let Some(old_room_id) = self.user_room_map.get(&user_id) {
            let old_room = *old_room_id;
            if old_room != room_id {
                self.remove_user_from_room(old_room, user_id).await?;
            }
        }
        
        let connection = RoomUserConnection::new(user_id, room_id, addr, nickname, writer);
        
        // 방이 존재하지 않으면 생성
        self.room_connections.entry(room_id).or_insert_with(HashMap::new);
        self.room_info.entry(room_id).or_insert_with(|| RoomInfo::new(room_id));
        
        // 사용자 연결 추가
        if let Some(mut room_users) = self.room_connections.get_mut(&room_id) {
            room_users.insert(user_id, connection.clone());
        }
        
        // 사용자 -> 방 매핑 업데이트
        self.user_room_map.insert(user_id, room_id);
        
        // 방 정보 업데이트
        if let Some(mut info) = self.room_info.get_mut(&room_id) {
            info.user_count = self.get_room_user_count(room_id);
            info.update_activity();
        }
        
        // 통계 업데이트 (기존)
        self.update_stats(|stats| {
            stats.total_connections += 1;
            stats.total_users = self.get_total_users();
            stats.total_rooms = self.room_connections.len() as u32;
        }).await;
        
        // 원자적 통계 업데이트
        self.atomic_stats.record_connection();
        if !self.room_info.contains_key(&room_id) {
            self.atomic_stats.record_room_created();
        }
        self.atomic_stats.record_room_join();
        
        // Redis에 동기화 (비동기)
        if let Some(redis_config) = &self.redis_config {
            let redis_config = redis_config.clone();
            let server_id = self.server_id.clone();
            let conn_clone = connection.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::sync_user_to_redis(redis_config, server_id, conn_clone).await {
                    error!("Redis 사용자 동기화 실패: {}", e);
                }
            });
        }
        
        info!("✅ 사용자 {} 방 {}에 추가 완료", user_id, room_id);
        Ok(())
    }
    
    /// 사용자를 방에서 제거
    pub async fn remove_user_from_room(&self, room_id: u32, user_id: u32) -> Result<()> {
        debug!("사용자 {} 방 {}에서 제거", user_id, room_id);
        
        // 방에서 사용자 제거
        let user_removed = if let Some(mut room_users) = self.room_connections.get_mut(&room_id) {
            room_users.remove(&user_id).is_some()
        } else {
            false
        };
        
        if user_removed {
            // 사용자 -> 방 매핑 제거
            self.user_room_map.remove(&user_id);
            
            // 방 정보 업데이트
            if let Some(mut info) = self.room_info.get_mut(&room_id) {
                info.user_count = self.get_room_user_count(room_id);
                info.update_activity();
            }
            
            // 빈 방이면 제거
            let room_deleted = self.get_room_user_count(room_id) == 0;
            if room_deleted {
                self.room_connections.remove(&room_id);
                self.room_info.remove(&room_id);
                debug!("빈 방 {} 제거됨", room_id);
                
                // 원자적 통계 - 방 삭제 기록
                self.atomic_stats.record_room_deleted();
            }
            
            // 통계 업데이트 (기존)
            self.update_stats(|stats| {
                stats.total_users = self.get_total_users();
                stats.total_rooms = self.room_connections.len() as u32;
            }).await;
            
            // 원자적 통계 업데이트
            self.atomic_stats.record_disconnection();
            self.atomic_stats.record_room_leave();
            
            // Redis에서 제거 (비동기)
            if let Some(redis_config) = &self.redis_config {
                let redis_config = redis_config.clone();
                let server_id = self.server_id.clone();
                tokio::spawn(async move {
                    if let Err(e) = Self::remove_user_from_redis(redis_config, server_id, room_id, user_id).await {
                        error!("Redis 사용자 제거 실패: {}", e);
                    }
                });
            }
            
            info!("✅ 사용자 {} 방 {}에서 제거 완료", user_id, room_id);
            Ok(())
        } else {
            Err(anyhow!("사용자 {}가 방 {}에 없습니다", user_id, room_id))
        }
    }
    
    /// 방의 모든 사용자에게 메시지 전송
    pub async fn send_to_room(&self, room_id: u32, message: &GameMessage) -> Result<usize> {
        debug!("방 {}에 메시지 전송: {:?}", room_id, message);
        
        let start_time = std::time::Instant::now();
        let mut success_count = 0;
        let mut failed_count = 0;
        
        if let Some(room_users) = self.room_connections.get(&room_id) {
            for (user_id, connection) in room_users.iter() {
                match connection.send_message(message).await {
                    Ok(()) => {
                        success_count += 1;
                        debug!("사용자 {}에게 메시지 전송 성공", user_id);
                    }
                    Err(e) => {
                        failed_count += 1;
                        warn!("사용자 {}에게 메시지 전송 실패: {}", user_id, e);
                    }
                }
            }
        } else {
            return Err(anyhow!("방 {}가 존재하지 않습니다", room_id));
        }
        
        let processing_time = start_time.elapsed();
        
        // 통계 업데이트 (기존)
        self.update_stats(|stats| {
            stats.total_messages_sent += success_count;
            stats.failed_messages += failed_count;
        }).await;
        
        // 원자적 통계 업데이트
        let message_type = match message {
            GameMessage::ChatMessage { .. } => "chat",
            GameMessage::HeartBeat => "heartbeat",
            GameMessage::RoomJoinSuccess { .. } | GameMessage::RoomLeaveSuccess { .. } |
            GameMessage::UserJoinedRoom { .. } | GameMessage::UserLeftRoom { .. } => "room",
            GameMessage::SystemMessage { .. } => "system",
            GameMessage::Error { .. } => "error",
            _ => "other",
        };
        
        self.atomic_stats.record_message_processing(message_type, processing_time);
        self.atomic_stats.record_broadcast_time(processing_time);
        
        // 에러 카운트 기록
        for _ in 0..failed_count {
            self.atomic_stats.record_broadcast_error();
        }
        
        info!("방 {} 메시지 전송 완료: 성공 {}, 실패 {}", room_id, success_count, failed_count);
        Ok(success_count as usize)
    }
    
    /// 특정 사용자에게 메시지 전송
    pub async fn send_to_user_in_room(&self, room_id: u32, user_id: u32, message: &GameMessage) -> Result<()> {
        if let Some(room_users) = self.room_connections.get(&room_id) {
            if let Some(connection) = room_users.get(&user_id) {
                connection.send_message(message).await?;
                
                // 통계 업데이트
                self.update_stats(|stats| {
                    stats.total_messages_sent += 1;
                }).await;
                
                debug!("사용자 {} (방 {})에게 메시지 전송 완료", user_id, room_id);
                Ok(())
            } else {
                Err(anyhow!("사용자 {}가 방 {}에 없습니다", user_id, room_id))
            }
        } else {
            Err(anyhow!("방 {}가 존재하지 않습니다", room_id))
        }
    }
    
    /// 방의 모든 사용자 목록 조회
    pub fn get_room_users(&self, room_id: u32) -> Vec<RoomUserConnection> {
        if let Some(room_users) = self.room_connections.get(&room_id) {
            room_users.values().cloned().collect()
        } else {
            Vec::new()
        }
    }
    
    /// 사용자가 속한 방 조회
    pub fn get_user_room(&self, user_id: u32) -> Option<u32> {
        self.user_room_map.get(&user_id).map(|room_id| *room_id)
    }
    
    /// 방의 사용자 수 조회
    pub fn get_room_user_count(&self, room_id: u32) -> u32 {
        self.room_connections.get(&room_id)
            .map(|users| users.len() as u32)
            .unwrap_or(0)
    }
    
    /// 총 사용자 수 조회
    pub fn get_total_users(&self) -> u32 {
        self.user_room_map.len() as u32
    }
    
    /// 총 방 수 조회
    pub fn get_total_rooms(&self) -> u32 {
        self.room_connections.len() as u32
    }
    
    /// 모든 방 목록 조회
    pub fn get_all_rooms(&self) -> Vec<RoomInfo> {
        self.room_info.iter().map(|entry| entry.value().clone()).collect()
    }
    
    /// 빈 방들 정리
    pub async fn cleanup_empty_rooms(&self) -> usize {
        let mut removed_count = 0;
        let empty_rooms: Vec<u32> = self.room_connections
            .iter()
            .filter(|entry| entry.value().is_empty())
            .map(|entry| *entry.key())
            .collect();
            
        for room_id in empty_rooms {
            self.room_connections.remove(&room_id);
            self.room_info.remove(&room_id);
            removed_count += 1;
            debug!("빈 방 {} 제거됨", room_id);
        }
        
        if removed_count > 0 {
            info!("빈 방 {}개 정리 완료", removed_count);
        }
        
        removed_count
    }
    
    /// 하트비트 타임아웃된 연결들 정리
    pub async fn cleanup_timeout_connections(&self) -> usize {
        let mut removed_count = 0;
        let mut timeout_users = Vec::new();
        
        // 타임아웃된 사용자들 찾기
        for room_entry in self.room_connections.iter() {
            let room_id = *room_entry.key();
            for (user_id, connection) in room_entry.value().iter() {
                if connection.is_heartbeat_timeout() {
                    timeout_users.push((room_id, *user_id));
                }
            }
        }
        
        // 타임아웃된 사용자들 제거
        for (room_id, user_id) in timeout_users {
            if let Ok(()) = self.remove_user_from_room(room_id, user_id).await {
                removed_count += 1;
                warn!("사용자 {} (방 {}) 하트비트 타임아웃으로 제거", user_id, room_id);
            }
        }
        
        removed_count
    }
    
    /// 사용자를 다른 방으로 이동
    pub async fn move_user_to_room(&self, user_id: u32, to_room_id: u32) -> Result<()> {
        if let Some(from_room_id) = self.get_user_room(user_id) {
            if from_room_id == to_room_id {
                return Ok(()); // 같은 방이면 이동 불필요
            }
            
            // 기존 연결 정보 가져오기
            let connection = if let Some(room_users) = self.room_connections.get(&from_room_id) {
                room_users.get(&user_id).cloned()
            } else {
                None
            };
            
            if let Some(mut conn) = connection {
                // 기존 방에서 제거
                self.remove_user_from_room(from_room_id, user_id).await?;
                
                // 새 방으로 이동 (room_id 업데이트)
                conn.room_id = to_room_id;
                
                if let Some(writer) = conn.writer.clone() {
                    self.add_user_to_room(to_room_id, user_id, conn.addr.clone(), conn.nickname.clone(), writer).await?;
                    info!("사용자 {} 방 {}에서 방 {}로 이동 완료", user_id, from_room_id, to_room_id);
                } else {
                    return Err(anyhow!("사용자 {}의 연결 정보가 유효하지 않습니다", user_id));
                }
            } else {
                return Err(anyhow!("사용자 {}의 연결 정보를 찾을 수 없습니다", user_id));
            }
        } else {
            return Err(anyhow!("사용자 {}가 어떤 방에도 속하지 않습니다", user_id));
        }
        
        Ok(())
    }
    
    /// 통계 업데이트
    async fn update_stats<F>(&self, update_fn: F) 
    where 
        F: FnOnce(&mut RoomConnectionStats)
    {
        if let Ok(mut stats) = self.stats.try_lock() {
            update_fn(&mut *stats);
        }
    }
    
    /// 통계 조회
    pub async fn get_stats(&self) -> RoomConnectionStats {
        self.stats.lock().await.clone()
    }
    
    /// 브로드캐스트 수신자 생성
    pub fn subscribe_broadcast(&self) -> broadcast::Receiver<(Option<u32>, GameMessage)> {
        self.broadcast_tx.subscribe()
    }
}

// Phase 2: Redis 백업 구현
impl RoomConnectionService {
    /// Redis 동기화 시작
    async fn start_redis_sync(&self) -> Result<()> {
        if self.redis_config.is_none() {
            return Ok(());
        }
        
        let room_connections = self.room_connections.clone();
        let redis_config = self.redis_config.as_ref().unwrap().clone();
        let server_id = self.server_id.clone();
        let stats = self.stats.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // 1분마다
            
            loop {
                interval.tick().await;
                
                if let Err(e) = Self::sync_all_to_redis(&room_connections, &redis_config, &server_id).await {
                    error!("Redis 전체 동기화 실패: {}", e);
                    if let Ok(mut stats) = stats.try_lock() {
                        stats.redis_sync_failures += 1;
                    }
                } else {
                    debug!("Redis 전체 동기화 완료");
                    if let Ok(mut stats) = stats.try_lock() {
                        stats.redis_sync_count += 1;
                    }
                }
            }
        });
        
        *self.sync_handle.lock().await = Some(handle);
        info!("Redis 동기화 태스크 시작");
        Ok(())
    }
    
    /// 모든 데이터를 Redis에 동기화
    async fn sync_all_to_redis(
        room_connections: &DashMap<u32, HashMap<u32, RoomUserConnection>>,
        redis_config: &RedisConfig,
        server_id: &str,
    ) -> Result<()> {
        let mut conn = redis_config.get_connection();
        let mut pipeline = redis::pipe();
        
        // 서버별 연결 정보 동기화
        for room_entry in room_connections.iter() {
            let room_id = *room_entry.key();
            let users = room_entry.value();
            
            for (user_id, connection) in users.iter() {
                let conn_json = serde_json::to_string(connection)?;
                
                pipeline
                    .hset(format!("tcp_server:{}:room:{}", server_id, room_id), user_id, &conn_json)
                    .sadd(format!("room:{}:users", room_id), user_id)
                    .hset(format!("user:{}:location", user_id), "server_id", server_id)
                    .hset(format!("user:{}:location", user_id), "room_id", room_id)
                    .expire(format!("user:{}:location", user_id), 7200); // 2시간
            }
        }
        
        pipeline.query_async::<_, ()>(&mut conn).await?;
        Ok(())
    }
    
    /// 개별 사용자를 Redis에 동기화
    async fn sync_user_to_redis(
        redis_config: Arc<RedisConfig>, 
        server_id: String, 
        connection: RoomUserConnection
    ) -> Result<()> {
        let mut conn = redis_config.get_connection();
        let conn_json = serde_json::to_string(&connection)?;
        
        let mut pipeline = redis::pipe();
        pipeline
            .hset(format!("tcp_server:{}:room:{}", server_id, connection.room_id), connection.user_id, &conn_json)
            .sadd(format!("room:{}:users", connection.room_id), connection.user_id)
            .hset(format!("user:{}:location", connection.user_id), "server_id", &server_id)
            .hset(format!("user:{}:location", connection.user_id), "room_id", connection.room_id)
            .expire(format!("user:{}:location", connection.user_id), 7200); // 2시간
            
        pipeline.query_async::<_, ()>(&mut conn).await?;
        Ok(())
    }
    
    /// Redis에서 사용자 제거
    async fn remove_user_from_redis(
        redis_config: Arc<RedisConfig>, 
        server_id: String, 
        room_id: u32,
        user_id: u32
    ) -> Result<()> {
        let mut conn = redis_config.get_connection();
        
        let mut pipeline = redis::pipe();
        pipeline
            .hdel(format!("tcp_server:{}:room:{}", server_id, room_id), user_id)
            .srem(format!("room:{}:users", room_id), user_id)
            .del(format!("user:{}:location", user_id));
            
        pipeline.query_async::<_, ()>(&mut conn).await?;
        Ok(())
    }
    
    /// Redis에서 데이터 복원 (서버 시작 시)
    pub async fn restore_from_redis(&self) -> Result<usize> {
        if let Some(redis_config) = &self.redis_config {
            let mut conn = redis_config.get_connection();
            let mut restored_count = 0;
            
            // 서버별 룸 패턴으로 데이터 조회
            let pattern = format!("tcp_server:{}:room:*", self.server_id);
            let keys: Vec<String> = conn.keys(&pattern).await?;
            
            for key in keys {
                if let Ok(room_data) = conn.hgetall::<String, HashMap<String, String>>(key.clone()).await {
                    // 키에서 room_id 추출
                    if let Some(room_id_str) = key.split(':').last() {
                        if let Ok(room_id) = room_id_str.parse::<u32>() {
                            for (user_id_str, conn_json) in room_data {
                                if let (Ok(user_id), Ok(connection)) = (
                                    user_id_str.parse::<u32>(),
                                    serde_json::from_str::<RoomUserConnection>(&conn_json)
                                ) {
                                    // Redis에서는 writer 정보가 없으므로 연결은 나중에 재설정 필요
                                    let mut room_users = HashMap::new();
                                    room_users.insert(user_id, connection);
                                    
                                    self.room_connections.insert(room_id, room_users);
                                    self.user_room_map.insert(user_id, room_id);
                                    restored_count += 1;
                                }
                            }
                            
                            // 방 정보 생성
                            self.room_info.insert(room_id, RoomInfo::new(room_id));
                        }
                    }
                }
            }
            
            if restored_count > 0 {
                info!("Redis에서 {} 연결 복원 완료", restored_count);
            }
            
            Ok(restored_count)
        } else {
            Ok(0)
        }
    }
    
    /// 원자적 통계 접근자
    pub fn get_atomic_stats(&self) -> Arc<AtomicStats> {
        self.atomic_stats.clone()
    }
    
    /// 현재 통계 스냅샷 조회
    pub fn get_performance_snapshot(&self) -> crate::service::atomic_stats::StatsSnapshot {
        self.atomic_stats.get_snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_room_connection_service() {
        let service = RoomConnectionService::new("test_server".to_string());
        
        // 기본 상태 확인
        assert_eq!(service.get_total_rooms(), 0);
        assert_eq!(service.get_total_users(), 0);
        
        // 방 목록이 비어있는지 확인
        assert!(service.get_all_rooms().is_empty());
    }
}