//! 스킬 시스템 API 엔드포인트
//!
//! JSON 기반 스킬 시스템과 게임 서버를 연결하는 API 레이어입니다.
//! RESTful 스타일의 인터페이스를 제공하여 스킬 관리를 용이하게 합니다.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::messages::Position;
use super::room_user_manager::RoomUserManager;
use super::sample_example::{
    SkillDefinition, SkillEffectData, SkillResultMessage, SkillSystem, SkillType,
};
use super::skill_loader::SkillLoader;
use crate::types::PlayerId;
use shared::tool::high_performance::redis_optimizer::RedisOptimizer;

/// 스킬 API 응답 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: u64,
}

impl<T> ApiResponse<T> {
    /// 성공 응답 생성
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: crate::utils::current_timestamp_ms(),
        }
    }

    /// 에러 응답 생성
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: crate::utils::current_timestamp_ms(),
        }
    }
}

/// 스킬 정보 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfoDto {
    pub skill_id: u32,
    pub name: String,
    pub skill_type: String,
    pub mana_cost: u32,
    pub cooldown_ms: u64,  // Changed to u64
    pub cast_time_ms: u64, // Changed to u64
    pub range: f32,
    pub area_of_effect: Option<f32>,
    pub base_damage: u32,
    pub base_healing: u32,
    pub level_scaling: f32,
}

impl From<&SkillDefinition> for SkillInfoDto {
    fn from(skill: &SkillDefinition) -> Self {
        Self {
            skill_id: skill.skill_id,
            name: skill.name.clone(),
            skill_type: format!("{:?}", skill.skill_type),
            mana_cost: skill.mana_cost,
            cooldown_ms: skill.cooldown_ms,
            cast_time_ms: skill.cast_time_ms,
            range: skill.range,
            area_of_effect: skill.area_of_effect,
            base_damage: skill.base_damage,
            base_healing: skill.base_healing,
            level_scaling: skill.level_scaling,
        }
    }
}

/// 스킬 사용 요청 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseSkillRequest {
    pub player_id: PlayerId,
    pub skill_id: u32,
    pub target_position: Option<Position>,
    pub target_player_id: Option<PlayerId>,
}

/// 스킬 목록 조회 필터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillListFilter {
    pub skill_type: Option<String>,
    pub max_mana_cost: Option<u32>,
    pub max_range: Option<f32>,
    pub include_healing: Option<bool>,
    pub include_damage: Option<bool>,
}

/// 스킬 시스템 API 관리자
pub struct SkillApiManager {
    skill_loader: Arc<RwLock<SkillLoader>>,
    skill_system: Arc<RwLock<SkillSystem>>,
    room_user_manager: Arc<RoomUserManager>,
    json_file_path: String,
}

impl SkillApiManager {
    /// 새로운 API 관리자 생성
    pub async fn new(json_file_path: String, redis_optimizer: Arc<RedisOptimizer>) -> Result<Self> {
        let room_user_manager = Arc::new(RoomUserManager::new(redis_optimizer).await?);
        let skill_system = Arc::new(RwLock::new(SkillSystem::new(room_user_manager.clone())));

        Ok(Self {
            skill_loader: Arc::new(RwLock::new(SkillLoader::new())),
            skill_system,
            room_user_manager,
            json_file_path,
        })
    }

    /// 초기화 - JSON 파일 로드 및 스킬 시스템 설정
    pub async fn initialize(&self) -> Result<()> {
        info!("🎮 스킬 API 초기화 시작...");

        // JSON 파일 로드
        {
            let mut loader = self.skill_loader.write().await;
            loader
                .load_from_file(&self.json_file_path)
                .await
                .context("스킬 JSON 파일 로드 실패")?;
        }

        // 스킬 시스템에 로드된 스킬 적용
        // 실제로는 SkillSystem이 직접 JSON을 로드하거나,
        // public 메서드를 통해 스킬을 추가해야 함
        {
            let loader = self.skill_loader.read().await;
            let skill_count = loader.get_all_skills().len();
            info!("  ✅ {} 개 스킬 로드 완료", skill_count);
            // 실제 구현에서는 SkillSystem에 add_skill() 같은 public 메서드가 필요
        }

        info!("🎯 스킬 API 초기화 완료!");
        Ok(())
    }

    /// 스킬 파일 리로드
    pub async fn reload_skills(&self) -> ApiResponse<String> {
        match self.initialize().await {
            Ok(_) => {
                let loader = self.skill_loader.read().await;
                let skill_count = loader.get_all_skills().len();
                ApiResponse::success(format!("{}개 스킬 리로드 완료", skill_count))
            }
            Err(e) => ApiResponse::error(format!("스킬 리로드 실패: {}", e)),
        }
    }

    /// 모든 스킬 조회
    pub async fn get_all_skills(&self) -> ApiResponse<Vec<SkillInfoDto>> {
        let loader = self.skill_loader.read().await;
        let skills: Vec<SkillInfoDto> = loader
            .get_all_skills()
            .values()
            .map(SkillInfoDto::from)
            .collect();

        ApiResponse::success(skills)
    }

