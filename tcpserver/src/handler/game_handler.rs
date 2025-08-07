//! 게임 로직 핸들러
//! 
//! Police Thief 게임 특화 로직을 처리합니다.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use tracing::{info, debug};
use serde::{Serialize, Deserialize};

use crate::service::{ConnectionService, MessageService};
use crate::tool::{Point2D, Rectangle, DataUtils};

/// 게임 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameState {
    Waiting,    // 플레이어 대기 중
    Starting,   // 게임 시작 중
    Playing,    // 게임 진행 중
    Ending,     // 게임 종료 중
    Finished,   // 게임 완료
}

/// 플레이어 역할
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlayerRole {
    Police,
    Thief,
    Spectator,
}

/// 플레이어 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub client_id: u32,
    pub nickname: String,
    pub role: PlayerRole,
    pub position: Point2D,
    pub is_alive: bool,
    pub score: u32,
    pub last_activity: i64,
}

/// 게임 룸
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRoom {
    pub room_id: u32,
    pub name: String,
    pub state: GameState,
    pub players: HashMap<u32, Player>,
    pub max_players: u32,
    pub game_area: Rectangle,
    pub created_at: i64,
    pub started_at: Option<i64>,
}

/// 게임 핸들러
pub struct GameHandler {
    connection_service: Arc<ConnectionService>,
    message_service: Arc<MessageService>,
    game_rooms: Arc<Mutex<HashMap<u32, GameRoom>>>,
    next_room_id: Arc<Mutex<u32>>,
    game_config: GameConfig,
}

/// 게임 설정
#[derive(Debug, Clone)]
pub struct GameConfig {
    pub max_rooms: u32,
    pub max_players_per_room: u32,
    pub game_area_width: f64,
    pub game_area_height: f64,
    pub police_catch_distance: f64,
    pub game_duration_secs: u64,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            max_rooms: 100,
            max_players_per_room: 10,
            game_area_width: 1000.0,
            game_area_height: 1000.0,
            police_catch_distance: 50.0,
            game_duration_secs: 300, // 5분
        }
    }
}

impl GameHandler {
    /// 새로운 게임 핸들러 생성
    pub fn new(
        connection_service: Arc<ConnectionService>,
        message_service: Arc<MessageService>,
        config: GameConfig,
    ) -> Self {
        Self {
            connection_service,
            message_service,
            game_rooms: Arc::new(Mutex::new(HashMap::new())),
            next_room_id: Arc::new(Mutex::new(1)),
            game_config: config,
        }
    }
    
    /// 기본 설정으로 생성
    pub fn with_default_config(
        connection_service: Arc<ConnectionService>,
        message_service: Arc<MessageService>,
    ) -> Self {
        Self::new(connection_service, message_service, GameConfig::default())
    }
    
    /// 새로운 게임 룸 생성
    pub async fn create_room(&self, creator_client_id: u32, room_name: String) -> Result<u32> {
        let room_count = self.game_rooms.lock().await.len();
        if room_count >= self.game_config.max_rooms as usize {
            return Err(anyhow!("최대 룸 수 초과: {}/{}", room_count, self.game_config.max_rooms));
        }
        
        let mut next_id = self.next_room_id.lock().await;
        let room_id = *next_id;
        *next_id += 1;
        drop(next_id);
        
        let game_area = Rectangle::new(
            0.0, 
            0.0, 
            self.game_config.game_area_width, 
            self.game_config.game_area_height
        );
        
        let room = GameRoom {
            room_id,
            name: room_name.clone(),
            state: GameState::Waiting,
            players: HashMap::new(),
            max_players: self.game_config.max_players_per_room,
            game_area,
            created_at: DataUtils::current_timestamp(),
            started_at: None,
        };
        
        self.game_rooms.lock().await.insert(room_id, room);
        
        info!("✅ 게임 룸 생성: {} (ID: {}, 생성자: {})", room_name, room_id, creator_client_id);
        Ok(room_id)
    }
    
    /// 플레이어를 룸에 추가
    pub async fn join_room(&self, client_id: u32, room_id: u32, nickname: String) -> Result<()> {
        let mut rooms = self.game_rooms.lock().await;
        
        let room = rooms.get_mut(&room_id)
            .ok_or_else(|| anyhow!("룸을 찾을 수 없습니다: {}", room_id))?;
        
        if room.players.len() >= room.max_players as usize {
            return Err(anyhow!("룸이 가득 참: {}/{}", room.players.len(), room.max_players));
        }
        
        if room.players.contains_key(&client_id) {
            return Err(anyhow!("이미 룸에 참가한 플레이어입니다"));
        }
        
        // 역할 자동 할당 (간단한 로직)
        let role = self.assign_player_role(&room.players);
        
        // 시작 위치 할당
        let position = self.assign_starting_position(&room.players, &role, &room.game_area);
        
        let player = Player {
            client_id,
            nickname: nickname.clone(),
            role,
            position,
            is_alive: true,
            score: 0,
            last_activity: DataUtils::current_timestamp(),
        };
        
        room.players.insert(client_id, player);
        
        info!("플레이어 {}({})가 룸 {}에 참가 (역할: {:?})", nickname, client_id, room_id, role);
        Ok(())
    }
    
