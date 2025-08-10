//! 플레이어 엔티티 관리
//!
//! Unity 클라이언트와 호환되는 플레이어 시스템 (클라이언트 관련 기능 제거됨)
//! 상태 효과, 레벨, 경험치 시스템이 모두 제거되었습니다.

use anyhow::{anyhow, Result};
use rand;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tracing::{debug, error, info, warn};

pub use crate::game::messages::PlayerId;
use crate::game::messages::{
    AttackTarget, DeathCause, Direction, PlayerState as PlayerStatus, Position, Velocity,
};

// Player type alias removed - using direct struct

/// 2D 속도 벡터 (Unity 클라이언트용)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Velocity2D {
    pub x: f32,
    pub y: f32,
}

impl Default for Velocity2D {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl Velocity2D {
    /// 정규화된 방향 벡터 반환
    pub fn normalize(self) -> Self {
        let magnitude = self.magnitude();
        if magnitude > 0.0 {
            Self {
                x: self.x / magnitude,
                y: self.y / magnitude,
            }
        } else {
            Self::default()
        }
    }

    /// 벡터의 크기 계산
    pub fn magnitude(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

/// 플레이어 스탯 (Unity 클라이언트용, 레벨 시스템 제거)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerStats {
    /// 최대 체력
    pub max_health: u32,
    /// 현재 체력
    pub current_health: u32,
    /// 최대 마나
    pub max_mana: u32,
    /// 현재 마나
    pub current_mana: u32,
    /// 공격력
    pub attack: u32,
    /// 방어력
    pub defense: u32,
    /// 이동속도
    pub move_speed: f32,
    /// 공격속도
    pub attack_speed: f32,
    /// 크리티컬 확률 (%)
    pub critical_chance: f32,
    /// 크리티컬 데미지 배율
    pub critical_damage: f32,
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self {
            max_health: 1000,
            current_health: 1000,
            max_mana: 500,
            current_mana: 500,
            attack: 100,
            defense: 50,
            move_speed: 5.0,      // Unity units per second
            attack_speed: 1.0,    // attacks per second
            critical_chance: 5.0, // 5%
            critical_damage: 1.5, // 150%
        }
    }
}

impl PlayerStats {
    /// 체력 회복
    pub fn heal(&mut self, amount: u32) -> u32 {
        let old_health = self.current_health;
        self.current_health = (self.current_health + amount).min(self.max_health);
        self.current_health - old_health
    }

    /// 피해 입음
    pub fn take_damage(&mut self, damage: u32) -> u32 {
        let actual_damage = damage.saturating_sub(self.defense / 2);
        let old_health = self.current_health;
        self.current_health = self.current_health.saturating_sub(actual_damage);
        old_health - self.current_health
    }

    /// 마나 소모
    pub fn consume_mana(&mut self, amount: u32) -> bool {
        if self.current_mana >= amount {
            self.current_mana -= amount;
            true
        } else {
            false
        }
    }

    /// 마나 회복
    pub fn restore_mana(&mut self, amount: u32) -> u32 {
        let old_mana = self.current_mana;
        self.current_mana = (self.current_mana + amount).min(self.max_mana);
        self.current_mana - old_mana
    }
    /// 생존 여부
    pub fn is_alive(&self) -> bool {
        self.current_health > 0
    }

    /// 체력 비율 (0.0 ~ 1.0)
    pub fn health_percentage(&self) -> f32 {
        self.current_health as f32 / self.max_health as f32
    }

    /// 마나 비율 (0.0 ~ 1.0)
    pub fn mana_percentage(&self) -> f32 {
        self.current_mana as f32 / self.max_mana as f32
    }
}

/// 플레이어 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerState {
    /// idle 상태
    Idle,
    /// 이동 중
    Moving,
    /// 공격 중
    Attacking,
    /// 스킬 시전 중
    CastingSkill,
    /// 죽음
    Dead,
    /// 스턴 상태
    Stunned,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self::Idle
    }
}

/// 플레이어 엔티티
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Player {
    /// 플레이어 ID
    pub id: PlayerId,
    /// 세션 ID (RUDP 연결)
    pub session_id: u64,
    /// 플레이어 이름
    pub name: String,
    /// 현재 3D 위치
    pub position: Position,
    /// 3D 이동 방향 및 속도
    pub velocity: Velocity,
    /// 바라보는 방향 (Unity Euler 각도)
    pub rotation: (f32, f32, f32), // (x, y, z) Euler angles
    /// 플레이어 상태
    pub state: PlayerState,
    /// 플레이어 스탯
    pub stats: PlayerStats,
    /// 스킬 쿨타임 (스킬 ID -> 쿨타임 종료 시간) - 직렬화에서 제외
    #[serde(skip)]
    pub skill_cooldowns: HashMap<u32, Instant>,
    /// 마지막 업데이트 시간 - 직렬화에서 제외
    #[serde(skip)]
    pub last_update: Instant,
    /// 마지막 공격 시간 - 직렬화에서 제외
    #[serde(skip)]
    pub last_attack: Instant,
    /// 참여 중인 방 ID
    pub room_id: Option<u32>,
    /// 시야 범위 (게임 단위)
    pub vision_range: f32,
    /// 공격 범위 (게임 단위)
    pub attack_range: f32,
    /// 무적 시간 (피격 후 잠시 무적) - 직렬화에서 제외
    #[serde(skip)]
    pub invulnerable_until: Option<Instant>,
    /// 플레이어 생성 시간 - 직렬화에서 제외
    #[serde(skip)]
    pub created_at: Instant,
    /// 온라인 여부
    pub is_online: bool,
}

