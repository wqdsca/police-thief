//! 통합 소셜 인증 서비스
//! 
//! 모든 OAuth 로직을 하나로 통합하여 중복 제거

use super::token::TokenService;
use super::types::*;
use crate::tool::error::AppError;
use anyhow::Result;
use redis::aio::ConnectionManager;
use reqwest;
use sqlx::MySqlPool;
use std::collections::HashMap;
use std::env;
use tracing::{info, warn};

/// 통합 소셜 인증 서비스
/// 
/// Arc<dyn Trait> 없이 구체 타입 직접 사용
pub struct SocialAuthService {
    pool: MySqlPool,
    redis: ConnectionManager,
    token_service: TokenService,
    google_config: OAuthConfig,
    kakao_config: OAuthConfig,
    apple_config: OAuthConfig,
}

impl SocialAuthService {
    /// 서비스 생성
    pub fn new(pool: MySqlPool, redis: ConnectionManager) -> Self {
        let token_service = TokenService::new(pool.clone());

        Self {
            pool,
            redis,
            token_service,
            google_config: Self::google_config(),
            kakao_config: Self::kakao_config(),
            apple_config: Self::apple_config(),
        }
    }

    /// 구글 OAuth 설정
    fn google_config() -> OAuthConfig {
        OAuthConfig {
            client_id: env::var("GOOGLE_CLIENT_ID")
                .unwrap_or_else(|_| "google_client_id".into()),
            client_secret: env::var("GOOGLE_CLIENT_SECRET")
                .unwrap_or_else(|_| "google_secret".into()),
            redirect_uri: env::var("GOOGLE_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:8080/auth/google/callback".into()),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
            token_url: "https://oauth2.googleapis.com/token",
            user_info_url: "https://www.googleapis.com/oauth2/v2/userinfo",
        }
    }

    /// 카카오 OAuth 설정
    fn kakao_config() -> OAuthConfig {
        OAuthConfig {
            client_id: env::var("KAKAO_CLIENT_ID")
                .unwrap_or_else(|_| "kakao_client_id".into()),
            client_secret: env::var("KAKAO_CLIENT_SECRET")
                .unwrap_or_else(|_| "kakao_secret".into()),
            redirect_uri: env::var("KAKAO_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:8080/auth/kakao/callback".into()),
            auth_url: "https://kauth.kakao.com/oauth/authorize",
            token_url: "https://kauth.kakao.com/oauth/token",
            user_info_url: "https://kapi.kakao.com/v2/user/me",
        }
    }

    /// 애플 OAuth 설정
    fn apple_config() -> OAuthConfig {
        OAuthConfig {
            client_id: env::var("APPLE_CLIENT_ID")
                .unwrap_or_else(|_| "com.yourcompany.policethief".into()),
            client_secret: String::new(), // Apple은 JWT 사용
            redirect_uri: env::var("APPLE_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:8080/auth/apple/callback".into()),
            auth_url: "https://appleid.apple.com/auth/authorize",
            token_url: "https://appleid.apple.com/auth/token",
            user_info_url: "https://appleid.apple.com/auth/keys",
        }
    }

    /// Step 1: 인증 URL 생성
    pub fn get_auth_url(&self, provider: Provider, state: &str) -> String {
        let config = match provider {
            Provider::Google => &self.google_config,
            Provider::Kakao => &self.kakao_config,
            Provider::Apple => &self.apple_config,
        };

        let mut params = vec![
            ("client_id", config.client_id.clone()),
            ("redirect_uri", config.redirect_uri.clone()),
            ("response_type", "code".into()),
            ("state", state.into()),
        ];

        // 제공자별 추가 파라미터
        match provider {
            Provider::Google => {
                params.push(("scope", "openid email profile".into()));
                params.push(("access_type", "offline".into()));
            }
            Provider::Kakao => {
                params.push(("scope", "account_email profile_nickname profile_image".into()));
            }
            Provider::Apple => {
                params.push(("scope", "name email".into()));
                params.push(("response_mode", "form_post".into()));
            }
        }

        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        format!("{}?{}", config.auth_url, query)
    }

    /// Step 2: Authorization Code를 토큰으로 교환
    pub async fn exchange_code(&self, provider: Provider, code: &str) -> Result<String> {
        let config = match provider {
            Provider::Google => &self.google_config,
            Provider::Kakao => &self.kakao_config,
            Provider::Apple => &self.apple_config,
        };

        let client = reqwest::Client::new();
        let mut params = HashMap::new();
        
        params.insert("grant_type", "authorization_code");
        params.insert("code", code);
        params.insert("redirect_uri", &config.redirect_uri);
        params.insert("client_id", &config.client_id);
        params.insert("client_secret", &config.client_secret);

        let response = client
            .post(config.token_url)
            .form(&params)
            .send()
            .await?;

        let token_response: OAuthTokenResponse = response.json().await?;
        Ok(token_response.access_token)
    }

