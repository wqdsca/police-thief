# gRPC 서비스 확장 가이드

## 📋 목차
1. [새로운 서비스 추가](#새로운-서비스-추가)
2. [비즈니스 로직 확장](#비즈니스-로직-확장)
3. [인증 및 권한 관리](#인증-및-권한-관리)
4. [데이터베이스 통합](#데이터베이스-통합)
5. [실시간 이벤트 시스템](#실시간-이벤트-시스템)

## 🚀 새로운 서비스 추가

### 1. 프로토콜 정의 (.proto)

```protobuf
// proto/game.proto
syntax = "proto3";

package game;

// 게임 관리 서비스
service GameService {
  // 게임 시작
  rpc StartGame(StartGameRequest) returns (StartGameResponse);
  
  // 게임 상태 조회
  rpc GetGameState(GetGameStateRequest) returns (GetGameStateResponse);
  
  // 플레이어 행동
  rpc PlayerAction(PlayerActionRequest) returns (PlayerActionResponse);
  
  // 게임 종료
  rpc EndGame(EndGameRequest) returns (EndGameResponse);
  
  // 게임 통계
  rpc GetGameStats(GetGameStatsRequest) returns (GetGameStatsResponse);
  
  // 실시간 게임 이벤트 스트림
  rpc GameEventStream(GameEventStreamRequest) returns (stream GameEvent);
}

message StartGameRequest {
  int32 room_id = 1;
  GameConfig config = 2;
}

message StartGameResponse {
  bool success = 1;
  string message = 2;
  GameState game_state = 3;
}

message GameConfig {
  int32 game_mode = 1;      // 0: 클래식, 1: 타임어택, 2: 팀전
  int32 round_duration = 2; // 라운드 시간(초)
  int32 max_rounds = 3;     // 최대 라운드 수
  bool allow_respawn = 4;   // 부활 허용
  int32 police_count = 5;   // 경찰 수
  int32 thief_count = 6;    // 도둑 수
  map<string, string> custom_rules = 7; // 커스텀 규칙
}

message GameState {
  int32 room_id = 1;
  GamePhase phase = 2;
  int32 current_round = 3;
  int32 remaining_time = 4;
  repeated PlayerGameInfo players = 5;
  GameScore score = 6;
  repeated GameEvent recent_events = 7;
  int64 updated_at = 8;
}

message PlayerGameInfo {
  int32 user_id = 1;
  string nickname = 2;
  PlayerRole role = 3;
  PlayerStatus status = 4;
  Position position = 5;
  PlayerStats stats = 6;
}

message Position {
  float x = 1;
  float y = 2;
  float z = 3;
  float rotation = 4;
}

message PlayerStats {
  int32 kills = 1;
  int32 deaths = 2;
  int32 arrests = 3;
  int32 escapes = 4;
  int32 score = 5;
}

message GameScore {
  int32 police_score = 1;
  int32 thief_score = 2;
  int32 police_rounds_won = 3;
  int32 thief_rounds_won = 4;
}

message GameEvent {
  int64 event_id = 1;
  EventType event_type = 2;
  int32 source_player_id = 3;
  int32 target_player_id = 4;
  string description = 5;
  map<string, string> metadata = 6;
  int64 timestamp = 7;
}

enum GamePhase {
  WAITING = 0;
  PREPARING = 1;
  IN_PROGRESS = 2;
  ROUND_END = 3;
  GAME_END = 4;
}

enum PlayerRole {
  SPECTATOR = 0;
  POLICE = 1;
  THIEF = 2;
}

enum PlayerStatus {
  ALIVE = 0;
  DEAD = 1;
  ARRESTED = 2;
  ESCAPED = 3;
}

enum EventType {
  GAME_START = 0;
  ROUND_START = 1;
  PLAYER_KILL = 2;
  PLAYER_ARREST = 3;
  PLAYER_ESCAPE = 4;
  ROUND_END = 5;
  GAME_END = 6;
}
```

### 2. 서비스 구현

```rust
// src/service/game_service.rs
use tonic::{Request, Response, Status};
use tokio_stream::{wrappers::ReceiverStream, Stream};
use std::pin::Pin;
use crate::proto::game::game_service_server::GameService;
use crate::proto::game::*;
use crate::auth::{AuthContext, AuthLevel};
use shared::service::redis::GameRedisService;

pub struct GameServiceImpl {
    game_redis: Arc<GameRedisService>,
    room_redis: Arc<RoomRedisService>,
    event_broadcaster: Arc<GameEventBroadcaster>,
    db_pool: Arc<SqlxPool>,
}

impl GameServiceImpl {
    pub fn new(
        game_redis: Arc<GameRedisService>,
        room_redis: Arc<RoomRedisService>,
        event_broadcaster: Arc<GameEventBroadcaster>,
        db_pool: Arc<SqlxPool>,
    ) -> Self {
        Self {
            game_redis,
            room_redis,
            event_broadcaster,
            db_pool,
        }
    }
}

#[tonic::async_trait]
impl GameService for GameServiceImpl {
    /// 게임 시작
    async fn start_game(
        &self,
        request: Request<StartGameRequest>,
    ) -> Result<Response<StartGameResponse>, Status> {
        let auth_context = AuthContext::from_request(&request, AuthLevel::Required)?;
        let req = request.into_inner();
        
        // 방 정보 확인 및 권한 검증
        let room_info = self.room_redis.get_room_info(req.room_id).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?
            .ok_or_else(|| Status::not_found("방을 찾을 수 없습니다."))?;
        
        // 방장인지 확인
        if room_info.host_id != auth_context.user_id {
            return Err(Status::permission_denied("방장만 게임을 시작할 수 있습니다."));
        }
        
        // 게임 시작 조건 확인
        self.validate_game_start_conditions(&room_info, &req)?;
        
        // 플레이어 역할 배정
        let players = self.assign_player_roles(&room_info, &req.config).await?;
        
        // 게임 상태 초기화
        let game_state = GameState {
            room_id: req.room_id,
            phase: GamePhase::Preparing as i32,
            current_round: 1,
            remaining_time: req.config.as_ref().map(|c| c.round_duration).unwrap_or(300),
            players: players.clone(),
            score: Some(GameScore {
                police_score: 0,
                thief_score: 0,
                police_rounds_won: 0,
                thief_rounds_won: 0,
            }),
            recent_events: vec![],
            updated_at: chrono::Utc::now().timestamp(),
        };
        
        // Redis에 게임 상태 저장
        self.game_redis.create_game_session(req.room_id, &game_state, &req.config).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
        
        // 방 상태를 게임 중으로 변경
        self.room_redis.update_room_status(req.room_id, RoomStatus::InProgress).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
        
        // 게임 시작 이벤트 브로드캐스트
        let start_event = GameEvent {
            event_id: self.generate_event_id(),
            event_type: EventType::GameStart as i32,
            source_player_id: auth_context.user_id,
            target_player_id: 0,
            description: "게임이 시작되었습니다.".to_string(),
            metadata: std::collections::HashMap::new(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        self.event_broadcaster.broadcast_event(req.room_id, start_event).await;
        
        // 게임 시작 작업을 백그라운드에서 처리
        let game_redis_clone = self.game_redis.clone();
        let room_id = req.room_id;
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await; // 5초 준비 시간
            let _ = game_redis_clone.start_game_round(room_id, 1).await;
        });
        
        tracing::info!("게임 시작: room_id={}, host_id={}", req.room_id, auth_context.user_id);
        
        Ok(Response::new(StartGameResponse {
            success: true,
            message: "게임이 시작되었습니다. 5초 후 첫 라운드가 시작됩니다.".to_string(),
            game_state: Some(game_state),
        }))
    }
    
    /// 플레이어 행동 처리
    async fn player_action(
        &self,
        request: Request<PlayerActionRequest>,
    ) -> Result<Response<PlayerActionResponse>, Status> {
        let auth_context = AuthContext::from_request(&request, AuthLevel::Required)?;
        let req = request.into_inner();
        
        // 게임 상태 조회
        let mut game_state = self.game_redis.get_game_state(req.room_id).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?
            .ok_or_else(|| Status::not_found("진행 중인 게임이 없습니다."))?;
        
        // 플레이어가 게임에 참여 중인지 확인
        let player = game_state.players.iter_mut()
            .find(|p| p.user_id == auth_context.user_id)
            .ok_or_else(|| Status::failed_precondition("게임에 참여중이 아닙니다."))?;
        
        // 플레이어 상태 확인
        if player.status != PlayerStatus::Alive as i32 {
            return Err(Status::failed_precondition("행동할 수 없는 상태입니다."));
        }
        
        // 액션 처리
        let result = self.process_player_action(&mut game_state, player, &req.action).await?;
        
        // 게임 상태 업데이트
        game_state.updated_at = chrono::Utc::now().timestamp();
        self.game_redis.update_game_state(req.room_id, &game_state).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
        
        // 액션 결과 이벤트 생성
        if let Some(event) = result.event {
            self.event_broadcaster.broadcast_event(req.room_id, event).await;
        }
        
        Ok(Response::new(PlayerActionResponse {
            success: result.success,
            message: result.message,
            game_state: Some(game_state),
            action_result: result.action_result,
        }))
    }
    
    /// 실시간 게임 이벤트 스트림
    async fn game_event_stream(
        &self,
        request: Request<GameEventStreamRequest>,
    ) -> Result<Response<Self::GameEventStreamStream>, Status> {
        let auth_context = AuthContext::from_request(&request, AuthLevel::Required)?;
        let req = request.into_inner();
        
        // 플레이어가 해당 방에 있는지 확인
        let user_room_id = self.room_redis.get_user_current_room(auth_context.user_id).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
        
        if user_room_id != Some(req.room_id) {
            return Err(Status::permission_denied("해당 방에 참여중이 아닙니다."));
        }
        
        // 이벤트 스트림 생성
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        
        // 이벤트 브로드캐스터에 구독자 등록
        self.event_broadcaster.subscribe(req.room_id, auth_context.user_id, tx).await;
        
        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(output_stream) as Self::GameEventStreamStream))
    }
    
    type GameEventStreamStream = Pin<Box<dyn Stream<Item = Result<GameEvent, Status>> + Send>>;
}

impl GameServiceImpl {
    /// 게임 시작 조건 검증
    fn validate_game_start_conditions(
        &self, 
        room_info: &RoomInfo, 
        req: &StartGameRequest
    ) -> Result<(), Status> {
        if room_info.status != RoomStatus::Waiting as i32 {
            return Err(Status::failed_precondition("이미 게임이 진행 중입니다."));
        }
        
        let min_players = req.config.as_ref()
            .map(|c| c.police_count + c.thief_count)
            .unwrap_or(4);
        
        if room_info.current_players < min_players {
            return Err(Status::failed_precondition(
                format!("최소 {}명의 플레이어가 필요합니다.", min_players)
            ));
        }
        
        Ok(())
    }
    
    /// 플레이어 역할 배정
    async fn assign_player_roles(
        &self, 
        room_info: &RoomInfo, 
        config: &Option<GameConfig>
    ) -> Result<Vec<PlayerGameInfo>, Status> {
        let players = &room_info.players;
        let mut assigned_players = Vec::new();
        
        let config = config.as_ref().unwrap();
        let police_count = config.police_count as usize;
        let thief_count = config.thief_count as usize;
        
        // 플레이어를 섞어서 랜덤 배정
        use rand::seq::SliceRandom;
        let mut shuffled_players = players.clone();
        shuffled_players.shuffle(&mut rand::thread_rng());
        
        for (i, player) in shuffled_players.iter().enumerate() {
            let role = if i < police_count {
                PlayerRole::Police
            } else if i < police_count + thief_count {
                PlayerRole::Thief
            } else {
                PlayerRole::Spectator
            };
            
            assigned_players.push(PlayerGameInfo {
                user_id: player.user_id,
                nickname: player.nickname.clone(),
                role: role as i32,
                status: PlayerStatus::Alive as i32,
                position: Some(Position {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    rotation: 0.0,
                }),
                stats: Some(PlayerStats {
                    kills: 0,
                    deaths: 0,
                    arrests: 0,
                    escapes: 0,
                    score: 0,
                }),
            });
        }
        
        Ok(assigned_players)
    }
    
    /// 플레이어 액션 처리
    async fn process_player_action(
        &self,
        game_state: &mut GameState,
        player: &mut PlayerGameInfo,
        action: &Option<PlayerAction>,
    ) -> Result<ActionResult, Status> {
        let action = action.as_ref().ok_or_else(|| Status::invalid_argument("액션이 없습니다."))?;
        
        match action.action_type {
            Some(action_type::ActionType::Move(ref move_action)) => {
                self.handle_move_action(player, move_action).await
            }
            Some(action_type::ActionType::Attack(ref attack_action)) => {
                self.handle_attack_action(game_state, player, attack_action).await
            }
            Some(action_type::ActionType::Arrest(ref arrest_action)) => {
                self.handle_arrest_action(game_state, player, arrest_action).await
            }
            Some(action_type::ActionType::Escape(ref escape_action)) => {
                self.handle_escape_action(game_state, player, escape_action).await
            }
            None => Err(Status::invalid_argument("알 수 없는 액션입니다.")),
        }
    }
    
    /// 이동 액션 처리
    async fn handle_move_action(
        &self,
        player: &mut PlayerGameInfo,
        move_action: &MoveAction,
    ) -> Result<ActionResult, Status> {
        // 위치 업데이트
        if let Some(position) = player.position.as_mut() {
            position.x = move_action.position.as_ref().unwrap().x;
            position.y = move_action.position.as_ref().unwrap().y;
            position.z = move_action.position.as_ref().unwrap().z;
            position.rotation = move_action.position.as_ref().unwrap().rotation;
        }
        
        Ok(ActionResult {
            success: true,
            message: "이동했습니다.".to_string(),
            event: None, // 이동은 별도 이벤트 없음
            action_result: Some(ActionResultData {
                result_type: Some(action_result_data::ResultType::MoveResult(MoveResult {
                    new_position: player.position.clone(),
                })),
            }),
        })
    }
    
    /// 공격 액션 처리
    async fn handle_attack_action(
        &self,
        game_state: &mut GameState,
        attacker: &mut PlayerGameInfo,
        attack_action: &AttackAction,
    ) -> Result<ActionResult, Status> {
        // 대상 플레이어 찾기
        let target = game_state.players.iter_mut()
            .find(|p| p.user_id == attack_action.target_id)
            .ok_or_else(|| Status::not_found("대상 플레이어를 찾을 수 없습니다."))?;
        
        // 공격 가능한지 확인
        if target.status != PlayerStatus::Alive as i32 {
            return Err(Status::failed_precondition("대상이 이미 사망했습니다."));
        }
        
        // 거리 확인
        let distance = self.calculate_distance(
            attacker.position.as_ref(),
            target.position.as_ref(),
        );
        
        if distance > 10.0 { // 10유닛 이내에서만 공격 가능
            return Err(Status::failed_precondition("대상이 너무 멀리 있습니다."));
        }
        
        // 공격 성공
        target.status = PlayerStatus::Dead as i32;
        if let Some(target_stats) = target.stats.as_mut() {
            target_stats.deaths += 1;
        }
        if let Some(attacker_stats) = attacker.stats.as_mut() {
            attacker_stats.kills += 1;
            attacker_stats.score += 10;
        }
        
        // 킬 이벤트 생성
        let kill_event = GameEvent {
            event_id: self.generate_event_id(),
            event_type: EventType::PlayerKill as i32,
            source_player_id: attacker.user_id,
            target_player_id: target.user_id,
            description: format!("{} 이(가) {} 을(를) 처치했습니다.", attacker.nickname, target.nickname),
            metadata: std::collections::HashMap::new(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        Ok(ActionResult {
            success: true,
            message: "공격에 성공했습니다.".to_string(),
            event: Some(kill_event),
            action_result: Some(ActionResultData {
                result_type: Some(action_result_data::ResultType::AttackResult(AttackResult {
                    target_id: target.user_id,
                    damage: attack_action.damage,
                    target_defeated: true,
                })),
            }),
        })
    }
    
    /// 거리 계산
    fn calculate_distance(pos1: Option<&Position>, pos2: Option<&Position>) -> f32 {
        match (pos1, pos2) {
            (Some(p1), Some(p2)) => {
                let dx = p1.x - p2.x;
                let dy = p1.y - p2.y;
                let dz = p1.z - p2.z;
                (dx * dx + dy * dy + dz * dz).sqrt()
            }
            _ => f32::MAX,
        }
    }
    
    /// 이벤트 ID 생성
    fn generate_event_id(&self) -> i64 {
        chrono::Utc::now().timestamp_nanos() / 1_000_000 // 밀리초로 변환
    }
}

struct ActionResult {
    success: bool,
    message: String,
    event: Option<GameEvent>,
    action_result: Option<ActionResultData>,
}
```

### 3. 실시간 이벤트 브로드캐스터

```rust
// src/event/game_event_broadcaster.rs
use tokio::sync::{mpsc, RwLock};
use std::collections::HashMap;
use crate::proto::game::GameEvent;

pub struct GameEventBroadcaster {
    // room_id -> (user_id -> sender)
    subscribers: Arc<RwLock<HashMap<i32, HashMap<i32, mpsc::Sender<Result<GameEvent, tonic::Status>>>>>>,
}

impl GameEventBroadcaster {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 구독자 등록
    pub async fn subscribe(
        &self,
        room_id: i32,
        user_id: i32,
        sender: mpsc::Sender<Result<GameEvent, tonic::Status>>,
    ) {
        let mut subscribers = self.subscribers.write().await;
        subscribers
            .entry(room_id)
            .or_insert_with(HashMap::new)
            .insert(user_id, sender);
        
        tracing::debug!("이벤트 스트림 구독: room_id={}, user_id={}", room_id, user_id);
    }
    
    /// 구독자 제거
    pub async fn unsubscribe(&self, room_id: i32, user_id: i32) {
        let mut subscribers = self.subscribers.write().await;
        if let Some(room_subscribers) = subscribers.get_mut(&room_id) {
            room_subscribers.remove(&user_id);
            
            // 방에 구독자가 없으면 제거
            if room_subscribers.is_empty() {
                subscribers.remove(&room_id);
            }
        }
        
        tracing::debug!("이벤트 스트림 구독 해제: room_id={}, user_id={}", room_id, user_id);
    }
    
    /// 이벤트 브로드캐스트
    pub async fn broadcast_event(&self, room_id: i32, event: GameEvent) {
        let subscribers = self.subscribers.read().await;
        
        if let Some(room_subscribers) = subscribers.get(&room_id) {
            let mut disconnected_users = Vec::new();
            
            for (&user_id, sender) in room_subscribers.iter() {
                match sender.try_send(Ok(event.clone())) {
                    Ok(_) => {
                        tracing::trace!("이벤트 전송 성공: user_id={}, event_type={}", 
                                       user_id, event.event_type);
                    }
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        tracing::warn!("이벤트 큐가 가득참: user_id={}", user_id);
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        tracing::debug!("연결이 끊어진 사용자: user_id={}", user_id);
                        disconnected_users.push(user_id);
                    }
                }
            }
            
            // 연결이 끊어진 사용자 정리
            if !disconnected_users.is_empty() {
                drop(subscribers); // read lock 해제
                for user_id in disconnected_users {
                    self.unsubscribe(room_id, user_id).await;
                }
            }
        }
    }
    
    /// 특정 사용자에게만 이벤트 전송
    pub async fn send_event_to_user(&self, room_id: i32, user_id: i32, event: GameEvent) {
        let subscribers = self.subscribers.read().await;
        
        if let Some(room_subscribers) = subscribers.get(&room_id) {
            if let Some(sender) = room_subscribers.get(&user_id) {
                if let Err(e) = sender.try_send(Ok(event)) {
                    tracing::warn!("개별 이벤트 전송 실패: user_id={}, error={:?}", user_id, e);
                }
            }
        }
    }
    
    /// 방의 구독자 수 조회
    pub async fn get_subscriber_count(&self, room_id: i32) -> usize {
        let subscribers = self.subscribers.read().await;
        subscribers.get(&room_id).map(|s| s.len()).unwrap_or(0)
    }
}
```

### 4. 커스텀 비즈니스 로직 확장

```rust
// src/business/game_logic.rs
use crate::proto::game::*;
use std::collections::HashMap;

/// 게임 규칙 엔진
pub struct GameRuleEngine {
    rules: HashMap<String, Box<dyn GameRule + Send + Sync>>,
}

pub trait GameRule {
    fn name(&self) -> &'static str;
    fn validate_action(&self, game_state: &GameState, player: &PlayerGameInfo, action: &PlayerAction) -> Result<(), String>;
    fn process_action(&self, game_state: &mut GameState, player: &mut PlayerGameInfo, action: &PlayerAction) -> ActionResult;
}

impl GameRuleEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            rules: HashMap::new(),
        };
        
        // 기본 게임 규칙들 등록
        engine.register_rule(Box::new(MovementRule));
        engine.register_rule(Box::new(AttackRule));
        engine.register_rule(Box::new(ArrestRule));
        engine.register_rule(Box::new(EscapeRule));
        
        engine
    }
    
    pub fn register_rule(&mut self, rule: Box<dyn GameRule + Send + Sync>) {
        self.rules.insert(rule.name().to_string(), rule);
    }
    
    pub fn validate_and_process_action(
        &self,
        game_state: &mut GameState,
        player: &mut PlayerGameInfo,
        action: &PlayerAction,
    ) -> Result<ActionResult, String> {
        // 모든 규칙에 대해 검증
        for rule in self.rules.values() {
            rule.validate_action(game_state, player, action)?;
        }
        
        // 액션 타입에 따른 처리
        match action.action_type {
            Some(action_type::ActionType::Move(_)) => {
                if let Some(rule) = self.rules.get("movement") {
                    Ok(rule.process_action(game_state, player, action))
                } else {
                    Err("Movement rule not found".to_string())
                }
            }
            Some(action_type::ActionType::Attack(_)) => {
                if let Some(rule) = self.rules.get("attack") {
                    Ok(rule.process_action(game_state, player, action))
                } else {
                    Err("Attack rule not found".to_string())
                }
            }
            // ... 다른 액션 타입들
            _ => Err("Unknown action type".to_string()),
        }
    }
}

/// 이동 규칙
struct MovementRule;

impl GameRule for MovementRule {
    fn name(&self) -> &'static str {
        "movement"
    }
    
    fn validate_action(&self, game_state: &GameState, player: &PlayerGameInfo, action: &PlayerAction) -> Result<(), String> {
        if let Some(action_type::ActionType::Move(ref move_action)) = action.action_type {
            // 플레이어가 살아있는지 확인
            if player.status != PlayerStatus::Alive as i32 {
                return Err("죽은 플레이어는 이동할 수 없습니다.".to_string());
            }
            
            // 이동 거리 제한 확인
            if let (Some(current_pos), Some(target_pos)) = (&player.position, &move_action.position) {
                let distance = ((target_pos.x - current_pos.x).powi(2) + 
                               (target_pos.y - current_pos.y).powi(2) + 
                               (target_pos.z - current_pos.z).powi(2)).sqrt();
                
                if distance > 50.0 { // 한 번에 최대 50유닛까지만 이동 가능
                    return Err("이동 거리가 너무 큽니다.".to_string());
                }
            }
        }
        
        Ok(())
    }
    
    fn process_action(&self, _game_state: &mut GameState, player: &mut PlayerGameInfo, action: &PlayerAction) -> ActionResult {
        if let Some(action_type::ActionType::Move(ref move_action)) = action.action_type {
            // 위치 업데이트
            player.position = move_action.position.clone();
            
            ActionResult {
                success: true,
                message: "이동 완료".to_string(),
                event: None,
                action_result: Some(ActionResultData {
                    result_type: Some(action_result_data::ResultType::MoveResult(MoveResult {
                        new_position: player.position.clone(),
                    })),
                }),
            }
        } else {
            ActionResult {
                success: false,
                message: "잘못된 액션 타입".to_string(),
                event: None,
                action_result: None,
            }
        }
    }
}

/// 공격 규칙
struct AttackRule;

impl GameRule for AttackRule {
    fn name(&self) -> &'static str {
        "attack"
    }
    
    fn validate_action(&self, game_state: &GameState, player: &PlayerGameInfo, action: &PlayerAction) -> Result<(), String> {
        if let Some(action_type::ActionType::Attack(ref attack_action)) = action.action_type {
            // 자신을 공격할 수 없음
            if attack_action.target_id == player.user_id {
                return Err("자신을 공격할 수 없습니다.".to_string());
            }
            
            // 대상 플레이어 존재 확인
            let target = game_state.players.iter()
                .find(|p| p.user_id == attack_action.target_id)
                .ok_or_else(|| "대상 플레이어를 찾을 수 없습니다.".to_string())?;
            
            // 같은 팀은 공격할 수 없음 (팀전 모드에서)
            if player.role == target.role {
                return Err("같은 팀을 공격할 수 없습니다.".to_string());
            }
            
            // 공격 쿨다운 확인 (마지막 공격 후 2초)
            // ... 쿨다운 로직 구현
        }
        
        Ok(())
    }
    
    fn process_action(&self, game_state: &mut GameState, player: &mut PlayerGameInfo, action: &PlayerAction) -> ActionResult {
        if let Some(action_type::ActionType::Attack(ref attack_action)) = action.action_type {
            // 대상 찾기 및 공격 처리
            if let Some(target) = game_state.players.iter_mut().find(|p| p.user_id == attack_action.target_id) {
                // 데미지 적용 로직 (단순화)
                target.status = PlayerStatus::Dead as i32;
                
                // 통계 업데이트
                if let Some(player_stats) = player.stats.as_mut() {
                    player_stats.kills += 1;
                    player_stats.score += 10;
                }
                if let Some(target_stats) = target.stats.as_mut() {
                    target_stats.deaths += 1;
                }
                
                // 킬 이벤트 생성
                let event = GameEvent {
                    event_id: chrono::Utc::now().timestamp_nanos() / 1_000_000,
                    event_type: EventType::PlayerKill as i32,
                    source_player_id: player.user_id,
                    target_player_id: target.user_id,
                    description: format!("{} 이(가) {} 을(를) 처치했습니다.", player.nickname, target.nickname),
                    metadata: HashMap::new(),
                    timestamp: chrono::Utc::now().timestamp(),
                };
                
                ActionResult {
                    success: true,
                    message: "공격 성공!".to_string(),
                    event: Some(event),
                    action_result: Some(ActionResultData {
                        result_type: Some(action_result_data::ResultType::AttackResult(AttackResult {
                            target_id: target.user_id,
                            damage: attack_action.damage,
                            target_defeated: true,
                        })),
                    }),
                }
            } else {
                ActionResult {
                    success: false,
                    message: "대상을 찾을 수 없습니다.".to_string(),
                    event: None,
                    action_result: None,
                }
            }
        } else {
            ActionResult {
                success: false,
                message: "잘못된 액션 타입".to_string(),
                event: None,
                action_result: None,
            }
        }
    }
}

/// 체포 규칙 (경찰 전용)
struct ArrestRule;

impl GameRule for ArrestRule {
    fn name(&self) -> &'static str {
        "arrest"
    }
    
    fn validate_action(&self, game_state: &GameState, player: &PlayerGameInfo, action: &PlayerAction) -> Result<(), String> {
        if let Some(action_type::ActionType::Arrest(ref arrest_action)) = action.action_type {
            // 경찰만 체포 가능
            if player.role != PlayerRole::Police as i32 {
                return Err("경찰만 체포할 수 있습니다.".to_string());
            }
            
            // 대상이 도둑인지 확인
            let target = game_state.players.iter()
                .find(|p| p.user_id == arrest_action.target_id)
                .ok_or_else(|| "대상을 찾을 수 없습니다.".to_string())?;
                
            if target.role != PlayerRole::Thief as i32 {
                return Err("도둑만 체포할 수 있습니다.".to_string());
            }
            
            if target.status != PlayerStatus::Alive as i32 {
                return Err("대상이 이미 체포되었거나 사망했습니다.".to_string());
            }
            
            // 거리 확인 (5유닛 이내에서만 체포 가능)
            if let (Some(player_pos), Some(target_pos)) = (&player.position, &target.position) {
                let distance = ((player_pos.x - target_pos.x).powi(2) + 
                               (player_pos.y - target_pos.y).powi(2) + 
                               (player_pos.z - target_pos.z).powi(2)).sqrt();
                
                if distance > 5.0 {
                    return Err("대상이 너무 멀리 있습니다.".to_string());
                }
            }
        }
        
        Ok(())
    }
    
    fn process_action(&self, game_state: &mut GameState, player: &mut PlayerGameInfo, action: &PlayerAction) -> ActionResult {
        if let Some(action_type::ActionType::Arrest(ref arrest_action)) = action.action_type {
            if let Some(target) = game_state.players.iter_mut().find(|p| p.user_id == arrest_action.target_id) {
                // 체포 처리
                target.status = PlayerStatus::Arrested as i32;
                
                // 통계 업데이트
                if let Some(player_stats) = player.stats.as_mut() {
                    player_stats.arrests += 1;
                    player_stats.score += 15; // 체포는 킬보다 점수가 높음
                }
                
                // 체포 이벤트 생성
                let event = GameEvent {
                    event_id: chrono::Utc::now().timestamp_nanos() / 1_000_000,
                    event_type: EventType::PlayerArrest as i32,
                    source_player_id: player.user_id,
                    target_player_id: target.user_id,
                    description: format!("{} 경찰이 {} 도둑을 체포했습니다.", player.nickname, target.nickname),
                    metadata: HashMap::new(),
                    timestamp: chrono::Utc::now().timestamp(),
                };
                
                ActionResult {
                    success: true,
                    message: "체포 성공!".to_string(),
                    event: Some(event),
                    action_result: Some(ActionResultData {
                        result_type: Some(action_result_data::ResultType::ArrestResult(ArrestResult {
                            target_id: target.user_id,
                            arrest_location: player.position.clone(),
                        })),
                    }),
                }
            } else {
                ActionResult {
                    success: false,
                    message: "대상을 찾을 수 없습니다.".to_string(),
                    event: None,
                    action_result: None,
                }
            }
        } else {
            ActionResult {
                success: false,
                message: "잘못된 액션 타입".to_string(),
                event: None,
                action_result: None,
            }
        }
    }
}

/// 탈출 규칙 (도둑 전용)
struct EscapeRule;

impl GameRule for EscapeRule {
    fn name(&self) -> &'static str {
        "escape"
    }
    
    fn validate_action(&self, _game_state: &GameState, player: &PlayerGameInfo, action: &PlayerAction) -> Result<(), String> {
        if let Some(action_type::ActionType::Escape(_)) = action.action_type {
            // 도둑만 탈출 가능
            if player.role != PlayerRole::Thief as i32 {
                return Err("도둑만 탈출할 수 있습니다.".to_string());
            }
            
            // 살아있어야 함
            if player.status != PlayerStatus::Alive as i32 {
                return Err("생존 상태에서만 탈출할 수 있습니다.".to_string());
            }
            
            // 탈출 지점에 있는지 확인
            if let Some(position) = &player.position {
                // 예시: 특정 좌표 범위가 탈출 지점
                let escape_zone_x = (90.0, 100.0);
                let escape_zone_y = (90.0, 100.0);
                
                if position.x < escape_zone_x.0 || position.x > escape_zone_x.1 ||
                   position.y < escape_zone_y.0 || position.y > escape_zone_y.1 {
                    return Err("탈출 지점에서만 탈출할 수 있습니다.".to_string());
                }
            }
        }
        
        Ok(())
    }
    
    fn process_action(&self, _game_state: &mut GameState, player: &mut PlayerGameInfo, action: &PlayerAction) -> ActionResult {
        if let Some(action_type::ActionType::Escape(ref _escape_action)) = action.action_type {
            // 탈출 처리
            player.status = PlayerStatus::Escaped as i32;
            
            // 통계 업데이트
            if let Some(player_stats) = player.stats.as_mut() {
                player_stats.escapes += 1;
                player_stats.score += 20; // 탈출은 최고 점수
            }
            
            // 탈출 이벤트 생성
            let event = GameEvent {
                event_id: chrono::Utc::now().timestamp_nanos() / 1_000_000,
                event_type: EventType::PlayerEscape as i32,
                source_player_id: player.user_id,
                target_player_id: 0,
                description: format!("{} 도둑이 탈출했습니다!", player.nickname),
                metadata: HashMap::new(),
                timestamp: chrono::Utc::now().timestamp(),
            };
            
            ActionResult {
                success: true,
                message: "탈출 성공!".to_string(),
                event: Some(event),
                action_result: Some(ActionResultData {
                    result_type: Some(action_result_data::ResultType::EscapeResult(EscapeResult {
                        escape_location: player.position.clone(),
                    })),
                }),
            }
        } else {
            ActionResult {
                success: false,
                message: "잘못된 액션 타입".to_string(),
                event: None,
                action_result: None,
            }
        }
    }
}
```

이 가이드는 gRPC 서버의 확장성과 유연성을 보여줍니다. Protocol Buffers 기반의 강타입 API, 실시간 이벤트 스트리밍, 비즈니스 규칙 엔진을 통해 복잡한 게임 로직도 체계적으로 관리할 수 있습니다. JWT 인증, Redis 캐싱, MariaDB 영구 저장소를 활용한 확장 가능한 아키텍처를 제공합니다.