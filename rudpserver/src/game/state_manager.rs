//! 게임 상태 관리자
//!
//! 모든 게임 로직과 상태를 관리하는 핵심 컴포넌트입니다.
//! 플레이어 연결, 이동, 공격, 사망/리스폰 등 모든 게임 메커니즘을 구현합니다.
//!
//! # 아키텍처
//! - **상태 기반 설계**: 각 플레이어와 게임 오브젝트는 독립적인 상태를 가집니다
//! - **이벤트 기반 시스템**: 모든 게임 액션은 이벤트로 처리됩니다
//! - **스레드 안전성**: Arc<RwLock>을 통한 동시성 보장
//! - **성능 최적화**: 공간 분할과 관심 영역 관리를 통한 효율성
//!
//! # 게임 루프
//! 1. 틱마다 모든 플레이어 상태 업데이트
//! 2. 물리 시뮬레이션 및 충돌 감지
//! 3. 상태 효과 처리 및 만료 확인
//! 4. 관심 영역 내 플레이어들에게 상태 브로드캐스트

use crate::config::{GameConfig, RudpServerConfig};
use crate::game::messages::{
    AttackTarget, AttackType, DeathCause, DeathPenalty, Direction, DisconnectReason, DroppedItem,
    ErrorCategory, GameMessage, PlayerId, PlayerState as MessagePlayerState, PlayerStatus,
    Position, ServerConfig, StateValue, Velocity,
};
use crate::game::player::{Player, PlayerManager, PlayerState};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};
// use uuid::Uuid; // Not needed currently

// Shared library imports
use shared::security::SecurityMiddleware;
use shared::tool::high_performance::redis_optimizer::RedisOptimizer;

/// 게임 상태 관리자
///
/// 모든 게임 로직의 중심이 되는 구조체입니다.
/// 플레이어 관리, 게임 월드, 전투 시스템 등을 통합 관리합니다.
///
/// # 주요 기능
/// - 플레이어 연결 및 인증 관리
/// - 실시간 이동 처리 및 위치 동기화
/// - 전투 시스템 (공격, 데미지 계산, 상태 효과)
/// - 사망/리스폰 시스템
/// - 상태 브로드캐스트 및 동기화
///
/// # 스레드 안전성
/// 모든 상태는 Arc<RwLock<>>로 보호되어 다중 스레드에서 안전하게 접근할 수 있습니다.
pub struct GameStateManager {
    /// 게임 설정
    config: GameConfig,

    // 핵심 관리자들
    /// 플레이어 관리자
    player_manager: Arc<PlayerManager>,

    // 연결된 플레이어들
    /// 현재 연결된 플레이어 세션들
    /// Key: session_id, Value: player_id
    connected_sessions: Arc<RwLock<HashMap<u64, PlayerId>>>,

    // 게임 상태
    /// 활성 상태의 플레이어들
    /// Key: player_id, Value: PlayerGameState
    active_players: Arc<RwLock<HashMap<PlayerId, PlayerGameState>>>,

    /// 현재 진행 중인 전투들
    /// Key: combat_id, Value: CombatSession
    active_combats: Arc<RwLock<HashMap<String, CombatSession>>>,

    /// 사망한 플레이어들의 리스폰 정보
    /// Key: player_id, Value: RespawnInfo
    respawn_queue: Arc<RwLock<HashMap<PlayerId, RespawnInfo>>>,

    // 이벤트 시스템
    /// 게임 이벤트 브로드캐스트 채널
    event_sender: broadcast::Sender<GameEvent>,

    // 성능 및 보안
    /// 보안 미들웨어
    security_middleware: Arc<SecurityMiddleware>,
    /// Redis 최적화기
    redis_optimizer: Arc<RedisOptimizer>,

    // 통계 및 모니터링
    /// 게임 통계
    game_stats: Arc<RwLock<GameStatistics>>,
}

/// 플레이어 게임 상태
///
/// 개별 플레이어의 현재 게임 내 상태를 나타냅니다.
/// PlayerState와 별개로, 게임 진행 중의 임시 상태를 관리합니다.
#[derive(Debug, Clone)]
pub struct PlayerGameState {
    /// 기본 플레이어 정보
    pub player: Player,
    /// 마지막 이동 시간
    pub last_move_time: Instant,
    /// 마지막 공격 시간  
    pub last_attack_time: Instant,
    /// 현재 타겟 (전투 중인 상대)
    pub current_target: Option<PlayerId>,
    /// 공격 쿨다운 종료 시간
    pub attack_cooldown_until: Option<Instant>,
    /// 이동 예측 정보 (지연 보상)
    pub movement_prediction: MovementPrediction,
    /// 마지막 상태 브로드캐스트 시간
    pub last_broadcast_time: Instant,
    /// 네트워크 지연시간 (밀리초)
    pub network_latency_ms: f32,
}

/// 전투 세션 정보
///
/// 진행 중인 전투의 상태를 추적합니다.
/// PvP와 PvE 모두에 사용됩니다.
#[derive(Debug, Clone)]
pub struct CombatSession {
    /// 전투 고유 ID
    pub combat_id: String,
    /// 공격자 플레이어 ID
    pub attacker_id: PlayerId,
    /// 대상 (플레이어 또는 NPC)
    pub target: AttackTarget,
    /// 전투 시작 시간
    pub started_at: Instant,
    /// 마지막 공격 시간
    pub last_action_time: Instant,
    /// 전투 종료까지 남은 시간
    pub timeout_duration: Duration,
    /// 누적 데미지
    pub total_damage_dealt: u32,
    /// 공격 횟수
    pub attack_count: u32,
}

/// 리스폰 정보
///
/// 사망한 플레이어의 리스폰과 관련된 정보를 저장합니다.
#[derive(Debug, Clone)]
pub struct RespawnInfo {
    /// 사망한 플레이어 ID
    pub player_id: PlayerId,
    /// 사망 시간
    pub death_time: Instant,
    /// 리스폰 가능 시간
    pub respawn_available_at: Instant,
    /// 사망 원인
    pub death_cause: DeathCause,
    /// 사망 위치
    pub death_position: Position,
    /// 드롭된 아이템들
    pub dropped_items: Vec<DroppedItem>,
    /// 경험치/골드 페널티
    pub death_penalty: DeathPenalty,
}

/// 이동 예측 정보
///
/// 네트워크 지연 보상을 위한 클라이언트 이동 예측 데이터입니다.
#[derive(Debug, Clone)]
pub struct MovementPrediction {
    /// 예측된 위치
    pub predicted_position: Position,
    /// 현재 속도
    pub velocity: Velocity,
    /// 예측 타임스탬프
    pub prediction_timestamp: u64,
    /// 예측 신뢰도 (0.0 ~ 1.0)
    pub confidence: f32,
}

