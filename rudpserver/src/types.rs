//! 공통 타입 정의 모듈

use serde::{Deserialize, Serialize};

/// 플레이어 ID 타입
pub type PlayerId = u32;

/// 세션 ID 타입
pub type SessionId = u64;

/// 방 ID 타입
pub type RoomId = u32;

/// 2D 위치 좌표
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

impl Position {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Position) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// 게임 결과 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameResult {
    Victory,
    Defeat,
    Draw,
}

/// 에러 코드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    Success = 0,
    InvalidInput = 1000,
    PlayerNotFound = 1001,
    SessionNotFound = 1002,
    RoomFull = 1003,
    RoomNotFound = 1004,
    InternalError = 2000,
    DatabaseError = 2001,
    NetworkError = 2002,
}
