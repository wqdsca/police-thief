//! RUDP 게임 서버 - 스킬 시스템 예시
//!
//! 새로운 기능 추가 방법을 보여주는 샘플 코드입니다.
//! 스킬 시스템을 예시로 하여 match 패턴을 활용한 구현 방법을 제시합니다.
//!
//! # 구현 패턴
//! 1. 메시지 타입 정의 (messages.rs)
//! 2. 상태 관리 구조체 정의
//! 3. 처리 로직 구현 (match 패턴 활용)
//! 4. Redis 저장/로드
//! 5. 이벤트 브로드캐스트

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::game::messages::{Direction, GameMessage, PlayerId, Position};
use crate::game::room_user_manager::{RoomUserInfo, RoomUserManager};

/// 스킬 ID 타입
pub type SkillId = u32;

/// 스킬 타입 열거형
///
/// 다양한 스킬 종류를 정의합니다. 새로운 스킬 추가 시 여기에 추가하세요.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SkillType {
    /// 기본 공격 스킬
    BasicAttack,
    /// 치유 스킬
    Heal,
    /// 순간이동 스킬
    Teleport,
    /// 방어막 스킬
    Shield,
    /// 범위 공격 스킬
    AreaDamage,
    /// 버프 스킬
    Buff { buff_type: BuffType },
    /// 디버프 스킬
    Debuff { debuff_type: DebuffType },
    /// 소환 스킬
    Summon { creature_type: String },
    /// 변신 스킬
    Transform { form: String },
    /// 사용자 정의 스킬 (확장용)
    Custom {
        skill_name: String,
        parameters: HashMap<String, String>,
    },
}

/// 버프 타입
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BuffType {
    /// 공격력 증가
    AttackBoost,
    /// 이동속도 증가
    SpeedBoost,
    /// 방어력 증가
    DefenseBoost,
    /// 체력 재생
    HealthRegeneration,
    /// 마나 재생
    ManaRegeneration,
}

/// 디버프 타입
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DebuffType {
    /// 독
    Poison,
    /// 슬로우
    Slow,
    /// 침묵 (스킬 사용 불가)
    Silence,
    /// 스턴 (행동 불가)
    Stun,
    /// 실명 (시야 감소)
    Blind,
}

/// 스킬 사용 요청 메시지
///
/// GameMessage에 추가할 메시지 타입 예시입니다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UseSkillMessage {
    /// 스킬 ID
    pub skill_id: SkillId,
    /// 스킬 타입
    pub skill_type: SkillType,
    /// 대상 위치 (선택적)
    pub target_position: Option<Position>,
    /// 대상 플레이어 (선택적)
    pub target_player: Option<PlayerId>,
    /// 스킬 방향
    pub direction: Direction,
    /// 추가 매개변수
    pub parameters: HashMap<String, f32>,
    /// 클라이언트 타임스탬프
    pub client_timestamp: u64,
}

/// 스킬 사용 결과 메시지
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillResultMessage {
    /// 스킬을 사용한 플레이어 ID
    pub caster_id: PlayerId,
    /// 스킬 ID
    pub skill_id: SkillId,
    /// 스킬 타입
    pub skill_type: SkillType,
    /// 성공 여부
    pub success: bool,
    /// 실제 효과를 받은 대상들
    pub affected_targets: Vec<PlayerId>,
    /// 스킬 효과 데이터
    pub effect_data: SkillEffectData,
    /// 서버 타임스탬프
    pub server_timestamp: u64,
    /// 실패 사유 (실패시)
    pub failure_reason: Option<String>,
}

/// 스킬 효과 데이터
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillEffectData {
    /// 데미지 (공격 스킬)
    pub damage: u32,
    /// 치유량 (치유 스킬)
    pub healing: u32,
    /// 지속시간 (초)
    pub duration: f32,
    /// 효과 범위
    pub area_of_effect: Option<f32>,
    /// 상태 효과들
    pub status_effects: Vec<String>,
    /// 추가 효과 데이터
    pub custom_data: HashMap<String, f32>,
}