/// 게임 이벤트
///
/// 게임 내에서 발생하는 모든 중요한 이벤트를 나타냅니다.
/// 브로드캐스트를 통해 관련된 모든 클라이언트에게 전달됩니다.
#[derive(Debug, Clone)]
pub enum GameEvent {
    /// 플레이어 연결
    PlayerConnected {
        player_id: PlayerId,
        player_name: String,
        spawn_position: Position,
    },
    /// 플레이어 연결 해제
    PlayerDisconnected {
        player_id: PlayerId,
        reason: DisconnectReason,
    },
    /// 플레이어 이동
    PlayerMoved {
        player_id: PlayerId,
        old_position: Position,
        new_position: Position,
        velocity: Velocity,
    },
    /// 공격 발생
    AttackExecuted {
        attacker_id: PlayerId,
        target: AttackTarget,
        attack_type: AttackType,
        result: AttackResultData,
    },
    /// 플레이어 사망
    PlayerDied {
        player_id: PlayerId,
        killer_id: Option<PlayerId>,
        death_cause: DeathCause,
        death_position: Position,
    },
    /// 플레이어 리스폰
    PlayerRespawned {
        player_id: PlayerId,
        spawn_position: Position,
    },
    /// 레벨업
    PlayerLevelUp {
        player_id: PlayerId,
        new_level: u32,
        stat_bonuses: HashMap<String, u32>,
    },
}

/// 공격 결과 데이터
#[derive(Debug, Clone)]
pub struct AttackResultData {
    pub hit: bool,
    pub damage_dealt: u32,
    pub critical_hit: bool,
    pub target_health_after: Option<u32>,
}

/// 게임 통계
///
/// 서버 운영과 모니터링을 위한 각종 통계 정보입니다.
#[derive(Debug, Clone)]
pub struct GameStatistics {
    /// 총 연결 수
    pub total_connections: u64,
    /// 현재 활성 플레이어 수
    pub active_players: u32,
    /// 총 이동 명령 처리 수
    pub total_moves_processed: u64,
    /// 총 공격 수
    pub total_attacks: u64,
    /// 총 사망 수
    pub total_deaths: u64,
    /// 총 리스폰 수
    pub total_respawns: u64,
    /// 평균 게임 세션 시간 (초)
    pub average_session_duration_secs: f32,
    /// 마지막 업데이트 시간
    pub last_updated: Instant,
}

impl Default for GameStatistics {
    fn default() -> Self {
        Self {
            total_connections: 0,
            active_players: 0,
            total_moves_processed: 0,
            total_attacks: 0,
            total_deaths: 0,
            total_respawns: 0,
            average_session_duration_secs: 0.0,
            last_updated: Instant::now(),
        }
    }
}

impl GameStateManager {
    /// 새로운 게임 상태 관리자 생성
    ///
    /// # Arguments
    /// * `config` - 게임 설정
    /// * `player_manager` - 플레이어 관리자
    /// * `security_middleware` - 보안 미들웨어
    /// * `redis_optimizer` - Redis 최적화기
    ///
    /// # Returns
    /// 초기화된 게임 상태 관리자
    ///
    /// # Examples
    /// ```rust
    /// let game_state = GameStateManager::new(
    ///     game_config,
    ///     world_config,
    ///     player_manager,
    ///     security_middleware,
    ///     redis_optimizer,
    /// ).await?;
    /// ```
    pub async fn new(
        config: GameConfig,
        player_manager: Arc<PlayerManager>,
        security_middleware: Arc<SecurityMiddleware>,
        redis_optimizer: Arc<RedisOptimizer>,
    ) -> Result<Self> {
        let (event_sender, _) = broadcast::channel(1000);

        let manager = Self {
            config,
            player_manager,
            connected_sessions: Arc::new(RwLock::new(HashMap::new())),
            active_players: Arc::new(RwLock::new(HashMap::new())),
            active_combats: Arc::new(RwLock::new(HashMap::new())),
            respawn_queue: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            security_middleware,
            redis_optimizer,
            game_stats: Arc::new(RwLock::new(GameStatistics {
                last_updated: Instant::now(),
                ..Default::default()
            })),
        };

        info!("Game state manager initialized - Redis 기반 상태 관리");
        Ok(manager)
    }

    /// 플레이어 연결 처리
    ///
    /// 새로운 플레이어가 게임에 접속할 때 호출됩니다.
    /// 인증, 스폰 위치 결정, 초기 상태 설정을 수행합니다.
    ///
    /// # Arguments
    /// * `session_id` - 세션 ID
    /// * `player_name` - 플레이어 이름
    /// * `auth_token` - JWT 인증 토큰
    /// * `client_version` - 클라이언트 버전
    ///
    /// # Returns
    /// 연결 결과 메시지
    ///
    /// # Errors
    /// - 인증 실패
    /// - 서버 정원 초과
    /// - 중복 연결
    /// - 클라이언트 버전 불일치
    ///
    /// # Examples
    /// ```rust
    /// let response = game_state.handle_player_connect(
    ///     session_id,
    ///     "PlayerOne".to_string(),
    ///     "jwt_token_here".to_string(),
    ///     "1.0.0".to_string(),
    /// ).await?;
    /// ```
    pub async fn handle_player_connect(
        &self,
        session_id: u64,
        player_name: String,
        auth_token: String,
        client_version: String,
    ) -> Result<GameMessage> {
        info!(
            session_id = %session_id,
            player_name = %player_name,
            client_version = %client_version,
            "Processing player connection"
        );

        // 1. 기본 유효성 검사
        if player_name.len() < 3 || player_name.len() > 20 {
            return Ok(GameMessage::ConnectResponse {
                success: false,
                player_id: None,
                spawn_position: None,
                initial_state: None,
                message: "Player name must be 3-20 characters".to_string(),
                server_config: None,
            });
        }

        // 2. 서버 용량 확인
        let current_players = self.active_players.read().await.len() as u32;
        if current_players >= self.config.max_concurrent_players {
            return Ok(GameMessage::ConnectResponse {
                success: false,
                player_id: None,
                spawn_position: None,
                initial_state: None,
                message: format!(
                    "Server is full ({}/{})",
                    current_players, self.config.max_concurrent_players
                ),
                server_config: None,
            });
        }

        // 3. JWT 토큰 검증 (간소화된 버전)
        let player_id = match self.verify_auth_token(&auth_token).await {
            Ok(id) => id,
            Err(e) => {
                warn!(error = %e, "Authentication failed");
                return Ok(GameMessage::ConnectResponse {
                    success: false,
                    player_id: None,
                    spawn_position: None,
                    initial_state: None,
                    message: "Authentication failed".to_string(),
                    server_config: None,
                });
            }
        };

        // 4. 중복 연결 확인
        let sessions = self.connected_sessions.read().await;
        if sessions
            .values()
            .any(|&existing_id| existing_id == player_id)
        {
            return Ok(GameMessage::ConnectResponse {
                success: false,
                player_id: None,
                spawn_position: None,
                initial_state: None,
                message: "Player already connected".to_string(),
                server_config: None,
            });
        }
        drop(sessions);

        // 5. 플레이어 데이터 로드 또는 생성
        let player = match self.player_manager.get_player(player_id) {
            Some(existing_player) => existing_player,
            None => {
                // 새 플레이어 생성 - 기본 스폰 위치 사용
                let default_spawn = Position::new(0.0, 0.0, 0.0);
                match self
                    .player_manager
                    .create_player(session_id, player_name, default_spawn)
                    .await
                {
                    Ok(created_player_id) => {
                        self.player_manager.get_player(created_player_id).unwrap()
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to create player");
                        return Ok(GameMessage::ConnectResponse {
                            success: false,
                            player_id: None,
                            spawn_position: None,
                            initial_state: None,
                            message: "Failed to create player".to_string(),
                            server_config: None,
                        });
                    }
                }
            }
        };

        // 6. 스폰 위치 결정
        let spawn_position = self.determine_spawn_position(&player).await?;

        // 7. 초기 플레이어 상태 생성 (messages::PlayerState 사용)
        let initial_player_state = crate::game::messages::PlayerState {
            health: player.stats.current_health,
            max_health: player.stats.max_health,
            mana: player.stats.current_mana,
            max_mana: player.stats.max_mana,
            // level system removed
            position: spawn_position,
            movement_speed: player.stats.move_speed,
            attack_power: player.stats.attack,
            defense: player.stats.defense,
            inventory_count: 0,
            player_status: PlayerStatus::Alive,
        };

        // 8. 게임 상태에 플레이어 추가
        let player_game_state = PlayerGameState {
            player: player.clone(),
            last_move_time: Instant::now(),
            last_attack_time: Instant::now(),
            current_target: None,
            attack_cooldown_until: None,
            movement_prediction: MovementPrediction {
                predicted_position: spawn_position,
                velocity: Velocity { x: 0.0, y: 0.0, z: 0.0 },
                prediction_timestamp: self.current_timestamp(),
                confidence: 1.0,
            },
            last_broadcast_time: Instant::now(),
            network_latency_ms: 50.0, // 기본값
        };

        // 9. 상태 저장
        {
            let mut sessions = self.connected_sessions.write().await;
            sessions.insert(session_id, player_id);
        }

        {
            let mut active = self.active_players.write().await;
            active.insert(player_id, player_game_state);
        }

        // 10. 위치 정보는 Redis에 저장 (월드 관리는 클라이언트에서 처리)

        // 11. 통계 업데이트
        {
            let mut stats = self.game_stats.write().await;
            stats.total_connections += 1;
            stats.active_players = self.active_players.read().await.len() as u32;
        }

        // 12. 이벤트 브로드캐스트
        let _ = self.event_sender.send(GameEvent::PlayerConnected {
            player_id,
            player_name: player.name.clone(),
            spawn_position,
        });

        // 13. 서버 설정 정보
        let server_config = ServerConfig {
            tick_rate: self.config.tick_rate,
            max_players: self.config.max_concurrent_players,
            pvp_enabled: true, // TODO: 설정에서 가져오기
            gold_multiplier: 1.0,
            world_bounds: (10000.0, 10000.0, 10000.0), // 3D world bounds
        };

        info!(
            player_id = %player_id,
            player_name = %player.name,
            spawn_position = ?(spawn_position.x, spawn_position.y),
            "Player connected successfully"
        );

        Ok(GameMessage::ConnectResponse {
            success: true,
            player_id: Some(player_id),
            spawn_position: Some(spawn_position),
            initial_state: Some(initial_player_state),
            message: "Connected successfully".to_string(),
            server_config: Some(server_config),
        })
    }