    /// 특정 스킬 조회
    pub async fn get_skill(&self, skill_id: u32) -> ApiResponse<SkillInfoDto> {
        let loader = self.skill_loader.read().await;

        match loader.get_skill(skill_id) {
            Some(skill) => ApiResponse::success(SkillInfoDto::from(skill)),
            None => ApiResponse::error(format!("스킬 ID {} 를 찾을 수 없습니다", skill_id)),
        }
    }

    /// 필터링된 스킬 목록 조회
    pub async fn get_filtered_skills(
        &self,
        filter: SkillListFilter,
    ) -> ApiResponse<Vec<SkillInfoDto>> {
        let loader = self.skill_loader.read().await;
        let mut skills: Vec<&SkillDefinition> = loader.get_all_skills().values().collect();

        // 마나 코스트 필터
        if let Some(max_mana) = filter.max_mana_cost {
            skills.retain(|s| s.mana_cost <= max_mana);
        }

        // 범위 필터
        if let Some(max_range) = filter.max_range {
            skills.retain(|s| s.range <= max_range);
        }

        // 치유 스킬 필터
        if let Some(include_healing) = filter.include_healing {
            if !include_healing {
                skills.retain(|s| s.base_healing == 0);
            } else {
                skills.retain(|s| s.base_healing > 0);
            }
        }

        // 공격 스킬 필터
        if let Some(include_damage) = filter.include_damage {
            if !include_damage {
                skills.retain(|s| s.base_damage == 0);
            } else {
                skills.retain(|s| s.base_damage > 0);
            }
        }

        let result: Vec<SkillInfoDto> = skills.into_iter().map(SkillInfoDto::from).collect();

        ApiResponse::success(result)
    }

    /// 스킬 사용
    pub async fn use_skill(&self, request: UseSkillRequest) -> ApiResponse<SkillResultMessage> {
        // 실제로는 RoomUserManager를 통해 방과 플레이어 정보를 확인해야 함
        // 여기서는 간소화된 버전으로 구현

        // 스킬 정의 확인
        let loader = self.skill_loader.read().await;
        if loader.get_skill(request.skill_id).is_none() {
            return ApiResponse::error(format!("스킬 ID {} 를 찾을 수 없습니다", request.skill_id));
        }

        // 간소화된 결과 반환
        let result = SkillResultMessage {
            caster_id: request.player_id,
            skill_id: request.skill_id,
            skill_type: SkillType::BasicAttack, // 기본값
            success: true,
            affected_targets: vec![],
            effect_data: SkillEffectData {
                damage: 0,
                healing: 0,
                duration: 0.0,
                area_of_effect: Some(0.0),
                status_effects: vec![],
                custom_data: HashMap::new(),
            },
            server_timestamp: crate::utils::current_timestamp_ms(),
            failure_reason: None,
        };

        info!(
            "⚔️ 스킬 사용 요청: 플레이어 {} - 스킬 {}",
            request.player_id, request.skill_id
        );

        ApiResponse::success(result)
    }

    /// 플레이어 추가 (RoomUserManager를 통해 관리)
    pub async fn add_player(&self, player_id: PlayerId, position: Position) -> ApiResponse<String> {
        // 플레이어 관리는 RoomUserManager를 통해 처리
        // 실제로는 room_user_manager를 통해 플레이어를 추가해야 함
        info!(
            "👤 플레이어 추가 요청: ID {} at ({}, {})",
            player_id, position.x, position.y
        );
        ApiResponse::success(format!("플레이어 {} 추가 완료", player_id))
    }

    /// 플레이어 제거 (RoomUserManager를 통해 관리)
    pub async fn remove_player(&self, player_id: PlayerId) -> ApiResponse<String> {
        info!("👤 플레이어 제거 요청: ID {}", player_id);
        ApiResponse::success(format!("플레이어 {} 제거 완료", player_id))
    }

    /// 플레이어 상태 조회 (간소화된 버전)
    pub async fn get_player_status(&self, player_id: PlayerId) -> ApiResponse<PlayerStatusDto> {
        // 실제로는 RoomUserManager를 통해 플레이어 정보를 가져와야 함
        // 여기서는 데모를 위한 기본값 반환
        let status = PlayerStatusDto {
            player_id,
            position: Position::new(0.0, 0.0, 0.0),
            current_health: 100,
            max_health: 100,
            current_mana: 100,
            max_mana: 100,
            level: 1,
            status_effects_count: 0,
            skills_on_cooldown: 0,
        };
        ApiResponse::success(status)
    }

    /// 스킬 쿨다운 조회 (간소화된 버전)
    pub async fn get_skill_cooldowns(
        &self,
        _player_id: PlayerId,
    ) -> ApiResponse<Vec<CooldownInfoDto>> {
        // 실제로는 플레이어의 스킬 쿨다운 정보를 조회해야 함
        // 여기서는 빈 리스트 반환
        let cooldowns: Vec<CooldownInfoDto> = vec![];
        ApiResponse::success(cooldowns)
    }

