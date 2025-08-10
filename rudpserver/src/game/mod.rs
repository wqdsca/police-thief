//! 게임 로직 모듈
//!
//! RUDP 게임 서버의 핵심 게임 로직을 담당하는 모듈입니다.
//!
//! # 모듈 구조
//! - `messages`: 게임 메시지 프로토콜 정의
//! - `state_manager`: 게임 상태 관리 (핵심 로직)
//! - `player`: 플레이어 엔티티 관리
//! - `room_user_manager`: Redis 기반 방별 사용자 정보 관리
//! - `sample_example`: 새 기능 추가 예시 (스킬 시스템)

pub mod messages;
pub mod player;
pub mod room_user_manager;
pub mod sample_example;
pub mod skill_api;
pub mod skill_loader;
pub mod state_manager;

// 주요 타입들을 재export
pub use messages::{Direction, GameMessage, PlayerId, PlayerState, Position};
pub use player::{Player, PlayerManager};
pub use room_user_manager::{RoomUserInfo, RoomUserManager};
pub use sample_example::{SkillResultMessage, SkillSystem, SkillType, UseSkillMessage};
pub use skill_loader::SkillLoader;
pub use state_manager::GameStateManager;
