//! Room Service gRPC Controller
//! 
//! 방 생성 및 조회 기능을 담당하는 gRPC 컨트롤러입니다.
//! RoomService trait을 구현하여 gRPC 서버에서 방 관련 요청을 처리합니다.
//! 최적화된 싱글톤 패턴으로 RoomIdGenerator 인스턴스를 재사용합니다.

use std::sync::Arc;
use tokio::sync::OnceCell;
use tonic::{Request, Response, Status};
use tracing::info;
use crate::service::room_service::RoomService as RoomSvc;
use crate::room::{
    room_service_server::RoomService,
    MakeRoomRequest, MakeRoomResponse,
    GetRoomListRequest, GetRoomListResponse,
};
use shared::tool::error::{AppError, helpers};
use shared::service::TokenService;
use shared::model::RoomInfo;
use shared::tool::current_time::CurrentTime;
use shared::tool::get_id::RoomIdGenerator;

/// 최적화된 RoomIdGenerator 인스턴스 (싱글톤)
static ROOM_ID_GENERATOR: OnceCell<Arc<RoomIdGenerator>> = OnceCell::const_new();

/// Room Service gRPC 컨트롤러
/// 
/// 방 생성 및 조회 기능을 처리하는 컨트롤러입니다.
/// RoomService trait을 구현하여 gRPC 요청을 비즈니스 로직으로 연결합니다.
/// 최적화된 싱글톤 패턴으로 RoomIdGenerator의 Redis 연결을 재사용합니다.
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
    /// 
    /// # Panics
    /// * JWT_SECRET_KEY 환경변수가 설정되지 않았거나 32자 미만일 경우
    pub fn new(svc: RoomSvc) -> Self { 
        let jwt_secret = std::env::var("JWT_SECRET_KEY")
            .expect("⚠️ SECURITY ERROR: JWT_SECRET_KEY environment variable is required for production");
        
        // 보안 검증: 최소 32자 이상의 시크릿 키 요구
        if jwt_secret.len() < 32 {
            panic!("⚠️ SECURITY ERROR: JWT_SECRET_KEY must be at least 32 characters long. Current length: {}", jwt_secret.len());
        }
        
        // 보안 검증: 약한 기본값 사용 방지
        if jwt_secret.to_lowercase().contains("default") || 
           jwt_secret.to_lowercase().contains("secret") ||
           jwt_secret.to_lowercase().contains("change") {
            panic!("⚠️ SECURITY ERROR: JWT_SECRET_KEY appears to contain default/weak values. Please use a cryptographically secure random key.");
        }
        
        let jwt_algorithm = std::env::var("JWT_ALGORITHM").unwrap_or_else(|_| "HS256".to_string());
        
        let token_service = TokenService::new(jwt_secret, jwt_algorithm);
        
        tracing::info!("🔐 JWT TokenService initialized with secure configuration");
        Self { svc, token_service } 
    }

    /// 최적화된 RoomIdGenerator 인스턴스를 가져옵니다.
    /// 
    /// 싱글톤 패턴으로 한 번만 초기화하고 재사용하여 Redis 연결 오버헤드를 제거합니다.
    /// 
    /// # Returns
    /// * `Result<Arc<RoomIdGenerator>, AppError>` - RoomIdGenerator 인스턴스
    async fn get_room_id_generator(&self) -> Result<Arc<RoomIdGenerator>, AppError> {
        ROOM_ID_GENERATOR
            .get_or_try_init(|| async {
                let generator = RoomIdGenerator::from_env().await
                    .map_err(|e| AppError::InternalError(format!("방 ID 생성기 초기화 실패: {e}")))?;
                Ok(Arc::new(generator))
            })
            .await
            .cloned()
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
        
        // 비즈니스 로직 호출 (최적화된 싱글톤 RoomIdGenerator 사용)
        let room_id_generator = self.get_room_id_generator().await.map_err(|e| e.to_status())?;
        let mut generator = Arc::try_unwrap(room_id_generator)
            .unwrap_or_else(|arc| (*arc).clone());
        
        let room_id = self
            .svc
            .make_room(RoomInfo {
                room_id: generator.get_room_id().await.map_err(|e| {
                    let app_error = AppError::InternalError(format!("방 ID 생성 실패: {e}"));
                    app_error.to_status()
                })?, 
                room_name: req_inner.room_name,
                max_player_num: req_inner.max_player_num as u16,
                current_player_num: 1,
                create_at: CurrentTime::new().current_time,
            })
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
        if rooms.len() == 0 {
            return Ok(Response::new(GetRoomListResponse { rooms: vec![] }));
        }
        
        // shared::model::RoomInfo를 room::RoomInfo로 변환 (optimized allocation)
        let mut proto_rooms = Vec::with_capacity(rooms.len());
        for room in rooms {
            proto_rooms.push(crate::room::RoomInfo {
                room_id: room.room_id as i32,
                room_name: room.room_name,
                current_player_num: room.current_player_num as i32,
                max_player_num: room.max_player_num as i32,
            });
        }
        
        Ok(Response::new(GetRoomListResponse { rooms: proto_rooms }))
    }
}
