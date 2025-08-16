//! gRPC Server Configuration
//!
//! gRPC 서버 설정 및 서비스 등록을 담당합니다.
//! RoomService와 UserService를 등록하고 서버를 시작합니다.

use std::net::SocketAddr;
use std::sync::Arc;
use tonic::transport::Server;

use crate::controller::{room_controller::RoomController, user_controller::UserController};
use crate::room::room_service_server::RoomServiceServer;
use crate::service::{
    room_service::{MockGameStateService, MockRoomRedisService, RoomService as RoomSvc},
    user_service::{
        MockSocialAuthService, MockUserDatabaseService, MockUserRedisService, UserService as UserSvc,
    },
};
use crate::user::user_service_server::UserServiceServer;

/// gRPC 서버를 시작합니다.
///
/// RoomService와 UserService를 등록하고 지정된 주소에서 서버를 시작합니다.
/// 서버는 비동기적으로 실행되며, Ctrl+C로 종료할 수 있습니다.
///
/// # Arguments
/// * `addr` - 서버가 바인딩할 소켓 주소
///
/// # Returns
/// * `anyhow::Result<()>` - 서버 시작 성공 여부
///
/// # Example
/// ```rust
/// use std::net::SocketAddr;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let addr: SocketAddr = "127.0.0.1:50051".parse()?;
///     start_server(addr).await
/// }
/// ```
pub async fn start_server(addr: SocketAddr) -> anyhow::Result<()> {
    // 컨트롤러에 비즈니스 로직 서비스 주입 (의존성 주입)
    let room_redis = Arc::new(MockRoomRedisService);
    let game_state = Arc::new(MockGameStateService);
    let room_service = RoomSvc::new(room_redis, game_state);
    let room_ctrl = RoomController::new(room_service)
        .map_err(|e| anyhow::anyhow!("Failed to initialize RoomController: {:?}", e))?;

    let social_auth_service = Arc::new(MockSocialAuthService);
    let user_redis = Arc::new(MockUserRedisService);
    let user_db = Arc::new(MockUserDatabaseService);
    let user_service = UserSvc::new(social_auth_service, user_redis, user_db);
    let user_ctrl = UserController::new(user_service)
        .map_err(|e| anyhow::anyhow!("Failed to initialize UserController: {:?}", e))?;

    // 서버 빌드 & 실행
    Server::builder()
        .add_service(RoomServiceServer::new(room_ctrl))
        .add_service(UserServiceServer::new(user_ctrl))
        .serve(addr)
        .await?;

    Ok(())
}
