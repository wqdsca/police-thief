//! 소셜 로그인 서비스 구현
//! OAuth 2.0을 사용한 카카오, 구글, 애플 로그인

use anyhow::Result;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use reqwest;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use std::collections::HashMap;

use super::token_service::{TokenService, UserInfo};

/// 소셜 로그인 제공자
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SocialProvider {
    Kakao,
    Google,
    Apple,
}

/// 소셜 로그인 사용자 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialUserInfo {
    pub provider: SocialProvider,
    pub provider_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub profile_image: Option<String>,
    pub verified_email: Option<bool>,
}

/// OAuth 설정
#[derive(Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
    pub user_info_url: String,
}

/// 소셜 인증 서비스
pub struct SocialAuthService {
    pool: MySqlPool,
    token_service: TokenService,
    kakao_config: OAuthConfig,
    google_config: OAuthConfig,
    apple_config: OAuthConfig,
}

impl SocialAuthService {
    /// 새 소셜 인증 서비스 생성
    pub fn new(pool: MySqlPool) -> Self {
        let token_service = TokenService::new(pool.clone());

        // 카카오 OAuth 설정
        let kakao_config = OAuthConfig {
            client_id: std::env::var("KAKAO_CLIENT_ID")
                .unwrap_or_else(|_| "kakao_client_id".to_string()),
            client_secret: std::env::var("KAKAO_CLIENT_SECRET").ok(),
            redirect_uri: std::env::var("KAKAO_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:8080/auth/kakao/callback".to_string()),
            auth_url: "https://kauth.kakao.com/oauth/authorize".to_string(),
            token_url: "https://kauth.kakao.com/oauth/token".to_string(),
            user_info_url: "https://kapi.kakao.com/v2/user/me".to_string(),
        };

        // 구글 OAuth 설정
        let google_config = OAuthConfig {
            client_id: std::env::var("GOOGLE_CLIENT_ID")
                .unwrap_or_else(|_| "google_client_id".to_string()),
            client_secret: std::env::var("GOOGLE_CLIENT_SECRET").ok(),
            redirect_uri: std::env::var("GOOGLE_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:8080/auth/google/callback".to_string()),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            user_info_url: "https://www.googleapis.com/oauth2/v2/userinfo".to_string(),
        };

        // 애플 OAuth 설정
        let apple_config = OAuthConfig {
            client_id: std::env::var("APPLE_CLIENT_ID")
                .unwrap_or_else(|_| "com.yourcompany.policethief".to_string()),
            client_secret: None, // Apple uses client secret JWT
            redirect_uri: std::env::var("APPLE_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:8080/auth/apple/callback".to_string()),
            auth_url: "https://appleid.apple.com/auth/authorize".to_string(),
            token_url: "https://appleid.apple.com/auth/token".to_string(),
            user_info_url: "https://appleid.apple.com/auth/keys".to_string(), // Apple doesn't have direct user info endpoint
        };

        Self {
            pool,
            token_service,
            kakao_config,
            google_config,
            apple_config,
        }
    }

    /// 인증 URL 생성
    pub fn get_auth_url(&self, provider: SocialProvider, state: &str) -> Result<String> {
        let config = match provider {
            SocialProvider::Kakao => &self.kakao_config,
            SocialProvider::Google => &self.google_config,
            SocialProvider::Apple => &self.apple_config,
        };

        let mut params = vec![
            ("client_id", config.client_id.clone()),
            ("redirect_uri", config.redirect_uri.clone()),
            ("response_type", "code".to_string()),
            ("state", state.to_string()),
        ];

        // 제공자별 추가 파라미터
        match provider {
            SocialProvider::Google => {
                params.push(("scope", "openid email profile".to_string()));
                params.push(("access_type", "offline".to_string()));
                params.push(("prompt", "consent".to_string()));
            }
            SocialProvider::Kakao => {
                params.push((
                    "scope",
                    "account_email profile_nickname profile_image".to_string(),
                ));
            }
            SocialProvider::Apple => {
                params.push(("scope", "name email".to_string()));
                params.push(("response_mode", "form_post".to_string()));
            }
        }

        let query_string: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect();

        Ok(format!("{}?{}", config.auth_url, query_string.join("&")))
    }