impl Player {
    /// 새로운 플레이어 생성
    pub fn new(id: PlayerId, session_id: u64, name: String, spawn_position: Position) -> Self {
        let now = Instant::now();

        Self {
            id,
            session_id,
            name,
            position: spawn_position,
            velocity: Velocity::default(),
            rotation: (0.0, 0.0, 0.0),
            state: PlayerState::Idle,
            stats: PlayerStats::default(),
            skill_cooldowns: HashMap::new(),
            last_update: now,
            last_attack: now - Duration::from_secs(10), // 처음에는 공격 가능
            room_id: None,
            vision_range: 500.0, // 500 units
            attack_range: 50.0,  // 50 units
            invulnerable_until: None,
            created_at: now,
            is_online: true,
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            id: 0,
            session_id: 0,
            name: String::new(),
            position: Position::default(),
            velocity: Velocity::default(),
            rotation: (0.0, 0.0, 0.0),
            state: PlayerState::Idle,
            stats: PlayerStats::default(),
            skill_cooldowns: HashMap::new(),
            last_update: now,
            last_attack: now - Duration::from_secs(10),
            room_id: None,
            vision_range: 500.0,
            attack_range: 50.0,
            invulnerable_until: None,
            created_at: now,
            is_online: false,
        }
    }
}

impl Player {
    /// 플레이어 업데이트 (60 TPS)
    pub fn update(&mut self, delta_time: f32) -> Vec<PlayerEvent> {
        let mut events = Vec::new();
        self.last_update = Instant::now();

        // 이동 업데이트
        if self.state == PlayerState::Moving {
            self.update_movement(delta_time);
        }

        // 자동 회복 (마나)
        if self.stats.current_mana < self.stats.max_mana {
            let mana_regen = (self.stats.max_mana as f32 * 0.02 * delta_time) as u32; // 초당 2% 회복
            if mana_regen > 0 {
                self.stats.restore_mana(mana_regen);
                events.push(PlayerEvent::ManaRegeneration {
                    player_id: self.id,
                    amount: mana_regen,
                    current_mana: self.stats.current_mana,
                });
            }
        }

        events
    }

    /// 이동 업데이트
    fn update_movement(&mut self, delta_time: f32) {
        if self.velocity.magnitude() > 0.0 {
            let speed_multiplier = self.get_speed_multiplier();
            let actual_speed = self.stats.move_speed * speed_multiplier;

            self.position.x += self.velocity.x * actual_speed * delta_time;
            self.position.y += self.velocity.y * actual_speed * delta_time;

            // 맵 경계 확인 (필요시 구현)
            self.clamp_to_world_bounds();
        } else {
            self.state = PlayerState::Idle;
        }
    }

    /// 월드 경계 제한
    fn clamp_to_world_bounds(&mut self) {
        // 맵 크기 설정 (예시: 10000 x 10000)
        const WORLD_MIN_X: f32 = 0.0;
        const WORLD_MAX_X: f32 = 10000.0;
        const WORLD_MIN_Y: f32 = 0.0;
        const WORLD_MAX_Y: f32 = 10000.0;

        self.position.x = self.position.x.clamp(WORLD_MIN_X, WORLD_MAX_X);
        self.position.y = self.position.y.clamp(WORLD_MIN_Y, WORLD_MAX_Y);
    }

    /// 속도 배율 계산
    fn get_speed_multiplier(&self) -> f32 {
        1.0 // 기본 속도만 사용 (상태 효과 제거됨)
    }

    /// 이동 시작
    pub fn start_moving(&mut self, direction: Velocity) -> Result<()> {
        if self.state == PlayerState::Dead || self.state == PlayerState::Stunned {
            return Err(anyhow!("Cannot move in current state: {:?}", self.state));
        }

        self.velocity = direction.normalize();
        self.state = PlayerState::Moving;

        Ok(())
    }

    /// 이동 정지
    pub fn stop_moving(&mut self) {
        self.velocity = Velocity::default();
        self.state = PlayerState::Idle;
    }

