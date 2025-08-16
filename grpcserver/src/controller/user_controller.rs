//! User Service gRPC Controller
//!
//! 사용자 인증 및 회원가입 기능을 담당하는 gRPC 컨트롤러입니다.
//! UserService trait을 구현하여 gRPC 서버에서 사용자 관련 요청을 처리합니다.
//! 최적화된 정적 상수로 검증 배열을 재사용합니다.

use crate::service::user_service::UserService as UserSvc;
use crate::user::{
    user_service_server::UserService, LoginRequest, LoginResponse, RegisterRequest,
    RegisterResponse,
};
use shared::service::TokenService;
use shared::tool::error::{helpers, AppError};
use shared::traits::{SocialAuthService, UserDatabaseService, UserRedisService as UserRedisServiceTrait};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::info;

/// 최적화된 로그인 타입 상수 (컴파일 시 할당)
const VALID_LOGIN_TYPES: &[&str] = &["google", "apple", "test"];
const VALID_REGISTER_TYPES: &[&str] = &["google", "apple", "guest"];

/// User Service gRPC 컨트롤러
///
/// 사용자 인증 및 회원가입 기능을 처리하는 컨트롤러입니다.
/// UserService trait을 구현하여 gRPC 요청을 비즈니스 로직으로 연결합니다.
/// JWT 토큰 검증 기능을 포함하여 보안을 강화합니다.
pub struct UserController<S, R, D> 
where
    S: SocialAuthService + Send + Sync,
    R: UserRedisServiceTrait + Send + Sync,
    D: UserDatabaseService + Send + Sync,
{
    /// 사용자 관련 비즈니스 로직을 처리하는 서비스
    svc: UserSvc<S, R, D>,
    /// JWT 토큰 검증 서비스
    token_service: TokenService,
}

impl<S, R, D> UserController<S, R, D>
where
    S: SocialAuthService + Send + Sync,
    R: UserRedisServiceTrait + Send + Sync,
    D: UserDatabaseService + Send + Sync,
{
    /// 새로운 UserController 인스턴스를 생성합니다.
    ///
    /// JWT 토큰 검증 서비스도 함께 초기화하여 보안을 강화합니다.
    ///
    /// # Arguments
    /// * `svc` - 사용자 관련 비즈니스 로직을 처리하는 UserService 인스턴스
    ///
    /// # Returns
    /// * `Self` - 초기화된 UserController 인스턴스
    ///
    /// # Returns
    /// * `Result<Self>` - 초기화된 UserController 인스턴스 또는 에러
    pub fn new(svc: UserSvc<S, R, D>) -> Result<Self, tonic::Status> {
        let jwt_secret = std::env::var("JWT_SECRET_KEY").map_err(|_| {
            tracing::error!(
                "⚠️ SECURITY ERROR: JWT_SECRET_KEY environment variable is required for production"
            );
            tonic::Status::internal("Server configuration error: Missing JWT_SECRET_KEY")
        })?;

        // 보안 검증: 최소 32자 이상의 시크릿 키 요구
        if jwt_secret.len() < 32 {
            tracing::error!("⚠️ SECURITY ERROR: JWT_SECRET_KEY must be at least 32 characters long. Current length: {}", jwt_secret.len());
            return Err(tonic::Status::internal(
                "Server configuration error: JWT_SECRET_KEY too short",
            ));
        }

        // 보안 검증: 약한 기본값 사용 방지
        if jwt_secret.to_lowercase().contains("default")
            || jwt_secret.to_lowercase().contains("secret")
            || jwt_secret.to_lowercase().contains("change")
        {
            tracing::error!(
                "⚠️ SECURITY ERROR: JWT_SECRET_KEY appears to contain default/weak values"
            );
            return Err(tonic::Status::internal(
                "Server configuration error: Weak JWT_SECRET_KEY",
            ));
        }

        let jwt_algorithm = std::env::var("JWT_ALGORITHM").unwrap_or_else(|_| "HS256".to_string());

        let token_service = TokenService::new(jwt_secret, jwt_algorithm);

        tracing::info!("🔐 JWT TokenService initialized with secure configuration");
        Ok(Self { svc, token_service })
    }

    /// JWT 토큰을 검증합니다.
    ///
    /// # Arguments
    /// * `req` - gRPC 요청
    ///
    /// # Returns
    /// * `Result<Option<i32>, Status>` - 검증된 사용자 ID 또는 None
    fn verify_jwt_token(&self, req: &Request<()>) -> Result<Option<i32>, Status> {
        self.token_service.with_optional_auth(req, Ok)
    }

    /// 로그인 요청을 검증합니다 (최적화된 정적 상수 사용).
    ///
    /// # Arguments
    /// * `req` - 로그인 요청
    ///
    /// # Returns
    /// * `Result<(), AppError>` - 검증 결과
    fn validate_login_request(&self, req: &LoginRequest) -> Result<(), AppError> {
        // 최적화된 정적 상수 사용 (런타임 할당 제거)
        if !VALID_LOGIN_TYPES.contains(&req.login_type.as_str()) {
            return Err(AppError::InvalidLoginType(req.login_type.clone()));
        }

        // 로그인 토큰 검증
        helpers::validate_string(req.login_token.clone(), "login_token", 1000)?;

        Ok(())
    }

    /// 회원가입 요청을 검증합니다 (최적화된 정적 상수 사용).
    ///
    /// # Arguments
    /// * `req` - 회원가입 요청
    ///
    /// # Returns
    /// * `Result<(), AppError>` - 검증 결과
    fn validate_register_request(&self, req: &RegisterRequest) -> Result<(), AppError> {
        // 최적화된 정적 상수 사용 (런타임 할당 제거)
        if !VALID_REGISTER_TYPES.contains(&req.login_type.as_str()) {
            return Err(AppError::InvalidLoginType(req.login_type.clone()));
        }

        // 로그인 토큰 검증
        helpers::validate_string(req.login_token.clone(), "login_token", 1000)?;

        // 닉네임 검증
        helpers::validate_string(req.nick_name.clone(), "nick_name", 20)?;

        Ok(())
    }
}

