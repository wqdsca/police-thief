//! 스킬 시스템 통합 테스트
//!
//! JSON 파일 로딩부터 실제 스킬 사용까지의 전체 플로우를 테스트합니다.
//! TDD 방식으로 개발되어 모든 시나리오에 대한 검증을 수행합니다.

use rudpserver::game::{
    messages::Position,
    sample_example::{
        BuffType, DebuffType, SkillResultMessage, SkillSystem, SkillType, UseSkillMessage,
    },
    skill_loader::SkillLoader,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 테스트용 JSON 파일 생성
async fn create_test_json_file() -> String {
    let test_json = r#"{
  "rudp_skills_architecture": {
    "metadata": {
      "version": "1.0.0-test",
      "description": "Test skill architecture",
      "last_updated": "2025-08-10",
      "total_skill_types": 3,
      "total_buff_types": 2,
      "total_debuff_types": 2,
      "server_type": "Test Server"
    },
    "skill_types": {
      "BasicAttack": {
        "id": "basic_attack",
        "name": "기본 공격",
        "description": "테스트 공격",
        "category": "공격",
        "requires_target": true,
        "can_be_aoe": false,
        "typical_properties": ["damage"]
      }
    },
    "buff_types": {},
    "debuff_types": {},
    "example_skills": {
      "test_fireball": {
        "skill_id": 100,
        "name": "Test Fireball",
        "korean_name": "테스트 파이어볼",
        "skill_type": "BasicAttack",
        "description": "테스트용 파이어볼",
        "properties": {
          "mana_cost": 30,
          "cooldown_ms": 3000,
          "cast_time_ms": 1000,
          "range": 500.0,
          "area_of_effect": 100.0,
          "base_damage": 50,
          "base_healing": 0,
          "level_scaling": 1.2
        },
        "visual_effects": {
          "cast_animation": "fire_cast",
          "projectile_effect": "fireball_projectile",
          "impact_effect": "fire_explosion"
        }
      },
      "test_heal": {
        "skill_id": 101,
        "name": "Test Heal",
        "korean_name": "테스트 치유",
        "skill_type": "Heal",
        "description": "테스트용 치유 스킬",
        "properties": {
          "mana_cost": 25,
          "cooldown_ms": 2000,
          "cast_time_ms": 800,
          "range": 300.0,
          "area_of_effect": 50.0,
          "base_damage": 0,
          "base_healing": 40,
          "level_scaling": 1.1
        },
        "visual_effects": {
          "healing_effect": "golden_light"
        }
      }
    },
    "skill_system_architecture": {
      "core_components": {},
      "skill_execution_flow": [],
      "performance_optimizations": [],
      "redis_integration": {
        "description": "Test Redis integration",
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
        "damage_scaling": {
          "basic_attack": 1.0
        },
        "mana_cost_scaling": {
          "low_level": 1.0
        },
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

    let path = "test_skills.json";
    std::fs::write(path, test_json).expect("테스트 JSON 파일 생성 실패");
    path.to_string()
}

/// 테스트용 JSON 파일 삭제
fn cleanup_test_file(path: &str) {
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn test_full_skill_flow() {
    // 1. 테스트 JSON 파일 생성
    let test_file = create_test_json_file().await;

    // 2. 스킬 로더 초기화 및 파일 로드
    let mut loader = SkillLoader::new();
    let load_result = loader.load_from_file(&test_file).await;
    assert!(
        load_result.is_ok(),
        "스킬 파일 로드 실패: {:?}",
        load_result
    );

    // 3. 스킬 로드 검증
    let all_skills = loader.get_all_skills();
    assert_eq!(all_skills.len(), 2, "예상 스킬 개수가 맞지 않음");

    let fireball = loader.get_skill(100);
    assert!(fireball.is_some(), "파이어볼 스킬이 로드되지 않음");
    assert_eq!(fireball.unwrap().name, "테스트 파이어볼");

    let heal = loader.get_skill(101);
    assert!(heal.is_some(), "치유 스킬이 로드되지 않음");
    assert_eq!(heal.unwrap().name, "테스트 치유");

    // 4. 스킬 시스템 초기화 (로드된 스킬 적용)
    let mut skill_system = SkillSystem::new();

    // 로드된 스킬을 스킬 시스템에 추가
    for (id, definition) in all_skills {
        skill_system
            .skill_definitions
            .insert(*id, definition.clone());
    }

    // 5. 플레이어 추가
    skill_system.add_player(1, Position { x: 0.0, y: 0.0 });
    skill_system.add_player(2, Position { x: 100.0, y: 100.0 });

    // 플레이어 1에게 충분한 마나 설정
    if let Some(player) = skill_system.players.get_mut(&1) {
        player.current_mana = 100;
    }

    // 6. 스킬 사용 테스트 - 파이어볼
    let use_skill_msg = UseSkillMessage {
        player_id: 1,
        skill_id: 100, // 테스트 파이어볼
        target_position: Some(Position { x: 100.0, y: 100.0 }),
        target_player_id: Some(2),
    };

    let result = skill_system.use_skill(use_skill_msg);
    assert!(result.success, "스킬 사용 실패: {}", result.message);
    assert_eq!(result.skill_id, 100);
    assert!(result.damage_dealt > 0, "데미지가 적용되지 않음");

    // 7. 쿨다운 검증
    let use_skill_again = UseSkillMessage {
        player_id: 1,
        skill_id: 100,
        target_position: Some(Position { x: 100.0, y: 100.0 }),
        target_player_id: Some(2),
    };

    let cooldown_result = skill_system.use_skill(use_skill_again);
    assert!(!cooldown_result.success, "쿨다운 중인데 스킬이 사용됨");
    assert!(
        cooldown_result.message.contains("쿨다운"),
        "쿨다운 메시지가 없음"
    );

    // 8. 치유 스킬 테스트
    if let Some(player) = skill_system.players.get_mut(&2) {
        player.current_health = 50; // 체력 감소
    }

    let heal_msg = UseSkillMessage {
        player_id: 1,
        skill_id: 101, // 테스트 치유
        target_position: Some(Position { x: 100.0, y: 100.0 }),
        target_player_id: Some(2),
    };

    let heal_result = skill_system.use_skill(heal_msg);
    assert!(heal_result.success, "치유 스킬 사용 실패");
    assert!(heal_result.healing_done > 0, "치유가 적용되지 않음");

    // 9. 플레이어 상태 확인
    if let Some(healed_player) = skill_system.players.get(&2) {
        assert!(healed_player.current_health > 50, "체력이 회복되지 않음");
    }

    // 정리
    cleanup_test_file(&test_file);
}

#[tokio::test]
async fn test_skill_filtering() {
    let test_file = create_test_json_file().await;

    let mut loader = SkillLoader::new();
    loader.load_from_file(&test_file).await.unwrap();

    // 범위 내 스킬 필터링
    let short_range = loader.get_skills_in_range(300.0);
    assert_eq!(short_range.len(), 1, "300 범위 내 스킬 개수가 맞지 않음");
    assert_eq!(short_range[0].skill_id, 101); // 치유 스킬만 해당

    // 마나 코스트 필터링
    let affordable = loader.get_affordable_skills(25);
    assert_eq!(
        affordable.len(),
        1,
        "25 마나로 사용 가능한 스킬 개수가 맞지 않음"
    );
    assert_eq!(affordable[0].skill_id, 101); // 치유 스킬만 해당

    // 타입별 필터링
    let attack_skills = loader.get_skills_by_type(&SkillType::BasicAttack);
    assert_eq!(attack_skills.len(), 1);
    assert_eq!(attack_skills[0].skill_id, 100);

    cleanup_test_file(&test_file);
}

#[tokio::test]
async fn test_balance_calculations() {
    let test_file = create_test_json_file().await;

    let mut loader = SkillLoader::new();
    loader.load_from_file(&test_file).await.unwrap();

    // 쿨다운 감소 계산
    let base_cooldown = 3000;
    let reduced_30 = loader.calculate_cooldown(base_cooldown, 30);
    assert_eq!(reduced_30, 2100, "30% 쿨다운 감소 계산 오류");

    let reduced_80 = loader.calculate_cooldown(base_cooldown, 80);
    assert_eq!(reduced_80, 600, "80% 쿨다운 감소 계산 오류");

    let reduced_100 = loader.calculate_cooldown(base_cooldown, 100);
    assert_eq!(reduced_100, 600, "최대 감소율 제한이 적용되지 않음");

    // 데미지 스케일링 계산
    let base_damage = 100;
    let lvl1_damage = loader.calculate_damage(base_damage, 1, 1.2);
    assert_eq!(lvl1_damage, 100, "레벨 1 데미지 계산 오류");

    let lvl5_damage = loader.calculate_damage(base_damage, 5, 1.2);
    assert_eq!(lvl5_damage, 207, "레벨 5 데미지 계산 오류"); // 100 * 1.2^4 ≈ 207

    cleanup_test_file(&test_file);
}

#[tokio::test]
async fn test_error_handling() {
    let loader = SkillLoader::new();

    // 존재하지 않는 파일 로드 시도
    let mut error_loader = SkillLoader::new();
    let error_result = error_loader.load_from_file("non_existent.json").await;
    assert!(error_result.is_err(), "존재하지 않는 파일이 로드됨");

    // 빈 스킬 시스템에서 스킬 조회
    let empty_skill = loader.get_skill(999);
    assert!(empty_skill.is_none(), "존재하지 않는 스킬이 반환됨");

    // 빈 필터링 결과
    let no_skills = loader.get_skills_in_range(0.0);
    assert!(no_skills.is_empty(), "범위 0에서 스킬이 반환됨");
}

#[tokio::test]
async fn test_concurrent_skill_usage() {
    use tokio::time::{sleep, Duration};

    let test_file = create_test_json_file().await;
    let mut loader = SkillLoader::new();
    loader.load_from_file(&test_file).await.unwrap();

    let skill_system = Arc::new(RwLock::new(SkillSystem::new()));

    // 로드된 스킬 적용
    {
        let mut system = skill_system.write().await;
        for (id, definition) in loader.get_all_skills() {
            system.skill_definitions.insert(*id, definition.clone());
        }

        // 플레이어 추가
        for i in 1..=5 {
            system.add_player(
                i,
                Position {
                    x: i as f32 * 10.0,
                    y: 0.0,
                },
            );
            if let Some(player) = system.players.get_mut(&i) {
                player.current_mana = 100;
            }
        }
    }

    // 동시에 여러 플레이어가 스킬 사용
    let mut handles = vec![];

    for player_id in 1..=5 {
        let system_clone = skill_system.clone();
        let handle = tokio::spawn(async move {
            let mut system = system_clone.write().await;
            let msg = UseSkillMessage {
                player_id,
                skill_id: 100,
                target_position: Some(Position { x: 50.0, y: 50.0 }),
                target_player_id: None,
            };
            system.use_skill(msg)
        });
        handles.push(handle);
    }

    // 모든 스킬 사용 완료 대기
    let mut success_count = 0;
    for handle in handles {
        let result = handle.await.unwrap();
        if result.success {
            success_count += 1;
        }
    }

    assert_eq!(success_count, 5, "모든 플레이어가 스킬을 사용하지 못함");

    cleanup_test_file(&test_file);
}

/// 실제 게임 시나리오 시뮬레이션 테스트
#[tokio::test]
async fn test_realistic_game_scenario() {
    let test_file = create_test_json_file().await;
    let mut loader = SkillLoader::new();
    loader.load_from_file(&test_file).await.unwrap();

    let mut skill_system = SkillSystem::new();

    // 스킬 적용
    for (id, definition) in loader.get_all_skills() {
        skill_system
            .skill_definitions
            .insert(*id, definition.clone());
    }

    // 시나리오: 2명의 플레이어가 전투
    skill_system.add_player(1, Position { x: 0.0, y: 0.0 }); // 공격자
    skill_system.add_player(2, Position { x: 100.0, y: 0.0 }); // 방어자

    // 초기 상태 설정
    if let Some(attacker) = skill_system.players.get_mut(&1) {
        attacker.current_mana = 200;
        attacker.level = 3;
    }

    if let Some(defender) = skill_system.players.get_mut(&2) {
        defender.current_health = 100;
        defender.current_mana = 200;
    }

    // 1. 공격자가 파이어볼 사용
    let attack = UseSkillMessage {
        player_id: 1,
        skill_id: 100,
        target_position: Some(Position { x: 100.0, y: 0.0 }),
        target_player_id: Some(2),
    };

    let attack_result = skill_system.use_skill(attack);
    assert!(attack_result.success);

    let defender_health_after_attack = skill_system
        .players
        .get(&2)
        .map(|p| p.current_health)
        .unwrap_or(0);

    assert!(
        defender_health_after_attack < 100,
        "방어자가 데미지를 받지 않음"
    );

    // 2. 방어자가 자신을 치유
    let self_heal = UseSkillMessage {
        player_id: 2,
        skill_id: 101,
        target_position: Some(Position { x: 100.0, y: 0.0 }),
        target_player_id: Some(2),
    };

    let heal_result = skill_system.use_skill(self_heal);
    assert!(heal_result.success);

    let defender_health_after_heal = skill_system
        .players
        .get(&2)
        .map(|p| p.current_health)
        .unwrap_or(0);

    assert!(
        defender_health_after_heal > defender_health_after_attack,
        "치유가 적용되지 않음"
    );

    // 3. 마나 소모 확인
    let attacker_mana = skill_system
        .players
        .get(&1)
        .map(|p| p.current_mana)
        .unwrap_or(0);
    assert!(attacker_mana < 200, "공격자 마나가 소모되지 않음");

    let defender_mana = skill_system
        .players
        .get(&2)
        .map(|p| p.current_mana)
        .unwrap_or(0);
    assert!(defender_mana < 200, "방어자 마나가 소모되지 않음");

    cleanup_test_file(&test_file);
}
