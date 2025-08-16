//! JWT 토큰 서비스
//! Access Token과 Refresh Token 관리

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use uuid::Uuid;

/// JWT 클레임
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,      // 사용자 ID
    pub username: String, // 사용자명
    pub nickname: String, // 닉네임
    pub role: String,     // 역할 (admin, user)
    pub exp: i64,         // 만료 시간
    pub iat: i64,         // 발급 시간
    pub jti: String,      // JWT ID (토큰 고유 식별자)
}

/// Refresh Token 클레임
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshClaims {
    pub sub: String, // 사용자 ID
    pub jti: String, // JWT ID
    pub exp: i64,    // 만료 시간
    pub iat: i64,    // 발급 시간
}

/// 토큰 쌍
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
    pub token_type: String,
}

/// 사용자 정보 (토큰 생성용)
#[derive(Debug, Clone)]
pub struct UserInfo {
    pub user_id: String,
    pub username: String,
    pub nickname: String,
    pub role: String,
}

/// 클라이언트 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub device_type: Option<String>,
    pub device_id: Option<String>,
    pub app_version: Option<String>,
    pub platform: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// 토큰 서비스
#[derive(Clone)]
pub struct TokenService {
    pool: MySqlPool,
    secret_key: String,
    access_token_duration: Duration,
    refresh_token_duration: Duration,
}

impl TokenService {
    /// 새 토큰 서비스 생성
    pub fn new(pool: MySqlPool) -> Self {
        let secret_key = std::env::var("JWT_SECRET_KEY")
            .unwrap_or_else(|_| "your-secret-key-min-256-bits-long-for-security".to_string());

        Self {
            pool,
            secret_key,
            access_token_duration: Duration::minutes(15), // Access Token: 15분
            refresh_token_duration: Duration::days(30),   // Refresh Token: 30일
        }
    }

    /// 토큰 쌍 생성 (Access + Refresh)
    pub async fn create_token_pair(
        &self,
        user_info: &UserInfo,
        client_info: Option<ClientInfo>,
    ) -> Result<TokenPair> {
        let now = Utc::now();
        let jti = Uuid::new_v4().to_string();

        let access_expires_at = now + self.access_token_duration;
        let refresh_expires_at = now + self.refresh_token_duration;

        // Access Token 생성
        let access_claims = Claims {
            sub: user_info.user_id.clone(),
            username: user_info.username.clone(),
            nickname: user_info.nickname.clone(),
            role: user_info.role.clone(),
            exp: access_expires_at.timestamp(),
            iat: now.timestamp(),
            jti: jti.clone(),
        };

        let access_token = encode(
            &Header::new(Algorithm::HS256),
            &access_claims,
            &EncodingKey::from_secret(self.secret_key.as_bytes()),
        )?;

        // Refresh Token 생성
        let refresh_claims = RefreshClaims {
            sub: user_info.user_id.clone(),
            jti: jti.clone(),
            exp: refresh_expires_at.timestamp(),
            iat: now.timestamp(),
        };

        let refresh_token = encode(
            &Header::new(Algorithm::HS256),
            &refresh_claims,
            &EncodingKey::from_secret(self.secret_key.as_bytes()),
        )?;

        // 데이터베이스에 토큰 저장
        self.store_tokens(
            &user_info.user_id,
            &access_token,
            &refresh_token,
            &access_expires_at,
            &refresh_expires_at,
            &client_info,
        )
        .await?;

        Ok(TokenPair {
            access_token,
            refresh_token,
            access_expires_at,
            refresh_expires_at,
            token_type: "Bearer".to_string(),
        })
    }

    /// Access Token 검증
    pub async fn verify_access_token(&self, token: &str) -> Result<Claims> {
        // JWT 토큰 디코딩
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret_key.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )?;

        let claims = token_data.claims;

        // 데이터베이스에서 토큰 유효성 확인
        let token_hash = self.hash_token(token);
        let exists: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM user_tokens 
             WHERE token_hash = ? 
               AND token_type = 'access' 
               AND expires_at > NOW() 
               AND revoked_at IS NULL",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;

        if exists.is_none() {
            return Err(anyhow::anyhow!("Token not found or expired in database"));
        }

