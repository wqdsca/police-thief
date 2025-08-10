//! 보안 미들웨어
//! 
//! - HTTP/TCP 보안 미들웨어
//! - 통합 보안 검증 레이어

use crate::security::{
    CryptoManager, InputValidator, JwtManager, RateLimiter, SecurityConfig, SecurityError,
};
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{info, warn};

/// 보안 미들웨어
pub struct SecurityMiddleware {
    jwt_manager: Arc<JwtManager>,
    input_validator: Arc<InputValidator>,
    rate_limiter: Arc<RateLimiter>,
    crypto_manager: Arc<CryptoManager>,
}

impl SecurityMiddleware {
    /// 새 보안 미들웨어 생성
    pub async fn new(config: SecurityConfig) -> Result<Self, SecurityError> {
        let jwt_manager = Arc::new(JwtManager::new(config.clone())?);
        let input_validator = Arc::new(InputValidator::new());
        let rate_limiter = Arc::new(RateLimiter::from_security_config(&config));
        let crypto_manager = Arc::new(CryptoManager::new(config.clone()));
        
        Ok(Self {
            jwt_manager,
            input_validator,
            rate_limiter,
            crypto_manager,
        })
    }
    
    /// 환경변수에서 보안 미들웨어 생성
    pub async fn from_env() -> Result<Self, SecurityError> {
        let config = SecurityConfig::from_env()?;
        Self::new(config).await
    }
    
    /// JWT 토큰 인증
    pub async fn authenticate(&self, token: &str) -> Result<crate::security::Claims, SecurityError> {
        self.jwt_manager.verify_token(token).await
    }
    
    /// Rate limiting 검사
    pub async fn check_rate_limit(&self, ip: IpAddr) -> Result<bool, SecurityError> {
        self.rate_limiter.is_allowed(ip).await
    }
    
    /// 입력 데이터 검증
    pub fn validate_input(&self, data: &str) -> Result<(), SecurityError> {
        self.input_validator.validate_json(data)
            .map(|_| ())
            .map_err(|e| SecurityError::InvalidInput(e))
    }
    
    /// 패킷 검증 (바이너리 데이터)
    pub async fn validate_packet(&self, data: &[u8]) -> Result<bool, SecurityError> {
        // 패킷 크기 검증
        if data.len() > 65536 { // 64KB 제한
            return Ok(false);
        }
        
        // 빈 패킷 검증
        if data.is_empty() {
            return Ok(false);
        }
        
        // 기본적인 헤더 크기 검증 (최소 8바이트)
        if data.len() < 8 {
            return Ok(false);
        }
        
        // 패킷 패턴 검증 (간단한 시그니처 체크)
        // 실제 프로덕션에서는 더 정교한 검증 로직 필요
        let signature = &data[0..4];
        let known_signatures = [
            [0x52, 0x55, 0x44, 0x50], // "RUDP"
            [0x43, 0x4F, 0x4E, 0x4E], // "CONN" 
            [0x44, 0x41, 0x54, 0x41], // "DATA"
            [0x41, 0x43, 0x4B, 0x00], // "ACK\0"
        ];
        
        let is_valid_signature = known_signatures.iter().any(|sig| sig == signature);
        
        if !is_valid_signature {
            warn!("Invalid packet signature: {:02X?}", signature);
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// 사용자 등록 처리 (예시)
    pub async fn register_user(&self, username: &str, password: &str, email: &str) -> Result<String, SecurityError> {
        // 입력 검증
        self.input_validator.validate(username, crate::security::InputType::Username)
            .map_err(|e| SecurityError::InvalidInput(e))?;
        self.input_validator.validate(password, crate::security::InputType::Password)
            .map_err(|e| SecurityError::InvalidInput(e))?;
        self.input_validator.validate(email, crate::security::InputType::Email)
            .map_err(|e| SecurityError::InvalidInput(e))?;
        
        // 비밀번호 해싱
        let _password_hash = self.crypto_manager.hash_password(password)?; // TODO: 데이터베이스에 저장
        
        // 세션 ID 생성
        let session_id = self.crypto_manager.generate_session_id();
        
        info!(
            username = %username,
            email = %email,
            session_id = %session_id,
            "User registered successfully"
        );
        
        Ok(session_id)
    }
    
    /// 사용자 로그인 처리 (예시)
    pub async fn login_user(&self, username: &str, password: &str, stored_hash: &str) -> Result<(String, String), SecurityError> {
        // 입력 검증
        self.input_validator.validate(username, crate::security::InputType::Username)
            .map_err(|e| SecurityError::InvalidInput(e))?;
        
        // 비밀번호 검증
        if !self.crypto_manager.verify_password(password, stored_hash)? {
            return Err(SecurityError::AuthenticationFailed("Invalid credentials".to_string()));
        }
        
        // 토큰 생성
        let access_token = self.jwt_manager.create_access_token(
            "user_id", // 실제로는 DB에서 가져옴
            username,
            vec!["user".to_string()]
        ).await?;
        
        let refresh_token = self.jwt_manager.create_refresh_token("user_id").await?;
        
        info!(
            username = %username,
            "User logged in successfully"
        );
        
        Ok((access_token, refresh_token))
    }
    
    /// 통합 보안 검사
    pub async fn security_check(
        &self,
        ip: IpAddr,
        token: Option<&str>,
        payload: Option<&str>
    ) -> Result<Option<crate::security::Claims>, SecurityError> {
        // Rate limiting 검사
        if !self.check_rate_limit(ip).await? {
            warn!("Rate limit exceeded for IP: {}", ip);
            return Err(SecurityError::RateLimitExceeded);
        }
        
        // 페이로드 검증 (있는 경우)
        if let Some(data) = payload {
            self.validate_input(data)?;
        }
        
        // 토큰 검증 (있는 경우)
        if let Some(token_str) = token {
            let claims = self.authenticate(token_str).await?;
            return Ok(Some(claims));
        }
        
        Ok(None)
    }
    
    /// JWT 관리자 참조 반환
    pub fn jwt_manager(&self) -> &JwtManager {
        &self.jwt_manager
    }
    
    /// Rate Limiter 참조 반환
    pub fn rate_limiter(&self) -> &RateLimiter {
        &self.rate_limiter
    }
    
    /// 입력 검증기 참조 반환
    pub fn input_validator(&self) -> &InputValidator {
        &self.input_validator
    }
    
    /// 암호화 관리자 참조 반환
    pub fn crypto_manager(&self) -> &CryptoManager {
        &self.crypto_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_security_middleware() {
        let config = SecurityConfig::default();
        let middleware = SecurityMiddleware::new(config).await.unwrap();
        
        // Rate limiting 테스트
        let test_ip = IpAddr::from_str("192.168.1.100").unwrap();
        assert!(middleware.check_rate_limit(test_ip).await.unwrap());
        
        // 입력 검증 테스트
        assert!(middleware.validate_input("{\"test\": \"valid\"}").is_ok());
        assert!(middleware.validate_input("<script>alert('xss')</script>").is_err());
    }
    
    #[tokio::test]
    async fn test_user_registration() {
        let config = SecurityConfig::default();
        let middleware = SecurityMiddleware::new(config).await.unwrap();
        
        // 유효한 등록
        assert!(middleware.register_user("testuser", "SecurePass123!", "test@example.com").await.is_ok());
        
        // 무효한 등록 (약한 비밀번호)
        assert!(middleware.register_user("testuser", "weak", "test@example.com").await.is_err());
    }
}