//! JWT 토큰 서비스

use super::types::{TokenPair, UserInfo};
use anyhow::Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use std::env;

/// JWT 클레임
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,      // user_id
    pub email: String,
    pub nickname: String,
    pub exp: i64,         // 만료 시간
    pub iat: i64,         // 발급 시간
}

/// 토큰 서비스
pub struct TokenService {
    pool: MySqlPool,
    secret: String,
}

impl TokenService {
    pub fn new(pool: MySqlPool) -> Self {
        let secret = env::var("JWT_SECRET")
            .unwrap_or_else(|_| "your-secret-key-min-256-bits".into());
        
        Self { pool, secret }
    }

    /// JWT 토큰 쌍 생성
    pub fn create_tokens(&self, user_info: &UserInfo) -> Result<TokenPair> {
        let now = Utc::now();
        
        // Access Token (1시간)
        let access_claims = Claims {
            sub: user_info.user_id.to_string(),
            email: user_info.email.clone(),
            nickname: user_info.nickname.clone(),
            exp: (now + Duration::hours(1)).timestamp(),
            iat: now.timestamp(),
        };
        
        let access_token = encode(
            &Header::default(),
            &access_claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )?;
        
        // Refresh Token (30일)
        let refresh_claims = Claims {
            sub: user_info.user_id.to_string(),
            email: user_info.email.clone(),
            nickname: user_info.nickname.clone(),
            exp: (now + Duration::days(30)).timestamp(),
            iat: now.timestamp(),
        };
        
        let refresh_token = encode(
            &Header::default(),
            &refresh_claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )?;
        
        Ok(TokenPair {
            access_token,
            refresh_token,
            token_type: "Bearer".into(),
            expires_in: 3600,
        })
    }

    /// 토큰 검증
    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )?;
        
        Ok(token_data.claims)
    }

    /// 토큰 갱신
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenPair> {
        let claims = self.verify_token(refresh_token)?;
        
        // DB에서 사용자 정보 조회
        let user_info = sqlx::query_as!(
            UserInfo,
            "SELECT u.user_id, u.username as email, u.nickname, 
                    sa.provider, sa.provider_id, sa.profile_image
             FROM users u
             JOIN social_accounts sa ON u.user_id = sa.user_id
             WHERE u.user_id = ?",
            claims.sub.parse::<i64>()?
        )
        .fetch_one(&self.pool)
        .await?;
        
        self.create_tokens(&user_info)
    }
}