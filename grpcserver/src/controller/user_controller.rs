// src/controller/user_controller.rs
use tonic::{Request, Response, Status};
use tracing::{info, error};
use crate::service::user_service::UserService as UserSvc;
use crate::user::{
    user_service_server::UserService,
    LoginRequest, LoginResponse,
    RegisterRequest, RegisterResponse,
};

pub struct UserController {
    svc: UserSvc,
}

impl UserController {
    pub fn new(svc: UserSvc) -> Self { Self { svc } }
}

#[tonic::async_trait]
impl UserService for UserController {
    async fn login_user(
        &self,
        req: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let r = req.into_inner();
        info!("로그인 요청: login_type={}", r.login_type);
        
        let (user_id, nick_name, access_token, refresh_token, is_register) = self
            .svc
            .login_user(r.login_type, r.login_token)
            .await
            .map_err(|e| {
                error!("로그인 실패: {}", e);
                Status::internal(e.to_string())
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

    async fn register_user(
        &self,
        req: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let r = req.into_inner();
        info!("회원가입 요청: login_type={}, nick={}", r.login_type, r.nick_name);
        
        self
            .svc
            .register_user(r.login_type, r.login_token, r.nick_name)
            .await
            .map_err(|e| {
                error!("회원가입 실패: {}", e);
                Status::internal(e.to_string())
            })?;
        
        info!("회원가입 성공");
        Ok(Response::new(RegisterResponse { success: true }))
    }
}