    /// 플레이어 역할 할당
    fn assign_player_role(&self, existing_players: &HashMap<u32, Player>) -> PlayerRole {
        let police_count = existing_players.values()
            .filter(|p| p.role == PlayerRole::Police)
            .count();
        let thief_count = existing_players.values()
            .filter(|p| p.role == PlayerRole::Thief)
            .count();
        
        // 간단한 밸런싱: 경찰 1명당 도둑 2-3명
        if police_count == 0 || (thief_count > 0 && thief_count / police_count >= 3) {
            PlayerRole::Police
        } else {
            PlayerRole::Thief
        }
    }
    
    /// 시작 위치 할당
    fn assign_starting_position(&self, _existing_players: &HashMap<u32, Player>, role: &PlayerRole, area: &Rectangle) -> Point2D {
        let center = area.center();
        
        match role {
            PlayerRole::Police => {
                // 경찰은 중앙 근처에서 시작
                Point2D::new(
                    center.x + (rand::random::<f64>() - 0.5) * 100.0,
                    center.y + (rand::random::<f64>() - 0.5) * 100.0,
                )
            }
            PlayerRole::Thief => {
                // 도둑은 가장자리에서 시작
                let edge = rand::random::<u8>() % 4;
                match edge {
                    0 => Point2D::new(rand::random::<f64>() * area.width, 0.0), // 상단
                    1 => Point2D::new(area.width, rand::random::<f64>() * area.height), // 우측
                    2 => Point2D::new(rand::random::<f64>() * area.width, area.height), // 하단
                    _ => Point2D::new(0.0, rand::random::<f64>() * area.height), // 좌측
                }
            }
            PlayerRole::Spectator => center, // 관전자는 중앙
        }
    }
    
    /// 플레이어 위치 업데이트
    pub async fn update_player_position(&self, client_id: u32, room_id: u32, new_position: Point2D) -> Result<()> {
        let mut rooms = self.game_rooms.lock().await;
        
        let room = rooms.get_mut(&room_id)
            .ok_or_else(|| anyhow!("룸을 찾을 수 없습니다: {}", room_id))?;
        
        let player = room.players.get_mut(&client_id)
            .ok_or_else(|| anyhow!("플레이어를 찾을 수 없습니다: {}", client_id))?;
        
        // 게임 영역 내부 확인
        if !room.game_area.contains_point(&new_position) {
            return Err(anyhow!("게임 영역을 벗어난 위치입니다"));
        }
        
        player.position = new_position;
        player.last_activity = DataUtils::current_timestamp();
        
        debug!("플레이어 {} 위치 업데이트: ({}, {})", client_id, new_position.x, new_position.y);
        
        // 충돌 감지 (경찰-도둑)
        if player.role == PlayerRole::Police {
            self.check_police_catches(room, client_id).await?;
        }
        
        Ok(())
    }
    
    /// 경찰의 도둑 체포 확인
    async fn check_police_catches(&self, room: &mut GameRoom, police_id: u32) -> Result<()> {
        let police_pos = room.players.get(&police_id)
            .ok_or_else(|| anyhow!("경찰을 찾을 수 없습니다"))?
            .position;
        
        let catch_distance = self.game_config.police_catch_distance;
        let mut caught_thieves = Vec::new();
        
        for (thief_id, thief) in room.players.iter() {
            if thief.role == PlayerRole::Thief && thief.is_alive {
                let distance = police_pos.distance_to(&thief.position);
                if distance <= catch_distance {
                    caught_thieves.push(*thief_id);
                }
            }
        }
        
        // 도둑 체포 처리
        for thief_id in caught_thieves {
            if let Some(thief) = room.players.get_mut(&thief_id) {
                thief.is_alive = false;
                info!("도둑 체포: 경찰 {} -> 도둑 {}", police_id, thief_id);
                
                // 점수 업데이트
                if let Some(police) = room.players.get_mut(&police_id) {
                    police.score += 100;
                }
            }
        }
        
        Ok(())
    }
    