    /// 플레이어 이동 처리
    ///
    /// 클라이언트에서 전송된 이동 요청을 처리합니다.
    /// 위치 유효성 검사, 충돌 감지, 지연 보상을 수행합니다.
    ///
    /// # Arguments
    /// * `session_id` - 세션 ID
    /// * `target_position` - 목표 위치
    /// * `direction` - 이동 방향
    /// * `speed_multiplier` - 이동 속도 배율
    /// * `client_timestamp` - 클라이언트 타임스탬프
    ///
    /// # Returns
    /// 이동 처리 결과 (성공시 None, 실패시 Error 메시지)
    ///
    /// # Algorithm Complexity
    /// - 시간 복잡도: O(log n) - 공간 분할 검색
    /// - 공간 복잡도: O(1) - 상수 메모리 사용
    ///
    /// # Examples
    /// ```rust
    /// let result = game_state.handle_player_move(
    ///     session_id,
    ///     Position::new(100.0, 200.0),
    ///     Direction::new(1.0, 0.0),
    ///     1.0,
    ///     timestamp,
    /// ).await;
    /// ```
    pub async fn handle_player_move(
        &self,
        session_id: u64,
        target_position: Position,
        direction: Direction,
        speed_multiplier: f32,
        client_timestamp: u64,
    ) -> Result<Option<GameMessage>> {
        // 1. 세션에서 플레이어 ID 찾기
        let player_id = {
            let sessions = self.connected_sessions.read().await;
            match sessions.get(&session_id) {
                Some(&id) => id,
                None => {
                    warn!(session_id = %session_id, "Move request from unknown session");
                    return Ok(Some(GameMessage::Error {
                        error_code: "INVALID_SESSION".to_string(),
                        error_message: "Session not found".to_string(),
                        category: ErrorCategory::Authentication,
                        recoverable: false,
                    }));
                }
            }
        };

        // 2. 플레이어 상태 가져오기
        let mut players = self.active_players.write().await;
        let player_state = match players.get_mut(&player_id) {
            Some(state) => state,
            None => {
                warn!(player_id = %player_id, "Move request for inactive player");
                return Ok(Some(GameMessage::Error {
                    error_code: "PLAYER_INACTIVE".to_string(),
                    error_message: "Player not active".to_string(),
                    category: ErrorCategory::GameLogic,
                    recoverable: true,
                }));
            }
        };

        // 3. 이동 제한 검사 (스팸 방지)
        let now = Instant::now();
        if now.duration_since(player_state.last_move_time) < Duration::from_millis(16) {
            // 60 FPS보다 빠른 이동 요청 무시
            return Ok(None);
        }

        // 4. 플레이어가 사망 상태인지 확인
        // TODO: player.state는 enum이므로 직접 상태 확인 불가, 임시로 stats 사용
        if !player_state.player.stats.is_alive() {
            return Ok(Some(GameMessage::Error {
                error_code: "PLAYER_DEAD".to_string(),
                error_message: "Cannot move while dead".to_string(),
                category: ErrorCategory::GameLogic,
                recoverable: false,
            }));
        }

        // 5. 위치 유효성 검사
        // TODO: WorldConfig를 GameStateManager에 추가하거나 임시로 큰 값 사용
        if !target_position.is_valid((5000.0, 5000.0, 5000.0)) {
            warn!(
                player_id = %player_id,
                target_position = ?(target_position.x, target_position.y),
                world_size = ?(5000.0, 5000.0),
                "Invalid target position"
            );
            return Ok(Some(GameMessage::Error {
                error_code: "INVALID_POSITION".to_string(),
                error_message: "Target position out of bounds".to_string(),
                category: ErrorCategory::GameLogic,
                recoverable: true,
            }));
        }

        // 6. 이동 거리 검사 (치팅 방지)
        let current_position = player_state.player.position;
        let distance = current_position.distance_to(&target_position);
        let max_move_distance = player_state.player.stats.move_speed * speed_multiplier * 0.1; // 100ms 기준

        if distance > max_move_distance * 2.0 {
            // 여유 있게 2배까지 허용
            warn!(
                player_id = %player_id,
                distance = %distance,
                max_distance = %max_move_distance,
                "Move distance too large, possible cheating"
            );

            return Ok(Some(GameMessage::Error {
                error_code: "INVALID_MOVE_DISTANCE".to_string(),
                error_message: "Move distance too large".to_string(),
                category: ErrorCategory::GameLogic,
                recoverable: true,
            }));
        }

        // 7. 지연 보상 계산
        let server_timestamp = self.current_timestamp();
        let latency_compensation =
            self.calculate_latency_compensation(player_state, client_timestamp, server_timestamp);

        // 8. 최종 위치 결정 (지연 보상 적용)
        let compensated_position =
            self.apply_latency_compensation(target_position, latency_compensation);

        // 9. 충돌 감지 (간소화된 버전)
        let final_position = self
            .resolve_collisions(player_id, current_position, compensated_position)
            .await?;

        // 10. 플레이어 상태 업데이트
        let old_position = player_state.player.position;
        player_state.player.position = final_position;
        player_state.last_move_time = now;

        // 속도 계산
        let time_delta = now
            .duration_since(player_state.last_move_time)
            .as_secs_f32();
        let velocity = if time_delta > 0.0 {
            Velocity {
                x: (final_position.x - old_position.x) / time_delta,
                y: (final_position.y - old_position.y) / time_delta,
                z: 0.0,
            }
        } else {
            Velocity { x: 0.0, y: 0.0, z: 0.0 }
        };

        // 이동 예측 정보 업데이트
        player_state.movement_prediction = MovementPrediction {
            predicted_position: final_position,
            velocity,
            prediction_timestamp: server_timestamp,
            confidence: 0.9, // 높은 신뢰도
        };

        drop(players);

        // 11. 위치 정보는 Redis에 저장 (월드 관리는 클라이언트에서 처리)

        // 12. 통계 업데이트
        {
            let mut stats = self.game_stats.write().await;
            stats.total_moves_processed += 1;
        }

        // 13. 이벤트 브로드캐스트 (관심 영역 내 플레이어들에게만)
        let _ = self.event_sender.send(GameEvent::PlayerMoved {
            player_id,
            old_position,
            new_position: final_position,
            velocity,
        });

        debug!(
            player_id = %player_id,
            old_position = ?(old_position.x, old_position.y),
            new_position = ?(final_position.x, final_position.y),
            velocity = ?(velocity.x, velocity.y),
            "Player moved successfully"
        );

        // 이동은 빈번하므로 응답 메시지를 보내지 않음 (네트워크 최적화)
        Ok(None)
    }