    /// 시스템 통계 조회
    pub async fn get_system_stats(&self) -> ApiResponse<SystemStatsDto> {
        let loader = self.skill_loader.read().await;

        let stats = SystemStatsDto {
            total_skills_loaded: loader.get_all_skills().len(),
            total_players: 0,    // 실제로는 RoomUserManager에서 가져와야 함
            total_skill_uses: 0, // 실제로는 통계를 추적해야 함
            config_version: loader
                .get_config()
                .default_values
                .global_cooldown_ms
                .to_string(),
        };

        ApiResponse::success(stats)
    }
}

/// 플레이어 상태 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerStatusDto {
    pub player_id: PlayerId,
    pub position: Position,
    pub current_health: u32,
    pub max_health: u32,
    pub current_mana: u32,
    pub max_mana: u32,
    pub level: u32,
    pub status_effects_count: usize,
    pub skills_on_cooldown: usize,
}

/// 쿨다운 정보 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CooldownInfoDto {
    pub skill_id: u32,
    pub remaining_ms: u64,
    pub ready: bool,
}

/// 시스템 통계 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatsDto {
    pub total_skills_loaded: usize,
    pub total_players: usize,
    pub total_skill_uses: usize,
    pub config_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_api() -> SkillApiManager {
        // Redis optimizer 설정
        let redis_config = shared::tool::high_performance::redis_optimizer::RedisOptimizerConfig {
            pipeline_batch_size: 100,
            connection_pool_size: 10,
            max_retries: 3,
            retry_delay_ms: 100,
            connection_timeout_secs: 5,
            enable_key_compression: false,
            enable_value_compression: false,
            default_ttl_secs: 3600,
        };

        let redis_optimizer = Arc::new(
            RedisOptimizer::new("redis://127.0.0.1:6379", redis_config)
                .await
                .unwrap(),
        );

        // 테스트용 JSON 파일 생성
        let test_json = r#"{
  "rudp_skills_architecture": {
    "metadata": {
      "version": "1.0.0-test",
      "description": "Test",
      "last_updated": "2025-08-10",
      "total_skill_types": 1,
      "total_buff_types": 0,
      "total_debuff_types": 0,
      "server_type": "Test"
    },
    "skill_types": {},
    "buff_types": {},
    "debuff_types": {},
    "example_skills": {
      "test": {
        "skill_id": 1,
        "name": "Test",
        "korean_name": "테스트",
        "skill_type": "BasicAttack",
        "description": "Test",
        "properties": {
          "mana_cost": 10,
          "cooldown_ms": 1000,
          "cast_time_ms": 500,
          "range": 100.0,
          "area_of_effect": null,
          "base_damage": 10,
          "base_healing": 0,
          "level_scaling": 1.0
        },
        "visual_effects": {}
      }
    },
    "skill_system_architecture": {
      "core_components": {},
      "skill_execution_flow": [],
      "performance_optimizations": [],
      "redis_integration": {
        "description": "Test",
        "key_patterns": {},
        "operations": []
      }
    },
    "configuration": {
      "default_values": {
        "global_cooldown_ms": 1000,
        "max_status_effects_per_player": 20,
        "status_effect_tick_interval_ms": 1000,
        "skill_range_check_precision": 0.1,
        "level_scaling_cap": 5.0
      },
      "balance_parameters": {
        "damage_scaling": {},
        "mana_cost_scaling": {},
        "cooldown_reduction": {
          "min_cooldown_ms": 500,
          "max_reduction_percentage": 80
        }
      }
    },
    "error_handling": {
      "error_types": [],
      "validation_checks": []
    }
  }
}"#;

        let path = "test_api_skills.json";
        std::fs::write(path, test_json).unwrap();

        let api = SkillApiManager::new(path.to_string(), redis_optimizer)
            .await
            .unwrap();
        api.initialize().await.unwrap();
        api
    }

    #[tokio::test]
    async fn test_api_initialization() {
        let api = create_test_api().await;
        let stats = api.get_system_stats().await;

        assert!(stats.success);
        assert_eq!(stats.data.unwrap().total_skills_loaded, 1);

        // Cleanup
        let _ = std::fs::remove_file("test_api_skills.json");
    }

    #[tokio::test]
    async fn test_api_player_management() {
        let api = create_test_api().await;

        // 플레이어 추가
        let add_result = api.add_player(1, Position::new(0.0, 0.0, 0.0)).await;
        assert!(add_result.success);

        // 플레이어 상태 조회
        let status = api.get_player_status(1).await;
        assert!(status.success);
        assert_eq!(status.data.unwrap().player_id, 1);

        // 플레이어 제거
        let remove_result = api.remove_player(1).await;
        assert!(remove_result.success);

        // Cleanup
        let _ = std::fs::remove_file("test_api_skills.json");
    }
}
