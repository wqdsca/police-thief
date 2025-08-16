//! REST API 인증 핸들러
//! 
//! 통합 소셜 인증 서비스를 사용하는 간단한 핸들러

use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use shared::auth::{SocialAuthService, Provider};
use shared::config::redis_config::RedisConfig;
use sqlx::MySqlPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// CSRF 상태 저장소
pub type StateStore = Arc<RwLock<HashMap<String, StateData>>>;

#[derive(Clone)]
pub struct StateData {
    pub provider: Provider,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 소셜 로그인 시작 요청
#[derive(Deserialize)]
pub struct StartLoginRequest {
    pub provider: String,
}

/// 소셜 로그인 시작 응답
#[derive(Serialize)]
pub struct StartLoginResponse {
    pub auth_url: String,
    pub state: String,
}

/// 콜백 요청
#[derive(Deserialize)]
pub struct CallbackRequest {
    pub code: String,
    pub state: String,
}

/// 토큰 응답
#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// REST API 핸들러 구조체
pub struct AuthApi {
    auth_service: SocialAuthService,
    state_store: StateStore,
}

impl AuthApi {
    pub async fn new(pool: MySqlPool) -> Result<Self, Box<dyn std::error::Error>> {
        let redis_config = RedisConfig::new().await?;
        let redis = redis_config.get_connection();
        
        Ok(Self {
            auth_service: SocialAuthService::new(pool, redis),
            state_store: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}

/// 소셜 로그인 시작
pub async fn start_login(
    req: web::Json<StartLoginRequest>,
    data: web::Data<AuthApi>,
) -> Result<HttpResponse> {
    let provider = Provider::from_str(&req.provider)
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Invalid provider"))?;
    
    let state = Uuid::new_v4().to_string();
    
    // CSRF 토큰 저장
    {
        let mut store = data.state_store.write().await;
        store.insert(state.clone(), StateData {
            provider,
            created_at: chrono::Utc::now(),
        });
        
        // 5분 이상 오래된 토큰 정리
        let now = chrono::Utc::now();
        store.retain(|_, data| (now - data.created_at).num_minutes() < 5);
    }
    
    let auth_url = data.auth_service.get_auth_url(provider, &state);
    
    Ok(HttpResponse::Ok().json(StartLoginResponse {
        auth_url,
        state,
    }))
}

/// 소셜 로그인 콜백 처리
pub async fn callback(
    provider_str: web::Path<String>,
    req: web::Query<CallbackRequest>,
    data: web::Data<AuthApi>,
) -> Result<HttpResponse> {
    let provider = Provider::from_str(&provider_str)
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Invalid provider"))?;
    
    // CSRF 토큰 검증
    {
        let mut store = data.state_store.write().await;
        match store.remove(&req.state) {
            Some(state_data) => {
                if state_data.provider != provider {
                    return Ok(HttpResponse::BadRequest().json(
                        serde_json::json!({"error": "Provider mismatch"})
                    ));
                }
            }
            None => {
                return Ok(HttpResponse::BadRequest().json(
                    serde_json::json!({"error": "Invalid or expired state"})
                ));
            }
        }
    }
    
    // 통합 서비스 사용
    let tokens = data.auth_service
        .login(provider, &req.code)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    Ok(HttpResponse::Ok().json(TokenResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_type: tokens.token_type,
        expires_in: tokens.expires_in,
    }))
}

/// 라우트 설정
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/social/start", web::post().to(start_login))
            .route("/{provider}/callback", web::get().to(callback))
    );
}