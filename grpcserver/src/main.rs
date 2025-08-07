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
mod tool;
// 3) 편리한 import
use controller::{room_controller::RoomController, user_controller::UserController};
use service::{room_service::RoomService, user_service::UserService};
use room::room_service_server::RoomServiceServer;
use user::user_service_server::UserServiceServer;

#[tokio::main]
async fn main() -> Result<()> {
    // .env 로드 - workspace root에서 .env 파일 찾기
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let env_path = workspace_root.join(".env");
    
    if env_path.exists() {
        from_path(&env_path).map_err(|e| anyhow::anyhow!("Failed to load .env: {}", e))?;
    } else {
        dotenv().ok(); // fallback to default .env loading
    }

    // 로깅 초기화
    let filter = EnvFilter::from_default_env().add_directive("info".parse().unwrap());
    fmt().with_env_filter(filter).init();

    // grpc_host, grpc_port 읽기
    let host = env::var("grpc_host").expect("grpc_host 환경변수가 설정되지 않았습니다.");
    let port = env::var("grpc_port").expect("grpc_port 환경변수가 설정되지 않았습니다.");
    let addr: SocketAddr = format!("{host}:{port}").parse()?;

    info!("▶ gRPC 서버 실행: {}", addr);

    // JWT 설정 확인 (선택적)
    let jwt_secret = env::var("JWT_SECRET_KEY").unwrap_or_else(|_| {
        info!("⚠️ JWT_SECRET_KEY가 설정되지 않았습니다. 토큰 검증이 비활성화됩니다.");
        "default_secret".to_string()
    });
    
    let jwt_algorithm = env::var("JWT_ALGORITHM").unwrap_or_else(|_| {
        info!("⚠️ JWT_ALGORITHM이 설정되지 않았습니다. 기본값 'HS256'을 사용합니다.");
        "HS256".to_string()
    });

    info!("🔐 JWT 설정: algorithm={}, secret_length={}", jwt_algorithm, jwt_secret.len());
    info!("💡 JWT 토큰 검증은 컨트롤러 레벨에서 구현됩니다.");

    // 컨트롤러에 비즈니스 로직 서비스 주입
    let room_ctrl = RoomController::new(RoomService::new());
    let user_ctrl = UserController::new(UserService::new());

    // 서버 빌드 & 실행 (기본 설정)
    Server::builder()
        .add_service(RoomServiceServer::new(room_ctrl))
        .add_service(UserServiceServer::new(user_ctrl))
        .serve(addr)
        .await?;

    Ok(())
}
