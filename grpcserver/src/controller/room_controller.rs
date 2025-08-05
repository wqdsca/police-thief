// src/controller/room_controller.rs
use tonic::{Request, Response, Status};
use tracing::{info, error};
use crate::service::room_service::RoomService as RoomSvc;
use crate::room::{
    room_service_server::RoomService,
    MakeRoomRequest, MakeRoomResponse,
    GetRoomListRequest, GetRoomListResponse,
};

pub struct RoomController {
    svc: RoomSvc,
}

impl RoomController {
    pub fn new(svc: RoomSvc) -> Self { Self { svc } }
}

#[tonic::async_trait]
impl RoomService for RoomController {
    async fn make_room(
        &self,
        req: Request<MakeRoomRequest>,
    ) -> Result<Response<MakeRoomResponse>, Status> {
        let req = req.into_inner();
        info!("방 생성 요청: user_id={}, room_name={}", req.user_id, req.room_name);
        
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

    async fn get_room_list(
        &self,
        req: Request<GetRoomListRequest>,
    ) -> Result<Response<GetRoomListResponse>, Status> {
        let last_id = req.into_inner().last_room_id;
        info!("방 리스트 조회 요청: last_id={}", last_id);
        
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