    /// 게임 시작
    pub async fn start_game(&self, room_id: u32) -> Result<()> {
        let mut rooms = self.game_rooms.lock().await;
        
        let room = rooms.get_mut(&room_id)
            .ok_or_else(|| anyhow!("룸을 찾을 수 없습니다: {}", room_id))?;
        
        if room.players.len() < 2 {
            return Err(anyhow!("게임 시작을 위해 최소 2명의 플레이어가 필요합니다"));
        }
        
        if room.state != GameState::Waiting {
            return Err(anyhow!("게임을 시작할 수 없는 상태입니다: {:?}", room.state));
        }
        
        room.state = GameState::Starting;
        room.started_at = Some(DataUtils::current_timestamp());
        
        info!("🎮 게임 시작: 룸 {} (플레이어 {}명)", room_id, room.players.len());
        
        // 3초 후 게임 상태를 Playing으로 변경
        let rooms_ref = self.game_rooms.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            
            if let Ok(mut rooms) = rooms_ref.try_lock() {
                if let Some(room) = rooms.get_mut(&room_id) {
                    if room.state == GameState::Starting {
                        room.state = GameState::Playing;
                        info!("🎯 게임 플레이 시작: 룸 {}", room_id);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// 게임 종료
    pub async fn end_game(&self, room_id: u32, reason: &str) -> Result<GameResult> {
        let mut rooms = self.game_rooms.lock().await;
        
        let room = rooms.get_mut(&room_id)
            .ok_or_else(|| anyhow!("룸을 찾을 수 없습니다: {}", room_id))?;
        
        if !matches!(room.state, GameState::Playing | GameState::Starting) {
            return Err(anyhow!("게임을 종료할 수 없는 상태입니다: {:?}", room.state));
        }
        
        room.state = GameState::Ending;
        
        // 게임 결과 계산
        let result = self.calculate_game_result(room);
        
        // 1초 후 게임 상태를 Finished로 변경
        let rooms_ref = self.game_rooms.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            
            if let Ok(mut rooms) = rooms_ref.try_lock() {
                if let Some(room) = rooms.get_mut(&room_id) {
                    room.state = GameState::Finished;
                    info!("🏁 게임 완료: 룸 {} ({})", room_id, reason);
                }
            }
        });
        
        info!("🏆 게임 종료: 룸 {} - {}", room_id, reason);
        Ok(result)
    }
    
    /// 게임 결과 계산
    fn calculate_game_result(&self, room: &GameRoom) -> GameResult {
        let alive_thieves = room.players.values()
            .filter(|p| p.role == PlayerRole::Thief && p.is_alive)
            .count();
        
        let total_thieves = room.players.values()
            .filter(|p| p.role == PlayerRole::Thief)
            .count();
        
        let police_count = room.players.values()
            .filter(|p| p.role == PlayerRole::Police)
            .count();
        
        let winner = if alive_thieves == 0 {
            PlayerRole::Police
        } else if police_count == 0 {
            PlayerRole::Thief
        } else {
            // 시간 초과 등의 경우 살아있는 도둑이 많으면 도둑 승리
            if alive_thieves > total_thieves / 2 {
                PlayerRole::Thief
            } else {
                PlayerRole::Police
            }
        };
        
        let duration_secs = room.started_at
            .map(|start| DataUtils::current_timestamp() - start)
            .unwrap_or(0);
        
        GameResult {
            room_id: room.room_id,
            winner,
            duration_seconds: duration_secs,
            total_players: room.players.len(),
            alive_thieves,
            total_thieves,
            police_count,
            final_scores: room.players.iter()
                .map(|(id, player)| (*id, player.score))
                .collect(),
        }
    }
    
    /// 룸 목록 조회
    pub async fn get_room_list(&self) -> Vec<RoomInfo> {
        let rooms = self.game_rooms.lock().await;
        
        rooms.values()
            .map(|room| RoomInfo {
                room_id: room.room_id,
                name: room.name.clone(),
                state: room.state.clone(),
                current_players: room.players.len(),
                max_players: room.max_players as usize,
                created_at: room.created_at,
            })
            .collect()
    }
    
    /// 룸 상세 정보 조회
    pub async fn get_room_details(&self, room_id: u32) -> Result<GameRoom> {
        let rooms = self.game_rooms.lock().await;
        
        rooms.get(&room_id)
            .cloned()
            .ok_or_else(|| anyhow!("룸을 찾을 수 없습니다: {}", room_id))
    }
    
    /// 플레이어를 룸에서 제거
    pub async fn leave_room(&self, client_id: u32, room_id: u32) -> Result<()> {
        let mut rooms = self.game_rooms.lock().await;
        
        let room = rooms.get_mut(&room_id)
            .ok_or_else(|| anyhow!("룸을 찾을 수 없습니다: {}", room_id))?;
        
        if let Some(player) = room.players.remove(&client_id) {
            info!("플레이어 {}({})가 룸 {}에서 나감", player.nickname, client_id, room_id);
            
            // 룸이 비었으면 삭제
            if room.players.is_empty() {
                rooms.remove(&room_id);
                info!("빈 룸 삭제: {}", room_id);
            }
            
            Ok(())
        } else {
            Err(anyhow!("플레이어가 룸에 없습니다: {}", client_id))
        }
    }
    
    /// 게임 통계 조회
    pub async fn get_game_stats(&self) -> GameStats {
        let rooms = self.game_rooms.lock().await;
        
        let total_rooms = rooms.len();
        let total_players = rooms.values().map(|r| r.players.len()).sum();
        let active_games = rooms.values()
            .filter(|r| matches!(r.state, GameState::Playing))
            .count();
        
        let mut state_distribution = HashMap::new();
        for room in rooms.values() {
            *state_distribution.entry(format!("{:?}", room.state)).or_insert(0) += 1;
        }
        
        GameStats {
            total_rooms,
            total_players,
            active_games,
            state_distribution,
            max_rooms: self.game_config.max_rooms,
            max_players_per_room: self.game_config.max_players_per_room,
        }
    }
    
    /// 룸 정리 (빈 룸, 오래된 룸)
    pub async fn cleanup_rooms(&self) -> usize {
        let mut rooms = self.game_rooms.lock().await;
        let current_time = DataUtils::current_timestamp();
        let mut removed_count = 0;
        
        let mut rooms_to_remove = Vec::new();
        
        for (room_id, room) in rooms.iter() {
            let should_remove = room.players.is_empty() || 
                (room.state == GameState::Finished && current_time - room.created_at > 3600); // 1시간 후 정리
            
            if should_remove {
                rooms_to_remove.push(*room_id);
            }
        }
        
        for room_id in rooms_to_remove {
            rooms.remove(&room_id);
            removed_count += 1;
            debug!("룸 정리: {}", room_id);
        }
        
        if removed_count > 0 {
            info!("룸 정리 완료: {}개", removed_count);
        }
        
        removed_count
    }
}

/// 룸 정보 (목록용)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub room_id: u32,
    pub name: String,
    pub state: GameState,
    pub current_players: usize,
    pub max_players: usize,
    pub created_at: i64,
}

/// 게임 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameResult {
    pub room_id: u32,
    pub winner: PlayerRole,
    pub duration_seconds: i64,
    pub total_players: usize,
    pub alive_thieves: usize,
    pub total_thieves: usize,
    pub police_count: usize,
    pub final_scores: HashMap<u32, u32>,
}

