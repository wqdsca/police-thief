//! 하트비트 시스템 테스트
//! 
//! HeartbeatManager 기능 및 상태 관리 테스트

use crate::service::{HeartbeatService, ConnectionService};
use std::sync::Arc;

/// 하트비트 관리자 생성 테스트
#[tokio::test]
async fn test_heartbeat_service_creation() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::with_default_config(connection_service);
    
    // 초기 상태 확인
    assert!(!heartbeat_service.is_running().await, "초기 상태는 중지되어 있어야 함");
    
    println!("✅ 하트비트 서비스 생성 테스트 통과");
}

/// 하트비트 시스템 시작/중지 테스트
#[tokio::test]
async fn test_heartbeat_start_stop() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::with_default_config(connection_service);
    
    // 초기 상태
    assert!(!heartbeat_service.is_running().await);
    
    // 시작 테스트
    heartbeat_service.start().await.unwrap();
    assert!(heartbeat_service.is_running().await, "시작 후에는 실행 중이어야 함");
    
    // 중지 테스트
    heartbeat_service.stop().await.unwrap();
    assert!(!heartbeat_service.is_running().await, "중지 후에는 실행이 멈춰야 함");
    
    println!("✅ 하트비트 시작/중지 테스트 통과");
}

/// 하트비트 활성 연결 수 조회 테스트
#[tokio::test]
async fn test_heartbeat_active_connections() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::with_default_config(connection_service.clone());
    
    // 초기 연결 수
    let initial_count = heartbeat_service.get_active_connections().await;
    assert_eq!(initial_count, 0, "초기 연결 수는 0이어야 함");
    
    // 연결 서비스의 연결 수와 일치하는지 확인
    let service_count = connection_service.get_connection_count().await;
    assert_eq!(initial_count, service_count, "하트비트와 연결 서비스의 연결 수가 일치해야 함");
    
    println!("✅ 하트비트 활성 연결 수 테스트 통과");
}

/// 수동 정리 기능 테스트
#[tokio::test]
async fn test_manual_cleanup() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::with_default_config(connection_service);
    
    // 연결이 없는 상태에서 수동 정리
    let cleanup_count = heartbeat_service.cleanup_now().await.unwrap();
    assert_eq!(cleanup_count, 0, "연결이 없을 때 정리 수는 0이어야 함");
    
    println!("✅ 수동 정리 기능 테스트 통과");
}

/// 기본 설정 생성 테스트
#[tokio::test]
async fn test_heartbeat_default_config() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::with_default_config(connection_service);
    
    // 기본 상태 확인
    assert!(!heartbeat_service.is_running().await, "기본 상태는 중지되어 있어야 함");
    assert_eq!(heartbeat_service.get_active_connections().await, 0, "기본 연결 수는 0이어야 함");
    
    // 기본 설정 확인
    let (interval, timeout) = heartbeat_service.get_config();
    assert_eq!(interval, 10, "기본 하트비트 간격은 10초여야 함");
    assert_eq!(timeout, 30, "기본 타임아웃은 30초여야 함");
    
    println!("✅ 하트비트 기본 설정 테스트 통과");
}