/// 플레이어 스킬 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSkillState {
    /// 플레이어 ID
    pub player_id: PlayerId,
    /// 현재 마나
    pub current_mana: u32,
    /// 최대 마나
    pub max_mana: u32,
    /// 스킬 쿨다운 정보 (스킬 ID -> 만료 시간)
    pub cooldowns: HashMap<SkillId, u64>,
    /// 활성 버프/디버프
    pub active_effects: Vec<ActiveEffect>,
    /// 마지막 스킬 사용 시간
    pub last_skill_time: u64,
}

/// 활성 효과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveEffect {
    /// 효과 ID
    pub effect_id: String,
    /// 효과 타입
    pub effect_type: String,
    /// 효과 값
    pub effect_value: f32,
    /// 만료 시간
    pub expires_at: u64,
    /// 스택 수
    pub stack_count: u32,
}

/// 스킬 시스템 관리자
///
/// 스킬 시스템의 모든 로직을 담당하는 구조체입니다.
pub struct SkillSystem {
    /// 방 사용자 관리자
    room_user_manager: std::sync::Arc<RoomUserManager>,

    /// 스킬 정의 데이터 (스킬 ID -> 스킬 정보)
    /// 실제로는 설정 파일이나 데이터베이스에서 로드
    skill_definitions: HashMap<SkillId, SkillDefinition>,
}

/// 스킬 정의
#[derive(Debug, Clone)]
pub struct SkillDefinition {
    pub skill_id: SkillId,
    pub name: String,
    pub skill_type: SkillType,
    pub mana_cost: u32,
    pub cooldown_ms: u64,
    pub cast_time_ms: u64,
    pub range: f32,
    pub area_of_effect: Option<f32>,
    pub base_damage: u32,
    pub base_healing: u32,
    pub level_scaling: f32,
}

impl SkillSystem {
    /// 새로운 스킬 시스템 생성
    pub fn new(room_user_manager: std::sync::Arc<RoomUserManager>) -> Self {
        let mut skill_definitions = HashMap::new();

        // 예시 스킬 정의들 (실제로는 설정 파일에서 로드)
        skill_definitions.insert(
            1,
            SkillDefinition {
                skill_id: 1,
                name: "파이어볼".to_string(),
                skill_type: SkillType::BasicAttack,
                mana_cost: 30,
                cooldown_ms: 3000,  // 3초
                cast_time_ms: 1000, // 1초
                range: 500.0,
                area_of_effect: Some(100.0),
                base_damage: 50,
                base_healing: 0,
                level_scaling: 1.2,
            },
        );

        skill_definitions.insert(
            2,
            SkillDefinition {
                skill_id: 2,
                name: "치유의 빛".to_string(),
                skill_type: SkillType::Heal,
                mana_cost: 40,
                cooldown_ms: 5000,  // 5초
                cast_time_ms: 2000, // 2초
                range: 300.0,
                area_of_effect: None,
                base_damage: 0,
                base_healing: 80,
                level_scaling: 1.1,
            },
        );

        skill_definitions.insert(
            3,
            SkillDefinition {
                skill_id: 3,
                name: "순간이동".to_string(),
                skill_type: SkillType::Teleport,
                mana_cost: 50,
                cooldown_ms: 10000, // 10초
                cast_time_ms: 500,  // 0.5초
                range: 800.0,
                area_of_effect: None,
                base_damage: 0,
                base_healing: 0,
                level_scaling: 1.0,
            },
        );

        Self {
            room_user_manager,
            skill_definitions,
        }
    }

