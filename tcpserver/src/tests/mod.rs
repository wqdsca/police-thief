//! TCP 서버 테스트 모듈
//! 
//! 각 기능별로 분리된 테스트 파일들을 관리합니다.

pub mod test_protocol;
pub mod test_connection;
pub mod test_heartbeat;
pub mod test_service;
pub mod test_handler;
pub mod test_tools;
pub mod all_test;
pub mod integration_test;

// 테스트 유틸리티
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use crate::service::{ConnectionService, HeartbeatService};

/// 테스트용 TCP 서버 생성
pub async fn create_test_server() -> std::io::Result<TcpListener> {
    TcpListener::bind("127.0.0.1:0").await
}

/// 테스트용 클라이언트 연결 생성
pub async fn create_test_client(addr: &str) -> std::io::Result<TcpStream> {
    TcpStream::connect(addr).await
}

/// 테스트용 연결 서비스 생성
pub fn create_test_connection_service() -> Arc<ConnectionService> {
    Arc::new(ConnectionService::new(100))
}

/// 테스트용 하트비트 서비스 생성
pub fn create_test_heartbeat_service() -> HeartbeatService {
    let connection_service = create_test_connection_service();
    HeartbeatService::with_default_config(connection_service)
}