        Ok(claims)
    }

    /// Refresh Token으로 새 Access Token 발급
    pub async fn refresh_access_token(
        &self,
        refresh_token: &str,
        client_info: Option<ClientInfo>,
    ) -> Result<TokenPair> {
        // Refresh Token 검증
        let _refresh_claims = decode::<RefreshClaims>(
            refresh_token,
            &DecodingKey::from_secret(self.secret_key.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )?;

        // 데이터베이스에서 Refresh Token 확인
        let refresh_hash = self.hash_token(refresh_token);
        let token_record: Option<(i64,)> = sqlx::query_as(
            "SELECT user_id FROM user_tokens 
             WHERE token_hash = ? 
               AND token_type = 'refresh' 
               AND expires_at > NOW() 
               AND revoked_at IS NULL",
        )
        .bind(refresh_hash)
        .fetch_optional(&self.pool)
        .await?;

        let user_id = match token_record {
            Some((user_id,)) => user_id,
            None => return Err(anyhow::anyhow!("Invalid or expired refresh token")),
        };

        // 사용자 정보 조회
        let user_info = self.get_user_info(user_id).await?;

        // 기존 토큰들 무효화
        self.revoke_user_tokens(user_id).await?;

        // 새 토큰 쌍 생성
        self.create_token_pair(&user_info, client_info).await
    }

    /// 토큰 무효화 (로그아웃)
    pub async fn revoke_token(&self, token: &str) -> Result<()> {
        let token_hash = self.hash_token(token);

        sqlx::query(
            "UPDATE user_tokens 
             SET revoked_at = NOW() 
             WHERE token_hash = ?",
        )
        .bind(token_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 사용자의 모든 토큰 무효화
    pub async fn revoke_user_tokens(&self, user_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE user_tokens 
             SET revoked_at = NOW() 
             WHERE user_id = ? AND revoked_at IS NULL",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 만료된 토큰 정리
    pub async fn cleanup_expired_tokens(&self) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM user_tokens 
             WHERE expires_at < NOW() OR revoked_at IS NOT NULL",
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// 사용자의 활성 토큰 수 조회
    pub async fn count_active_tokens(&self, user_id: i64) -> Result<i64> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) as count FROM user_tokens 
             WHERE user_id = ? 
               AND expires_at > NOW() 
               AND revoked_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count.0)
    }

    /// 사용자의 활성 세션 목록
    pub async fn get_user_sessions(&self, user_id: i64) -> Result<Vec<SessionInfo>> {
        let sessions: Vec<(u64, Option<serde_json::Value>, DateTime<Utc>, DateTime<Utc>)> =
            sqlx::query_as(
                "SELECT token_id, client_info, created_at, expires_at 
             FROM user_tokens 
             WHERE user_id = ? 
               AND token_type = 'access' 
               AND expires_at > NOW() 
               AND revoked_at IS NULL 
             ORDER BY created_at DESC",
            )
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;

        let mut result = Vec::new();
        for (token_id, client_info_json, created_at, expires_at) in sessions {
            let client_info: Option<ClientInfo> =
                client_info_json.and_then(|info| serde_json::from_value(info).ok());

            result.push(SessionInfo {
                token_id,
                client_info,
                created_at,
                expires_at,
            });
        }

        Ok(result)
    }

    // Private methods

    /// 토큰 해시 생성
    fn hash_token(&self, token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 토큰 데이터베이스 저장
    async fn store_tokens(
        &self,
        user_id: &str,
        access_token: &str,
        refresh_token: &str,
        access_expires_at: &DateTime<Utc>,
        refresh_expires_at: &DateTime<Utc>,
        client_info: &Option<ClientInfo>,
    ) -> Result<()> {
        let user_id: i64 = user_id.parse()?;
        let access_hash = self.hash_token(access_token);
        let refresh_hash = self.hash_token(refresh_token);
        let client_json = client_info
            .as_ref()
            .and_then(|info| serde_json::to_value(info).ok());

        // 트랜잭션으로 두 토큰을 모두 저장
        let mut tx = self.pool.begin().await?;

        // Access Token 저장
        sqlx::query(
            "INSERT INTO user_tokens (user_id, token_type, token_hash, client_info, expires_at)
             VALUES (?, 'access', ?, ?, ?)",
        )
        .bind(user_id)
        .bind(access_hash)
        .bind(client_json.clone())
        .bind(access_expires_at)
        .execute(&mut *tx)
        .await?;

        // Refresh Token 저장
        sqlx::query(
            "INSERT INTO user_tokens (user_id, token_type, token_hash, client_info, expires_at)
             VALUES (?, 'refresh', ?, ?, ?)",
        )
        .bind(user_id)
        .bind(refresh_hash)
        .bind(client_json)
        .bind(refresh_expires_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// 사용자 정보 조회
    async fn get_user_info(&self, user_id: i64) -> Result<UserInfo> {
        let user: (i64, String, String, String) = sqlx::query_as(
            "SELECT user_id, username, nickname, 'user' as role 
             FROM users 
             WHERE user_id = ? AND status = 'active'",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(UserInfo {
            user_id: user.0.to_string(),
            username: user.1,
            nickname: user.2,
            role: user.3,
        })
    }
}

/// 세션 정보
#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub token_id: u64,
    pub client_info: Option<ClientInfo>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::MySqlPool;

    async fn create_test_pool() -> MySqlPool {
        // 테스트용 데이터베이스 연결
        // 실제 테스트에서는 test database를 사용해야 함
        MySqlPool::connect("mysql://test:test@localhost/test_db")
            .await
            .expect("Failed to connect to test database")
    }

    #[tokio::test]
    async fn test_token_creation_and_verification() {
        let pool = create_test_pool().await;
        let service = TokenService::new(pool);

        let user_info = UserInfo {
            user_id: "1".to_string(),
            username: "testuser".to_string(),
            nickname: "Test User".to_string(),
            role: "user".to_string(),
        };

        // 토큰 쌍 생성
        let token_pair = service
            .create_token_pair(&user_info, None)
            .await
            .expect("Failed to create token pair");

        // Access Token 검증
        let claims = service
            .verify_access_token(&token_pair.access_token)
            .await
            .expect("Failed to verify access token");

        assert_eq!(claims.sub, user_info.user_id);
        assert_eq!(claims.username, user_info.username);
        assert_eq!(claims.nickname, user_info.nickname);
    }

    #[tokio::test]
    async fn test_token_refresh() {
        let pool = create_test_pool().await;
        let service = TokenService::new(pool);

        let user_info = UserInfo {
            user_id: "1".to_string(),
            username: "testuser".to_string(),
            nickname: "Test User".to_string(),
            role: "user".to_string(),
        };

        // 초기 토큰 생성
        let original_tokens = service
            .create_token_pair(&user_info, None)
            .await
            .expect("Failed to create initial tokens");

        // Refresh Token으로 새 토큰 발급
        let refreshed_tokens = service
            .refresh_access_token(&original_tokens.refresh_token, None)
            .await
            .expect("Failed to refresh tokens");

        // 새 토큰이 다른지 확인
        assert_ne!(original_tokens.access_token, refreshed_tokens.access_token);
        assert_ne!(
            original_tokens.refresh_token,
            refreshed_tokens.refresh_token
        );

        // 새 Access Token이 유효한지 확인
        let claims = service
            .verify_access_token(&refreshed_tokens.access_token)
            .await
            .expect("New access token should be valid");

        assert_eq!(claims.sub, user_info.user_id);
    }

    #[tokio::test]
    async fn test_token_hash() {
        // Create a dummy service instance for testing
        // Note: In a real test, you would use a test database connection
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "mysql://test:test@localhost/test_db".to_string());

        // Try to connect, or create a dummy pool for testing
        let pool = match MySqlPool::connect(&database_url).await {
            Ok(pool) => pool,
            Err(_) => {
                // Skip test if no database is available
                println!("Skipping test - no database connection available");
                return;
            }
        };

        let service = TokenService::new(pool);
        let token = "sample.jwt.token";
        let hash1 = service.hash_token(token);
        let hash2 = service.hash_token(token);

        assert_eq!(hash1, hash2);
        assert!(hash1.len() == 64); // SHA-256 결과는 64자리 hex
    }
}