    /// 스킬 사용 처리 (메인 로직)
    ///
    /// 이 함수가 GameStateManager에서 호출되는 핵심 함수입니다.
    ///
    /// # Arguments
    /// * `room_id` - 방 ID
    /// * `caster_id` - 스킬 사용자 ID
    /// * `skill_message` - 스킬 사용 메시지
    ///
    /// # Returns
    /// 스킬 사용 결과
    pub async fn handle_skill_use(
        &self,
        room_id: u16,
        caster_id: PlayerId,
        skill_message: UseSkillMessage,
    ) -> Result<SkillResultMessage> {
        debug!(
            caster_id = %caster_id,
            skill_id = %skill_message.skill_id,
            skill_type = ?skill_message.skill_type,
            "스킬 사용 요청 처리"
        );

        // 1. 스킬 정의 조회
        let skill_def = match self.skill_definitions.get(&skill_message.skill_id) {
            Some(def) => def,
            None => {
                return Ok(SkillResultMessage {
                    caster_id,
                    skill_id: skill_message.skill_id,
                    skill_type: skill_message.skill_type,
                    success: false,
                    affected_targets: vec![],
                    effect_data: SkillEffectData::default(),
                    server_timestamp: crate::utils::current_timestamp_ms(),
                    failure_reason: Some("Unknown skill".to_string()),
                });
            }
        };

        // 2. 사용자 정보 조회
        let caster_info = match self
            .room_user_manager
            .get_user_in_room(room_id, caster_id)
            .await?
        {
            Some(info) => info,
            None => {
                return Ok(SkillResultMessage {
                    caster_id,
                    skill_id: skill_message.skill_id,
                    skill_type: skill_message.skill_type,
                    success: false,
                    affected_targets: vec![],
                    effect_data: SkillEffectData::default(),
                    server_timestamp: crate::utils::current_timestamp_ms(),
                    failure_reason: Some("Caster not found".to_string()),
                });
            }
        };

        // 3. 스킬 타입별 처리 (핵심 match 패턴)
        let result = match &skill_message.skill_type {
            SkillType::BasicAttack => {
                self.handle_attack_skill(
                    room_id,
                    caster_id,
                    &caster_info,
                    skill_def,
                    &skill_message,
                )
                .await?
            }

            SkillType::Heal => {
                self.handle_heal_skill(room_id, caster_id, &caster_info, skill_def, &skill_message)
                    .await?
            }

            SkillType::Teleport => {
                self.handle_teleport_skill(
                    room_id,
                    caster_id,
                    &caster_info,
                    skill_def,
                    &skill_message,
                )
                .await?
            }

            SkillType::Shield => {
                self.handle_shield_skill(
                    room_id,
                    caster_id,
                    &caster_info,
                    skill_def,
                    &skill_message,
                )
                .await?
            }

            SkillType::AreaDamage => {
                self.handle_area_damage_skill(
                    room_id,
                    caster_id,
                    &caster_info,
                    skill_def,
                    &skill_message,
                )
                .await?
            }

            SkillType::Buff { buff_type } => {
                self.handle_buff_skill(
                    room_id,
                    caster_id,
                    &caster_info,
                    skill_def,
                    &skill_message,
                    buff_type,
                )
                .await?
            }

            SkillType::Debuff { debuff_type } => {
                self.handle_debuff_skill(
                    room_id,
                    caster_id,
                    &caster_info,
                    skill_def,
                    &skill_message,
                    debuff_type,
                )
                .await?
            }

            SkillType::Summon { creature_type } => {
                self.handle_summon_skill(
                    room_id,
                    caster_id,
                    &caster_info,
                    skill_def,
                    &skill_message,
                    creature_type,
                )
                .await?
            }

            SkillType::Transform { form } => {
                self.handle_transform_skill(
                    room_id,
                    caster_id,
                    &caster_info,
                    skill_def,
                    &skill_message,
                    form,
                )
                .await?
            }

            SkillType::Custom {
                skill_name,
                parameters,
            } => {
                self.handle_custom_skill(
                    room_id,
                    caster_id,
                    &caster_info,
                    skill_def,
                    &skill_message,
                    skill_name,
                    parameters,
                )
                .await?
            }
        };

        // 4. 스킬 사용 성공 시 쿨다운 및 마나 소모 처리
        if result.success {
            self.consume_resources(room_id, caster_id, skill_def)
                .await?;
        }

        info!(
            caster_id = %caster_id,
            skill_id = %skill_message.skill_id,
            success = %result.success,
            affected_count = %result.affected_targets.len(),
            "스킬 사용 결과"
        );

        Ok(result)
    }