    /// Authorization Code를 Access Token으로 교환
    pub async fn exchange_code_for_token(
        &self,
        provider: SocialProvider,
        code: &str,
    ) -> Result<String> {
        let config = match provider {
            SocialProvider::Kakao => &self.kakao_config,
            SocialProvider::Google => &self.google_config,
            SocialProvider::Apple => &self.apple_config,
        };

        let client = reqwest::Client::new();

        let mut params = HashMap::new();
        params.insert("grant_type", "authorization_code");
        params.insert("code", code);
        params.insert("redirect_uri", &config.redirect_uri);
        params.insert("client_id", &config.client_id);

        // 제공자별 처리
        let response = match provider {
            SocialProvider::Kakao | SocialProvider::Google => {
                if let Some(secret) = &config.client_secret {
                    params.insert("client_secret", secret);
                }

                client.post(&config.token_url).form(&params).send().await?
            }
            SocialProvider::Apple => {
                // Apple은 client_secret으로 JWT를 생성해야 함
                let client_secret = self.generate_apple_client_secret()?;
                params.insert("client_secret", &client_secret);

                client.post(&config.token_url).form(&params).send().await?
            }
        };

        let token_response: TokenResponseData = response.json().await?;
        Ok(token_response.access_token)
    }

    /// Access Token으로 사용자 정보 가져오기
    pub async fn get_user_info(
        &self,
        provider: SocialProvider,
        access_token: &str,
    ) -> Result<SocialUserInfo> {
        let config = match provider {
            SocialProvider::Kakao => &self.kakao_config,
            SocialProvider::Google => &self.google_config,
            SocialProvider::Apple => &self.apple_config,
        };

        match provider {
            SocialProvider::Kakao => {
                let client = reqwest::Client::new();
                let response = client
                    .get(&config.user_info_url)
                    .header("Authorization", format!("Bearer {}", access_token))
                    .send()
                    .await?;

                let kakao_user: KakaoUserInfo = response.json().await?;

                Ok(SocialUserInfo {
                    provider: SocialProvider::Kakao,
                    provider_id: kakao_user.id.to_string(),
                    email: kakao_user.kakao_account.email,
                    name: kakao_user.properties.nickname,
                    profile_image: kakao_user.properties.profile_image,
                    verified_email: kakao_user.kakao_account.email_verified,
                })
            }
            SocialProvider::Google => {
                let client = reqwest::Client::new();
                let response = client
                    .get(&config.user_info_url)
                    .header("Authorization", format!("Bearer {}", access_token))
                    .send()
                    .await?;

                let google_user: GoogleUserInfo = response.json().await?;

                Ok(SocialUserInfo {
                    provider: SocialProvider::Google,
                    provider_id: google_user.id,
                    email: Some(google_user.email),
                    name: Some(google_user.name),
                    profile_image: Some(google_user.picture),
                    verified_email: Some(google_user.verified_email),
                })
            }
            SocialProvider::Apple => {
                // Apple은 ID Token에서 정보를 추출
                let claims = self.decode_apple_id_token(access_token)?;

                Ok(SocialUserInfo {
                    provider: SocialProvider::Apple,
                    provider_id: claims.sub,
                    email: claims.email,
                    name: None, // Apple은 첫 로그인 시에만 이름 제공
                    profile_image: None,
                    verified_email: claims.email_verified,
                })
            }
        }
    }

    /// 소셜 로그인 처리 (신규 가입 또는 로그인)
    pub async fn social_login(
        &self,
        provider: SocialProvider,
        code: &str,
    ) -> Result<super::token_service::TokenPair> {
        // 1. Authorization Code를 Access Token으로 교환
        let access_token = self.exchange_code_for_token(provider, code).await?;

        // 2. Access Token으로 사용자 정보 가져오기
        let social_user_info = self.get_user_info(provider, &access_token).await?;

        // 3. 데이터베이스에서 사용자 확인 또는 생성
        let user_info = self.get_or_create_user(social_user_info).await?;

        // 4. JWT 토큰 생성
        let token_pair = self
            .token_service
            .create_token_pair(&user_info, None)
            .await?;

        Ok(token_pair)
    }

