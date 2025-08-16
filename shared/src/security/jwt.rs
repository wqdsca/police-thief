//! JWT 토큰 관리 - 보안 강화된 구현
//!
//! - 토큰 만료 시간 관리
//! - Refresh token 지원
//! - 토큰 블랙리스트 관리
//! - 보안 검증 강화

use crate::security::{SecurityConfig, SecurityError};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// JWT 클레임 구조
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// 사용자 ID
    pub sub: String,
    /// 사용자 이름
    pub username: String,
    /// 사용자 역할
    pub roles: Vec<String>,
    /// 토큰 발급 시간
    pub iat: i64,
    /// 토큰 만료 시간
    pub exp: i64,
    /// JWT ID (고유 식별자)
    pub jti: String,
    /// 발급자
    pub iss: String,
}

/// Refresh 토큰 클레임
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RefreshClaims {
    pub sub: String,
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
    pub token_type: String, // "refresh"
}

/// JWT 관리자
pub struct JwtManager {
    config: SecurityConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
    /// 블랙리스트에 등록된 토큰들
    blacklist: Arc<RwLock<HashSet<String>>>,
}

impl JwtManager {
    /// 새 JWT 관리자 생성
    pub fn new(config: SecurityConfig) -> Result<Self, SecurityError> {
        let encoding_key = EncodingKey::from_secret(config.jwt_secret.as_ref());
        let decoding_key = DecodingKey::from_secret(config.jwt_secret.as_ref());

        let mut validation = Validation::new(
            Algorithm::from_str(&config.jwt_algorithm)
                .map_err(|e| SecurityError::InvalidToken(format!("Invalid algorithm: {e}")))?,
        );
        validation.set_issuer(&["police-thief-game"]);

        Ok(Self {
            config,
            encoding_key,
            decoding_key,
            validation,
            blacklist: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    /// Access 토큰 생성
    pub async fn create_access_token(
        &self,
        user_id: &str,
        username: &str,
        roles: Vec<String>,
    ) -> Result<String, SecurityError> {
        let now = Utc::now();
        let expiration = now + Duration::hours(self.config.jwt_expiration_hours as i64);

        let claims = Claims {
            sub: user_id.to_string(),
            username: username.to_string(),
            roles,
            iat: now.timestamp(),
            exp: expiration.timestamp(),
            jti: Uuid::new_v4().to_string(),
            iss: "police-thief-game".to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| SecurityError::InvalidToken(format!("Token encoding failed: {e}")))
    }

    /// Refresh 토큰 생성
    pub async fn create_refresh_token(&self, user_id: &str) -> Result<String, SecurityError> {
        let now = Utc::now();
        let expiration = now + Duration::days(self.config.jwt_refresh_expiration_days as i64);

        let claims = RefreshClaims {
            sub: user_id.to_string(),
            jti: Uuid::new_v4().to_string(),
            iat: now.timestamp(),
            exp: expiration.timestamp(),
            token_type: "refresh".to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| SecurityError::InvalidToken(format!("Refresh token encoding failed: {e}")))
    }

    /// 토큰 검증 및 클레임 추출
    pub async fn verify_token(&self, token: &str) -> Result<Claims, SecurityError> {
        // 블랙리스트 확인
        if self.is_blacklisted(token).await {
            return Err(SecurityError::InvalidToken(
                "Token is blacklisted".to_string(),
            ));
        }

        let token_data =
            decode::<Claims>(token, &self.decoding_key, &self.validation).map_err(|e| {
                match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        SecurityError::TokenExpired
                    }
                    _ => SecurityError::InvalidToken(format!("Token validation failed: {e}")),
                }
            })?;

        // 만료 시간 재확인
        let now = Utc::now().timestamp();
        if token_data.claims.exp < now {
            return Err(SecurityError::TokenExpired);
        }

        Ok(token_data.claims)
    }

    /// Refresh 토큰으로 새 Access 토큰 생성
    pub async fn refresh_access_token(&self, refresh_token: &str) -> Result<String, SecurityError> {
        // Refresh 토큰 검증
        let refresh_claims =
            decode::<RefreshClaims>(refresh_token, &self.decoding_key, &self.validation)
                .map_err(|e| SecurityError::InvalidToken(format!("Refresh token invalid: {e}")))?
                .claims;

        // Refresh 토큰 타입 확인
        if refresh_claims.token_type != "refresh" {
            return Err(SecurityError::InvalidToken(
                "Invalid token type".to_string(),
            ));
        }

        // 만료 확인
        let now = Utc::now().timestamp();
        if refresh_claims.exp < now {
            return Err(SecurityError::TokenExpired);
        }

        // 블랙리스트 확인
        if self.is_blacklisted(refresh_token).await {
            return Err(SecurityError::InvalidToken(
                "Refresh token is blacklisted".to_string(),
            ));
        }

        // 사용자 정보 조회 (실제로는 데이터베이스에서)
        // 여기서는 기본값 사용
        self.create_access_token(&refresh_claims.sub, "user", vec!["user".to_string()])
            .await
    }

    /// 토큰 블랙리스트 추가 (로그아웃)
    pub async fn blacklist_token(&self, token: &str) -> Result<(), SecurityError> {
        // JWT ID 추출
        let token_data =
            decode::<Claims>(token, &self.decoding_key, &self.validation).map_err(|e| {
                SecurityError::InvalidToken(format!("Cannot blacklist invalid token: {e}"))
            })?;

        let jti = token_data.claims.jti.clone();
        let user_id = token_data.claims.sub.clone();

        let mut blacklist = self.blacklist.write().await;
        blacklist.insert(token_data.claims.jti);

        tracing::info!(
            user_id = %user_id,
            jti = %jti,
            "Token added to blacklist"
        );

        Ok(())
    }

    /// 토큰이 블랙리스트에 있는지 확인
    pub async fn is_blacklisted(&self, token: &str) -> bool {
        if let Ok(token_data) = decode::<Claims>(token, &self.decoding_key, &self.validation) {
            let blacklist = self.blacklist.read().await;
            blacklist.contains(&token_data.claims.jti)
        } else {
            false
        }
    }

    /// 만료된 토큰들을 블랙리스트에서 정리
    pub async fn cleanup_expired_tokens(&self) {
        let _now = Utc::now().timestamp(); // TODO: 만료시간 기반 정리 구현
        let mut blacklist = self.blacklist.write().await;

        // 실제 구현에서는 토큰의 만료시간을 저장하여 정리해야 함
        // 여기서는 단순화된 구현
        let initial_size = blacklist.len();
        blacklist.retain(|_| true); // 실제로는 만료시간 체크

        let cleaned = initial_size - blacklist.len();
        if cleaned > 0 {
            tracing::info!("Cleaned {} expired tokens from blacklist", cleaned);
        }
    }

    /// 토큰에서 사용자 ID 추출
    pub async fn extract_user_id(&self, token: &str) -> Result<String, SecurityError> {
        let claims = self.verify_token(token).await?;
        Ok(claims.sub)
    }

    /// 토큰에서 사용자 역할 추출
    pub async fn extract_user_roles(&self, token: &str) -> Result<Vec<String>, SecurityError> {
        let claims = self.verify_token(token).await?;
        Ok(claims.roles)
    }

    /// 관리자 권한 확인
    pub async fn verify_admin_access(&self, token: &str) -> Result<Claims, SecurityError> {
        let claims = self.verify_token(token).await?;

        if !claims.roles.contains(&"admin".to_string()) {
            return Err(SecurityError::AuthorizationDenied(
                "Admin access required".to_string(),
            ));
        }

        Ok(claims)
    }
}

// Algorithm 확장을 위한 helper
trait FromStr {
    fn from_str(s: &str) -> Result<Algorithm, String>;
}

impl FromStr for Algorithm {
    fn from_str(s: &str) -> Result<Algorithm, String> {
        match s {
            "HS256" => Ok(Algorithm::HS256),
            "HS384" => Ok(Algorithm::HS384),
            "HS512" => Ok(Algorithm::HS512),
            _ => Err(format!("Unsupported algorithm: {s}")),
        }
    }
}

mod tests {
    

    #[tokio::test]
    async fn test_jwt_creation_and_verification() {
        let config = SecurityConfig::default();
        let jwt_manager = JwtManager::new(config).map_err(|e| format!("JWT Manager creation failed: {}", e)).expect("Failed to create JWT manager");

        // 토큰 생성
        let token = jwt_manager
            .create_access_token("user123", "testuser", vec!["user".to_string()])
            .await
            .expect("Async test assertion");

        // 토큰 검증
        let claims = jwt_manager.verify_token(&token).await.expect("Token verification failed");
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.username, "testuser");
    }

    #[tokio::test]
    async fn test_token_blacklist() {
        let config = SecurityConfig::default();
        let jwt_manager = JwtManager::new(config).expect("Failed to create JWT manager");

        let token = jwt_manager
            .create_access_token("user123", "testuser", vec!["user".to_string()])
            .await
            .expect("Failed to create access token");

        // 처음에는 유효
        assert!(jwt_manager.verify_token(&token).await.is_ok());

        // 블랙리스트 추가
        jwt_manager.blacklist_token(&token).await.expect("Failed to blacklist token");

        // 이제 무효
        assert!(jwt_manager.verify_token(&token).await.is_err());
    }
}
