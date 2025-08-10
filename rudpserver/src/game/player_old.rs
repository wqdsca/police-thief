//! 플레이어 시스템
//!
//! Unity 클라이언트를 위한 실시간 3D 게임에서 플레이어의 상태, 움직임, 전투를 관리하는 핵심 시스템
//!
//! # Features
//! - 플레이어 상태 관리 (HP, MP, 스탯)
//! - 실시간 3D 위치 동기화 (60 TPS)
//! - 스킬 시스템 및 쿨타임 관리
//! - 인벤토리 및 장비 시스템
//! - 버프/디버프 상태 효과 관리

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::info;

use crate::types::*;
use shared::tool::high_performance::redis_optimizer::RedisOptimizer;

/// 플레이어 ID 타입
pub type PlayerId = u32;

// Position은 messages.rs에서 정의되어 있으므로 re-export만 사용
pub use crate::game::messages::Position;

// Position 구현은 messages.rs에서 이미 정의되어 있음

/// 3D 이동 벡터 (Unity 호환)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Velocity {
    pub x: f32,
    pub y: f32, // Unity Y-up
    pub z: f32,
}

impl Velocity {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn magnitude(&self) -> f32 {
        (self.x.powi(2) + self.y.powi(2) + self.z.powi(2)).sqrt()
    }

    pub fn normalize(&self) -> Self {
        let mag = self.magnitude();
        if mag > 0.0 {
            Self {
                x: self.x / mag,
                y: self.y / mag,
                z: self.z / mag,
            }
        } else {
            *self
        }
    }
}

impl Default for Velocity {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
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

        // 무적 시간 해제 확인
        if let Some(invuln_until) = self.invulnerable_until {
            if Instant::now() >= invuln_until {
                self.invulnerable_until = None;
                events.push(PlayerEvent::InvulnerabilityEnded { player_id: self.id });
            }
        }

        events
    }

    /// 상태 효과 업데이트 (deprecated - 상태 효과 제거됨)
    fn update_status_effects(&mut self, _delta_time: f32, _events: &mut Vec<PlayerEvent>) {
        // Status effects have been removed from the game
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

    /// 스킬 사용
    pub fn use_skill(
        &mut self,
        skill_id: u32,
        target_position: Option<Position>,
    ) -> Result<SkillResult> {
        if self.state == PlayerState::Dead || self.state == PlayerState::Stunned {
            return Err(anyhow!("Cannot use skill in current state"));
        }

        // 스킬 쿨타임 확인
        if let Some(cooldown_end) = self.skill_cooldowns.get(&skill_id) {
            if Instant::now() < *cooldown_end {
                let remaining = cooldown_end.duration_since(Instant::now());
                return Err(anyhow!("Skill on cooldown: {:?} remaining", remaining));
            }
        }

        // 스킬 데이터 가져오기 (실제로는 스킬 시스템에서)
        let skill_data = self.get_skill_data(skill_id)?;

        // 마나 확인
        if !self.stats.consume_mana(skill_data.mana_cost) {
            return Err(anyhow!("Not enough mana"));
        }

        // 스킬 시전
        self.state = PlayerState::CastingSkill;
        self.skill_cooldowns
            .insert(skill_id, Instant::now() + skill_data.cooldown);

        Ok(SkillResult {
            skill_id,
            caster_id: self.id,
            target_position,
            damage: skill_data.damage,
            range: skill_data.range,
            effect_type: skill_data.effect_type.clone(),
            cast_time: skill_data.cast_time,
        })
    }

    /// 스킬 데이터 가져오기 (임시 구현)
    fn get_skill_data(&self, skill_id: u32) -> Result<SkillData> {
        match skill_id {
            1 => Ok(SkillData {
                id: 1,
                name: "Fireball".to_string(),
                damage: self.stats.attack * 2,
                mana_cost: 50,
                cooldown: Duration::from_secs(5),
                range: 200.0,
                cast_time: Duration::from_millis(800),
                effect_type: Some(StatusEffectType::Burn),
            }),
            2 => Ok(SkillData {
                id: 2,
                name: "Heal".to_string(),
                damage: 0, // 힐은 damage 필드를 힐량으로 사용
                mana_cost: 30,
                cooldown: Duration::from_secs(3),
                range: 0.0, // 자가 버프
                cast_time: Duration::from_millis(1000),
                effect_type: None,
            }),
            3 => Ok(SkillData {
                id: 3,
                name: "Speed Boost".to_string(),
                damage: 0,
                mana_cost: 40,
                cooldown: Duration::from_secs(10),
                range: 0.0,
                cast_time: Duration::from_millis(500),
                effect_type: Some(StatusEffectType::SpeedUp),
            }),
            _ => Err(anyhow!("Unknown skill ID: {}", skill_id)),
        }
    }

    /// 상태 효과 추가
    pub fn add_status_effect(&mut self, effect: StatusEffect) -> Vec<PlayerEvent> {
        let mut events = Vec::new();
        let effect_type = effect.effect_type;

        // 기존 효과가 있다면 덮어쓰기
        if self.status_effects.contains_key(&effect_type) {
            events.push(PlayerEvent::StatusEffectRemoved {
                player_id: self.id,
                effect_type,
            });
        }

        self.status_effects.insert(effect_type, effect);

        events.push(PlayerEvent::StatusEffectAdded {
            player_id: self.id,
            effect_type,
            duration: self.status_effects[&effect_type].total_duration,
        });

        events
    }

    /// 상태 효과 제거
    pub fn remove_status_effect(
        &mut self,
        effect_type: StatusEffectType,
        events: &mut Vec<PlayerEvent>,
    ) {
        if self.status_effects.remove(&effect_type).is_some() {
            events.push(PlayerEvent::StatusEffectRemoved {
                player_id: self.id,
                effect_type,
            });
        }
    }

    /// 속도 배율 계산 (상태 효과 반영)
    fn get_speed_multiplier(&self) -> f32 {
        let mut multiplier = 1.0;

        if self.status_effects.contains_key(&StatusEffectType::SpeedUp) {
            multiplier *= 1.5; // 50% 증가
        }

        if self
            .status_effects
            .contains_key(&StatusEffectType::SlowDown)
        {
            multiplier *= 0.5; // 50% 감소
        }

        multiplier
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

        // 모든 상태 효과 제거
        for effect_type in self.status_effects.keys().cloned().collect::<Vec<_>>() {
            self.remove_status_effect(effect_type, &mut events);
        }

        // 무적 시간 설정 (5초)
        self.invulnerable_until = Some(Instant::now() + Duration::from_secs(5));

        events.push(PlayerEvent::PlayerRespawn {
            player_id: self.id,
            position: self.position,
        });

        events
    }

    // 경험치 시스템 제거됨 - 레벨 없음

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
            level: self.stats.level,
            position: self.position,
            health_percentage: self.stats.health_percentage(),
            mana_percentage: self.stats.mana_percentage(),
            state: self.state,
            is_online: self.is_online,
        }
    }
}

