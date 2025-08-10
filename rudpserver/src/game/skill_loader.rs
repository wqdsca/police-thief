//! JSON 기반 스킬 시스템 로더
//!
//! 외부 JSON 파일에서 스킬 정의를 로드하고 관리하는 모듈입니다.
//! TDD 방식으로 개발되어 높은 신뢰성과 유지보수성을 보장합니다.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{error, info, warn};

use super::sample_example::{BuffType, DebuffType, SkillDefinition, SkillType};

/// JSON 스킬 아키텍처 루트 구조체
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsArchitecture {
    pub rudp_skills_architecture: RudpSkillsArchitecture,
}

/// RUDP 스킬 시스템 아키텍처
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RudpSkillsArchitecture {
    pub metadata: SkillMetadata,
    pub skill_types: HashMap<String, SkillTypeDefinition>,
    pub buff_types: HashMap<String, BuffTypeDefinition>,
    pub debuff_types: HashMap<String, DebuffTypeDefinition>,
    pub example_skills: HashMap<String, ExampleSkillDefinition>,
    pub skill_system_architecture: SkillSystemArchitecture,
    pub configuration: SkillConfiguration,
    pub error_handling: ErrorHandling,
}

/// 스킬 메타데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub version: String,
    pub description: String,
    pub last_updated: String,
    pub total_skill_types: u32,
    pub total_buff_types: u32,
    pub total_debuff_types: u32,
    pub server_type: String,
}

/// 스킬 타입 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTypeDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub requires_target: serde_json::Value, // bool 또는 "varies"
    pub can_be_aoe: serde_json::Value,      // bool 또는 "varies"
    pub typical_properties: Vec<String>,
}

/// 버프 타입 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuffTypeDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub effect_type: String,
    pub target_stat: String,
    pub stacks: bool,
    pub typical_duration_seconds: u32,
    #[serde(default)]
    pub typical_boost_percentage: Option<u32>,
    #[serde(default)]
    pub typical_heal_per_second: Option<u32>,
    #[serde(default)]
    pub typical_restore_per_second: Option<u32>,
}

/// 디버프 타입 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebuffTypeDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub effect_type: String,
    pub target_stat: String,
    pub stacks: bool,
    pub typical_duration_seconds: u32,
    #[serde(default)]
    pub typical_damage_per_second: Option<u32>,
    #[serde(default)]
    pub typical_reduction_percentage: Option<u32>,
    #[serde(default)]
    pub typical_vision_reduction_percentage: Option<u32>,
    #[serde(default)]
    pub blocks_all_skills: Option<bool>,
    #[serde(default)]
    pub blocks_movement: Option<bool>,
    #[serde(default)]
    pub blocks_skills: Option<bool>,
    #[serde(default)]
    pub blocks_attacks: Option<bool>,
}

/// 예제 스킬 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleSkillDefinition {
    pub skill_id: u32,
    pub name: String,
    pub korean_name: String,
    pub skill_type: String,
    pub description: String,
    pub properties: SkillProperties,
    pub visual_effects: VisualEffects,
}

/// 스킬 속성
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillProperties {
    pub mana_cost: u32,
    pub cooldown_ms: u32,
    pub cast_time_ms: u32,
    pub range: f32,
    pub area_of_effect: Option<f32>,
    pub base_damage: u32,
    pub base_healing: u32,
    pub level_scaling: f32,
}

/// 시각 효과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualEffects {
    #[serde(default)]
    pub cast_animation: Option<String>,
    #[serde(default)]
    pub projectile_effect: Option<String>,
    #[serde(default)]
    pub impact_effect: Option<String>,
    #[serde(default)]
    pub healing_effect: Option<String>,
    #[serde(default)]
    pub completion_effect: Option<String>,
    #[serde(default)]
    pub disappear_effect: Option<String>,
    #[serde(default)]
    pub appear_effect: Option<String>,
}

/// 스킬 시스템 아키텍처
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSystemArchitecture {
    pub core_components: HashMap<String, ComponentDefinition>,
    pub skill_execution_flow: Vec<String>,
    pub performance_optimizations: Vec<String>,
    pub redis_integration: RedisIntegration,
}

/// 컴포넌트 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDefinition {
    pub description: String,
    pub fields: Vec<String>,
}

/// Redis 통합 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisIntegration {
    pub description: String,
    pub key_patterns: HashMap<String, String>,
    pub operations: Vec<String>,
}

/// 스킬 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfiguration {
    pub default_values: DefaultValues,
    pub balance_parameters: BalanceParameters,
}

/// 기본값 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultValues {
    pub global_cooldown_ms: u32,
    pub max_status_effects_per_player: u32,
    pub status_effect_tick_interval_ms: u32,
    pub skill_range_check_precision: f32,
    pub level_scaling_cap: f32,
}

