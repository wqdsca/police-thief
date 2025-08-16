//! 소셜 로그인 REST API 핸들러

use crate::service::{SocialAuthService, SocialProvider};
use actix_web::{web, Error, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// OAuth 상태 저장 (CSRF 방지)
pub type StateStore = Arc<RwLock<HashMap<String, StateData>>>;

#[derive(Clone)]
pub struct StateData {
    pub provider: SocialProvider,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 소셜 로그인 시작 요청
#[derive(Debug, Deserialize, Serialize)]
pub struct SocialLoginRequest {
    pub provider: String,
}

/// 소셜 로그인 콜백 요청
#[derive(Debug, Deserialize)]
pub struct CallbackRequest {
    pub code: String,
    pub state: String,
}

/// 소셜 로그인 응답
#[derive(Debug, Serialize)]
pub struct SocialLoginResponse {
    pub auth_url: String,
    pub state: String,
}

/// 토큰 응답
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// 소셜 로그인 시작 핸들러
pub async fn social_login_start(
    req: web::Json<SocialLoginRequest>,
    pool: web::Data<MySqlPool>,
    state_store: web::Data<StateStore>,
) -> Result<HttpResponse, Error> {
    let provider = match req.provider.to_lowercase().as_str() {
        "kakao" => SocialProvider::Kakao,
        "google" => SocialProvider::Google,
        "apple" => SocialProvider::Apple,
        _ => {
            return Ok(
                HttpResponse::BadRequest().json(serde_json::json!({ "error": "Invalid provider" }))
            );
        }
    };

    let auth_service = SocialAuthService::new((**pool).clone());

    // CSRF 방지를 위한 state 생성
    let state = Uuid::new_v4().to_string();

    // state 저장 (5분 후 만료)
    {
        let mut store = state_store.write().await;
        store.insert(
            state.clone(),
            StateData {
                provider,
                created_at: chrono::Utc::now(),
            },
        );

        // 오래된 state 정리 (5분 이상)
        let now = chrono::Utc::now();
        store.retain(|_, data| (now - data.created_at).num_minutes() < 5);
    }

    // 인증 URL 생성
    let auth_url = auth_service
        .get_auth_url(provider, &state)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    Ok(HttpResponse::Ok().json(SocialLoginResponse { auth_url, state }))
}

/// 카카오 로그인 콜백 핸들러
pub async fn kakao_callback(
    req: web::Query<CallbackRequest>,
    pool: web::Data<MySqlPool>,
    state_store: web::Data<StateStore>,
) -> Result<HttpResponse, Error> {
    handle_callback(SocialProvider::Kakao, req.into_inner(), pool, state_store).await
}

/// 구글 로그인 콜백 핸들러
pub async fn google_callback(
    req: web::Query<CallbackRequest>,
    pool: web::Data<MySqlPool>,
    state_store: web::Data<StateStore>,
) -> Result<HttpResponse, Error> {
    handle_callback(SocialProvider::Google, req.into_inner(), pool, state_store).await
}

/// 애플 로그인 콜백 핸들러
pub async fn apple_callback(
    req: web::Form<CallbackRequest>, // Apple은 POST로 전송
    pool: web::Data<MySqlPool>,
    state_store: web::Data<StateStore>,
) -> Result<HttpResponse, Error> {
    handle_callback(SocialProvider::Apple, req.into_inner(), pool, state_store).await
}

/// 공통 콜백 처리
async fn handle_callback(
    provider: SocialProvider,
    req: CallbackRequest,
    pool: web::Data<MySqlPool>,
    state_store: web::Data<StateStore>,
) -> Result<HttpResponse, Error> {
    // state 검증
    {
        let mut store = state_store.write().await;
        match store.remove(&req.state) {
            Some(data) => {
                if data.provider != provider {
                    return Ok(HttpResponse::BadRequest()
                        .json(serde_json::json!({ "error": "Provider mismatch" })));
                }
            }
            None => {
                return Ok(HttpResponse::BadRequest()
                    .json(serde_json::json!({ "error": "Invalid or expired state" })));
            }
        }
    }

    let auth_service = SocialAuthService::new((**pool).clone());

    // 소셜 로그인 처리
    let token_pair = auth_service
        .social_login(provider, &req.code)
        .await
        .map_err(|e| {
            tracing::error!("Social login failed: {}", e);
            actix_web::error::ErrorInternalServerError("Login failed")
        })?;

    // 토큰 응답
    Ok(HttpResponse::Ok().json(TokenResponse {
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        token_type: token_pair.token_type,
        expires_in: (token_pair.access_expires_at - chrono::Utc::now()).num_seconds(),
    }))
}

/// 소셜 로그인 라우트 설정
pub fn configure_social_auth_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/social/login", web::post().to(social_login_start))
            .route("/kakao/callback", web::get().to(kakao_callback))
            .route("/google/callback", web::get().to(google_callback))
            .route("/apple/callback", web::post().to(apple_callback)),
    );
}