/// 스킬 데이터
#[derive(Debug, Clone)]
pub struct SkillData {
    pub id: u32,
    pub name: String,
    pub damage: u32,
    pub mana_cost: u32,
    pub cooldown: Duration,
    pub range: f32,
    pub cast_time: Duration,
    pub effect_type: Option<StatusEffectType>,
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

/// 스킬 결과
#[derive(Debug, Clone, Serialize)]
pub struct SkillResult {
    pub skill_id: u32,
    pub caster_id: PlayerId,
    pub target_position: Option<Position>,
    pub damage: u32,
    pub range: f32,
    pub effect_type: Option<StatusEffectType>,
    pub cast_time: Duration,
}

/// 사망 원인
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeathCause {
    /// 플레이어 공격
    Player(PlayerId),
    /// 상태 효과
    StatusEffect(StatusEffectType),
    /// 환경 (낙사 등)
    Environment,
    /// 기타
    Other,
}

/// 플레이어 이벤트
#[derive(Debug, Clone, Serialize)]
pub enum PlayerEvent {
    /// 상태 효과 추가
    StatusEffectAdded {
        player_id: PlayerId,
        effect_type: StatusEffectType,
        duration: f32,
    },
    /// 상태 효과 제거
    StatusEffectRemoved {
        player_id: PlayerId,
        effect_type: StatusEffectType,
    },
    /// 상태 효과로 인한 피해
    StatusDamage {
        player_id: PlayerId,
        effect_type: StatusEffectType,
        damage: u32,
        remaining_health: u32,
    },
    /// 상태 효과로 인한 치료
    StatusHeal {
        player_id: PlayerId,
        amount: u32,
        current_health: u32,
    },
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
    /// 무적 시간 종료
    InvulnerabilityEnded { player_id: PlayerId },
    /// 마나 자동 회복
    ManaRegeneration {
        player_id: PlayerId,
        amount: u32,
        current_mana: u32,
    },
    /// 경험치 획득
    ExperienceGained {
        player_id: PlayerId,
        amount: u64,
        total_experience: u64,
    },
    /// 레벨업
    LevelUp {
        player_id: PlayerId,
        new_level: u32,
        stats: PlayerStats,
    },
}

/// 플레이어 요약 정보 (가벼운 전송용)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSummary {
    pub id: PlayerId,
    pub name: String,
    pub level: u32,
    pub position: Position,
    pub health_percentage: f32,
    pub mana_percentage: f32,
    pub state: PlayerState,
    pub is_online: bool,
}