    /// 공격 스킬 처리
    async fn handle_attack_skill(
        &self,
        room_id: u16,
        caster_id: PlayerId,
        _caster_info: &RoomUserInfo,
        skill_def: &SkillDefinition,
        skill_message: &UseSkillMessage,
    ) -> Result<SkillResultMessage> {
        let mut affected_targets = Vec::new();
        let mut total_damage = 0;

        // 대상 결정
        if let Some(target_id) = skill_message.target_player {
            if let Some(target_info) = self
                .room_user_manager
                .get_user_in_room(room_id, target_id)
                .await?
            {
                // 거리 체크
                let distance = calculate_distance(&_caster_info.position, &target_info.position);
                if distance <= skill_def.range {
                    affected_targets.push(target_id);

                    // 데미지 계산
                    let damage = self.calculate_damage(skill_def, _caster_info.level);
                    total_damage = damage;

                    // *** 체력 감소 로직 (주석 처리 - 개발 시 활성화) ***
                    // self.apply_damage(room_id, target_id, damage).await?;

                    debug!(
                        target_id = %target_id,
                        damage = %damage,
                        "공격 스킬 적용 (체력 로직 주석 처리됨)"
                    );
                }
            }
        }

        Ok(SkillResultMessage {
            caster_id,
            skill_id: skill_message.skill_id,
            skill_type: skill_message.skill_type.clone(),
            success: !affected_targets.is_empty(),
            affected_targets: affected_targets.clone(),
            effect_data: SkillEffectData {
                damage: total_damage,
                healing: 0,
                duration: 0.0,
                area_of_effect: skill_def.area_of_effect,
                status_effects: vec![],
                custom_data: HashMap::new(),
            },
            server_timestamp: crate::utils::current_timestamp_ms(),
            failure_reason: if affected_targets.is_empty() {
                Some("No valid targets".to_string())
            } else {
                None
            },
        })
    }

    /// 치유 스킬 처리
    async fn handle_heal_skill(
        &self,
        room_id: u16,
        caster_id: PlayerId,
        _caster_info: &RoomUserInfo,
        skill_def: &SkillDefinition,
        skill_message: &UseSkillMessage,
    ) -> Result<SkillResultMessage> {
        let mut affected_targets = Vec::new();
        let mut total_healing = 0;

        // 대상이 지정되지 않았으면 자신을 치유
        let target_id = skill_message.target_player.unwrap_or(caster_id);

        if let Some(_target_info) = self
            .room_user_manager
            .get_user_in_room(room_id, target_id)
            .await?
        {
            let distance = if target_id == caster_id {
                0.0
            } else {
                calculate_distance(&_caster_info.position, &_target_info.position)
            };

            if distance <= skill_def.range {
                affected_targets.push(target_id);

                // 치유량 계산
                let healing = self.calculate_healing(skill_def, _caster_info.level);
                total_healing = healing;

                // *** 체력 회복 로직 (주석 처리 - 개발 시 활성화) ***
                // self.apply_healing(room_id, target_id, healing).await?;

                debug!(
                    target_id = %target_id,
                    healing = %healing,
                    "치유 스킬 적용 (체력 로직 주석 처리됨)"
                );
            }
        }

        Ok(SkillResultMessage {
            caster_id,
            skill_id: skill_message.skill_id,
            skill_type: skill_message.skill_type.clone(),
            success: !affected_targets.is_empty(),
            affected_targets: affected_targets.clone(),
            effect_data: SkillEffectData {
                damage: 0,
                healing: total_healing,
                duration: 0.0,
                area_of_effect: skill_def.area_of_effect,
                status_effects: vec!["healing".to_string()],
                custom_data: HashMap::new(),
            },
            server_timestamp: crate::utils::current_timestamp_ms(),
            failure_reason: if affected_targets.is_empty() {
                Some("Target out of range".to_string())
            } else {
                None
            },
        })
    }

