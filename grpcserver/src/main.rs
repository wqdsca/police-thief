use anyhow::Result;
use dotenv::{dotenv, from_path};
use std::{env, net::SocketAddr, path::PathBuf};
use tonic::transport::Server;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

// 1) 프로토에서 생성된 코드를 같은 크레이트 루트에 포함
pub mod room {
    tonic::include_proto!("room");
}
pub mod user {
    tonic::include_proto!("user");
}

// 2) 도메인 로직·컨트롤러 모듈
mod service;
mod controller;

// 3) 편리한 import
use controller::{room_controller::RoomController, user_controller::UserController};
use service::{room_service::RoomService, user_service::UserService};
use room::room_service_server::RoomServiceServer;
use user::user_service_server::UserServiceServer;

#[tokio::main]
async fn main() -> Result<()> {
    // .env 로드
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../.env");
    dotenv().ok();
    from_path(&p).ok();

    // 로깅 초기화
    let filter = EnvFilter::from_default_env().add_directive("info".parse().unwrap());
    fmt().with_env_filter(filter).init();

    // grpc_host, grpc_port 읽기
    let host = env::var("grpc_host")?;
    let port = env::var("grpc_port")?;
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    info!("▶ gRPC 서버 실행: {}", addr);

    // 컨트롤러에 비즈니스 로직 서비스 주입
    let room_ctrl = RoomController::new(RoomService::new());
    let user_ctrl = UserController::new(UserService::new());

    // 서버 빌드 & 실행
    Server::builder()
        .add_service(RoomServiceServer::new(room_ctrl))
        .add_service(UserServiceServer::new(user_ctrl))
        .serve(addr)
        .await?;

    Ok(())
}
