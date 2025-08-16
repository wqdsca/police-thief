//! gRPC Auth Service 구현
//! Refresh Token을 포함한 완전한 인증 서비스

use sqlx::MySqlPool;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

use super::token_service::{
    ClientInfo as TokenClientInfo, TokenService, UserInfo as TokenUserInfo,
};

// Generated proto code를 include
pub mod auth {
    tonic::include_proto!("auth");
}

pub use auth::*;

/// gRPC Auth Service 구현
pub struct AuthGrpcService {
    token_service: Arc<TokenService>,
    pool: MySqlPool,
}

impl AuthGrpcService {
    pub fn new(pool: MySqlPool) -> Self {
        let token_service = Arc::new(TokenService::new(pool.clone()));

        Self {
            token_service,
            pool,
        }
    }

    /// 사용자 인증 (데이터베이스 기반)
    async fn authenticate_user(
        &self,
        username: &str,
        password: &str,
    ) -> Result<TokenUserInfo, Status> {
        // 관리자 계정 특별 처리 (개발 환경용)
        if username == "admin" && password == "admin" && cfg!(debug_assertions) {
            return Ok(TokenUserInfo {
                user_id: "1".to_string(),
                username: "admin".to_string(),
                nickname: "Administrator".to_string(),
                role: "admin".to_string(),
            });
        }

        // 데이터베이스에서 사용자 조회
        let user_record: Option<(i64, String, String, String, Option<i32>, Option<i32>, Option<i32>, Option<i32>, Option<f64>)> =
            sqlx::query_as(
                "SELECT user_id, username, nickname, password_hash, level, total_games, win_count, lose_count, win_rate
                 FROM users 
                 WHERE username = ? AND status = 'active'"
            )
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error during authentication: {}", e);
                Status::internal("Database error")
            })?;

        let (
            user_id,
            db_username,
            nickname,
            password_hash,
            _level,
            _total_games,
            _win_count,
            _lose_count,
            _win_rate,
        ) = match user_record {
            Some(user) => user,
            None => {
                warn!("Login attempt for non-existent user: {}", username);
                return Err(Status::unauthenticated("Invalid credentials"));
            }
        };

        // 패스워드 검증 (bcrypt)
        let password_valid = bcrypt::verify(password, &password_hash).map_err(|e| {
            error!("Password verification error: {}", e);
            Status::internal("Authentication error")
        })?;

        if password_valid {
            Ok(TokenUserInfo {
                user_id: user_id.to_string(),
                username: db_username,
                nickname,
                role: "user".to_string(),
            })
        } else {
            Err(Status::unauthenticated("Invalid credentials"))
        }
    }

    /// Proto ClientInfo를 Internal ClientInfo로 변환
    fn convert_client_info(&self, proto_client: Option<ClientInfo>) -> Option<TokenClientInfo> {
        proto_client.map(|client| TokenClientInfo {
            device_type: client.device_type,
            device_id: client.device_id,
            app_version: client.app_version,
            platform: client.platform,
            ip_address: client.ip_address,
            user_agent: client.user_agent,
        })
    }

    /// Internal UserInfo를 Proto UserInfo로 변환
    async fn convert_user_info(&self, user_info: &TokenUserInfo) -> Result<UserInfo, Status> {
        // 데이터베이스에서 추가 정보 조회
        let user_stats: Option<(
            Option<i32>,
            Option<i32>,
            Option<i32>,
            Option<i32>,
            Option<f64>,
            Option<chrono::DateTime<chrono::Utc>>,
        )> = sqlx::query_as(
            "SELECT level, total_games, win_count, lose_count, win_rate, last_login_at
                 FROM users 
                 WHERE user_id = ?",
        )
        .bind(user_info.user_id.parse::<i64>().unwrap_or(0))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch user stats: {}", e);
            Status::internal("Database error")
        })?;

        let (level, total_games, win_count, lose_count, win_rate, last_login_at) =
            user_stats.unwrap_or((None, None, None, None, None, None));

        Ok(UserInfo {
            user_id: user_info.user_id.clone(),
            username: user_info.username.clone(),
            nickname: user_info.nickname.clone(),
            role: user_info.role.clone(),
            level,
            total_games,
            win_count,
            lose_count,
            win_rate: win_rate.map(|rate| rate as f32),
            last_login_at: last_login_at.map(|dt| dt.timestamp()).unwrap_or(0),
        })
    }
}