/// 플레이어 관리자
pub struct PlayerManager {
    /// 모든 플레이어들 (ID -> Player)
    players: Arc<DashMap<PlayerId, Player>>,
    /// 세션 ID -> 플레이어 ID 매핑
    session_to_player: Arc<RwLock<HashMap<u64, PlayerId>>>,
    /// Redis 최적화기
    redis_optimizer: Arc<RedisOptimizer>,
    /// 다음 플레이어 ID
    next_player_id: Arc<std::sync::atomic::AtomicU32>,
}

impl PlayerManager {
    /// 새로운 플레이어 관리자 생성
    pub fn new(redis_optimizer: Arc<RedisOptimizer>) -> Self {
        Self {
            players: Arc::new(DashMap::new()),
            session_to_player: Arc::new(RwLock::new(HashMap::new())),
            redis_optimizer,
            next_player_id: Arc::new(std::sync::atomic::AtomicU32::new(1)),
        }
    }

    /// 새로운 플레이어 생성
    pub async fn create_player(
        &self,
        session_id: u64,
        name: String,
        spawn_position: Position,
    ) -> Result<PlayerId> {
        let player_id = self
            .next_player_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let player = Player::new(player_id, session_id, name, spawn_position);

        // 메모리에 저장
        self.players.insert(player_id, player);

        // 세션 매핑 저장
        {
            let mut session_map = self.session_to_player.write().await;
            session_map.insert(session_id, player_id);
        }

        // Redis에 저장 (영구 저장)
        self.save_player_to_redis(player_id).await?;

        info!(
            player_id = %player_id,
            session_id = %session_id,
            "Player created"
        );

        Ok(player_id)
    }

    /// 플레이어 가져오기
    pub fn get_player(&self, player_id: PlayerId) -> Option<Player> {
        self.players.get(&player_id).map(|entry| entry.clone())
    }

    /// 세션 ID로 플레이어 가져오기
    pub async fn get_player_by_session(&self, session_id: u64) -> Option<Player> {
        let session_map = self.session_to_player.read().await;
        if let Some(player_id) = session_map.get(&session_id) {
            self.get_player(*player_id)
        } else {
            None
        }
    }

    /// 모든 플레이어 업데이트 (60 TPS)
    pub async fn update_all_players(&self, delta_time: f32) -> Vec<PlayerEvent> {
        let mut all_events = Vec::new();

        for entry in self.players.iter() {
            let mut player = entry.value().clone();
            let events = player.update(delta_time);
            all_events.extend(events);
            // TODO: 변경된 player를 다시 저장해야 함
        }

        all_events
    }

    /// 플레이어 제거 (연결 해제 시)
    pub async fn remove_player(&self, player_id: PlayerId) -> Result<()> {
        if let Some(player) = self.get_player(player_id) {
            let session_id = player.session_id;

            // 세션 매핑 제거
            {
                let mut session_map = self.session_to_player.write().await;
                session_map.remove(&session_id);
            }

            // 메모리에서 제거
            self.players.remove(&player_id);

            info!(
                player_id = %player_id,
                session_id = %session_id,
                "Player removed"
            );
        }

        Ok(())
    }

    /// 위치 기반 플레이어 검색
    pub async fn get_players_in_range(&self, center: Position, range: f32) -> Vec<Player> {
        let mut players_in_range = Vec::new();

        for entry in self.players.iter() {
            let player = entry.value();

            if player.position.distance_to(&center) <= range {
                players_in_range.push(player.clone());
            }
        }

        players_in_range
    }

    /// 플레이어를 Redis에 저장
    async fn save_player_to_redis(&self, player_id: PlayerId) -> Result<()> {
        if let Some(player) = self.get_player(player_id) {
            let player_data = serde_json::to_vec(&player)?;

            let key = format!("player:{}", player_id);
            self.redis_optimizer
                .set(&key, &player_data, Some(7200))
                .await?; // 2시간 TTL
        }

        Ok(())
    }

    /// Redis에서 플레이어 로드
    pub async fn load_player_from_redis(&self, player_id: PlayerId) -> Result<Option<Player>> {
        let key = format!("player:{}", player_id);

        if let Some(data) = self.redis_optimizer.get(&key).await? {
            let player: Player = serde_json::from_slice(&data)?;
            Ok(Some(player))
        } else {
            Ok(None)
        }
    }

    /// 활성 플레이어 수
    pub fn get_active_player_count(&self) -> usize {
        self.players.len()
    }

