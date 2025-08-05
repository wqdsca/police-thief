//! gRPC Server Library
//! 
//! Police Thief 게임을 위한 gRPC 서버 라이브러리입니다.
//! RoomService와 UserService를 제공하며, 클라이언트와의 통신을 담당합니다.
//! 
//! # Features
//! 
//! - **Room Service**: 방 생성 및 조회 기능
//! - **User Service**: 사용자 인증 및 회원가입 기능
//! - **Integration Tests**: gRPC 클라이언트 테스트
//! 
//! # Example
//! 
//! ```rust
//! use grpcserver::server::start_server;
//! use std::net::SocketAddr;
//! 
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let addr: SocketAddr = "127.0.0.1:50051".parse()?;
//!     start_server(addr).await
//! }
//! ```

/// Protocol Buffer 모듈들
/// 
/// gRPC 서비스 정의를 포함하는 모듈들입니다.
/// tonic::include_proto! 매크로를 사용하여 proto 파일을 Rust 코드로 변환합니다.

/// Room Service Protocol Buffer 정의
/// 
/// 방 생성 및 조회 관련 gRPC 서비스와 메시지 정의를 포함합니다.
pub mod room { 
    tonic::include_proto!("room"); 
}

/// User Service Protocol Buffer 정의
/// 
/// 사용자 인증 및 회원가입 관련 gRPC 서비스와 메시지 정의를 포함합니다.
pub mod user { 
    tonic::include_proto!("user"); 
}

/// Controller 모듈
/// 
/// gRPC 요청을 처리하는 컨트롤러들을 포함합니다.
/// RoomController와 UserController가 정의되어 있습니다.
pub mod controller;

/// Service 모듈
/// 
/// 비즈니스 로직을 처리하는 서비스들을 포함합니다.
/// RoomService와 UserService가 정의되어 있습니다.
pub mod service;

/// Server 모듈
/// 
/// gRPC 서버 설정 및 시작 기능을 포함합니다.
pub mod server;

/// Test 모듈
/// 
/// gRPC 클라이언트 테스트 코드를 포함합니다.
pub mod test;

// Public API exports
pub use controller::*;
pub use service::*;
pub use server::*;
pub use test::*;