    /// 순간이동 스킬 처리
    async fn handle_teleport_skill(
        &self,
        room_id: u16,
        caster_id: PlayerId,
        caster_info: &RoomUserInfo,
        skill_def: &SkillDefinition,
        skill_message: &UseSkillMessage,
    ) -> Result<SkillResultMessage> {
        if let Some(target_pos) = skill_message.target_position {
            let distance = calculate_distance(&caster_info.position, &target_pos);

            if distance <= skill_def.range {
                // *** 위치 이동 로직 (주석 처리 - 개발 시 활성화) ***
                // self.room_user_manager.update_user_in_room(room_id, caster_id, |user| {
                //     user.position = target_pos;
                // }).await?;

                debug!(
                    caster_id = %caster_id,
                    from_pos = ?(caster_info.position.x, caster_info.position.y),
                    to_pos = ?(target_pos.x, target_pos.y),
                    "순간이동 스킬 적용 (위치 로직 주석 처리됨)"
                );

                return Ok(SkillResultMessage {
                    caster_id,
                    skill_id: skill_message.skill_id,
                    skill_type: skill_message.skill_type.clone(),
                    success: true,
                    affected_targets: vec![caster_id],
                    effect_data: SkillEffectData {
                        damage: 0,
                        healing: 0,
                        duration: 0.0,
                        area_of_effect: None,
                        status_effects: vec!["teleported".to_string()],
                        custom_data: [
                            ("from_x".to_string(), caster_info.position.x),
                            ("from_y".to_string(), caster_info.position.y),
                            ("to_x".to_string(), target_pos.x),
                            ("to_y".to_string(), target_pos.y),
                        ]
                        .into_iter()
                        .collect(),
                    },
                    server_timestamp: crate::utils::current_timestamp_ms(),
                    failure_reason: None,
                });
            }
        }

        Ok(SkillResultMessage {
            caster_id,
            skill_id: skill_message.skill_id,
            skill_type: skill_message.skill_type.clone(),
            success: false,
            affected_targets: vec![],
            effect_data: SkillEffectData::default(),
            server_timestamp: crate::utils::current_timestamp_ms(),
            failure_reason: Some("Invalid teleport position or out of range".to_string()),
        })
    }

    /// 방어막 스킬 처리
    async fn handle_shield_skill(
        &self,
        _room_id: u16,
        caster_id: PlayerId,
        _caster_info: &RoomUserInfo,
        skill_def: &SkillDefinition,
        skill_message: &UseSkillMessage,
    ) -> Result<SkillResultMessage> {
        // 방어막 스킬 로직 (예시)
        let shield_amount = skill_def.base_healing; // 방어막 양을 base_healing으로 사용
        let duration = 30.0; // 30초 지속

        // *** 방어막 적용 로직 (주석 처리 - 개발 시 활성화) ***
        // self.apply_shield(room_id, caster_id, shield_amount, duration).await?;

        debug!(
            caster_id = %caster_id,
            shield_amount = %shield_amount,
            duration = %duration,
            "방어막 스킬 적용 (방어막 로직 주석 처리됨)"
        );

        Ok(SkillResultMessage {
            caster_id,
            skill_id: skill_message.skill_id,
            skill_type: skill_message.skill_type.clone(),
            success: true,
            affected_targets: vec![caster_id],
            effect_data: SkillEffectData {
                damage: 0,
                healing: shield_amount,
                duration,
                area_of_effect: None,
                status_effects: vec!["shield".to_string()],
                custom_data: HashMap::new(),
            },
            server_timestamp: crate::utils::current_timestamp_ms(),
            failure_reason: None,
        })
    }

    /// 범위 공격 스킬 처리
    async fn handle_area_damage_skill(
        &self,
        room_id: u16,
        caster_id: PlayerId,
        _caster_info: &RoomUserInfo,
        skill_def: &SkillDefinition,
        skill_message: &UseSkillMessage,
    ) -> Result<SkillResultMessage> {
        let center_pos = skill_message
            .target_position
            .unwrap_or(_caster_info.position);
        let aoe_radius = skill_def.area_of_effect.unwrap_or(200.0);

        // 범위 내 모든 플레이어 검색
        let room_users = self.room_user_manager.get_room_users(room_id).await?;
        let mut affected_targets = Vec::new();

        for user in &room_users {
            if user.player_id == caster_id {
                continue; // 자신은 제외
            }

            let distance = calculate_distance(&center_pos, &user.position);
            if distance <= aoe_radius {
                affected_targets.push(user.player_id);

                let damage = self.calculate_damage(skill_def, _caster_info.level);

                // *** 체력 감소 로직 (주석 처리 - 개발 시 활성화) ***
                // self.apply_damage(room_id, user.player_id, damage).await?;

                debug!(
                    target_id = %user.player_id,
                    damage = %damage,
                    distance = %distance,
                    "범위 공격 스킬 적용 (체력 로직 주석 처리됨)"
                );
            }
        }

        Ok(SkillResultMessage {
            caster_id,
            skill_id: skill_message.skill_id,
            skill_type: skill_message.skill_type.clone(),
            success: !affected_targets.is_empty(),
            affected_targets: affected_targets.clone(),
            effect_data: SkillEffectData {
                damage: self.calculate_damage(skill_def, _caster_info.level),
                healing: 0,
                duration: 0.0,
                area_of_effect: Some(aoe_radius),
                status_effects: vec!["area_damage".to_string()],
                custom_data: [
                    ("center_x".to_string(), center_pos.x),
                    ("center_y".to_string(), center_pos.y),
                ]
                .into_iter()
                .collect(),
            },
            server_timestamp: crate::utils::current_timestamp_ms(),
            failure_reason: None,
        })
    }

