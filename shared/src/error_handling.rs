//! 통합 에러 처리 시스템
//!
//! 프로젝트 전체에서 사용할 표준화된 에러 처리 메커니즘 제공
//! 
//! # 설계 원칙
//! - Zero Panic: unwrap() 대신 Result 사용
//! - 명확한 에러 컨텍스트 제공
//! - 복구 가능한 에러와 치명적 에러 구분
//! - 성능 영향 최소화

use std::fmt;
use thiserror::Error;
use anyhow::{Context as _, Result as AnyhowResult};

/// 프로젝트 전체 통합 에러 타입
#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("네트워크 오류: {message}")]
    Network { 
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("데이터베이스 오류: {message}")]
    Database {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Redis 오류: {message}")]
    Redis {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("설정 오류: {message}")]
    Configuration {
        message: String,
    },
    
    #[error("파싱 오류: {message}")]
    Parse {
        message: String,
        input: String,
    },
    
    #[error("유효성 검사 실패: {message}")]
    Validation {
        message: String,
        field: Option<String>,
    },
    
    #[error("인증 실패: {message}")]
    Authentication {
        message: String,
    },
    
    #[error("권한 부족: {message}")]
    Authorization {
        message: String,
    },
    
    #[error("리소스를 찾을 수 없음: {resource}")]
    NotFound {
        resource: String,
    },
    
    #[error("충돌 발생: {message}")]
    Conflict {
        message: String,
    },
    
    #[error("타임아웃: {operation}")]
    Timeout {
        operation: String,
        duration_ms: u64,
    },
    
    #[error("Rate Limit 초과")]
    RateLimitExceeded,
    
    #[error("내부 서버 오류: {message}")]
    Internal {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

/// Result 타입 별칭
pub type ProjectResult<T> = Result<T, ProjectError>;

/// 에러 복구 전략
pub enum RecoveryStrategy {
    /// 재시도 가능
    Retry { max_attempts: u32, delay_ms: u64 },
    /// 대체 값 사용
    Fallback,
    /// 회로 차단기 활성화
    CircuitBreak,
    /// 복구 불가능 - 종료
    Fatal,
}

impl ProjectError {
    /// 에러에 대한 복구 전략 제안
    pub fn recovery_strategy(&self) -> RecoveryStrategy {
        match self {
            Self::Network { .. } | Self::Timeout { .. } => {
                RecoveryStrategy::Retry { 
                    max_attempts: 3, 
                    delay_ms: 1000 
                }
            }
            Self::Database { .. } | Self::Redis { .. } => {
                RecoveryStrategy::CircuitBreak
            }
            Self::NotFound { .. } => RecoveryStrategy::Fallback,
            Self::Configuration { .. } | Self::Internal { .. } => {
                RecoveryStrategy::Fatal
            }
            _ => RecoveryStrategy::Fallback,
        }
    }
    
    /// HTTP 상태 코드로 변환
    pub fn status_code(&self) -> u16 {
        match self {
            Self::NotFound { .. } => 404,
            Self::Authentication { .. } => 401,
            Self::Authorization { .. } => 403,
            Self::Validation { .. } | Self::Parse { .. } => 400,
            Self::Conflict { .. } => 409,
            Self::RateLimitExceeded => 429,
            Self::Timeout { .. } => 408,
            _ => 500,
        }
    }
}

/// Safe unwrap 대체 함수들
pub trait SafeUnwrap<T> {
    /// unwrap() 대신 사용할 안전한 메서드
    fn safe_unwrap(self, context: &str) -> ProjectResult<T>;
    
    /// expect() 대신 사용할 안전한 메서드
    fn safe_expect(self, message: &str) -> ProjectResult<T>;
}

impl<T> SafeUnwrap<T> for Option<T> {
    fn safe_unwrap(self, context: &str) -> ProjectResult<T> {
        self.ok_or_else(|| ProjectError::Internal {
            message: format!("Unexpected None value: {}", context),
            source: None,
        })
    }
    
    fn safe_expect(self, message: &str) -> ProjectResult<T> {
        self.ok_or_else(|| ProjectError::Internal {
            message: message.to_string(),
            source: None,
        })
    }
}

impl<T, E> SafeUnwrap<T> for Result<T, E> 
where 
    E: std::error::Error + Send + Sync + 'static
{
    fn safe_unwrap(self, context: &str) -> ProjectResult<T> {
        self.map_err(|e| ProjectError::Internal {
            message: format!("Error in {}: {}", context, e),
            source: Some(Box::new(e)),
        })
    }
    
    fn safe_expect(self, message: &str) -> ProjectResult<T> {
        self.map_err(|e| ProjectError::Internal {
            message: message.to_string(),
            source: Some(Box::new(e)),
        })
    }
}

/// 에러 체인 빌더
pub struct ErrorChain {
    errors: Vec<String>,
}

impl ErrorChain {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }
    
    pub fn add(mut self, error: impl fmt::Display) -> Self {
        self.errors.push(error.to_string());
        self
    }
    
    pub fn build(self) -> ProjectError {
        ProjectError::Internal {
            message: self.errors.join(" -> "),
            source: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_safe_unwrap_option() {
        let some_value: Option<i32> = Some(42);
        let result = some_value.safe_unwrap("test context");
        assert!(result.is_ok());
        assert_eq!(result.expect("Test assertion failed"), 42);
        
        let none_value: Option<i32> = None;
        let result = none_value.safe_unwrap("test context");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_recovery_strategy() {
        let network_error = ProjectError::Network {
            message: "Connection failed".to_string(),
            source: None,
        };
        
        match network_error.recovery_strategy() {
            RecoveryStrategy::Retry { max_attempts, .. } => {
                assert_eq!(max_attempts, 3);
            }
            _ => panic!("Expected Retry strategy"),
        }
    }
    
    #[test]
    fn test_status_code_mapping() {
        let not_found = ProjectError::NotFound {
            resource: "user".to_string(),
        };
        assert_eq!(not_found.status_code(), 404);
        
        let auth_error = ProjectError::Authentication {
            message: "Invalid token".to_string(),
        };
        assert_eq!(auth_error.status_code(), 401);
    }
    
    #[test]
    fn test_error_chain() {
        let error = ErrorChain::new()
            .add("Database connection failed")
            .add("Pool exhausted")
            .add("Timeout after 30s")
            .build();
            
        match error {
            ProjectError::Internal { message, .. } => {
                assert!(message.contains("Database connection failed"));
                assert!(message.contains("Pool exhausted"));
                assert!(message.contains("Timeout after 30s"));
            }
            _ => panic!("Expected Internal error"),
        }
    }
}