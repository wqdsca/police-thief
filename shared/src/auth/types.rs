//! 인증 관련 공통 타입 정의

use serde::{Deserialize, Serialize};

/// OAuth 제공자
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Google,
    Kakao,
    Apple,
}

impl Provider {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "google" => Some(Provider::Google),
            "kakao" => Some(Provider::Kakao),
            "apple" => Some(Provider::Apple),
            _ => None,
        }
    }
}

/// JWT 토큰 쌍
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// 사용자 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub user_id: i64,
    pub_nick_name:String,
}

/// OAuth 설정
#[derive(Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub user_info_url: &'static str,
}

/// OAuth 토큰 응답
#[derive(Debug, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

/// 구글 사용자 정보
#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub id: String,
    pub email: String,
    pub verified_email: bool,
    pub name: String,
    pub picture: String,
}

/// 카카오 사용자 정보
#[derive(Debug, Deserialize)]
pub struct KakaoUserInfo {
    pub id: i64,
    pub properties: KakaoProperties,
    pub kakao_account: KakaoAccount,
}

#[derive(Debug, Deserialize)]
pub struct KakaoProperties {
    pub nickname: Option<String>,
    pub profile_image: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct KakaoAccount {
    pub email: Option<String>,
    pub email_verified: Option<bool>,
}

/// 애플 ID 토큰 클레임
#[derive(Debug, Deserialize)]
pub struct AppleIdTokenClaims {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
}