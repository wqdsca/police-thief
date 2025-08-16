//! 관리자 API 인증 미들웨어

use crate::service::{ClientInfo, TokenService, UserInfo};
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorUnauthorized,
    web, Error, HttpMessage,
};
use chrono::{DateTime, Utc};
use futures::future::LocalBoxFuture;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use std::future::{ready, Ready};
use std::rc::Rc;

// Claims는 service::token_service에서 import

/// 인증 설정
#[derive(Clone)]
pub struct AuthConfig {
    pub token_service: TokenService,
    pub require_admin: bool,
}

impl AuthConfig {
    pub fn new(pool: MySqlPool) -> Self {
        Self {
            token_service: TokenService::new(pool),
            require_admin: true,
        }
    }
}

/// 인증 미들웨어
pub struct AuthMiddleware {
    config: AuthConfig,
}

impl AuthMiddleware {
    pub fn new(config: AuthConfig) -> Self {
        Self { config }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService {
            service: Rc::new(service),
            config: self.config.clone(),
        }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: Rc<S>,
    config: AuthConfig,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Config is already available via self.config in the method

        // 특정 경로는 인증 없이 허용 (예: 헬스체크)
        let path = req.path();
        if path == "/health" || path == "/api/admin/login" {
            let fut = self.service.call(req);
            return Box::pin(async move { fut.await });
        }

        // Authorization 헤더에서 토큰 추출
        let auth_header = req.headers().get("Authorization");

        let token = match auth_header {
            Some(header_value) => match header_value.to_str() {
                Ok(auth_str) => {
                    if auth_str.starts_with("Bearer ") {
                        &auth_str[7..]
                    } else {
                        return Box::pin(async move {
                            Err(ErrorUnauthorized("Invalid authorization header format"))
                        });
                    }
                }
                Err(_) => {
                    return Box::pin(async move {
                        Err(ErrorUnauthorized("Invalid authorization header"))
                    });
                }
            },
            None => {
                return Box::pin(
                    async move { Err(ErrorUnauthorized("Missing authorization header")) },
                );
            }
        };

        // 토큰 검증 (비동기)
        let token_service = self.config.token_service.clone();
        let require_admin = self.config.require_admin;
        let token_str = token.to_string();
        let service = self.service.clone(); // Clone the Rc service

        Box::pin(async move {
            match token_service.verify_access_token(&token_str).await {
                Ok(claims) => {
                    // 관리자 권한 확인
                    if require_admin && claims.role != "admin" {
                        return Err(ErrorUnauthorized("Admin access required"));
                    }

                    // 클레임을 요청 확장에 저장
                    req.extensions_mut().insert(claims);

                    service.call(req).await
                }
                Err(_) => Err(ErrorUnauthorized("Invalid or expired token")),
            }
        })
    }
}

/// 로그인 요청
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// 로그인 응답 (토큰 쌍 포함)
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
    pub user_info: UserLoginInfo,
}

/// 사용자 로그인 정보
#[derive(Debug, Serialize)]
pub struct UserLoginInfo {
    pub user_id: String,
    pub username: String,
    pub nickname: String,
    pub level: Option<i32>,
    pub total_games: Option<i32>,
    pub win_rate: Option<f32>,
}

/// 토큰 새로고침 요청
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

/// 토큰 새로고침 응답
#[derive(Debug, Serialize)]
pub struct RefreshTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
}

/// 로그인 핸들러
pub async fn login_handler(
    req: web::Json<LoginRequest>,
    token_service: web::Data<TokenService>,
    http_req: actix_web::HttpRequest,
) -> Result<actix_web::HttpResponse, Error> {
    // 사용자 인증 (개발 환경에서만 admin/admin 허용)
    if req.username == "admin" && req.password == "admin" && cfg!(debug_assertions) {
        let user_info = UserInfo {
            user_id: "1".to_string(),
            username: "admin".to_string(),
            nickname: "Administrator".to_string(),
            role: "admin".to_string(),
        };

        // 클라이언트 정보 수집 (HTTP 헤더에서)
        let client_info = Some(ClientInfo {
            device_type: None,
            device_id: None,
            app_version: None,
            platform: None,
            ip_address: http_req.peer_addr().map(|addr| addr.ip().to_string()),
            user_agent: http_req
                .headers()
                .get("user-agent")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string()),
        });

        match token_service
            .create_token_pair(&user_info, client_info)
            .await
        {
            Ok(token_pair) => {
                Ok(actix_web::HttpResponse::Ok().json(LoginResponse {
                    access_token: token_pair.access_token,
                    refresh_token: token_pair.refresh_token,
                    token_type: token_pair.token_type,
                    access_expires_at: token_pair.access_expires_at,
                    refresh_expires_at: token_pair.refresh_expires_at,
                    user_info: UserLoginInfo {
                        user_id: user_info.user_id,
                        username: user_info.username,
                        nickname: user_info.nickname,
                        level: Some(1), // 테스트 데이터
                        total_games: Some(0),
                        win_rate: Some(0.0),
                    },
                }))
            }
            Err(e) => Err(ErrorUnauthorized(format!("Token creation failed: {}", e))),
        }
    } else {
        Err(ErrorUnauthorized("Invalid credentials"))
    }
}

/// 토큰 새로고침 핸들러
pub async fn refresh_token_handler(
    req: web::Json<RefreshTokenRequest>,
    token_service: web::Data<TokenService>,
    http_req: actix_web::HttpRequest,
) -> Result<actix_web::HttpResponse, Error> {
    // 클라이언트 정보 수집
    let client_info = Some(ClientInfo {
        device_type: None,
        device_id: None,
        app_version: None,
        platform: None,
        ip_address: http_req.peer_addr().map(|addr| addr.ip().to_string()),
        user_agent: http_req
            .headers()
            .get("user-agent")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string()),
    });

    match token_service
        .refresh_access_token(&req.refresh_token, client_info)
        .await
    {
        Ok(token_pair) => Ok(actix_web::HttpResponse::Ok().json(RefreshTokenResponse {
            access_token: token_pair.access_token,
            refresh_token: token_pair.refresh_token,
            token_type: token_pair.token_type,
            access_expires_at: token_pair.access_expires_at,
            refresh_expires_at: token_pair.refresh_expires_at,
        })),
        Err(e) => Err(ErrorUnauthorized(format!("Token refresh failed: {}", e))),
    }
}

/// 로그아웃 핸들러
pub async fn logout_handler(
    req: actix_web::HttpRequest,
    token_service: web::Data<TokenService>,
) -> Result<actix_web::HttpResponse, Error> {
    // Authorization 헤더에서 토큰 추출
    let auth_header = req.headers().get("Authorization");

    if let Some(header_value) = auth_header {
        if let Ok(auth_str) = header_value.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];

                match token_service.revoke_token(token).await {
                    Ok(_) => {
                        return Ok(actix_web::HttpResponse::Ok()
                            .json(serde_json::json!({ "message": "Logged out successfully" })));
                    }
                    Err(e) => {
                        tracing::error!("Failed to revoke token: {}", e);
                    }
                }
            }
        }
    }

    Ok(actix_web::HttpResponse::Ok().json(serde_json::json!({ "message": "Logged out" })))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 테스트는 service::token_service에서 처리
}