    /// 버프 스킬 처리
    async fn handle_buff_skill(
        &self,
        _room_id: u16,
        caster_id: PlayerId,
        _caster_info: &RoomUserInfo,
        _skill_def: &SkillDefinition,
        skill_message: &UseSkillMessage,
        buff_type: &BuffType,
    ) -> Result<SkillResultMessage> {
        let target_id = skill_message.target_player.unwrap_or(caster_id);
        let duration = 60.0; // 60초 지속

        let (buff_name, effect_value) = match buff_type {
            BuffType::AttackBoost => ("attack_boost", 20.0), // 공격력 20% 증가
            BuffType::SpeedBoost => ("speed_boost", 30.0),   // 이동속도 30% 증가
            BuffType::DefenseBoost => ("defense_boost", 25.0), // 방어력 25% 증가
            BuffType::HealthRegeneration => ("health_regen", 5.0), // 초당 5 HP 회복
            BuffType::ManaRegeneration => ("mana_regen", 3.0), // 초당 3 MP 회복
        };

        // *** 버프 적용 로직 (주석 처리 - 개발 시 활성화) ***
        // self.apply_buff(room_id, target_id, buff_name, effect_value, duration).await?;

        debug!(
            target_id = %target_id,
            buff_type = ?buff_type,
            effect_value = %effect_value,
            duration = %duration,
            "버프 스킬 적용 (버프 로직 주석 처리됨)"
        );

        Ok(SkillResultMessage {
            caster_id,
            skill_id: skill_message.skill_id,
            skill_type: skill_message.skill_type.clone(),
            success: true,
            affected_targets: vec![target_id],
            effect_data: SkillEffectData {
                damage: 0,
                healing: 0,
                duration,
                area_of_effect: None,
                status_effects: vec![buff_name.to_string()],
                custom_data: [("effect_value".to_string(), effect_value)]
                    .into_iter()
                    .collect(),
            },
            server_timestamp: crate::utils::current_timestamp_ms(),
            failure_reason: None,
        })
    }

    /// 디버프 스킬 처리
    async fn handle_debuff_skill(
        &self,
        _room_id: u16,
        caster_id: PlayerId,
        _caster_info: &RoomUserInfo,
        _skill_def: &SkillDefinition,
        skill_message: &UseSkillMessage,
        debuff_type: &DebuffType,
    ) -> Result<SkillResultMessage> {
        if let Some(target_id) = skill_message.target_player {
            let duration = 20.0; // 20초 지속

            let (debuff_name, effect_value) = match debuff_type {
                DebuffType::Poison => ("poison", 3.0),   // 초당 3 데미지
                DebuffType::Slow => ("slow", -50.0),     // 이동속도 50% 감소
                DebuffType::Silence => ("silence", 0.0), // 스킬 사용 불가
                DebuffType::Stun => ("stun", 0.0),       // 행동 불가
                DebuffType::Blind => ("blind", -70.0),   // 시야 70% 감소
            };

            // *** 디버프 적용 로직 (주석 처리 - 개발 시 활성화) ***
            // self.apply_debuff(room_id, target_id, debuff_name, effect_value, duration).await?;

            debug!(
                target_id = %target_id,
                debuff_type = ?debuff_type,
                effect_value = %effect_value,
                duration = %duration,
                "디버프 스킬 적용 (디버프 로직 주석 처리됨)"
            );

            return Ok(SkillResultMessage {
                caster_id,
                skill_id: skill_message.skill_id,
                skill_type: skill_message.skill_type.clone(),
                success: true,
                affected_targets: vec![target_id],
                effect_data: SkillEffectData {
                    damage: 0,
                    healing: 0,
                    duration,
                    area_of_effect: None,
                    status_effects: vec![debuff_name.to_string()],
                    custom_data: [("effect_value".to_string(), effect_value)]
                        .into_iter()
                        .collect(),
                },
                server_timestamp: crate::utils::current_timestamp_ms(),
                failure_reason: None,
            });
        }

        Ok(SkillResultMessage {
            caster_id,
            skill_id: skill_message.skill_id,
            skill_type: skill_message.skill_type.clone(),
            success: false,
            affected_targets: vec![],
            effect_data: SkillEffectData::default(),
            server_timestamp: crate::utils::current_timestamp_ms(),
            failure_reason: Some("No target specified".to_string()),
        })
    }

