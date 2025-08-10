# TCP 서버 메시지 핸들러 확장 가이드

## 📋 목차
1. [메시지 핸들러 구조](#메시지-핸들러-구조)
2. [새로운 핸들러 생성](#새로운-핸들러-생성)
3. [실전 예시](#실전-예시)
4. [성능 최적화](#성능-최적화)

## 🔧 메시지 핸들러 구조

### 현재 핸들러 계층구조
```
Message Handlers
├── ChatRoomMessageHandler (라우팅 허브)
├── ChatRoomHandler (채팅방 로직)
├── ConnectionHandler (연결 관리)
├── MessageHandler (메시지 처리)
├── RoomHandler (방 관리)
└── FriendHandler (친구 시스템)
```

## 🚀 새로운 핸들러 생성

### 1. 게임 로직 핸들러 예시

#### Step 1: 게임 상태 정의
```rust
// handlers/game_handler.rs
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use crate::protocol::GameMessage;
use crate::service::RoomConnectionService;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub room_id: u32,
    pub status: GameStatus,
    pub players: Vec<Player>,
    pub current_turn: Option<u32>,
    pub round: u32,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameStatus {
    Waiting,
    InProgress,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub user_id: u32,
    pub nickname: String,
    pub score: i32,
    pub ready: bool,
    pub position: Option<Position>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub direction: f32,
}
```

#### Step 2: 게임 핸들러 구현
```rust
pub struct GameHandler {
    room_service: Arc<RoomConnectionService>,
    games: Arc<RwLock<HashMap<u32, GameState>>>, // room_id -> GameState
    config: GameConfig,
}

#[derive(Debug, Clone)]
pub struct GameConfig {
    pub max_players_per_room: u32,
    pub turn_timeout_secs: u64,
    pub round_limit: u32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            max_players_per_room: 10,
            turn_timeout_secs: 30,
            round_limit: 10,
        }
    }
}

impl GameHandler {
    pub fn new(room_service: Arc<RoomConnectionService>) -> Self {
        Self {
            room_service,
            games: Arc::new(RwLock::new(HashMap::new())),
            config: GameConfig::default(),
        }
    }

    /// 게임 시작
    pub async fn start_game(&self, room_id: u32, initiator_id: u32) -> Result<()> {
        let mut games = self.games.write().await;
        
        // 이미 게임이 진행중인지 확인
        if let Some(game) = games.get(&room_id) {
            if matches!(game.status, GameStatus::InProgress) {
                return Err(anyhow!("Game already in progress"));
            }
        }
        
        // 방 정보 확인
        let room_users = self.room_service.get_room_users(room_id);
        if room_users.len() < 2 {
            return Err(anyhow!("Need at least 2 players to start"));
        }
        
        // 플레이어 생성
        let players: Vec<Player> = room_users.iter().map(|user| Player {
            user_id: user.user_id,
            nickname: user.nickname.clone(),
            score: 0,
            ready: user.user_id == initiator_id, // 시작자는 자동 준비
            position: None,
        }).collect();
        
        // 게임 상태 생성
        let game_state = GameState {
            room_id,
            status: GameStatus::Waiting,
            players,
            current_turn: None,
            round: 0,
            started_at: chrono::Utc::now(),
        };
        
        games.insert(room_id, game_state);
        
        // 모든 플레이어에게 게임 시작 알림
        let start_message = GameMessage::GameStarted {
            room_id,
            players: games.get(&room_id).unwrap().players.clone(),
        };
        
        self.room_service.send_to_room(room_id, &start_message).await?;
        
        tracing::info!("Game started in room {} by user {}", room_id, initiator_id);
        Ok(())
    }
    
    /// 플레이어 준비 상태 토글
    pub async fn toggle_ready(&self, room_id: u32, user_id: u32) -> Result<()> {
        let mut games = self.games.write().await;
        
        let game = games.get_mut(&room_id)
            .ok_or_else(|| anyhow!("Game not found"))?;
            
        if !matches!(game.status, GameStatus::Waiting) {
            return Err(anyhow!("Cannot change ready state during game"));
        }
        
        // 플레이어 찾기 및 준비 상태 변경
        if let Some(player) = game.players.iter_mut().find(|p| p.user_id == user_id) {
            player.ready = !player.ready;
            
            // 모든 플레이어가 준비되면 게임 시작
            let all_ready = game.players.iter().all(|p| p.ready);
            if all_ready && game.players.len() >= 2 {
                game.status = GameStatus::InProgress;
                game.current_turn = Some(game.players[0].user_id);
                game.round = 1;
                
                let game_started_message = GameMessage::GameInProgress {
                    room_id,
                    current_turn: game.current_turn,
                    round: game.round,
                };
                
                drop(games); // lock 해제
                
                self.room_service.send_to_room(room_id, &game_started_message).await?;
            } else {
                // 준비 상태 업데이트만 전송
                let ready_update = GameMessage::PlayerReadyUpdate {
                    room_id,
                    user_id,
                    ready: player.ready,
                };
                
                drop(games); // lock 해제
                
                self.room_service.send_to_room(room_id, &ready_update).await?;
            }
        }
        
        Ok(())
    }
    
    /// 플레이어 행동 처리
    pub async fn handle_player_action(
        &self,
        room_id: u32,
        user_id: u32,
        action: PlayerAction,
    ) -> Result<()> {
        let mut games = self.games.write().await;
        
        let game = games.get_mut(&room_id)
            .ok_or_else(|| anyhow!("Game not found"))?;
            
        if !matches!(game.status, GameStatus::InProgress) {
            return Err(anyhow!("Game not in progress"));
        }
        
        // 현재 턴인지 확인
        if game.current_turn != Some(user_id) {
            return Err(anyhow!("Not your turn"));
        }
        
        // 행동에 따른 처리
        match action {
            PlayerAction::Move { x, y, direction } => {
                if let Some(player) = game.players.iter_mut().find(|p| p.user_id == user_id) {
                    player.position = Some(Position { x, y, direction });
                }
                
                // 다음 플레이어로 턴 넘기기
                self.next_turn(game).await?;
            }
            PlayerAction::Attack { target_id, damage } => {
                // 공격 로직 구현
                if let Some(target) = game.players.iter_mut().find(|p| p.user_id == target_id) {
                    target.score -= damage;
                }
                
                self.next_turn(game).await?;
            }
            PlayerAction::UseItem { item_id } => {
                // 아이템 사용 로직
                self.next_turn(game).await?;
            }
        }
        
        // 게임 상태 브로드캐스트
        let action_result = GameMessage::ActionResult {
            room_id,
            user_id,
            action: action.clone(),
            game_state: game.clone(),
        };
        
        drop(games); // lock 해제
        
        self.room_service.send_to_room(room_id, &action_result).await?;
        
        Ok(())
    }
    
    /// 다음 턴으로 넘기기
    async fn next_turn(&self, game: &mut GameState) -> Result<()> {
        let current_idx = game.players.iter()
            .position(|p| Some(p.user_id) == game.current_turn)
            .ok_or_else(|| anyhow!("Current player not found"))?;
            
        let next_idx = (current_idx + 1) % game.players.len();
        game.current_turn = Some(game.players[next_idx].user_id);
        
        // 라운드 체크 (모든 플레이어가 한 번씩 플레이했으면 라운드 증가)
        if next_idx == 0 {
            game.round += 1;
            
            // 게임 종료 조건 체크
            if game.round > self.config.round_limit {
                game.status = GameStatus::Finished;
                self.end_game(game).await?;
            }
        }
        
        Ok(())
    }
    
    /// 게임 종료 처리
    async fn end_game(&self, game: &GameState) -> Result<()> {
        // 승자 결정
        let winner = game.players.iter()
            .max_by_key(|p| p.score)
            .cloned();
            
        let game_ended = GameMessage::GameEnded {
            room_id: game.room_id,
            winner,
            final_scores: game.players.iter()
                .map(|p| (p.user_id, p.score))
                .collect(),
        };
        
        self.room_service.send_to_room(game.room_id, &game_ended).await?;
        
        tracing::info!("Game ended in room {}", game.room_id);
        Ok(())
    }
    
    /// 게임 상태 조회
    pub async fn get_game_state(&self, room_id: u32) -> Option<GameState> {
        let games = self.games.read().await;
        games.get(&room_id).cloned()
    }
    
    /// 진행중인 게임 목록
    pub async fn get_active_games(&self) -> Vec<u32> {
        let games = self.games.read().await;
        games.iter()
            .filter(|(_, game)| matches!(game.status, GameStatus::InProgress))
            .map(|(room_id, _)| *room_id)
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlayerAction {
    Move { x: f32, y: f32, direction: f32 },
    Attack { target_id: u32, damage: i32 },
    UseItem { item_id: String },
}
```

#### Step 3: 프로토콜에 게임 메시지 추가
```rust
// protocol.rs에 추가
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameMessage {
    // ... 기존 메시지들
    
    // 게임 관련 메시지들
    StartGame {
        room_id: u32,
    },
    
    GameStarted {
        room_id: u32,
        players: Vec<Player>,
    },
    
    ToggleReady {
        room_id: u32,
    },
    
    PlayerReadyUpdate {
        room_id: u32,
        user_id: u32,
        ready: bool,
    },
    
    GameInProgress {
        room_id: u32,
        current_turn: Option<u32>,
        round: u32,
    },
    
    PlayerAction {
        room_id: u32,
        action: PlayerAction,
    },
    
    ActionResult {
        room_id: u32,
        user_id: u32,
        action: PlayerAction,
        game_state: GameState,
    },
    
    GameEnded {
        room_id: u32,
        winner: Option<Player>,
        final_scores: Vec<(u32, i32)>,
    },
}
```

#### Step 4: ChatRoomMessageHandler에 통합
```rust
// handler/chat_room_message_handler.rs에 추가
use crate::handler::game_handler::GameHandler;

impl ChatRoomMessageHandler {
    pub fn new(room_service: Arc<RoomConnectionService>) -> Self {
        let chat_handler = Arc::new(ChatRoomHandler::new(room_service.clone()));
        let game_handler = Arc::new(GameHandler::new(room_service.clone())); // 추가
        
        Self {
            room_service,
            chat_handler,
            game_handler, // 추가
        }
    }
    
    // 메시지 라우팅에 게임 메시지 추가
    async fn route_message(&self, user_id: u32, message: GameMessage) -> Result<()> {
        match message {
            // ... 기존 라우팅 로직
            
            // 게임 메시지 라우팅
            GameMessage::StartGame { room_id } => {
                self.game_handler.start_game(room_id, user_id).await?;
            }
            
            GameMessage::ToggleReady { room_id } => {
                self.game_handler.toggle_ready(room_id, user_id).await?;
            }
            
            GameMessage::PlayerAction { room_id, action } => {
                self.game_handler.handle_player_action(room_id, user_id, action).await?;
            }
            
            _ => {
                // 기존 핸들러로 전달
                self.chat_handler.handle_message(user_id, message).await?;
            }
        }
        Ok(())
    }
}
```

## 🔧 고급 확장 패턴

### 1. 이벤트 기반 아키텍처
```rust
// events/game_events.rs
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum GameEvent {
    PlayerJoined { room_id: u32, user_id: u32 },
    GameStarted { room_id: u32 },
    PlayerAction { room_id: u32, user_id: u32, action: PlayerAction },
    GameEnded { room_id: u32, winner_id: Option<u32> },
}

pub struct GameEventBus {
    sender: broadcast::Sender<GameEvent>,
}

impl GameEventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self { sender }
    }
    
    pub fn publish(&self, event: GameEvent) {
        let _ = self.sender.send(event);
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<GameEvent> {
        self.sender.subscribe()
    }
}

// 이벤트 리스너 예시
pub struct GameStatsCollector {
    event_bus: GameEventBus,
    stats: Arc<RwLock<HashMap<u32, GameStats>>>,
}

impl GameStatsCollector {
    pub async fn start_listening(&self) {
        let mut receiver = self.event_bus.subscribe();
        
        while let Ok(event) = receiver.recv().await {
            match event {
                GameEvent::GameStarted { room_id } => {
                    // 게임 통계 수집 시작
                    self.start_game_tracking(room_id).await;
                }
                GameEvent::GameEnded { room_id, winner_id } => {
                    // 게임 통계 저장
                    self.save_game_stats(room_id, winner_id).await;
                }
                _ => {}
            }
        }
    }
}
```

### 2. 플러그인 시스템
```rust
// plugins/mod.rs
use async_trait::async_trait;

#[async_trait]
pub trait GamePlugin: Send + Sync {
    fn name(&self) -> &'static str;
    
    async fn on_game_start(&self, room_id: u32) -> Result<()>;
    async fn on_player_action(&self, room_id: u32, user_id: u32, action: &PlayerAction) -> Result<()>;
    async fn on_game_end(&self, room_id: u32, winner_id: Option<u32>) -> Result<()>;
}

pub struct PluginManager {
    plugins: Vec<Box<dyn GamePlugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }
    
    pub fn register_plugin(&mut self, plugin: Box<dyn GamePlugin>) {
        tracing::info!("Registering plugin: {}", plugin.name());
        self.plugins.push(plugin);
    }
    
    pub async fn on_game_start(&self, room_id: u32) -> Result<()> {
        for plugin in &self.plugins {
            if let Err(e) = plugin.on_game_start(room_id).await {
                tracing::error!("Plugin {} error on game start: {}", plugin.name(), e);
            }
        }
        Ok(())
    }
}

// 플러그인 예시: 레벨 시스템
pub struct LevelSystemPlugin {
    user_levels: Arc<RwLock<HashMap<u32, u32>>>,
}

#[async_trait]
impl GamePlugin for LevelSystemPlugin {
    fn name(&self) -> &'static str {
        "LevelSystem"
    }
    
    async fn on_game_end(&self, room_id: u32, winner_id: Option<u32>) -> Result<()> {
        if let Some(winner) = winner_id {
            let mut levels = self.user_levels.write().await;
            let current_level = levels.get(&winner).cloned().unwrap_or(1);
            levels.insert(winner, current_level + 1);
            
            tracing::info!("User {} leveled up to {}", winner, current_level + 1);
        }
        Ok(())
    }
}
```

## ⚡ 성능 최적화 팁

### 1. 비동기 스케줄러 활용
```rust
// 게임 로직을 우선순위별로 스케줄링
impl GameHandler {
    async fn process_critical_action(&self, action: PlayerAction) -> Result<()> {
        // Critical 우선순위로 스케줄링
        server.schedule_async_task(async move {
            // 중요한 게임 로직 처리
        }, TaskPriority::Critical).await?;
        
        Ok(())
    }
}
```

### 2. 메모리 풀 활용
```rust
// 대용량 게임 데이터 처리
impl GameHandler {
    async fn process_large_game_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        // 메모리 풀에서 버퍼 할당
        if let Some(buffer) = server.allocate_buffer(data.len() * 2) {
            let mut result_data = buffer.get_buffer();
            
            // 데이터 처리
            result_data.extend_from_slice(data);
            process_game_logic(&mut result_data);
            
            let result = result_data.clone();
            
            // 버퍼 반환
            server.deallocate_buffer(buffer);
            
            Ok(result)
        } else {
            // 풀에서 할당 실패시 일반 할당
            let mut result_data = Vec::with_capacity(data.len() * 2);
            result_data.extend_from_slice(data);
            process_game_logic(&mut result_data);
            Ok(result_data)
        }
    }
}
```

### 3. 배치 처리 최적화
```rust
// 여러 플레이어의 행동을 배치로 처리
impl GameHandler {
    pub async fn process_actions_batch(&self, actions: Vec<(u32, PlayerAction)>) -> Result<()> {
        // 룸별로 그룹화
        let mut room_actions: HashMap<u32, Vec<(u32, PlayerAction)>> = HashMap::new();
        
        for (user_id, action) in actions {
            if let Some(room_id) = self.room_service.get_user_room(user_id) {
                room_actions.entry(room_id).or_insert_with(Vec::new).push((user_id, action));
            }
        }
        
        // 룸별로 병렬 처리
        let mut tasks = Vec::new();
        for (room_id, room_actions) in room_actions {
            let handler = self.clone();
            let task = tokio::spawn(async move {
                handler.process_room_actions(room_id, room_actions).await
            });
            tasks.push(task);
        }
        
        // 모든 작업 완료 대기
        for task in tasks {
            task.await??;
        }
        
        Ok(())
    }
}
```