//! 게임 속성 관리 모듈
//!
//! 게임의 모든 기본값과 설정을 중앙에서 관리합니다.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// 게임 기본 속성 구조체
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDefaults {
    pub player: PlayerDefaults,
    pub spawn: SpawnDefaults,
    pub network: NetworkDefaults,
    pub room: RoomDefaults,
    pub combat: CombatDefaults,
    pub performance: PerformanceDefaults,
}

/// 플레이어 기본 속성
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerDefaults {
    pub max_health: u32,
    pub initial_health: u32,
    pub max_mana: u32,
    pub initial_mana: u32,
    pub attack: u32,
    pub defense: u32,
    pub movement_speed: f32,
    pub attack_speed: f32,
    pub critical_chance: f32,
    pub critical_damage: f32,
    pub vision_range: f32,
    pub attack_range: f32,
}

/// 스폰 관련 기본값
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnDefaults {
    pub default_x: f32,
    pub default_y: f32,
    pub default_z: f32,
    pub respawn_delay_ms: u64,
    pub invulnerable_duration_ms: u64,
}

/// 네트워크 동기화 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDefaults {
    pub position_sync_rate: u32,
    pub state_sync_rate: u32,
    pub interpolation_delay_ms: u32,
}

/// 방 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomDefaults {
    pub max_players_per_room: u32,
    pub min_players_to_start: u32,
    pub room_timeout_seconds: u64,
}

/// 전투 시스템 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatDefaults {
    pub damage_variance: f32,
    pub dodge_chance_base: f32,
    pub block_damage_reduction: f32,
}

/// 성능 최적화 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceDefaults {
    pub tick_rate: u32,
    pub max_message_size: usize,
    pub message_compression_threshold: usize,
}

impl GameDefaults {
    /// TOML 파일에서 기본값 로드
    pub fn from_toml_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let defaults: GameDefaults = toml::from_str(&contents)?;
        Ok(defaults)
    }

    /// 기본값으로 초기화
    pub fn default() -> Self {
        Self {
            player: PlayerDefaults {
                max_health: 1000,
                initial_health: 1000,
                max_mana: 500,
                initial_mana: 500,
                attack: 100,
                defense: 50,
                movement_speed: 5.0,
                attack_speed: 1.0,
                critical_chance: 5.0,
                critical_damage: 1.5,
                vision_range: 50.0,
                attack_range: 10.0,
            },
            spawn: SpawnDefaults {
                default_x: 0.0,
                default_y: 0.0,
                default_z: 0.0,
                respawn_delay_ms: 5000,
                invulnerable_duration_ms: 3000,
            },
            network: NetworkDefaults {
                position_sync_rate: 20,
                state_sync_rate: 10,
                interpolation_delay_ms: 100,
            },
            room: RoomDefaults {
                max_players_per_room: 20,
                min_players_to_start: 2,
                room_timeout_seconds: 3600,
            },
            combat: CombatDefaults {
                damage_variance: 0.1,
                dodge_chance_base: 5.0,
                block_damage_reduction: 0.5,
            },
            performance: PerformanceDefaults {
                tick_rate: 60,
                max_message_size: 65536,
                message_compression_threshold: 1024,
            },
        }
    }
}

/// 전역 게임 기본값 인스턴스
static mut GAME_DEFAULTS: Option<GameDefaults> = None;
static INIT: std::sync::Once = std::sync::Once::new();

/// 게임 기본값 초기화
pub fn init_defaults() -> &'static GameDefaults {
    unsafe {
        INIT.call_once(|| {
            let defaults = match GameDefaults::from_toml_file("property/game_defaults.toml") {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("기본값 파일 로드 실패, 기본값 사용: {}", e);
                    GameDefaults::default()
                }
            };
            GAME_DEFAULTS = Some(defaults);
        });
        GAME_DEFAULTS.as_ref().unwrap()
    }
}

/// 게임 기본값 가져오기
pub fn get_defaults() -> &'static GameDefaults {
    unsafe {
        if GAME_DEFAULTS.is_none() {
            init_defaults()
        } else {
            GAME_DEFAULTS.as_ref().unwrap()
        }
    }
}