    /// 플레이어 통계
    pub async fn get_player_stats(&self) -> PlayerManagerStats {
        let total_players = self.players.len();
        let mut online_players = 0;
        let mut total_level = 0u64;

        for entry in self.players.iter() {
            let player = entry.value();

            if player.is_online {
                online_players += 1;
            }
            total_level += player.stats.level as u64;
        }

        let average_level = if total_players > 0 {
            total_level as f64 / total_players as f64
        } else {
            0.0
        };

        PlayerManagerStats {
            total_players,
            online_players,
            average_level,
        }
    }
}

/// 플레이어 관리자 통계
#[derive(Debug, Clone, Serialize)]
pub struct PlayerManagerStats {
    pub total_players: usize,
    pub online_players: usize,
    pub average_level: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_creation() {
        let position = Position::new(100.0, 200.0);
        let player = Player::new(1, 12345, "TestPlayer".to_string(), position);

        assert_eq!(player.id, 1);
        assert_eq!(player.session_id, 12345);
        assert_eq!(player.name, "TestPlayer");
        assert_eq!(player.position, position);
        assert_eq!(player.state, PlayerState::Idle);
        assert!(player.stats.is_alive());
    }

    #[test]
    fn test_position_distance() {
        let pos1 = Position::new(0.0, 0.0);
        let pos2 = Position::new(3.0, 4.0);

        assert_eq!(pos1.distance_to(&pos2), 5.0);
    }

    #[test]
    fn test_velocity_normalization() {
        let velocity = Velocity::new(3.0, 4.0);
        let normalized = velocity.normalize();

        assert!((normalized.magnitude() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_player_stats() {
        let mut stats = PlayerStats::default();

        // 피해 테스트
        let damage_dealt = stats.take_damage(100);
        assert_eq!(damage_dealt, 75); // 100 - (50/2) = 75 actual damage
        assert_eq!(stats.current_health, 925);

        // 치료 테스트
        let healed = stats.heal(50);
        assert_eq!(healed, 50);
        assert_eq!(stats.current_health, 975);

        // 마나 소모 테스트
        assert!(stats.consume_mana(100));
        assert_eq!(stats.current_mana, 400);
        assert!(!stats.consume_mana(500)); // 부족한 마나
    }

    #[test]
    fn test_status_effect() {
        let mut effect = StatusEffect::new(StatusEffectType::Poison, 50.0, 5.0);

        assert!(!effect.is_expired());
        assert_eq!(effect.effect_type, StatusEffectType::Poison);

        // 시간 진행 시뮬레이션
        let tick_damage = effect.update(1.0);
        assert!(tick_damage.is_some());

        // 5초 후 만료 확인
        effect.update(4.0);
        assert!(effect.is_expired());
    }

    #[tokio::test]
    async fn test_player_movement() {
        let position = Position::new(100.0, 100.0);
        let mut player = Player::new(1, 12345, "TestPlayer".to_string(), position);

        // 이동 시작
        let direction = Velocity::new(1.0, 0.0);
        assert!(player.start_moving(direction).is_ok());
        assert_eq!(player.state, PlayerState::Moving);

        // 1초 후 위치 업데이트 (속도: 200 units/sec)
        let events = player.update(1.0);
        assert_eq!(player.position.x, 300.0); // 100 + 200*1
        assert_eq!(player.position.y, 100.0);

        // 이동 정지
        player.stop_moving();
        assert_eq!(player.state, PlayerState::Idle);
    }

    #[tokio::test]
    async fn test_player_attack() {
        let position = Position::new(100.0, 100.0);
        let mut player = Player::new(1, 12345, "TestPlayer".to_string(), position);

        // 공격 범위 내 타겟
        let target_pos = Position::new(120.0, 100.0);
        let result = player.attack(target_pos);
        assert!(result.is_ok());

        let attack_result = result.unwrap();
        assert_eq!(attack_result.attacker_id, 1);
        assert_eq!(attack_result.damage, 100); // 기본 공격력

        // 공격 범위 밖 타겟
        let far_target = Position::new(200.0, 100.0);
        let result = player.attack(far_target);
        assert!(result.is_err());
    }

    #[test]
    fn test_experience_and_level_up() {
        let mut stats = PlayerStats::default();

        // 경험치 획득 (레벨업 없음)
        let level_up = stats.gain_experience(500);
        assert!(!level_up);
        assert_eq!(stats.experience, 500);
        assert_eq!(stats.level, 1);

        // 레벨업
        let level_up = stats.gain_experience(500);
        assert!(level_up);
        assert_eq!(stats.level, 2);
        assert_eq!(stats.max_health, 1100); // 레벨업으로 증가
    }
}