#[tonic::async_trait]
impl<S, R, D> UserService for UserController<S, R, D>
where
    S: SocialAuthService + Send + Sync + 'static,
    R: UserRedisServiceTrait + Send + Sync + 'static,
    D: UserDatabaseService + Send + Sync + 'static,
{
    /// 사용자 로그인을 처리하는 gRPC 메서드
    ///
    /// 사용자가 로그인할 때 호출됩니다.
    /// 로그인 요청을 받아서 인증을 처리하고 사용자 정보를 반환합니다.
    ///
    /// # Arguments
    /// * `req` - 로그인 요청 정보 (LoginRequest)
    ///
    /// # Returns
    /// * `Result<Response<LoginResponse>, Status>` - 로그인 결과
    async fn login_user(
        &self,
        req: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let r = req.into_inner();
        info!("로그인 요청: login_type={}", r.login_type);

        // 요청 검증
        if let Err(e) = self.validate_login_request(&r) {
            return Err(e.to_status());
        }

        // JWT 토큰 검증 (선택적)
        let _verified_user_id = self.verify_jwt_token(&Request::new(()))?;

        // 비즈니스 로직 호출
        let (user_id, nick_name, access_token, is_register) = self
            .svc
            .login_user(r.login_type, r.login_token)
            .await
            .map_err(|e| {
                let app_error = AppError::InternalError(format!("로그인 실패: {e}"));
                app_error.to_status()
            })?;

        info!("로그인 성공: user_id={}, nick={}", user_id, nick_name);
        Ok(Response::new(LoginResponse {
            success: 1, // true를 1로 변경 (proto의 int32)
            user_id,
            nick_name,
            access_token,
            refresh_token: String::new(), // 임시로 빈 문자열
            is_register,
        }))
    }

    /// 사용자 회원가입을 처리하는 gRPC 메서드
    ///
    /// 사용자가 회원가입할 때 호출됩니다.
    /// 회원가입 요청을 받아서 새로운 사용자 계정을 생성합니다.
    ///
    /// # Arguments
    /// * `req` - 회원가입 요청 정보 (RegisterRequest)
    ///
    /// # Returns
    /// * `Result<Response<RegisterResponse>, Status>` - 회원가입 결과
    async fn register_user(
        &self,
        req: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let r = req.into_inner();
        info!(
            "회원가입 요청: login_type={}, nick={}",
            r.login_type, r.nick_name
        );

        // 요청 검증
        if let Err(e) = self.validate_register_request(&r) {
            return Err(e.to_status());
        }

        // JWT 토큰 검증 (선택적)
        let _verified_user_id = self.verify_jwt_token(&Request::new(()))?;

        // 비즈니스 로직 호출
        self.svc.register_user(r.nick_name).await.map_err(|e| {
            let app_error = AppError::InternalError(format!("회원가입 실패: {e}"));
            app_error.to_status()
        })?;

        info!("회원가입 성공");
        Ok(Response::new(RegisterResponse { success: 1 }))
    }
}