    /// Step 3: 사용자 정보 가져오기
    pub async fn get_user_info(
        &self,
        provider: Provider,
        access_token: &str,
    ) -> Result<UserInfo> {
        let config = match provider {
            Provider::Google => &self.google_config,
            Provider::Kakao => &self.kakao_config,
            Provider::Apple => &self.apple_config,
        };

        let client = reqwest::Client::new();
        let response = client
            .get(config.user_info_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        match provider {
            Provider::Google => {
                let google_user: GoogleUserInfo = response.json().await?;
                Ok(UserInfo {
                    user_id: 0, // DB에서 할당
                    email: google_user.email,
                    nickname: google_user.name,
                    provider,
                    provider_id: google_user.id,
                    profile_image: Some(google_user.picture),
                })
            }
            Provider::Kakao => {
                let kakao_user: KakaoUserInfo = response.json().await?;
                Ok(UserInfo {
                    user_id: 0,
                    email: kakao_user.kakao_account.email.unwrap_or_default(),
                    nickname: kakao_user.properties.nickname.unwrap_or_else(|| "User".into()),
                    provider,
                    provider_id: kakao_user.id.to_string(),
                    profile_image: kakao_user.properties.profile_image,
                })
            }
            Provider::Apple => {
                // Apple은 ID 토큰에서 정보 추출
                // For now, return error indicating Apple ID is not yet supported
                return Err(anyhow::anyhow!("Apple ID authentication is not yet implemented"));
        }
    }

    /// Step 4: 완전한 로그인 플로우
    pub async fn login(&self, provider: Provider, code: &str) -> Result<TokenPair> {
        // 1. Code를 Access Token으로 교환
        let access_token = self.exchange_code(provider, code).await?;
        
        // 2. 사용자 정보 가져오기
        let mut user_info = self.get_user_info(provider, &access_token).await?;
        
        // 3. DB에서 사용자 확인 또는 생성
        user_info.user_id = self.get_or_create_user(&user_info).await?;
        
        // 4. Redis에 세션 저장
        self.save_session(&user_info).await?;
        
        // 5. JWT 토큰 생성
        let tokens = self.token_service.create_tokens(&user_info)?;
        
        info!("소셜 로그인 성공: user_id={}, provider={:?}", user_info.user_id, provider);
        
        Ok(tokens)
    }

    /// 사용자 확인 또는 생성
    async fn get_or_create_user(&self, user_info: &UserInfo) -> Result<i64> {
        // 기존 사용자 확인
        let existing = sqlx::query!(
            "SELECT user_id FROM social_accounts WHERE provider = ? AND provider_id = ?",
            format!("{:?}", user_info.provider),
            user_info.provider_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(record) = existing {
            // 마지막 로그인 시간 업데이트
            sqlx::query!(
                "UPDATE users SET last_login_at = NOW() WHERE user_id = ?",
                record.user_id
            )
            .execute(&self.pool)
            .await?;
            
            return Ok(record.user_id);
        }

        // 신규 사용자 생성
        let mut tx = self.pool.begin().await?;
        
        let result = sqlx::query!(
            "INSERT INTO users (username, nickname, password_hash, status) VALUES (?, ?, 'SOCIAL', 'active')",
            user_info.email,
            user_info.nickname
        )
        .execute(&mut *tx)
        .await?;
        
        let user_id = result.last_insert_id() as i64;
        
        // 소셜 계정 연결
        sqlx::query!(
            "INSERT INTO social_accounts (user_id, provider, provider_id, email, profile_image) 
             VALUES (?, ?, ?, ?, ?)",
            user_id,
            format!("{:?}", user_info.provider),
            user_info.provider_id,
            user_info.email,
            user_info.profile_image
        )
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        info!("신규 사용자 생성: user_id={}", user_id);
        Ok(user_id)
    }

    /// Redis에 세션 저장
    async fn save_session(&self, user_info: &UserInfo) -> Result<()> {
        use redis::AsyncCommands;
        
        let key = format!("session:{}", user_info.user_id);
        let value = serde_json::to_string(user_info)?;
        
        let mut conn = self.redis.clone();
        conn.setex(key, value, 3600).await?;
        
        Ok(())
    }
}

// URL 인코딩 헬퍼
mod urlencoding {
    pub fn encode(s: &str) -> String {
        percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
    }
}