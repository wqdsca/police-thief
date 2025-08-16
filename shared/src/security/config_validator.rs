//! 보안 설정 검증 모듈
//!
//! 환경 변수 및 보안 설정이 프로덕션 수준인지 검증합니다.

use crate::security::SecurityError;
use std::env;
use tracing::{error, warn};

/// 보안 설정 검증기
pub struct SecurityConfigValidator;

impl SecurityConfigValidator {
    /// JWT Secret Key 검증
    pub fn validate_jwt_secret() -> Result<String, SecurityError> {
        let jwt_secret = env::var("JWT_SECRET_KEY").map_err(|_| {
            SecurityError::ConfigurationError(
                "JWT_SECRET_KEY environment variable not set. \
                 Please set a secure 256-bit key for production use."
                    .to_string(),
            )
        })?;

        // 프로덕션 환경에서 기본값 사용 방지
        if jwt_secret.contains("REPLACE_WITH_SECURE")
            || jwt_secret.contains("your_production_secret")
            || jwt_secret.len() < 32
        {
            let env_mode = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());

            if env_mode == "production" {
                error!("CRITICAL: Insecure JWT_SECRET_KEY detected in production!");
                return Err(SecurityError::ConfigurationError(
                    "JWT_SECRET_KEY is not secure for production use. \
                     Please generate a secure key: openssl rand -base64 32"
                        .to_string(),
                ));
            } else {
                warn!(
                    "Using insecure JWT_SECRET_KEY in {} environment. \
                     This must be changed before production deployment.",
                    env_mode
                );
            }
        }

        // 키 엔트로피 검증 (간단한 체크)
        if !Self::has_sufficient_entropy(&jwt_secret) {
            warn!("JWT_SECRET_KEY may have insufficient entropy. Consider using a cryptographically secure random generator.");
        }

        Ok(jwt_secret)
    }

    /// 엔트로피 검증 (간단한 휴리스틱)
    fn has_sufficient_entropy(key: &str) -> bool {
        // 최소 32자 이상
        if key.len() < 32 {
            return false;
        }

        // 다양한 문자 종류 포함 여부 확인
        let has_upper = key.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = key.chars().any(|c| c.is_ascii_lowercase());
        let has_digit = key.chars().any(|c| c.is_ascii_digit());
        let has_special = key.chars().any(|c| !c.is_ascii_alphanumeric());

        // 최소 3가지 이상 문자 종류 포함
        [has_upper, has_lower, has_digit, has_special]
            .iter()
            .filter(|&&x| x)
            .count()
            >= 3
    }

    /// TLS 설정 검증
    pub fn validate_tls_config() -> Result<(), SecurityError> {
        let enable_tls = env::var("ENABLE_TLS")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let env_mode = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());

        if env_mode == "production" && !enable_tls {
            warn!("TLS is disabled in production environment. This is a security risk!");
            // 프로덕션에서는 경고만 하고 진행 (일부 내부 네트워크 환경 고려)
        }

        if enable_tls {
            // TLS 인증서 경로 확인
            let cert_path = env::var("TLS_CERT_PATH").map_err(|_| {
                SecurityError::ConfigurationError(
                    "TLS_CERT_PATH not set but TLS is enabled".to_string(),
                )
            })?;

            let key_path = env::var("TLS_KEY_PATH").map_err(|_| {
                SecurityError::ConfigurationError(
                    "TLS_KEY_PATH not set but TLS is enabled".to_string(),
                )
            })?;

            // 파일 존재 여부 확인
            if !std::path::Path::new(&cert_path).exists() {
                return Err(SecurityError::ConfigurationError(format!(
                    "TLS certificate file not found: {}",
                    cert_path
                )));
            }

            if !std::path::Path::new(&key_path).exists() {
                return Err(SecurityError::ConfigurationError(format!(
                    "TLS key file not found: {}",
                    key_path
                )));
            }
        }

        Ok(())
    }

    /// 전체 보안 설정 검증
    pub fn validate_all() -> Result<(), SecurityError> {
        // JWT Secret 검증
        Self::validate_jwt_secret()?;

        // TLS 설정 검증
        Self::validate_tls_config()?;

        // Rate Limiting 설정 확인
        let rate_limit = env::var("RATE_LIMIT_PER_MINUTE")
            .unwrap_or_else(|_| "100".to_string())
            .parse::<u32>()
            .unwrap_or(100);

        if rate_limit > 1000 {
            warn!(
                "Rate limit is set very high ({}). This may expose the system to DoS attacks.",
                rate_limit
            );
        }

        // 로그인 시도 제한 확인
        let max_login_attempts = env::var("MAX_LOGIN_ATTEMPTS")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<u32>()
            .unwrap_or(5);

        if max_login_attempts > 10 {
            warn!("Max login attempts is set very high ({}). This may expose the system to brute force attacks.", max_login_attempts);
        }

        Ok(())
    }

    /// 프로덕션 준비 상태 확인
    pub fn check_production_readiness() -> bool {
        let mut is_ready = true;

        // 환경 변수 확인
        if env::var("ENVIRONMENT").unwrap_or_default() != "production" {
            warn!("Environment is not set to production");
        }

        // JWT Secret 검증
        if Self::validate_jwt_secret().is_err() {
            error!("JWT secret validation failed");
            is_ready = false;
        }

        // TLS 검증
        if Self::validate_tls_config().is_err() {
            warn!("TLS configuration validation failed");
            // TLS는 선택사항이므로 준비 상태에는 영향 없음
        }

        // 데이터베이스 비밀번호 확인
        if let Ok(db_password) = env::var("db_password") {
            if db_password == "YOUR_DB_PASSWORD_HERE" || db_password.len() < 8 {
                error!("Database password is not secure");
                is_ready = false;
            }
        }

        // Prometheus 모니터링 확인
        let enable_monitoring = env::var("ENABLE_MONITORING")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        if !enable_monitoring {
            warn!("Monitoring is disabled. This is not recommended for production.");
        }

        is_ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_check() {
        // 낮은 엔트로피
        assert!(!SecurityConfigValidator::has_sufficient_entropy(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ));
        assert!(!SecurityConfigValidator::has_sufficient_entropy(
            "12345678901234567890123456789012"
        ));

        // 높은 엔트로피
        assert!(SecurityConfigValidator::has_sufficient_entropy(
            "Abc123!@#DefGhi456$%^JklMno789&*"
        ));
        assert!(SecurityConfigValidator::has_sufficient_entropy(
            "xK9@mP2$vL5#nQ8*bR4&tY7!wZ3^aE6"
        ));
    }

    #[test]
    fn test_jwt_validation_with_env() {
        // 테스트 환경에서는 경고만 발생
        std::env::set_var("ENVIRONMENT", "test");
        std::env::set_var("JWT_SECRET_KEY", "test_key_only_for_testing");

        // 테스트 환경에서는 성공해야 함
        assert!(SecurityConfigValidator::validate_jwt_secret().is_ok());

        // 환경 변수 정리
        std::env::remove_var("ENVIRONMENT");
        std::env::remove_var("JWT_SECRET_KEY");
    }
}