#[tonic::async_trait]
impl auth_service_server::AuthService for AuthGrpcService {
    /// 로그인
    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let req = request.into_inner();

        info!("Login attempt for user: {}", req.username);

        // 사용자 인증
        let user_info = self.authenticate_user(&req.username, &req.password).await?;

        // 클라이언트 정보 변환
        let client_info = self.convert_client_info(req.client_info);

        // 토큰 쌍 생성
        let token_pair = self
            .token_service
            .create_token_pair(&user_info, client_info)
            .await
            .map_err(|e| {
                error!("Failed to create token pair: {}", e);
                Status::internal("Token creation failed")
            })?;

        // 사용자 정보 변환
        let user_info_proto = self.convert_user_info(&user_info).await?;

        // 로그인 시간 업데이트
        let _ = sqlx::query("UPDATE users SET last_login_at = NOW() WHERE user_id = ?")
            .bind(user_info.user_id.parse::<i64>().unwrap_or(0))
            .execute(&self.pool)
            .await;

        info!("Login successful for user: {}", req.username);

        Ok(Response::new(LoginResponse {
            access_token: token_pair.access_token,
            refresh_token: token_pair.refresh_token,
            token_type: token_pair.token_type,
            access_expires_at: token_pair.access_expires_at.timestamp(),
            refresh_expires_at: token_pair.refresh_expires_at.timestamp(),
            user_info: Some(user_info_proto),
        }))
    }

    /// Refresh Token으로 새 토큰 발급
    async fn refresh_token(
        &self,
        request: Request<RefreshTokenRequest>,
    ) -> Result<Response<RefreshTokenResponse>, Status> {
        let req = request.into_inner();

        info!("Refresh token request");

        // 클라이언트 정보 변환
        let client_info = self.convert_client_info(req.client_info);

        // 새 토큰 쌍 발급
        let token_pair = self
            .token_service
            .refresh_access_token(&req.refresh_token, client_info)
            .await
            .map_err(|e| {
                warn!("Failed to refresh token: {}", e);
                Status::unauthenticated("Invalid or expired refresh token")
            })?;

        info!("Token refresh successful");

        Ok(Response::new(RefreshTokenResponse {
            access_token: token_pair.access_token,
            refresh_token: token_pair.refresh_token,
            token_type: token_pair.token_type,
            access_expires_at: token_pair.access_expires_at.timestamp(),
            refresh_expires_at: token_pair.refresh_expires_at.timestamp(),
        }))
    }

    /// 토큰 검증
    async fn verify_token(
        &self,
        request: Request<VerifyTokenRequest>,
    ) -> Result<Response<VerifyTokenResponse>, Status> {
        let req = request.into_inner();

        match self
            .token_service
            .verify_access_token(&req.access_token)
            .await
        {
            Ok(claims) => {
                let user_info = TokenUserInfo {
                    user_id: claims.sub,
                    username: claims.username,
                    nickname: claims.nickname,
                    role: claims.role,
                };

                let user_info_proto = self.convert_user_info(&user_info).await?;

                Ok(Response::new(VerifyTokenResponse {
                    valid: true,
                    user_info: Some(user_info_proto),
                    error_message: None,
                    expires_at: claims.exp,
                }))
            }
            Err(e) => Ok(Response::new(VerifyTokenResponse {
                valid: false,
                user_info: None,
                error_message: Some(format!("Token validation failed: {}", e)),
                expires_at: 0,
            })),
        }
    }

    /// 로그아웃
    async fn logout(
        &self,
        request: Request<LogoutRequest>,
    ) -> Result<Response<LogoutResponse>, Status> {
        let req = request.into_inner();

        match self.token_service.revoke_token(&req.access_token).await {
            Ok(_) => {
                info!("Logout successful");
                Ok(Response::new(LogoutResponse {
                    success: true,
                    message: "Logged out successfully".to_string(),
                }))
            }
            Err(e) => {
                warn!("Logout failed: {}", e);
                Ok(Response::new(LogoutResponse {
                    success: false,
                    message: format!("Logout failed: {}", e),
                }))
            }
        }
    }

    /// 사용자 세션 목록 조회
    async fn get_user_sessions(
        &self,
        request: Request<GetUserSessionsRequest>,
    ) -> Result<Response<GetUserSessionsResponse>, Status> {
        let req = request.into_inner();

        // 먼저 토큰을 검증해서 사용자 ID 추출
        let claims = self
            .token_service
            .verify_access_token(&req.access_token)
            .await
            .map_err(|_| Status::unauthenticated("Invalid access token"))?;

        let user_id: i64 = claims
            .sub
            .parse()
            .map_err(|_| Status::internal("Invalid user ID"))?;

        // 사용자 세션 목록 조회
        let sessions = self
            .token_service
            .get_user_sessions(user_id)
            .await
            .map_err(|e| {
                error!("Failed to get user sessions: {}", e);
                Status::internal("Failed to retrieve sessions")
            })?;

        let proto_sessions: Vec<SessionInfo> = sessions
            .into_iter()
            .map(|session| {
                let client_info = session.client_info.map(|info| ClientInfo {
                    device_type: info.device_type,
                    device_id: info.device_id,
                    app_version: info.app_version,
                    platform: info.platform,
                    ip_address: info.ip_address,
                    user_agent: info.user_agent,
                });

                SessionInfo {
                    token_id: session.token_id,
                    client_info,
                    created_at: session.created_at.timestamp(),
                    expires_at: session.expires_at.timestamp(),
                    is_current: false, // Not the current token being used
                }
            })
            .collect();

        Ok(Response::new(GetUserSessionsResponse {
            sessions: proto_sessions,
        }))
    }

    /// 특정 세션 무효화
    async fn revoke_session(
        &self,
        request: Request<RevokeSessionRequest>,
    ) -> Result<Response<RevokeSessionResponse>, Status> {
        let req = request.into_inner();

        // 토큰 검증
        let claims = self
            .token_service
            .verify_access_token(&req.access_token)
            .await
            .map_err(|_| Status::unauthenticated("Invalid access token"))?;

        let user_id: i64 = claims
            .sub
            .parse()
            .map_err(|_| Status::internal("Invalid user ID"))?;

        // 특정 토큰 무효화 (보안: 자신의 토큰만 무효화 가능)
        let result = sqlx::query(
            "UPDATE user_tokens 
             SET revoked_at = NOW() 
             WHERE token_id = ? AND user_id = ?",
        )
        .bind(req.session_token_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to revoke session: {}", e);
            Status::internal("Failed to revoke session")
        })?;

        if result.rows_affected() > 0 {
            Ok(Response::new(RevokeSessionResponse {
                success: true,
                message: "Session revoked successfully".to_string(),
            }))
        } else {
            Ok(Response::new(RevokeSessionResponse {
                success: false,
                message: "Session not found or already revoked".to_string(),
            }))
        }
    }
}

/// gRPC 서버 시작
pub async fn start_auth_grpc_server(
    pool: MySqlPool,
    addr: std::net::SocketAddr,
) -> anyhow::Result<()> {
    let auth_service = AuthGrpcService::new(pool);

    info!("Starting Auth gRPC server on {}", addr);

    tonic::transport::Server::builder()
        .add_service(auth_service_server::AuthServiceServer::new(auth_service))
        .serve(addr)
        .await
        .map_err(|e| anyhow::anyhow!("Auth gRPC server error: {}", e))?;

    Ok(())
}