    /// 소환 스킬 처리
    async fn handle_summon_skill(
        &self,
        _room_id: u16,
        caster_id: PlayerId,
        caster_info: &RoomUserInfo,
        _skill_def: &SkillDefinition,
        skill_message: &UseSkillMessage,
        creature_type: &str,
    ) -> Result<SkillResultMessage> {
        let summon_pos = skill_message
            .target_position
            .unwrap_or(caster_info.position);

        // *** 소환 로직 (주석 처리 - 개발 시 활성화) ***
        // let creature_id = self.spawn_creature(room_id, creature_type, summon_pos, caster_id).await?;

        debug!(
            caster_id = %caster_id,
            creature_type = %creature_type,
            summon_pos = ?(summon_pos.x, summon_pos.y),
            "소환 스킬 적용 (소환 로직 주석 처리됨)"
        );

        Ok(SkillResultMessage {
            caster_id,
            skill_id: skill_message.skill_id,
            skill_type: skill_message.skill_type.clone(),
            success: true,
            affected_targets: vec![caster_id],
            effect_data: SkillEffectData {
                damage: 0,
                healing: 0,
                duration: 300.0, // 5분 지속
                area_of_effect: None,
                status_effects: vec![format!("summoned_{}", creature_type)],
                custom_data: [
                    ("creature_type".to_string(), creature_type.len() as f32),
                    ("summon_x".to_string(), summon_pos.x),
                    ("summon_y".to_string(), summon_pos.y),
                ]
                .into_iter()
                .collect(),
            },
            server_timestamp: crate::utils::current_timestamp_ms(),
            failure_reason: None,
        })
    }

    /// 변신 스킬 처리
    async fn handle_transform_skill(
        &self,
        _room_id: u16,
        caster_id: PlayerId,
        _caster_info: &RoomUserInfo,
        _skill_def: &SkillDefinition,
        skill_message: &UseSkillMessage,
        form: &str,
    ) -> Result<SkillResultMessage> {
        let duration = 180.0; // 3분 지속

        // *** 변신 로직 (주석 처리 - 개발 시 활성화) ***
        // self.apply_transformation(room_id, caster_id, form, duration).await?;

        debug!(
            caster_id = %caster_id,
            form = %form,
            duration = %duration,
            "변신 스킬 적용 (변신 로직 주석 처리됨)"
        );

        Ok(SkillResultMessage {
            caster_id,
            skill_id: skill_message.skill_id,
            skill_type: skill_message.skill_type.clone(),
            success: true,
            affected_targets: vec![caster_id],
            effect_data: SkillEffectData {
                damage: 0,
                healing: 0,
                duration,
                area_of_effect: None,
                status_effects: vec![format!("transform_{}", form)],
                custom_data: [("form".to_string(), form.len() as f32)]
                    .into_iter()
                    .collect(),
            },
            server_timestamp: crate::utils::current_timestamp_ms(),
            failure_reason: None,
        })
    }

