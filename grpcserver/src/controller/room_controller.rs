//! Room Service gRPC Controller
//! 
//! 방 생성 및 조회 기능을 담당하는 gRPC 컨트롤러입니다.
//! RoomService trait을 구현하여 gRPC 서버에서 방 관련 요청을 처리합니다.

use tonic::{Request, Response, Status};
use tracing::{info, error};
use crate::service::room_service::RoomService as RoomSvc;
use crate::room::{
    room_service_server::RoomService,
    MakeRoomRequest, MakeRoomResponse,
    GetRoomListRequest, GetRoomListResponse,
};

/// Room Service gRPC 컨트롤러
/// 
/// 방 생성 및 조회 기능을 처리하는 컨트롤러입니다.
/// RoomService trait을 구현하여 gRPC 요청을 비즈니스 로직으로 연결합니다.
pub struct RoomController {
    /// 방 관련 비즈니스 로직을 처리하는 서비스
    svc: RoomSvc,
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
        Self { svc } 
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
        let req = req.into_inner();
        info!("방 생성 요청: user_id={}, room_name={}", req.user_id, req.room_name);
        
        // 비즈니스 로직 호출
        let room_id = self
            .svc
            .make_room(req.user_id, req.nick_name, req.room_name, req.max_player_num)
            .await
            .map_err(|e| {
                error!("방 생성 실패: {}", e);
                Status::internal(e.to_string())
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
        let last_id = req.into_inner().last_room_id;
        info!("방 리스트 조회 요청: last_id={}", last_id);
        
        // 비즈니스 로직 호출
        let rooms = self
            .svc
            .get_room_list(last_id)
            .await
            .map_err(|e| {
                error!("방 리스트 조회 실패: {}", e);
                Status::internal(e.to_string())
            })?;
        
        info!("방 리스트 조회 성공: {}개 방", rooms.len());
        Ok(Response::new(GetRoomListResponse { rooms }))
    }
}