    /// 공격 실행
    pub fn attack(&mut self, target_position: Position) -> Result<AttackResult> {
        if self.state == PlayerState::Dead {
            return Err(anyhow!("Dead player cannot attack"));
        }

        if self.state == PlayerState::Stunned || self.state == PlayerState::CastingSkill {
            return Err(anyhow!("Player is unable to attack in current state"));
        }

        // 공격 쿨타임 확인
        let attack_interval = Duration::from_secs_f32(1.0 / self.stats.attack_speed);
        if self.last_attack.elapsed() < attack_interval {
            return Err(anyhow!("Attack on cooldown"));
        }

        // 공격 범위 확인
        let distance = self.position.distance_to(&target_position);
        if distance > self.attack_range {
            return Err(anyhow!("Target out of range"));
        }

        // 공격 실행
        self.state = PlayerState::Attacking;
        self.last_attack = Instant::now();

        // 크리티컬 계산
        let is_critical = rand::random::<f32>() < (self.stats.critical_chance / 100.0);
        let damage = if is_critical {
            (self.stats.attack as f32 * self.stats.critical_damage) as u32
        } else {
            self.stats.attack
        };

        Ok(AttackResult {
            damage,
            is_critical,
            attacker_id: self.id,
            target_position,
            attack_range: self.attack_range,
        })
    }

    /// 피해 받기
    pub fn receive_damage(&mut self, damage: u32, _attacker_id: Option<PlayerId>) -> DamageResult {
        if self.state == PlayerState::Dead {
            return DamageResult {
                damage_dealt: 0,
                remaining_health: 0,
                is_killed: false,
                was_invulnerable: false,
            };
        }

        // 무적 시간 확인
        if let Some(invuln_until) = self.invulnerable_until {
            if Instant::now() < invuln_until {
                return DamageResult {
                    damage_dealt: 0,
                    remaining_health: self.stats.current_health,
                    is_killed: false,
                    was_invulnerable: true,
                };
            }
        }

        let actual_damage = self.stats.take_damage(damage);
        let is_killed = !self.stats.is_alive();

        if is_killed {
            self.state = PlayerState::Dead;
        } else {
            // 피격 후 0.5초 무적 시간
            self.invulnerable_until = Some(Instant::now() + Duration::from_millis(500));
        }

        DamageResult {
            damage_dealt: actual_damage,
            remaining_health: self.stats.current_health,
            is_killed,
            was_invulnerable: false,
        }
    }

    /// 플레이어 부활
    pub fn respawn(&mut self, spawn_position: Position) -> Vec<PlayerEvent> {
        let mut events = Vec::new();

        self.position = spawn_position;
        self.velocity = Velocity::default();
        self.state = PlayerState::Idle;

        // 체력/마나 완전 회복
        self.stats.current_health = self.stats.max_health;
        self.stats.current_mana = self.stats.max_mana;

        // 상태 효과 제거 불필요 (상태 효과 시스템 제거됨)

        // 무적 시간 설정 (5초)
        self.invulnerable_until = Some(Instant::now() + Duration::from_secs(5));

        events.push(PlayerEvent::PlayerRespawn {
            player_id: self.id,
            position: self.position,
        });

        events
    }

    /// 시야 범위 내 확인
    pub fn is_in_vision_range(&self, target_position: &Position) -> bool {
        self.position.distance_to(target_position) <= self.vision_range
    }

    /// 공격 범위 내 확인
    pub fn is_in_attack_range(&self, target_position: &Position) -> bool {
        self.position.distance_to(target_position) <= self.attack_range
    }

    /// 플레이어 정보 요약
    pub fn get_summary(&self) -> PlayerSummary {
        PlayerSummary {
            id: self.id,
            name: self.name.clone(),
            position: self.position,
            health_percentage: self.stats.health_percentage(),
            mana_percentage: self.stats.mana_percentage(),
            state: self.state,
            is_online: self.is_online,
        }
    }
}

/// 공격 결과
#[derive(Debug, Clone, Serialize)]
pub struct AttackResult {
    pub damage: u32,
    pub is_critical: bool,
    pub attacker_id: PlayerId,
    pub target_position: Position,
    pub attack_range: f32,
}

/// 피해 결과
#[derive(Debug, Clone, Serialize)]
pub struct DamageResult {
    pub damage_dealt: u32,
    pub remaining_health: u32,
    pub is_killed: bool,
    pub was_invulnerable: bool,
}

/// 플레이어 이벤트
#[derive(Debug, Clone, Serialize)]
pub enum PlayerEvent {
    /// 플레이어 사망
    PlayerDeath {
        player_id: PlayerId,
        cause: DeathCause,
    },
    /// 플레이어 부활
    PlayerRespawn {
        player_id: PlayerId,
        position: Position,
    },
    /// 마나 회복
    ManaRegeneration {
        player_id: PlayerId,
        amount: u32,
        current_mana: u32,
    },
}

/// 플레이어 요약 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSummary {
    pub id: PlayerId,
    pub name: String,
    pub position: Position,
    pub health_percentage: f32,
    pub mana_percentage: f32,
    pub state: PlayerState,
    pub is_online: bool,
}

/// 플레이어 매니저 (스텁 - 기본 구현)
pub struct PlayerManager;

impl PlayerManager {
    pub fn new() -> Self {
        Self
    }

    pub fn get_player(&self, _player_id: PlayerId) -> Option<Player> {
        // Stub implementation
        None
    }

    pub async fn create_player(&self, session_id: u64, _name: String, _spawn_position: Position) -> Result<PlayerId> {
        // Stub implementation - generate dummy ID
        Ok(session_id as PlayerId)
    }
}