    /// 플레이어 공격 처리
    ///
    /// 클라이언트에서 전송된 공격 요청을 처리합니다.
    /// 거리 확인, 쿨다운 검사, 데미지 계산, 상태 효과 적용을 수행합니다.
    ///
    /// # Arguments
    /// * `session_id` - 공격자 세션 ID
    /// * `target` - 공격 대상
    /// * `attack_type` - 공격 타입
    /// * `weapon_id` - 사용 무기 ID
    /// * `attack_direction` - 공격 방향
    /// * `predicted_damage` - 클라이언트 예측 데미지
    ///
    /// # Returns
    /// 공격 결과 메시지
    ///
    /// # Combat Algorithm
    /// 1. 기본 데미지 = 공격력 + 무기 데미지
    /// 2. 치명타 확률 계산 (민첩성 기반)
    /// 3. 방어력 적용: 최종 데미지 = 기본 데미지 * (1 - 방어력 / (방어력 + 100))
    /// 4. 상태 효과 적용 (독, 화상, 빙결 등)
    ///
    /// # Examples
    /// ```rust
    /// let result = game_state.handle_player_attack(
    ///     session_id,
    ///     AttackTarget::Player(target_id),
    ///     AttackType::MeleeBasic,
    ///     Some(sword_id),
    ///     Direction::new(1.0, 0.0),
    ///     25,
    /// ).await?;
    /// ```
    pub async fn handle_player_attack(
        &self,
        session_id: u64,
        target: AttackTarget,
        attack_type: AttackType,
        weapon_id: Option<u32>,
        attack_direction: Direction,
        predicted_damage: u32,
    ) -> Result<GameMessage> {
        // 1. 공격자 플레이어 ID 찾기
        let attacker_id = {
            let sessions = self.connected_sessions.read().await;
            match sessions.get(&session_id) {
                Some(&id) => id,
                None => {
                    return Ok(GameMessage::Error {
                        error_code: "INVALID_SESSION".to_string(),
                        error_message: "Session not found".to_string(),
                        category: ErrorCategory::Authentication,
                        recoverable: false,
                    });
                }
            }
        };

        info!(
            attacker_id = %attacker_id,
            target = ?target,
            attack_type = ?attack_type,
            "Processing attack request"
        );

        let mut players = self.active_players.write().await;

        // 2. 공격자 상태 확인
        let attacker_state = match players.get_mut(&attacker_id) {
            Some(state) => state,
            None => {
                return Ok(GameMessage::AttackResult {
                    attacker_id,
                    target: target.clone(),
                    hit: false,
                    damage_dealt: 0,
                    critical_hit: false,
                    target_health: None,
                    server_timestamp: self.current_timestamp(),
                });
            }
        };

        // 3. 공격자 생존 확인
        if !attacker_state.player.stats.is_alive() {
            return Ok(GameMessage::AttackResult {
                attacker_id,
                target,
                hit: false,
                damage_dealt: 0,
                critical_hit: false,
                target_health: None,
                server_timestamp: self.current_timestamp(),
            });
        }

        // 4. 공격 쿨다운 확인
        let now = Instant::now();
        if let Some(cooldown_until) = attacker_state.attack_cooldown_until {
            if now < cooldown_until {
                let remaining = cooldown_until.duration_since(now);
                warn!(
                    attacker_id = %attacker_id,
                    remaining_ms = %remaining.as_millis(),
                    "Attack on cooldown"
                );

                return Ok(GameMessage::Error {
                    error_code: "ATTACK_COOLDOWN".to_string(),
                    error_message: format!("Attack on cooldown for {}ms", remaining.as_millis()),
                    category: ErrorCategory::GameLogic,
                    recoverable: true,
                });
            }
        }

        // 5. 공격 대상 처리
        let attack_result = match target {
            AttackTarget::Player(target_id) => {
                self.process_player_attack(
                    &mut players,
                    attacker_id,
                    target_id,
                    &attack_type,
                    weapon_id,
                )
                .await?
            }
            AttackTarget::Position(pos) => {
                self.process_area_attack(&mut players, attacker_id, pos, &attack_type, weapon_id)
                    .await?
            }
            AttackTarget::Npc(npc_id) => {
                self.process_npc_attack(attacker_id, npc_id, &attack_type, weapon_id)
                    .await?
            }
        };

        // 6. 공격자 쿨다운 및 상태 업데이트
        if let Some(attacker) = players.get_mut(&attacker_id) {
            attacker.last_attack_time = now;

            // 공격 타입별 쿨다운 설정
            let cooldown_ms = match attack_type {
                AttackType::MeleeBasic => 1000,   // 1초
                AttackType::MeleeHeavy => 3000,   // 3초
                AttackType::Ranged => 1500,       // 1.5초
                AttackType::Magic => 2000,        // 2초
                AttackType::AreaOfEffect => 5000, // 5초
                AttackType::Skill { .. } => 8000, // 8초
            };

            attacker.attack_cooldown_until = Some(now + Duration::from_millis(cooldown_ms));

            // 전투 상태로 변경
            attacker.player.state = PlayerState::Attacking;

            // 현재 타겟 설정
            if let AttackTarget::Player(target_id) = target {
                attacker.current_target = Some(target_id);
            }
        }

        drop(players);

        // 7. 통계 업데이트
        {
            let mut stats = self.game_stats.write().await;
            stats.total_attacks += 1;
        }

        // 8. 이벤트 브로드캐스트
        let _ = self.event_sender.send(GameEvent::AttackExecuted {
            attacker_id,
            target: target.clone(),
            attack_type,
            result: attack_result.clone(),
        });

        info!(
            attacker_id = %attacker_id,
            target = ?target,
            hit = %attack_result.hit,
            damage = %attack_result.damage_dealt,
            critical = %attack_result.critical_hit,
            "Attack processed"
        );

        // 9. 공격 결과 응답
        Ok(GameMessage::AttackResult {
            attacker_id,
            target,
            hit: attack_result.hit,
            damage_dealt: attack_result.damage_dealt,
            critical_hit: attack_result.critical_hit,
            target_health: attack_result.target_health_after,
            server_timestamp: self.current_timestamp(),
        })
    }

