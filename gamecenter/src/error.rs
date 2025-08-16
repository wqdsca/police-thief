//! 게임센터 에러 타입 확장
//!
//! shared::tool::error::AppError를 확장하여 게임센터 특화 에러 타입을 제공합니다.

use shared::tool::error::AppError;
use thiserror::Error;

/// 게임센터 특화 에러 타입
#[derive(Debug, Error)]
pub enum GameCenterError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Server startup failed: {0}")]
    ServerStartupError(String),
}

// AppError로 변환
impl From<GameCenterError> for AppError {
    fn from(err: GameCenterError) -> Self {
        match err {
            GameCenterError::NotFound(msg) => {
                AppError::InternalError(format!("Not found: {}", msg))
            }
            GameCenterError::ValidationError(msg) => AppError::InvalidInput(msg),
            GameCenterError::SerializationError(msg) => AppError::InvalidFormat(msg),
            GameCenterError::ConfigError(msg) => {
                AppError::InternalError(format!("Config: {}", msg))
            }
            GameCenterError::ServerStartupError(msg) => AppError::ServiceUnavailable(msg),
        }
    }
}

// 편의를 위한 타입 별칭
pub type GameResult<T> = Result<T, GameCenterError>;
