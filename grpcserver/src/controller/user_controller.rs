//! User Service gRPC Controller
//! 
//! 사용자 인증 및 회원가입 기능을 담당하는 gRPC 컨트롤러입니다.
//! UserService trait을 구현하여 gRPC 서버에서 사용자 관련 요청을 처리합니다.

use tonic::{Request, Response, Status};
use tracing::info;
use crate::service::user_service::UserService as UserSvc;
use crate::user::{
    user_service_server::UserService,
    LoginRequest, LoginResponse,
    RegisterRequest, RegisterResponse,
};
use shared::tool::error::{AppError, helpers};

/// User Service gRPC 컨트롤러
/// 
/// 사용자 인증 및 회원가입 기능을 처리하는 컨트롤러입니다.
/// UserService trait을 구현하여 gRPC 요청을 비즈니스 로직으로 연결합니다.
pub struct UserController {
    /// 사용자 관련 비즈니스 로직을 처리하는 서비스
    svc: UserSvc,
}

impl UserController {
    /// 새로운 UserController 인스턴스를 생성합니다.
    /// 
    /// # Arguments
    /// * `svc` - 사용자 관련 비즈니스 로직을 처리하는 UserService 인스턴스
    /// 
    /// # Returns
    /// * `Self` - 초기화된 UserController 인스턴스
    pub fn new(svc: UserSvc) -> Self { 
        Self { svc } 
    }

    /// 로그인 요청을 검증합니다.
    /// 
    /// # Arguments
    /// * `req` - 로그인 요청
    /// 
    /// # Returns
    /// * `Result<(), AppError>` - 검증 결과
    fn validate_login_request(&self, req: &LoginRequest) -> Result<(), AppError> {
        // 로그인 타입 검증
        let valid_login_types = ["google", "apple", "test"];
        if !valid_login_types.contains(&req.login_type.as_str()) {
            return Err(AppError::InvalidLoginType(req.login_type.clone()));
        }

        // 로그인 토큰 검증
        helpers::validate_string(req.login_token.clone(), "login_token", 1000)?;

        Ok(())
    }

    /// 회원가입 요청을 검증합니다.
    /// 
    /// # Arguments
    /// * `req` - 회원가입 요청
    /// 
    /// # Returns
    /// * `Result<(), AppError>` - 검증 결과
    fn validate_register_request(&self, req: &RegisterRequest) -> Result<(), AppError> {
        // 로그인 타입 검증
        let valid_login_types = ["google", "apple", "guest"];
        if !valid_login_types.contains(&req.login_type.as_str()) {
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
impl UserService for UserController {
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
        
        // 비즈니스 로직 호출
        let (user_id, nick_name, access_token, refresh_token, is_register) = self
            .svc
            .login_user(r.login_type, r.login_token)
            .await
            .map_err(|e| {
                let app_error = AppError::InternalError(format!("로그인 실패: {e}"));
                app_error.to_status()
            })?;
        
        info!("로그인 성공: user_id={}, nick={}", user_id, nick_name);
        Ok(Response::new(LoginResponse {
            success: true,
            user_id,
            nick_name,
            access_token,
            refresh_token,
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
        info!("회원가입 요청: login_type={}, nick={}", r.login_type, r.nick_name);
        
        // 요청 검증
        if let Err(e) = self.validate_register_request(&r) {
            return Err(e.to_status());
        }
        
        // 비즈니스 로직 호출
        self
            .svc
            .register_user(r.login_type, r.login_token, r.nick_name)
            .await
            .map_err(|e| {
                let app_error = AppError::InternalError(format!("회원가입 실패: {e}"));
                app_error.to_status()
            })?;
        
        info!("회원가입 성공");
        Ok(Response::new(RegisterResponse { success: true }))
    }
}