    /// 사용자 정의 스킬 처리
    async fn handle_custom_skill(
        &self,
        _room_id: u16,
        caster_id: PlayerId,
        _caster_info: &RoomUserInfo,
        _skill_def: &SkillDefinition,
        skill_message: &UseSkillMessage,
        skill_name: &str,
        parameters: &HashMap<String, String>,
    ) -> Result<SkillResultMessage> {
        // 사용자 정의 스킬은 추가 구현 필요
        warn!(
            caster_id = %caster_id,
            skill_name = %skill_name,
            parameter_count = %parameters.len(),
            "사용자 정의 스킬 처리 - 추가 구현 필요"
        );

        Ok(SkillResultMessage {
            caster_id,
            skill_id: skill_message.skill_id,
            skill_type: skill_message.skill_type.clone(),
            success: false,
            affected_targets: vec![],
            effect_data: SkillEffectData::default(),
            server_timestamp: crate::utils::current_timestamp_ms(),
            failure_reason: Some("Custom skill not implemented".to_string()),
        })
    }

    /// 리소스 소모 (마나, 쿨다운)
    async fn consume_resources(
        &self,
        room_id: u16,
        player_id: PlayerId,
        skill_def: &SkillDefinition,
    ) -> Result<()> {
        // *** 마나 소모 및 쿨다운 적용 로직 (주석 처리 - 개발 시 활성화) ***
        /*
        self.room_user_manager.update_user_in_room(room_id, player_id, |user| {
            // 마나 소모
            if let Some(current_mana) = user.game_data.get("current_mana").and_then(|s| s.parse::<u32>().ok()) {
                let new_mana = current_mana.saturating_sub(skill_def.mana_cost);
                user.game_data.insert("current_mana".to_string(), new_mana.to_string());
            }

            // 쿨다운 설정
            let cooldown_end = crate::utils::current_timestamp_ms() + skill_def.cooldown_ms;
            user.game_data.insert(format!("cooldown_{}", skill_def.skill_id), cooldown_end.to_string());
        }).await?;
        */

        debug!(
            player_id = %player_id,
            skill_id = %skill_def.skill_id,
            mana_cost = %skill_def.mana_cost,
            cooldown_ms = %skill_def.cooldown_ms,
            "리소스 소모 (마나/쿨다운 로직 주석 처리됨)"
        );

        Ok(())
    }

    // 헬퍼 메서드들

    /// 데미지 계산
    fn calculate_damage(&self, skill_def: &SkillDefinition, caster_level: u32) -> u32 {
        let level_multiplier = 1.0 + (caster_level - 1) as f32 * skill_def.level_scaling * 0.1;
        (skill_def.base_damage as f32 * level_multiplier) as u32
    }

    /// 치유량 계산
    fn calculate_healing(&self, skill_def: &SkillDefinition, caster_level: u32) -> u32 {
        let level_multiplier = 1.0 + (caster_level - 1) as f32 * skill_def.level_scaling * 0.1;
        (skill_def.base_healing as f32 * level_multiplier) as u32
    }
}

impl Default for SkillEffectData {
    fn default() -> Self {
        Self {
            damage: 0,
            healing: 0,
            duration: 0.0,
            area_of_effect: None,
            status_effects: vec![],
            custom_data: HashMap::new(),
        }
    }
}

/// 거리 계산 헬퍼 함수
fn calculate_distance(pos1: &Position, pos2: &Position) -> f32 {
    let dx = pos1.x - pos2.x;
    let dy = pos1.y - pos2.y;
    (dx * dx + dy * dy).sqrt()
}

// GameMessage에 추가할 메시지 타입들 (messages.rs에 추가)
//
// 새로운 기능 추가 시 이 패턴을 따르세요:
// GameMessage 열거형에 추가
// UseSkill {
//     skill_message: UseSkillMessage,
// },
//
// SkillResult {
//     result: SkillResultMessage,
// },

// GameStateManager에서 스킬 메시지 처리 방법 (state_manager.rs에 추가)
//
// handle_game_message 함수의 match 문에 추가
// GameMessage::UseSkill { skill_message } => {
//     let skill_system = SkillSystem::new(self.room_user_manager.clone());
//     let room_id = self.get_player_room(session_id).await?;
//     let player_id = self.get_player_id_from_session(session_id).await?;
//
//     let result = skill_system.handle_skill_use(room_id, player_id, skill_message).await?;
//     Ok(Some(GameMessage::SkillResult { result }))
// }
