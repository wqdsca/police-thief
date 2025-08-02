use std::fmt;
use std::error::Error as StdError;
use serde::{Serialize, Deserialize};

/// 프로젝트 전체에서 사용하는 오류 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppError {
    /// 데이터베이스 관련 오류
    Database {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },
    
    /// Redis 관련 오류
    Redis {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        operation: Option<String>,
    },
    
    /// 네트워크 관련 오류
    Network {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
    
    /// 인증/권한 관련 오류
    Auth {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        user_id: Option<u32>,
    },
    
    /// 유효성 검사 오류
    Validation {
        message: String,
        field: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
    
    /// 비즈니스 로직 오류
    Business {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },
    
    /// 시스템 오류
    System {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        component: Option<String>,
    },
    
    /// 외부 API 오류
    ExternalApi {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        api_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },
    
    /// 직렬화/역직렬화 오류
    Serialization {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        format: Option<String>,
    },
    
    /// 리소스 없음 오류
    NotFound {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        resource_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        resource_id: Option<String>,
    },
    
    /// 충돌 오류
    Conflict {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        resource: Option<String>,
    },
    
    /// 제한 초과 오류
    RateLimit {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        retry_after: Option<u64>,
    },
    
    /// 알 수 없는 오류
    Unknown {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<String>,
    },
}

impl AppError {
    /// 오류 메시지 반환
    pub fn message(&self) -> &str {
        match self {
            AppError::Database { message, .. } => message,
            AppError::Redis { message, .. } => message,
            AppError::Network { message, .. } => message,
            AppError::Auth { message, .. } => message,
            AppError::Validation { message, .. } => message,
            AppError::Business { message, .. } => message,
            AppError::System { message, .. } => message,
            AppError::ExternalApi { message, .. } => message,
            AppError::Serialization { message, .. } => message,
            AppError::NotFound { message, .. } => message,
            AppError::Conflict { message, .. } => message,
            AppError::RateLimit { message, .. } => message,
            AppError::Unknown { message, .. } => message,
        }
    }
    
    /// 오류 코드 반환
    pub fn code(&self) -> &str {
        match self {
            AppError::Database { .. } => "DATABASE_ERROR",
            AppError::Redis { .. } => "REDIS_ERROR",
            AppError::Network { .. } => "NETWORK_ERROR",
            AppError::Auth { .. } => "AUTH_ERROR",
            AppError::Validation { .. } => "VALIDATION_ERROR",
            AppError::Business { .. } => "BUSINESS_ERROR",
            AppError::System { .. } => "SYSTEM_ERROR",
            AppError::ExternalApi { .. } => "EXTERNAL_API_ERROR",
            AppError::Serialization { .. } => "SERIALIZATION_ERROR",
            AppError::NotFound { .. } => "NOT_FOUND",
            AppError::Conflict { .. } => "CONFLICT",
            AppError::RateLimit { .. } => "RATE_LIMIT",
            AppError::Unknown { .. } => "UNKNOWN_ERROR",
        }
    }
    
    /// HTTP 상태 코드 반환
    pub fn status_code(&self) -> u16 {
        match self {
            AppError::Database { .. } => 500,
            AppError::Redis { .. } => 500,
            AppError::Network { .. } => 503,
            AppError::Auth { .. } => 401,
            AppError::Validation { .. } => 400,
            AppError::Business { .. } => 400,
            AppError::System { .. } => 500,
            AppError::ExternalApi { .. } => 502,
            AppError::Serialization { .. } => 400,
            AppError::NotFound { .. } => 404,
            AppError::Conflict { .. } => 409,
            AppError::RateLimit { .. } => 429,
            AppError::Unknown { .. } => 500,
        }
    }
    
    /// Redis 오류 생성
    pub fn redis(message: impl Into<String>, operation: Option<String>) -> Self {
        AppError::Redis {
            message: message.into(),
            operation,
        }
    }
    
    /// 직렬화 오류 생성
    pub fn serialization(message: impl Into<String>, format: Option<String>) -> Self {
        AppError::Serialization {
            message: message.into(),
            format,
        }
    }
    
    /// 리소스 없음 오류 생성
    pub fn not_found(message: impl Into<String>, resource_type: Option<String>, resource_id: Option<String>) -> Self {
        AppError::NotFound {
            message: message.into(),
            resource_type,
            resource_id,
        }
    }
    
    /// 비즈니스 오류 생성
    pub fn business(message: impl Into<String>, code: Option<String>) -> Self {
        AppError::Business {
            message: message.into(),
            code,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

impl StdError for AppError {}

impl From<redis::RedisError> for AppError {
    fn from(err: redis::RedisError) -> Self {
        AppError::Redis {
            message: err.to_string(),
            operation: None,
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Serialization {
            message: err.to_string(),
            format: Some("JSON".to_string()),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Unknown {
            message: err.to_string(),
            details: None,
        }
    }
}

/// Result 타입 별칭
pub type AppResult<T> = Result<T, AppError>;

/// 오류 응답 구조체
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub status_code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl From<AppError> for ErrorResponse {
    fn from(err: AppError) -> Self {
        ErrorResponse {
            error: err.code().to_string(),
            message: err.message().to_string(),
            status_code: err.status_code(),
            details: None,
        }
    }
}