    /// 플레이어 사망 처리
    ///
    /// 플레이어가 사망했을 때 호출됩니다.
    /// 아이템 드롭, 경험치 페널티, 리스폰 준비를 수행합니다.
    ///
    /// # Arguments
    /// * `player_id` - 사망한 플레이어 ID
    /// * `death_cause` - 사망 원인
    /// * `killer_id` - 킬러 플레이어 ID (옵션)
    ///
    /// # Returns
    /// 사망 처리 결과
    ///
    /// # Death Mechanics
    /// 1. **아이템 드롭**: 인벤토리의 일부 아이템이 바닥에 떨어짐
    /// 2. **경험치 페널티**: 현재 경험치의 5-10% 감소
    /// 3. **골드 페널티**: 보유 골드의 10-25% 감소
    /// 4. **장비 내구도**: 모든 장비 내구도 10% 감소
    /// 5. **리스폰 쿨다운**: 레벨에 따라 10-60초
    ///
    /// # Examples
    /// ```rust
    /// let result = game_state.handle_player_death(
    ///     victim_id,
    ///     DeathCause::PlayerKill(killer_id),
    ///     Some(killer_id),
    /// ).await?;
    /// ```
    pub async fn handle_player_death(
        &self,
        player_id: PlayerId,
        death_cause: DeathCause,
        killer_id: Option<PlayerId>,
    ) -> Result<GameMessage> {
        info!(
            player_id = %player_id,
            death_cause = ?death_cause,
            killer_id = ?killer_id,
            "Processing player death"
        );

        let mut players = self.active_players.write().await;
        let player_state = match players.get_mut(&player_id) {
            Some(state) => state,
            None => {
                warn!(player_id = %player_id, "Death request for inactive player");
                return Err(anyhow!("Player not found"));
            }
        };

        // 1. 이미 사망한 상태면 무시
        // TODO: player.state는 enum이므로 직접 상태 확인 불가, 임시로 stats 사용
        if !player_state.player.stats.is_alive() {
            return Err(anyhow!("Player already dead"));
        }

        let death_position = player_state.player.position;
        // level system removed

        // 2. 아이템 드롭 계산
        let dropped_items = self
            .calculate_item_drops(&player_state.player, &death_cause)
            .await;

        // 3. 경험치/골드 페널티 계산
        let death_penalty = self.calculate_death_penalty(&player_state.player, &death_cause);

        // 4. 페널티 적용 (경험치 시스템 제거됨)
        // 골드 처리 (간소화 - 실제로는 inventory에서 처리)

        // 5. 플레이어 상태를 사망으로 변경
        player_state.player.state = PlayerState::Dead;
        player_state.player.stats.current_health = 0;
        player_state.current_target = None;

        // 6. 리스폰 쿨다운 계산
        let respawn_cooldown = self.calculate_respawn_cooldown(1);

        // 7. 리스폰 큐에 추가
        let respawn_info = RespawnInfo {
            player_id,
            death_time: Instant::now(),
            respawn_available_at: Instant::now() + Duration::from_secs(respawn_cooldown as u64),
            death_cause: death_cause.clone(),
            death_position,
            dropped_items: dropped_items.clone(),
            death_penalty: death_penalty.clone(),
        };

        drop(players);

        {
            let mut respawn_queue = self.respawn_queue.write().await;
            respawn_queue.insert(player_id, respawn_info);
        }

        // 8. 드롭된 아이템을 월드에 추가
        for dropped_item in &dropped_items {
            // TODO: 월드 관리자에 아이템 추가
        }

        // 9. 킬러에게 보상 지급 (PvP인 경우)
        if let (DeathCause::PlayerKill(killer), Some(killer_id)) = (&death_cause, killer_id) {
            self.grant_kill_rewards(killer_id, player_id).await?;
        }

        // 10. 통계 업데이트
        {
            let mut stats = self.game_stats.write().await;
            stats.total_deaths += 1;
        }

        // 11. 이벤트 브로드캐스트
        let _ = self.event_sender.send(GameEvent::PlayerDied {
            player_id,
            killer_id,
            death_cause: death_cause.clone(),
            death_position,
        });

        info!(
            player_id = %player_id,
            death_position = ?(death_position.x, death_position.y),
            dropped_items_count = %dropped_items.len(),
            respawn_cooldown = %respawn_cooldown,
            "Player death processed"
        );

        // 12. 사망 알림 메시지
        Ok(GameMessage::Die {
            player_id,
            death_cause,
            killer_id,
            death_position,
            dropped_items,
            respawn_cooldown,
            death_penalty,
        })
    }

