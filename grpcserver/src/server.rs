// src/server.rs
use tonic::transport::Server;
use std::net::SocketAddr;

use crate::controller::{
    room_controller::RoomController,
    user_controller::UserController,
};
use crate::service::{
    room_service::RoomService as RoomSvc,
    user_service::UserService as UserSvc,
};
use crate::room::room_service_server::RoomServiceServer;
use crate::user::user_service_server::UserServiceServer;

pub async fn start_server(addr: SocketAddr) -> anyhow::Result<()> {
    let room_ctrl = RoomController::new(RoomSvc::new());
    let user_ctrl = UserController::new(UserSvc::new());

    Server::builder()
        .add_service(RoomServiceServer::new(room_ctrl))
        .add_service(UserServiceServer::new(user_ctrl))
        .serve(addr)
        .await?;
    Ok(())
}