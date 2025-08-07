//! Room Service gRPC Controller
//! 
//! 방 생성 및 조회 기능을 담당하는 gRPC 컨트롤러입니다.
//! RoomService trait을 구현하여 gRPC 서버에서 방 관련 요청을 처리합니다.

use tonic::{Request, Response, Status};
use tracing::info;
use crate::service::room_service::RoomService as RoomSvc;
use crate::room::{
    room_service_server::RoomService,
    MakeRoomRequest, MakeRoomResponse,
    GetRoomListRequest, GetRoomListResponse,
};
use crate::tool::error::{AppError, helpers};
use shared::service::TokenService;

/// Room Service gRPC 컨트롤러
/// 
/// 방 생성 및 조회 기능을 처리하는 컨트롤러입니다.
/// RoomService trait을 구현하여 gRPC 요청을 비즈니스 로직으로 연결합니다.
pub struct RoomController {
    /// 방 관련 비즈니스 로직을 처리하는 서비스
    svc: RoomSvc,
    /// JWT 토큰 검증 서비스
    token_service: TokenService,
}

impl RoomController {
    /// 새로운 RoomController 인스턴스를 생성합니다.
    /// 
    /// # Arguments
    /// * `svc` - 방 관련 비즈니스 로직을 처리하는 RoomService 인스턴스
    /// 
    /// # Returns
    /// * `Self` - 초기화된 RoomController 인스턴스
    pub fn new(svc: RoomSvc) -> Self { 
        let token_service = TokenService::new(
            std::env::var("JWT_SECRET_KEY").unwrap_or_else(|_| "default_secret".to_string()),
            std::env::var("JWT_ALGORITHM").unwrap_or_else(|_| "HS256".to_string()),
        );
        Self { svc, token_service } 
    }

    /// 방 생성 요청을 검증합니다.
    /// 
    /// # Arguments
    /// * `req` - 방 생성 요청
    /// 
    /// # Returns
    /// * `Result<(), AppError>` - 검증 결과
    fn validate_make_room_request(&self, req: &MakeRoomRequest) -> Result<(), AppError> {
        // 사용자 ID 검증
        if req.user_id <= 0 {
            return Err(AppError::InvalidInput("user_id must be positive".to_string()));
        }

        // 닉네임 검증
        helpers::validate_string(req.nick_name.clone(), "nick_name", 20)?;

        // 방 이름 검증
        helpers::validate_string(req.room_name.clone(), "room_name", 50)?;

        // 최대 플레이어 수 검증
        helpers::validate_range(req.max_player_num, "max_player_num", 2, 10)?;

        Ok(())
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
}

#[tonic::async_trait]
impl RoomService for RoomController {
    /// 방을 생성하는 gRPC 메서드
    /// 
    /// 사용자가 새로운 방을 생성할 때 호출됩니다.
    /// 방 생성 요청을 받아서 비즈니스 로직을 처리하고 결과를 반환합니다.
    /// 
    /// # Arguments
    /// * `req` - 방 생성 요청 정보 (MakeRoomRequest)
    /// 
    /// # Returns
    /// * `Result<Response<MakeRoomResponse>, Status>` - 방 생성 결과
    async fn make_room(
        &self,
        req: Request<MakeRoomRequest>,
    ) -> Result<Response<MakeRoomResponse>, Status> {
        let req_inner = req.into_inner();
        info!("방 생성 요청: user_id={}, room_name={}", req_inner.user_id, req_inner.room_name);
        
        // 요청 검증
        if let Err(e) = self.validate_make_room_request(&req_inner) {
            return Err(e.to_status());
        }
        
        // JWT 토큰 검증 (선택적)
        let verified_user_id = self.verify_jwt_token(&Request::new(()))?;
        if let Some(user_id) = verified_user_id {
            // 토큰이 있으면 사용자 ID 검증
            if user_id != req_inner.user_id {
                return Err(Status::permission_denied("User ID mismatch"));
            }
        }
        
        // 비즈니스 로직 호출
        let room_id = self
            .svc
            .make_room(req_inner.user_id, req_inner.nick_name, req_inner.room_name, req_inner.max_player_num)
            .await
            .map_err(|e| {
                let app_error = AppError::InternalError(format!("방 생성 실패: {e}"));
                app_error.to_status()
            })?;
        
        info!("방 생성 성공: room_id={}", room_id);
        Ok(Response::new(MakeRoomResponse { success: true, room_id }))
    }

    /// 방 리스트를 조회하는 gRPC 메서드
    /// 
    /// 사용자가 방 목록을 조회할 때 호출됩니다.
    /// 마지막으로 조회한 방 ID 이후의 방들을 반환합니다.
    /// 
    /// # Arguments
    /// * `req` - 방 리스트 조회 요청 정보 (GetRoomListRequest)
    /// 
    /// # Returns
    /// * `Result<Response<GetRoomListResponse>, Status>` - 방 리스트 조회 결과
    async fn get_room_list(
        &self,
        req: Request<GetRoomListRequest>,
    ) -> Result<Response<GetRoomListResponse>, Status> {
        let req_inner = req.into_inner();
        let last_id = req_inner.last_room_id;
        info!("방 리스트 조회 요청: last_id={}", last_id);
        
        // last_room_id 검증 (음수는 허용, 페이징 처리용)
        if last_id < -1 {
            return Err(AppError::InvalidInput("last_room_id must be >= -1".to_string()).to_status());
        }
        
        // JWT 토큰 검증 (선택적)
        let _verified_user_id = self.verify_jwt_token(&Request::new(()))?;
        
        // 비즈니스 로직 호출
        let rooms = self
            .svc
            .get_room_list(last_id)
            .await
            .map_err(|e| {
                let app_error = AppError::InternalError(format!("방 리스트 조회 실패: {e}"));
                app_error.to_status()
            })?;
        
        info!("방 리스트 조회 성공: {}개 방", rooms.len());
        Ok(Response::new(GetRoomListResponse { rooms }))
    }
}