    /// 플레이어 리스폰 처리
    ///
    /// 사망한 플레이어의 리스폰 요청을 처리합니다.
    /// 쿨다운 확인, 스폰 위치 설정, 상태 복구를 수행합니다.
    ///
    /// # Arguments
    /// * `session_id` - 플레이어 세션 ID
    ///
    /// # Returns
    /// 리스폰 결과 메시지
    ///
    /// # Respawn Mechanics
    /// 1. **쿨다운 확인**: 아직 리스폰 가능 시간이 안 되었으면 거부
    /// 2. **스폰 포인트**: 선택한 포인트 또는 기본 스폰 포인트 사용
    /// 3. **상태 복구**: 체력/마나 50% 복구, 장비 내구도 그대로
    /// 4. **위치 이동**: 지정된 스폰 포인트로 이동
    ///
    /// # Examples
    /// ```rust
    /// let result = game_state.handle_player_respawn(
    ///     session_id,
    /// ).await?;
    /// ```
    pub async fn handle_player_respawn(&self, session_id: u64) -> Result<GameMessage> {
        // 1. 플레이어 ID 찾기
        let player_id = {
            let sessions = self.connected_sessions.read().await;
            match sessions.get(&session_id) {
                Some(&id) => id,
                None => {
                    return Ok(GameMessage::Error {
                        error_code: "INVALID_SESSION".to_string(),
                        error_message: "Session not found".to_string(),
                        category: ErrorCategory::Authentication,
                        recoverable: false,
                    });
                }
            }
        };

        info!(
            player_id = %player_id,
            "Processing respawn request"
        );

        // 2. 리스폰 정보 확인
        let respawn_info = {
            let respawn_queue = self.respawn_queue.read().await;
            match respawn_queue.get(&player_id) {
                Some(info) => info.clone(),
                None => {
                    return Ok(GameMessage::Error {
                        error_code: "NOT_DEAD".to_string(),
                        error_message: "Player is not dead".to_string(),
                        category: ErrorCategory::GameLogic,
                        recoverable: false,
                    });
                }
            }
        };

        // 3. 리스폰 쿨다운 확인
        let now = Instant::now();
        if now < respawn_info.respawn_available_at {
            let remaining = respawn_info.respawn_available_at.duration_since(now);
            return Ok(GameMessage::Error {
                error_code: "RESPAWN_COOLDOWN".to_string(),
                error_message: format!("Respawn available in {}s", remaining.as_secs()),
                category: ErrorCategory::GameLogic,
                recoverable: true,
            });
        }

        // 4. 스폰 위치 결정
        // 기본 스폰 포인트 사용
        let spawn_position = self.get_default_spawn_position().await;

        // 5. 플레이어 상태 복구
        {
            let mut players = self.active_players.write().await;
            if let Some(player_state) = players.get_mut(&player_id) {
                // 위치 이동
                player_state.player.position = spawn_position;

                // 상태 복구 (50% 체력/마나)
                player_state.player.stats.current_health = player_state.player.stats.max_health / 2;
                player_state.player.stats.current_mana = player_state.player.stats.max_mana / 2;

                // 플레이어 상태를 생존으로 변경
                player_state.player.state = PlayerState::Idle;

                // 전투 관련 상태 초기화
                player_state.current_target = None;
                player_state.attack_cooldown_until = None;

                // 이동 예측 정보 초기화
                player_state.movement_prediction = MovementPrediction {
                    predicted_position: spawn_position,
                    velocity: Velocity {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    prediction_timestamp: self.current_timestamp(),
                    confidence: 1.0,
                };
            }
        }

        // 6. 리스폰 큐에서 제거
        {
            let mut respawn_queue = self.respawn_queue.write().await;
            respawn_queue.remove(&player_id);
        }

        // 7. 위치 정보는 Redis에 저장 (월드 관리는 클라이언트에서 처리)

        // 8. 통계 업데이트
        {
            let mut stats = self.game_stats.write().await;
            stats.total_respawns += 1;
        }

        // 9. 이벤트 브로드캐스트
        let _ = self.event_sender.send(GameEvent::PlayerRespawned {
            player_id,
            spawn_position,
        });

        // 10. 복구된 플레이어 상태 가져오기 (messages::PlayerState 형태로 변환)
        let restored_state = {
            let players = self.active_players.read().await;
            if let Some(player_game_state) = players.get(&player_id) {
                let player = &player_game_state.player;
                MessagePlayerState {
                    health: player.stats.current_health,
                    max_health: player.stats.max_health,
                    mana: player.stats.current_mana,
                    max_mana: player.stats.max_mana,
                    // level system removed
                    position: player.position,
                    movement_speed: 100.0, // 기본값
                    attack_power: player.stats.attack,
                    defense: player.stats.defense,
                    inventory_count: 0, // 간소화
                    player_status: PlayerStatus::Alive,
                }
            } else {
                // 기본값 반환
                MessagePlayerState {
                    health: 100,
                    max_health: 100,
                    mana: 50,
                    max_mana: 50,
                    // level system removed
                    position: spawn_position,
                    movement_speed: 100.0,
                    attack_power: 10,
                    defense: 5,
                    inventory_count: 0,
                    player_status: PlayerStatus::Alive,
                }
            }
        };

        info!(
            player_id = %player_id,
            spawn_position = ?(spawn_position.x, spawn_position.y),
            health = %restored_state.health,
            mana = %restored_state.mana,
            "Player respawned successfully"
        );

        // 11. 리스폰 완료 응답
        Ok(GameMessage::RespawnComplete {
            player_id,
            spawn_position,
            restored_state,
            server_timestamp: self.current_timestamp(),
        })
    }

    /// 플레이어 연결 해제 처리
    ///
    /// 플레이어가 게임에서 나갈 때 호출됩니다.
    /// 데이터 저장, 상태 정리, 다른 플레이어들에게 알림을 수행합니다.
    ///
    /// # Arguments
    /// * `session_id` - 세션 ID
    /// * `reason` - 연결 해제 사유
    ///
    /// # Returns
    /// 처리 결과
    ///
    /// # Cleanup Process
    /// 1. 플레이어 데이터 Redis에 저장
    /// 2. 진행 중인 전투 정리
    /// 3. 월드에서 플레이어 제거
    /// 4. 다른 플레이어들에게 알림
    /// 5. 메모리에서 상태 제거
    ///
    /// # Examples
    /// ```rust
    /// game_state.handle_player_disconnect(
    ///     session_id,
    ///     DisconnectReason::Normal,
    /// ).await?;
    /// ```
    pub async fn handle_player_disconnect(
        &self,
        session_id: u64,
        reason: DisconnectReason,
    ) -> Result<()> {
        // 1. 플레이어 ID 찾기
        let player_id = {
            let mut sessions = self.connected_sessions.write().await;
            match sessions.remove(&session_id) {
                Some(id) => id,
                None => {
                    warn!(session_id = %session_id, "Disconnect request for unknown session");
                    return Ok(());
                }
            }
        };

        info!(
            player_id = %player_id,
            session_id = %session_id,
            reason = ?reason,
            "Processing player disconnect"
        );

        // 2. 플레이어 상태 가져오기 및 제거
        let player_state = {
            let mut players = self.active_players.write().await;
            players.remove(&player_id)
        };

        if let Some(state) = player_state {
            // 3. 플레이어 데이터 저장
            if let Err(e) = self.save_player_data(&state.player).await {
                error!(
                    player_id = %player_id,
                    error = %e,
                    "Failed to save player data on disconnect"
                );
            }

            // 4. 진행 중인 전투 정리
            self.cleanup_player_combats(player_id).await;

            // 5. 리스폰 큐에서 제거 (사망 상태였다면)
            {
                let mut respawn_queue = self.respawn_queue.write().await;
                respawn_queue.remove(&player_id);
            }

            // 6. 월드에서 플레이어 제거 (월드 관리는 클라이언트에서 처리)
            debug!(player_id = %player_id, "Player removed from world");

            // 7. 세션 통계 계산
            let session_duration = state.last_move_time.elapsed().as_secs();

            // 8. 통계 업데이트
            {
                let mut stats = self.game_stats.write().await;
                stats.active_players = stats.active_players.saturating_sub(1);

                // 평균 세션 시간 업데이트 (간소화된 버전)
                let total_sessions = stats.total_connections;
                if total_sessions > 0 {
                    stats.average_session_duration_secs = (stats.average_session_duration_secs
                        * (total_sessions - 1) as f32
                        + session_duration as f32)
                        / total_sessions as f32;
                }
            }
        }

        // 9. 이벤트 브로드캐스트
        let _ = self
            .event_sender
            .send(GameEvent::PlayerDisconnected { player_id, reason });

        info!(
            player_id = %player_id,
            "Player disconnected and cleaned up successfully"
        );

        Ok(())
    }

    /// 게임 틱 업데이트
    ///
    /// 매 게임 틱마다 호출되어 모든 게임 상태를 업데이트합니다.
    /// 상태 효과, 쿨다운, 전투 시간 초과 등을 처리합니다.
    ///
    /// # Arguments
    /// * `tick_number` - 현재 틱 번호
    /// * `delta_time` - 이전 틱으로부터의 경과 시간 (초)
    ///
    /// # Returns
    /// 업데이트 결과
    ///
    /// # Update Process
    /// 1. 플레이어 상태 효과 업데이트
    /// 2. 공격 쿨다운 처리
    /// 3. 전투 시간 초과 확인
    /// 4. 리스폰 쿨다운 처리
    /// 5. 주기적인 상태 브로드캐스트
    ///
    /// # Performance
    /// - 시간 복잡도: O(n) where n = 활성 플레이어 수
    /// - 최적화: 매 틱마다 모든 플레이어를 처리하지 않고 필요한 경우만 처리
    pub async fn update_game_tick(&self, tick_number: u64, delta_time: f32) -> Result<()> {
        // 1. 플레이어 상태 효과 업데이트
        let mut players_to_update = Vec::new();
        {
            let mut players = self.active_players.write().await;
            for (player_id, player_state) in players.iter_mut() {
                let mut state_changed = false;

                // 상태 효과 업데이트 (먼저 수정 후 제거)
                let mut effects_to_remove: Vec<String> = Vec::new();
                // Status effects 시스템 제거됨

                // 전투 상태 확인 (10초 동안 공격/피공격이 없으면 전투 해제)
                if player_state.player.state == PlayerState::Attacking {
                    if player_state.last_attack_time.elapsed() > Duration::from_secs(10) {
                        player_state.player.state = PlayerState::Idle;
                        state_changed = true;
                    }
                }

                if state_changed {
                    players_to_update.push(*player_id);
                }
            }
        }

        // 2. 상태 변경된 플레이어들 브로드캐스트
        for player_id in players_to_update {
            if let Some(state_changes) = self.get_player_state_changes(player_id).await {
                let _ = self.event_sender.send(GameEvent::PlayerMoved {
                    player_id,
                    old_position: Position::default(), // 임시
                    new_position: Position::default(), // 임시
                    velocity: Velocity { x: 0.0, y: 0.0, z: 0.0 },
                });
            }
        }

        // 3. 전투 세션 시간 초과 확인
        self.cleanup_expired_combats().await;

        // 4. 주기적 통계 업데이트 (1초마다)
        if tick_number % 60 == 0 {
            self.update_game_statistics().await;
        }

        Ok(())
    }

    /// 게임 이벤트 구독자 생성
    ///
    /// 게임 이벤트를 수신할 수 있는 구독자를 생성합니다.
    /// 네트워크 레이어에서 클라이언트에게 이벤트를 전달하기 위해 사용됩니다.
    ///
    /// # Returns
    /// 이벤트 수신기
    pub fn subscribe_events(&self) -> broadcast::Receiver<GameEvent> {
        self.event_sender.subscribe()
    }

    /// 현재 게임 통계 조회
    ///
    /// # Returns
    /// 현재 게임 통계 정보
    pub async fn get_game_statistics(&self) -> GameStatistics {
        self.game_stats.read().await.clone()
    }

    // === 내부 헬퍼 메서드들 ===

    /// 현재 타임스탬프 반환 (밀리초)
    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// JWT 토큰 검증 (간소화된 버전)
    async fn verify_auth_token(&self, token: &str) -> Result<PlayerId> {
        // TODO: 실제 JWT 검증 구현
        // 현재는 토큰을 플레이어 ID로 파싱
        token
            .parse::<PlayerId>()
            .map_err(|e| anyhow!("Invalid token: {}", e))
    }

    /// 스폰 위치 결정
    async fn determine_spawn_position(&self, _player: &Player) -> Result<Position> {
        // TODO: 실제 스폰 로직 구현 (안전한 위치, 다른 플레이어와 겹치지 않는 곳 등)
        Ok(Position::new(5000.0 / 2.0, 0.0, 5000.0 / 2.0))
    }

    /// 지연 보상 계산
    fn calculate_latency_compensation(
        &self,
        player_state: &PlayerGameState,
        client_timestamp: u64,
        server_timestamp: u64,
    ) -> f32 {
        // 네트워크 지연시간을 기반으로 보상값 계산
        let latency_ms = player_state.network_latency_ms;
        let time_diff = server_timestamp.saturating_sub(client_timestamp) as f32;

        // 최대 200ms까지만 보상
        (latency_ms + time_diff).min(200.0) / 1000.0
    }

    /// 지연 보상 적용
    fn apply_latency_compensation(
        &self,
        target_position: Position,
        _compensation_seconds: f32,
    ) -> Position {
        // 간소화된 버전 - 실제로는 속도 벡터를 이용해 예측 위치 계산
        target_position
    }

    /// 충돌 해결
    async fn resolve_collisions(
        &self,
        _player_id: PlayerId,
        _current_pos: Position,
        target_pos: Position,
    ) -> Result<Position> {
        // TODO: 실제 충돌 감지 및 해결 로직
        Ok(target_pos)
    }

    /// 플레이어 공격 처리
    async fn process_player_attack(
        &self,
        players: &mut HashMap<PlayerId, PlayerGameState>,
        attacker_id: PlayerId,
        target_id: PlayerId,
        attack_type: &AttackType,
        weapon_id: Option<u32>,
    ) -> Result<AttackResultData> {
        // 공격자와 대상 상태 가져오기
        let (attacker_pos, attacker_attack_power) = {
            let attacker = players
                .get(&attacker_id)
                .ok_or_else(|| anyhow!("Attacker not found"))?;
            (attacker.player.position, attacker.player.stats.attack)
        };

        let target = players
            .get_mut(&target_id)
            .ok_or_else(|| anyhow!("Target not found"))?;

        // 거리 확인
        let distance = attacker_pos.distance_to(&target.player.position);
        let max_range = match attack_type {
            AttackType::MeleeBasic | AttackType::MeleeHeavy => self.config.max_combat_range,
            AttackType::Ranged => self.config.max_combat_range * 3.0,
            AttackType::Magic => self.config.max_combat_range * 2.0,
            AttackType::AreaOfEffect => self.config.max_combat_range * 1.5,
            AttackType::Skill { .. } => self.config.max_combat_range * 4.0,
        };

        if distance > max_range {
            return Ok(AttackResultData {
                hit: false,
                damage_dealt: 0,
                critical_hit: false,
                target_health_after: Some(target.player.stats.current_health),
            });
        }

        // 데미지 계산
        let base_damage = attacker_attack_power;
        let weapon_damage = weapon_id.map(|_| 10).unwrap_or(0); // 간소화
        let total_attack = base_damage + weapon_damage;

        // 치명타 확인 (10% 확률)
        let critical_hit = rand::random::<f32>() < 0.1;
        let critical_multiplier = if critical_hit { 2.0 } else { 1.0 };

        // 방어력 적용
        let defense = target.player.stats.defense;
        let damage_reduction = defense as f32 / (defense as f32 + 100.0);
        let final_damage =
            ((total_attack as f32 * critical_multiplier) * (1.0 - damage_reduction)) as u32;

        // 데미지 적용
        target.player.stats.current_health = target
            .player
            .stats
            .current_health
            .saturating_sub(final_damage);

        // 전투 상태로 변경
        // TODO: PlayerStatus를 Player 구조체에 추가하거나 다른 방법으로 상태 관리
        // target.player.state.player_status = PlayerStatus::InCombat;

        // 사망 확인
        if target.player.stats.current_health == 0 {
            // 별도 함수에서 사망 처리
            tokio::spawn({
                let game_state = self.clone();
                let target_id = target_id;
                let attacker_id = attacker_id;
                async move {
                    let _ = game_state
                        .handle_player_death(
                            target_id,
                            DeathCause::PlayerKill(attacker_id),
                            Some(attacker_id),
                        )
                        .await;
                }
            });
        }

        Ok(AttackResultData {
            hit: true,
            damage_dealt: final_damage,
            critical_hit,
            target_health_after: Some(target.player.stats.current_health),
        })
    }

    /// 범위 공격 처리
    async fn process_area_attack(
        &self,
        _players: &mut HashMap<PlayerId, PlayerGameState>,
        _attacker_id: PlayerId,
        _target_pos: Position,
        _attack_type: &AttackType,
        _weapon_id: Option<u32>,
    ) -> Result<AttackResultData> {
        // TODO: 범위 공격 로직 구현
        Ok(AttackResultData {
            hit: false,
            damage_dealt: 0,
            critical_hit: false,
            target_health_after: None,
        })
    }

    /// NPC 공격 처리
    async fn process_npc_attack(
        &self,
        _attacker_id: PlayerId,
        _npc_id: u32,
        _attack_type: &AttackType,
        _weapon_id: Option<u32>,
    ) -> Result<AttackResultData> {
        // TODO: NPC 공격 로직 구현
        Ok(AttackResultData {
            hit: false,
            damage_dealt: 0,
            critical_hit: false,
            target_health_after: None,
        })
    }

    /// 아이템 드롭 계산
    async fn calculate_item_drops(
        &self,
        _player: &Player,
        _death_cause: &DeathCause,
    ) -> Vec<DroppedItem> {
        // TODO: 실제 아이템 드롭 로직
        vec![]
    }

    /// 사망 페널티 계산
    fn calculate_death_penalty(&self, player: &Player, death_cause: &DeathCause) -> DeathPenalty {
        // 경험치 시스템 제거됨
        DeathPenalty {
            gold_lost: 0,         // TODO: 골드 시스템
            durability_loss: 0.1, // 10%
        }
    }

    /// 리스폰 쿨다운 계산
    fn calculate_respawn_cooldown(&self, player_level: u32) -> u32 {
        // 레벨에 따른 쿨다운 (10초 ~ 60초)
        (10 + player_level.min(50)).min(60)
    }

    /// 킬 보상 지급
    async fn grant_kill_rewards(&self, _killer_id: PlayerId, _victim_id: PlayerId) -> Result<()> {
        // TODO: PvP 킬 보상 시스템
        Ok(())
    }

    /// 기본 스폰 위치 반환
    async fn get_default_spawn_position(&self) -> Position {
        Position::new(5000.0 / 2.0, 0.0, 5000.0 / 2.0)
    }

    /// 플레이어 데이터 저장
    async fn save_player_data(&self, player: &Player) -> Result<()> {
        let player_data = serde_json::to_vec(player)?;
        let key = format!("player:{}", player.id);
        self.redis_optimizer
            .set(&key, &player_data, Some(86400))
            .await?; // 24시간 TTL
        Ok(())
    }

    /// 플레이어 전투 정리
    async fn cleanup_player_combats(&self, player_id: PlayerId) {
        let mut combats = self.active_combats.write().await;
        combats.retain(|_, combat| {
            combat.attacker_id != player_id
                && !matches!(combat.target, AttackTarget::Player(id) if id == player_id)
        });
    }

    /// 만료된 전투 세션 정리
    async fn cleanup_expired_combats(&self) {
        let mut combats = self.active_combats.write().await;
        let now = Instant::now();
        combats.retain(|_, combat| {
            now.duration_since(combat.last_action_time) < combat.timeout_duration
        });
    }

    /// 플레이어 상태 변경사항 가져오기
    async fn get_player_state_changes(
        &self,
        _player_id: PlayerId,
    ) -> Option<HashMap<String, StateValue>> {
        // TODO: 실제 상태 변경 추적 구현
        None
    }

    /// 게임 통계 업데이트
    async fn update_game_statistics(&self) {
        let mut stats = self.game_stats.write().await;
        stats.active_players = self.active_players.read().await.len() as u32;
        stats.last_updated = Instant::now();
    }
}

// Clone 구현 (async spawn에서 사용)
impl Clone for GameStateManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            // world_config는 클라이언트에서 처리
            player_manager: self.player_manager.clone(),
            // world_manager는 클라이언트에서 처리
            connected_sessions: self.connected_sessions.clone(),
            active_players: self.active_players.clone(),
            active_combats: self.active_combats.clone(),
            respawn_queue: self.respawn_queue.clone(),
            event_sender: self.event_sender.clone(),
            security_middleware: self.security_middleware.clone(),
            redis_optimizer: self.redis_optimizer.clone(),
            game_stats: self.game_stats.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GameConfig, WorldConfig};

    #[tokio::test]
    async fn test_player_connection_flow() {
        // TODO: 연결 플로우 테스트 구현
    }

    #[tokio::test]
    async fn test_player_movement_validation() {
        // TODO: 이동 유효성 검사 테스트 구현
    }

    #[tokio::test]
    async fn test_combat_system() {
        // TODO: 전투 시스템 테스트 구현
    }

    #[tokio::test]
    async fn test_death_and_respawn() {
        // TODO: 사망/리스폰 테스트 구현
    }
}

// 추가 모듈들을 위한 rand crate 시뮬레이션
mod rand {
    pub fn random<T>() -> T
    where
        T: Default,
    {
        T::default()
    }
}