/// 게임 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStats {
    pub total_rooms: usize,
    pub total_players: usize,
    pub active_games: usize,
    pub state_distribution: HashMap<String, u32>,
    pub max_rooms: u32,
    pub max_players_per_room: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_game_handler() {
        let connection_service = Arc::new(ConnectionService::new(100));
        let message_service = Arc::new(MessageService::new(connection_service.clone()));
        let game_handler = GameHandler::with_default_config(connection_service, message_service);
        
        // 룸 생성
        let room_id = game_handler.create_room(1, "테스트 룸".to_string()).await.unwrap();
        assert_eq!(room_id, 1);
        
        // 플레이어 참가
        assert!(game_handler.join_room(1, room_id, "Player1".to_string()).await.is_ok());
        assert!(game_handler.join_room(2, room_id, "Player2".to_string()).await.is_ok());
        
        // 룸 목록 확인
        let rooms = game_handler.get_room_list().await;
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].current_players, 2);
        
        // 게임 시작
        assert!(game_handler.start_game(room_id).await.is_ok());
    }
    
    #[test]
    fn test_player_role_assignment() {
        let connection_service = Arc::new(ConnectionService::new(100));
        let message_service = Arc::new(MessageService::new(connection_service.clone()));
        let game_handler = GameHandler::with_default_config(connection_service, message_service);
        
        let empty_players = HashMap::new();
        let role1 = game_handler.assign_player_role(&empty_players);
        assert_eq!(role1, PlayerRole::Police); // 첫 번째는 경찰
        
        // 경찰이 있는 상태에서 추가
        let mut players = HashMap::new();
        players.insert(1, Player {
            client_id: 1,
            nickname: "Police1".to_string(),
            role: PlayerRole::Police,
            position: Point2D::origin(),
            is_alive: true,
            score: 0,
            last_activity: 0,
        });
        
        let role2 = game_handler.assign_player_role(&players);
        assert_eq!(role2, PlayerRole::Thief); // 두 번째는 도둑
    }
}