    /// 사용자 확인 또는 생성
    async fn get_or_create_user(&self, social_info: SocialUserInfo) -> Result<UserInfo> {
        // 먼저 소셜 계정으로 연결된 사용자 찾기
        let existing_user: Option<(i64, String, String)> = sqlx::query_as(
            "SELECT u.user_id, u.username, u.nickname 
             FROM users u 
             JOIN social_accounts sa ON u.user_id = sa.user_id 
             WHERE sa.provider = ? AND sa.provider_id = ?",
        )
        .bind(format!("{:?}", social_info.provider))
        .bind(&social_info.provider_id)
        .fetch_optional(&self.pool)
        .await?;

        let user_info = if let Some((user_id, username, nickname)) = existing_user {
            // 기존 사용자
            UserInfo {
                user_id: user_id.to_string(),
                username,
                nickname,
                role: "user".to_string(),
            }
        } else {
            // 신규 사용자 생성
            let username = social_info.email.clone().unwrap_or_else(|| {
                format!(
                    "{}_{}",
                    format!("{:?}", social_info.provider).to_lowercase(),
                    social_info.provider_id
                )
            });
            let nickname = social_info
                .name
                .clone()
                .unwrap_or_else(|| format!("User{}", rand::random::<u32>()));

            // 트랜잭션으로 사용자와 소셜 계정 정보 생성
            let mut tx = self.pool.begin().await?;

            // 사용자 생성
            let result = sqlx::query(
                "INSERT INTO users (username, nickname, password_hash, status) 
                 VALUES (?, ?, ?, 'active')",
            )
            .bind(&username)
            .bind(&nickname)
            .bind("SOCIAL_LOGIN") // 소셜 로그인 사용자는 패스워드 없음
            .execute(&mut *tx)
            .await?;

            let user_id = result.last_insert_id() as i64;

            // 소셜 계정 정보 저장
            sqlx::query(
                "INSERT INTO social_accounts (user_id, provider, provider_id, email, profile_image) 
                 VALUES (?, ?, ?, ?, ?)"
            )
            .bind(user_id)
            .bind(format!("{:?}", social_info.provider))
            .bind(&social_info.provider_id)
            .bind(&social_info.email)
            .bind(&social_info.profile_image)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;

            UserInfo {
                user_id: user_id.to_string(),
                username,
                nickname,
                role: "user".to_string(),
            }
        };

        // 마지막 로그인 시간 업데이트
        sqlx::query("UPDATE users SET last_login_at = NOW() WHERE user_id = ?")
            .bind(user_info.user_id.parse::<i64>()?)
            .execute(&self.pool)
            .await?;

        Ok(user_info)
    }

    /// Apple Client Secret JWT 생성
    fn generate_apple_client_secret(&self) -> Result<String> {
        use chrono::{Duration, Utc};
        use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

        #[derive(Debug, Serialize)]
        struct AppleClaims {
            iss: String,
            sub: String,
            aud: String,
            iat: i64,
            exp: i64,
        }

        let team_id = std::env::var("APPLE_TEAM_ID")?;
        let client_id = std::env::var("APPLE_CLIENT_ID")?;
        let key_id = std::env::var("APPLE_KEY_ID")?;
        let private_key_path = std::env::var("APPLE_PRIVATE_KEY_PATH")?;

        let private_key = std::fs::read_to_string(private_key_path)?;

        let now = Utc::now();
        let claims = AppleClaims {
            iss: team_id,
            sub: client_id,
            aud: "https://appleid.apple.com".to_string(),
            iat: now.timestamp(),
            exp: (now + Duration::minutes(10)).timestamp(),
        };

        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(key_id);

        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_ec_pem(private_key.as_bytes())?,
        )?;

        Ok(token)
    }

    /// Apple ID Token 디코딩
    fn decode_apple_id_token(&self, id_token: &str) -> Result<AppleIdTokenClaims> {
        // Apple의 공개 키로 검증하는 로직 (실제로는 Apple의 JWKS endpoint에서 키를 가져와야 함)
        // 여기서는 간단히 디코딩만 수행
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow::anyhow!("Invalid ID token format"));
        }

        let payload = parts[1];
        let decoded = URL_SAFE_NO_PAD.decode(payload)?;
        let claims: AppleIdTokenClaims = serde_json::from_slice(&decoded)?;

        Ok(claims)
    }
}

// Response 구조체들
#[derive(Debug, Deserialize)]
struct TokenResponseData {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    expires_in: Option<i64>,
    #[allow(dead_code)]
    refresh_token: Option<String>,
    #[allow(dead_code)]
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KakaoUserInfo {
    id: i64,
    properties: KakaoProperties,
    kakao_account: KakaoAccount,
}

#[derive(Debug, Deserialize)]
struct KakaoProperties {
    nickname: Option<String>,
    profile_image: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KakaoAccount {
    email: Option<String>,
    email_verified: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    id: String,
    email: String,
    verified_email: bool,
    name: String,
    picture: String,
}

#[derive(Debug, Deserialize)]
struct AppleIdTokenClaims {
    sub: String,
    email: Option<String>,
    email_verified: Option<bool>,
}

// URL encoding helper
mod urlencoding {
    pub fn encode(s: &str) -> String {
        percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
    }
}