/// 밸런스 파라미터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceParameters {
    pub damage_scaling: HashMap<String, f32>,
    pub mana_cost_scaling: HashMap<String, f32>,
    pub cooldown_reduction: CooldownReduction,
}

/// 쿨다운 감소 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CooldownReduction {
    pub min_cooldown_ms: u32,
    pub max_reduction_percentage: u32,
}

/// 에러 처리 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandling {
    pub error_types: Vec<String>,
    pub validation_checks: Vec<String>,
}

/// 스킬 로더 - JSON 파일에서 스킬 데이터를 로드하고 변환
pub struct SkillLoader {
    architecture: Option<SkillsArchitecture>,
    skill_definitions: HashMap<u32, SkillDefinition>,
    config: SkillConfiguration,
}

impl SkillLoader {
    /// 새로운 스킬 로더 생성
    pub fn new() -> Self {
        Self {
            architecture: None,
            skill_definitions: HashMap::new(),
            config: Self::default_config(),
        }
    }

    /// 기본 설정 반환
    fn default_config() -> SkillConfiguration {
        SkillConfiguration {
            default_values: DefaultValues {
                global_cooldown_ms: 1000,
                max_status_effects_per_player: 20,
                status_effect_tick_interval_ms: 1000,
                skill_range_check_precision: 0.1,
                level_scaling_cap: 5.0,
            },
            balance_parameters: BalanceParameters {
                damage_scaling: HashMap::from([
                    ("basic_attack".to_string(), 1.0),
                    ("area_damage".to_string(), 0.8),
                    ("magic_attack".to_string(), 1.2),
                ]),
                mana_cost_scaling: HashMap::from([
                    ("low_level".to_string(), 1.0),
                    ("mid_level".to_string(), 1.5),
                    ("high_level".to_string(), 2.0),
                ]),
                cooldown_reduction: CooldownReduction {
                    min_cooldown_ms: 500,
                    max_reduction_percentage: 80,
                },
            },
        }
    }

    /// JSON 파일에서 스킬 아키텍처 로드
    pub async fn load_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        info!("📖 스킬 아키텍처 로드 중: {}", path.display());

        let json_content = fs::read_to_string(path).context("JSON 파일 읽기 실패")?;

        let architecture: SkillsArchitecture =
            serde_json::from_str(&json_content).context("JSON 파싱 실패")?;

        // 메타데이터 검증
        self.validate_metadata(&architecture.rudp_skills_architecture.metadata)?;

        // 설정 업데이트
        self.config = architecture.rudp_skills_architecture.configuration.clone();

        // 예제 스킬들을 실제 스킬 정의로 변환
        self.convert_example_skills(&architecture.rudp_skills_architecture.example_skills)?;

        self.architecture = Some(architecture);

        info!(
            "✅ 스킬 아키텍처 로드 완료: {} 스킬 타입, {} 버프, {} 디버프, {} 예제 스킬",
            self.architecture
                .as_ref()
                .unwrap()
                .rudp_skills_architecture
                .skill_types
                .len(),
            self.architecture
                .as_ref()
                .unwrap()
                .rudp_skills_architecture
                .buff_types
                .len(),
            self.architecture
                .as_ref()
                .unwrap()
                .rudp_skills_architecture
                .debuff_types
                .len(),
            self.skill_definitions.len()
        );

