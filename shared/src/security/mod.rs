//! 보안 모듈 - JWT, 입력검증, Rate Limiting, 암호화
//! 
//! 모든 서비스에서 사용할 수 있는 보안 기능을 제공합니다.

pub mod jwt;
pub mod validation;
pub mod rate_limiter;
pub mod crypto;
pub mod middleware;
pub mod redis_command_validator;
pub mod access_control;
pub mod security_auditor;

pub use jwt::*;
pub use validation::*;
pub use rate_limiter::*;
pub use crypto::*;
pub use middleware::*;
pub use redis_command_validator::*;
pub use access_control::*;
pub use security_auditor::*;

use thiserror::Error;

/// 보안 관련 에러
#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Authorization denied: {0}")]
    AuthorizationDenied(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Invalid token: {0}")]
    InvalidToken(String),
    
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    
    #[error("Message too large: {current} bytes (max: {max})")]
    MessageTooLarge { current: usize, max: usize },
}

/// 보안 설정
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// JWT 비밀키
    pub jwt_secret: String,
    /// JWT 알고리즘
    pub jwt_algorithm: String,
    /// JWT 만료시간 (시간)
    pub jwt_expiration_hours: u64,
    /// Refresh 토큰 만료시간 (일)
    pub jwt_refresh_expiration_days: u64,
    /// Rate limit (분당 요청수)
    pub rate_limit_rpm: u64,
    /// 최대 메시지 크기 (바이트)
    pub max_message_size: usize,
    /// bcrypt 라운드
    pub bcrypt_rounds: u32,
    /// CORS 허용 origin
    pub cors_allowed_origins: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        // ⚠️ 경고: 이 기본값은 개발 환경용입니다. 
        // 프로덕션에서는 반드시 SecurityConfig::from_env()를 사용하세요.
        tracing::warn!(
            "⚠️  SECURITY WARNING: Using default security configuration. \
             This should ONLY be used in development environment. \
             Use SecurityConfig::from_env() for production."
        );
        
        Self {
            jwt_secret: "INSECURE_DEFAULT_DEVELOPMENT_ONLY_DO_NOT_USE_IN_PRODUCTION".to_string(),
            jwt_algorithm: "HS256".to_string(),
            jwt_expiration_hours: 1, // 기본값을 짧게 설정
            jwt_refresh_expiration_days: 7, // 기본값을 짧게 설정
            rate_limit_rpm: 60, // 더 엄격한 기본값
            max_message_size: 32768, // 32KB로 감소
            bcrypt_rounds: 12,
            cors_allowed_origins: vec!["http://localhost:3000".to_string()],
        }
    }
}

impl SecurityConfig {
    /// 환경변수에서 보안 설정 로드 (프로덕션 권장)
    pub fn from_env() -> Result<Self, SecurityError> {
        use std::env;
        
        let jwt_secret = env::var("JWT_SECRET_KEY")
            .map_err(|_| SecurityError::InvalidInput(
                "JWT_SECRET_KEY environment variable is required for production".to_string()
            ))?;
            
        // 보안 강화: 최소 길이 검증
        if jwt_secret.len() < 32 {
            return Err(SecurityError::InvalidInput(
                format!("JWT_SECRET_KEY must be at least 32 characters. Current: {}", jwt_secret.len())
            ));
        }
        
        // 보안 강화: 약한 기본값 방지
        let lower_secret = jwt_secret.to_lowercase();
        if lower_secret.contains("default") || 
           lower_secret.contains("secret") ||
           lower_secret.contains("change") ||
           lower_secret.contains("your_") ||
           lower_secret.contains("please") ||
           lower_secret.contains("example") ||
           lower_secret.contains("insecure") {
            return Err(SecurityError::InvalidInput(
                "JWT_SECRET_KEY contains weak/default values. Use a cryptographically secure random key".to_string()
            ));
        }
        
        // 보안 설정 값 검증 및 로드
        let jwt_expiration_hours = env::var("JWT_EXPIRATION_HOURS")
            .unwrap_or_else(|_| "1".to_string()) // 더 안전한 기본값
            .parse()
            .unwrap_or(1);
            
        let rate_limit_rpm = env::var("RATE_LIMIT_RPM")
            .unwrap_or_else(|_| "60".to_string()) // 더 엄격한 기본값
            .parse()
            .unwrap_or(60);
            
        let bcrypt_rounds = env::var("BCRYPT_ROUNDS")
            .unwrap_or_else(|_| "12".to_string())
            .parse()
            .unwrap_or(12);
            
        // bcrypt 라운드 보안 검증
        if bcrypt_rounds < 10 || bcrypt_rounds > 15 {
            return Err(SecurityError::InvalidInput(
                "BCRYPT_ROUNDS must be between 10 and 15 for security".to_string()
            ));
        }
        
        let config = Self {
            jwt_secret,
            jwt_algorithm: env::var("JWT_ALGORITHM").unwrap_or_else(|_| "HS256".to_string()),
            jwt_expiration_hours,
            jwt_refresh_expiration_days: env::var("JWT_REFRESH_EXPIRATION_DAYS")
                .unwrap_or_else(|_| "7".to_string()) // 더 안전한 기본값
                .parse()
                .unwrap_or(7),
            rate_limit_rpm,
            max_message_size: env::var("MAX_MESSAGE_SIZE")
                .unwrap_or_else(|_| "32768".to_string()) // 32KB로 감소
                .parse()
                .unwrap_or(32768),
            bcrypt_rounds,
            cors_allowed_origins: env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:3000".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
        };
        
        // 보안 설정 로깅
        tracing::info!("🔐 Security Configuration Loaded:");
        tracing::info!("  └─ JWT Expiration: {} hours", config.jwt_expiration_hours);
        tracing::info!("  └─ Rate Limit: {} RPM", config.rate_limit_rpm);
        tracing::info!("  └─ Max Message Size: {} bytes", config.max_message_size);
        tracing::info!("  └─ BCrypt Rounds: {}", config.bcrypt_rounds);
        tracing::info!("  └─ Security Level: ✅ PRODUCTION");
        
        Ok(config)
    }
}