        Ok(())
    }

    /// 메타데이터 검증
    fn validate_metadata(&self, metadata: &SkillMetadata) -> Result<()> {
        if metadata.version.is_empty() {
            return Err(anyhow::anyhow!("버전 정보가 없습니다"));
        }

        if metadata.total_skill_types == 0 {
            return Err(anyhow::anyhow!("스킬 타입이 정의되지 않았습니다"));
        }

        info!("📋 스킬 시스템 버전: {}", metadata.version);
        info!("📝 설명: {}", metadata.description);

        Ok(())
    }

    /// 예제 스킬을 실제 스킬 정의로 변환
    fn convert_example_skills(
        &mut self,
        example_skills: &HashMap<String, ExampleSkillDefinition>,
    ) -> Result<()> {
        for (skill_key, example) in example_skills {
            let skill_type = self.parse_skill_type(&example.skill_type)?;

            let skill_def = SkillDefinition {
                skill_id: example.skill_id,
                name: example.korean_name.clone(), // 한국어 이름 사용
                skill_type,
                mana_cost: example.properties.mana_cost,
                cooldown_ms: example.properties.cooldown_ms as u64,
                cast_time_ms: example.properties.cast_time_ms as u64,
                range: example.properties.range,
                area_of_effect: example.properties.area_of_effect,
                base_damage: example.properties.base_damage,
                base_healing: example.properties.base_healing,
                level_scaling: example.properties.level_scaling,
            };

            self.skill_definitions.insert(example.skill_id, skill_def);
            info!(
                "  ➕ 스킬 로드: [{}] {} (ID: {})",
                skill_key, example.korean_name, example.skill_id
            );
        }

        Ok(())
    }

    /// 문자열을 스킬 타입으로 파싱
    fn parse_skill_type(&self, type_str: &str) -> Result<SkillType> {
        match type_str {
            "BasicAttack" => Ok(SkillType::BasicAttack),
            "Heal" => Ok(SkillType::Heal),
            "Teleport" => Ok(SkillType::Teleport),
            "Shield" => Ok(SkillType::Shield),
            "AreaDamage" => Ok(SkillType::AreaDamage),
            "Buff" => Ok(SkillType::Buff {
                buff_type: BuffType::AttackBoost,
            }), // 기본값
            "Debuff" => Ok(SkillType::Debuff {
                debuff_type: DebuffType::Slow,
            }), // 기본값
            "Summon" => Ok(SkillType::Summon {
                creature_type: "default".to_string(),
            }), // 기본값
            "Transform" => Ok(SkillType::Transform {
                form: "default".to_string(),
            }), // 기본값
            "Custom" => Ok(SkillType::Custom {
                skill_name: "custom".to_string(),
                parameters: HashMap::new(),
            }),
            _ => Err(anyhow::anyhow!("알 수 없는 스킬 타입: {}", type_str)),
        }
    }

    /// 스킬 정의 가져오기
    pub fn get_skill(&self, skill_id: u32) -> Option<&SkillDefinition> {
        self.skill_definitions.get(&skill_id)
    }

    /// 모든 스킬 정의 가져오기
    pub fn get_all_skills(&self) -> &HashMap<u32, SkillDefinition> {
        &self.skill_definitions
    }

    /// 설정 가져오기
    pub fn get_config(&self) -> &SkillConfiguration {
        &self.config
    }

    /// 스킬 타입별로 스킬 필터링 (단순 타입만 비교)
    pub fn get_skills_by_type(&self, skill_type: &SkillType) -> Vec<&SkillDefinition> {
        self.skill_definitions
            .values()
            .filter(|skill| match (&skill.skill_type, skill_type) {
                (SkillType::BasicAttack, SkillType::BasicAttack) => true,
                (SkillType::Heal, SkillType::Heal) => true,
                (SkillType::Teleport, SkillType::Teleport) => true,
                (SkillType::Shield, SkillType::Shield) => true,
                (SkillType::AreaDamage, SkillType::AreaDamage) => true,
                (SkillType::Summon { .. }, SkillType::Summon { .. }) => true,
                (SkillType::Buff { .. }, SkillType::Buff { .. }) => true,
                (SkillType::Debuff { .. }, SkillType::Debuff { .. }) => true,
                (SkillType::Transform { .. }, SkillType::Transform { .. }) => true,
                (SkillType::Custom { .. }, SkillType::Custom { .. }) => true,
                _ => false,
            })
            .collect()
    }

    /// 범위 내 스킬 필터링
    pub fn get_skills_in_range(&self, max_range: f32) -> Vec<&SkillDefinition> {
        self.skill_definitions
            .values()
            .filter(|skill| skill.range <= max_range)
            .collect()
    }

    /// 마나 코스트 이하 스킬 필터링
    pub fn get_affordable_skills(&self, available_mana: u32) -> Vec<&SkillDefinition> {
        self.skill_definitions
            .values()
            .filter(|skill| skill.mana_cost <= available_mana)
            .collect()
    }

    /// 쿨다운 적용 계산
    pub fn calculate_cooldown(
        &self,
        base_cooldown_ms: u32,
        cooldown_reduction_percent: u32,
    ) -> u32 {
        let max_reduction = self
            .config
            .balance_parameters
            .cooldown_reduction
            .max_reduction_percentage;
        let actual_reduction = cooldown_reduction_percent.min(max_reduction);

        let reduced_cooldown = base_cooldown_ms * (100 - actual_reduction) / 100;
        reduced_cooldown.max(
            self.config
                .balance_parameters
                .cooldown_reduction
                .min_cooldown_ms,
        )
    }

    /// 데미지 스케일링 계산
    pub fn calculate_damage(&self, base_damage: u32, level: u32, level_scaling: f32) -> u32 {
        let capped_scaling = level_scaling.min(self.config.default_values.level_scaling_cap);
        let scaled_damage = base_damage as f32 * capped_scaling.powi(level as i32 - 1);
        scaled_damage as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_load_skill_architecture() {
        let mut loader = SkillLoader::new();

        // 실제 JSON 파일 로드 테스트
        let result = loader.load_from_file("skills_architecture.json").await;

        assert!(result.is_ok(), "JSON 파일 로드 실패: {:?}", result);
        assert!(!loader.get_all_skills().is_empty(), "스킬이 로드되지 않음");
    }

    #[tokio::test]
    async fn test_skill_conversion() {
        let mut loader = SkillLoader::new();

        // 테스트용 예제 스킬 생성
        let mut example_skills = HashMap::new();
        example_skills.insert(
            "test_skill".to_string(),
            ExampleSkillDefinition {
                skill_id: 999,
                name: "Test Skill".to_string(),
                korean_name: "테스트 스킬".to_string(),
                skill_type: "BasicAttack".to_string(),
                description: "테스트용 스킬".to_string(),
                properties: SkillProperties {
                    mana_cost: 10,
                    cooldown_ms: 1000,
                    cast_time_ms: 500,
                    range: 100.0,
                    area_of_effect: Some(50.0),
                    base_damage: 25,
                    base_healing: 0,
                    level_scaling: 1.1,
                },
                visual_effects: VisualEffects {
                    cast_animation: Some("test_cast".to_string()),
                    projectile_effect: None,
                    impact_effect: None,
                    healing_effect: None,
                    completion_effect: None,
                    disappear_effect: None,
                    appear_effect: None,
                },
            },
        );

        let result = loader.convert_example_skills(&example_skills);
        assert!(result.is_ok());

        let skill = loader.get_skill(999);
        assert!(skill.is_some());
        assert_eq!(skill.unwrap().name, "테스트 스킬");
        assert_eq!(skill.unwrap().mana_cost, 10);
    }

    #[test]
    fn test_cooldown_calculation() {
        let loader = SkillLoader::new();

        // 기본 쿨다운 3000ms, 50% 감소
        let cooldown = loader.calculate_cooldown(3000, 50);
        assert_eq!(cooldown, 1500);

        // 최대 감소율(80%) 초과 시도
        let cooldown = loader.calculate_cooldown(3000, 90);
        assert_eq!(cooldown, 600); // 80% 감소만 적용

        // 최소 쿨다운 보장
        let cooldown = loader.calculate_cooldown(600, 50);
        assert_eq!(cooldown, 500); // 최소 500ms
    }

    #[test]
    fn test_damage_calculation() {
        let loader = SkillLoader::new();

        // 레벨 1: 기본 데미지
        let damage = loader.calculate_damage(100, 1, 1.2);
        assert_eq!(damage, 100);

        // 레벨 2: 1.2배
        let damage = loader.calculate_damage(100, 2, 1.2);
        assert_eq!(damage, 120);

        // 레벨 3: 1.44배
        let damage = loader.calculate_damage(100, 3, 1.2);
        assert_eq!(damage, 144);
    }

    #[test]
    fn test_skill_filtering() {
        let mut loader = SkillLoader::new();

        // 테스트 스킬 추가
        loader.skill_definitions.insert(
            1,
            SkillDefinition {
                skill_id: 1,
                name: "근거리 공격".to_string(),
                skill_type: SkillType::BasicAttack,
                mana_cost: 10,
                cooldown_ms: 1000,
                cast_time_ms: 500,
                range: 50.0,
                area_of_effect: None,
                base_damage: 20,
                base_healing: 0,
                level_scaling: 1.1,
            },
        );

        loader.skill_definitions.insert(
            2,
            SkillDefinition {
                skill_id: 2,
                name: "원거리 공격".to_string(),
                skill_type: SkillType::BasicAttack,
                mana_cost: 20,
                cooldown_ms: 2000,
                cast_time_ms: 1000,
                range: 200.0,
                area_of_effect: None,
                base_damage: 30,
                base_healing: 0,
                level_scaling: 1.2,
            },
        );

        loader.skill_definitions.insert(
            3,
            SkillDefinition {
                skill_id: 3,
                name: "치유".to_string(),
                skill_type: SkillType::Heal,
                mana_cost: 15,
                cooldown_ms: 3000,
                cast_time_ms: 1500,
                range: 100.0,
                area_of_effect: Some(50.0),
                base_damage: 0,
                base_healing: 40,
                level_scaling: 1.15,
            },
        );

        // 타입별 필터링
        let attack_skills = loader.get_skills_by_type(&SkillType::BasicAttack);
        assert_eq!(attack_skills.len(), 2);

        // 범위별 필터링
        let short_range_skills = loader.get_skills_in_range(100.0);
        assert_eq!(short_range_skills.len(), 2); // 근거리 공격, 치유

        // 마나별 필터링
        let affordable_skills = loader.get_affordable_skills(15);
        assert_eq!(affordable_skills.len(), 2); // 근거리 공격, 치유
    }
}
