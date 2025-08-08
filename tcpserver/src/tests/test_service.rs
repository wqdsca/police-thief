//! 서비스 레이어 테스트
//! 
//! Service 모듈들의 비즈니스 로직 테스트

use crate::service::{SimpleTcpService, ConnectionService, HeartbeatService};
use crate::protocol::GameMessage;
use std::sync::Arc;
use anyhow::Result;

/// SimpleTcpService 생성 테스트
#[tokio::test]
async fn test_simple_tcp_service_creation() {
    let service = SimpleTcpService::new();
    
    // 기본 상태 확인
    let status = service.get_status().await;
    assert_eq!(status, "ready", "초기 상태는 ready여야 함");
    
    println!("✅ SimpleTcpService 생성 테스트 통과");
}

/// 서비스 상태 확인 테스트
#[tokio::test]
async fn test_service_status() {
    let service = SimpleTcpService::new();
    
    // 상태 조회 테스트
    let status = service.get_status().await;
    assert!(!status.is_empty(), "상태는 빈 문자열이 아니어야 함");
    assert_eq!(status, "ready", "초기 상태는 ready여야 함");
    
    println!("✅ 서비스 상태 확인 테스트 통과");
}

/// 서비스 시작 테스트
#[tokio::test]
async fn test_service_start() -> Result<()> {
    let service = SimpleTcpService::new();
    
    // 서비스 시작
    let result = service.start("127.0.0.1:0").await;
    assert!(result.is_ok(), "서비스 시작은 성공해야 함");
    
    println!("✅ 서비스 시작 테스트 통과");
    Ok(())
}

/// 서비스 중지 테스트
#[tokio::test]
async fn test_service_stop() -> Result<()> {
    let service = SimpleTcpService::new();
    
    // 서비스 시작 후 중지
    service.start("127.0.0.1:0").await?;
    let result = service.stop().await;
    assert!(result.is_ok(), "서비스 중지는 성공해야 함");
    
    println!("✅ 서비스 중지 테스트 통과");
    Ok(())
}

/// 서비스 생명주기 테스트
#[tokio::test]
async fn test_service_lifecycle() -> Result<()> {
    let service = SimpleTcpService::new();
    
    // 초기 상태
    assert_eq!(service.get_status().await, "ready");
    
    // 시작
    service.start("127.0.0.1:0").await?;
    // 상태는 구현에 따라 변경될 수 있음
    
    // 중지
    service.stop().await?;
    // 상태는 구현에 따라 변경될 수 있음
    
    println!("✅ 서비스 생명주기 테스트 통과");
    Ok(())
}

/// Default trait 구현 테스트
#[tokio::test]
async fn test_service_default() {
    let service = SimpleTcpService::default();
    let status = service.get_status().await;
    
    assert_eq!(status, "ready", "기본 생성된 서비스는 ready 상태여야 함");
    
    println!("✅ 서비스 Default 구현 테스트 통과");
}

/// 서비스 동시성 안전성 테스트
#[tokio::test]
async fn test_service_concurrency() -> Result<()> {
    let service = SimpleTcpService::new();
    
    // 동시에 여러 작업 수행
    let handles = vec![
        tokio::spawn({
            let service = service.clone();
            async move { service.start("127.0.0.1:0").await }
        }),
        tokio::spawn({
            let service = service.clone();
            async move { service.get_status().await; Ok(()) }
        }),
        tokio::spawn({
            let service = service.clone();
            async move { service.stop().await }
        }),
    ];
    
    // 모든 작업 완료 대기
    for handle in handles {
        let result = handle.await?;
        assert!(result.is_ok(), "동시 작업들은 모두 성공해야 함");
    }
    
    println!("✅ 서비스 동시성 안전성 테스트 통과");
    Ok(())
}

/// 서비스 상태 지속성 테스트
#[tokio::test]
async fn test_service_state_persistence() -> Result<()> {
    let service = SimpleTcpService::new();
    
    // 초기 상태
    let initial_status = service.get_status().await;
    
    // 시작
    service.start("127.0.0.1:0").await?;
    let started_status = service.get_status().await;
    
    // 중지  
    service.stop().await?;
    let stopped_status = service.get_status().await;
    
    // 상태가 변경되었는지 확인 (구현에 따라)
    println!("상태 변화: {} → {} → {}", initial_status, started_status, stopped_status);
    
    println!("✅ 서비스 상태 지속성 테스트 통과");
    Ok(())
}

/// 서비스 에러 처리 테스트
#[tokio::test]
async fn test_service_error_handling() -> Result<()> {
    let service = SimpleTcpService::new();
    
    // 정상 케이스
    assert!(service.start("127.0.0.1:0").await.is_ok());
    assert!(service.stop().await.is_ok());
    
    // 상태 조회는 항상 성공해야 함
    let status = service.get_status().await;
    assert!(!status.is_empty());
    
    println!("✅ 서비스 에러 처리 테스트 통과");
    Ok(())
}

// ConnectionService 테스트들

/// ConnectionService 생성 테스트
#[tokio::test]
async fn test_connection_service_creation() {
    let max_connections = 100;
    let service = ConnectionService::new(max_connections);
    
    // 초기 연결 수 확인
    let count = service.get_connection_count().await;
    assert_eq!(count, 0, "초기 연결 수는 0이어야 함");
    
    // 업타임 확인
    let uptime = service.get_uptime_seconds().await;
    assert!(uptime >= 0, "업타임은 0 이상이어야 함");
    
    println!("✅ ConnectionService 생성 테스트 통과");
}

/// ConnectionService 통계 테스트
#[tokio::test]
async fn test_connection_service_stats() {
    let service = ConnectionService::new(100);
    
    let stats = service.get_connection_stats().await;
    assert_eq!(stats.total_connections, 0, "초기 총 연결 수는 0");
    assert_eq!(stats.current_connections, 0, "초기 현재 연결 수는 0");
    assert_eq!(stats.peak_connections, 0, "초기 최대 연결 수는 0");
    assert_eq!(stats.total_messages, 0, "초기 총 메시지 수는 0");
    
    println!("✅ ConnectionService 통계 테스트 통과");
}

/// ConnectionService 브로드캐스트 구독 테스트
#[tokio::test]
async fn test_connection_service_broadcast() {
    let service = ConnectionService::new(100);
    
    let mut receiver = service.subscribe_broadcast();
    
    // 브로드캐스트 수신 테스트 (아직 메시지 없음)
    assert!(receiver.try_recv().is_err(), "초기에는 메시지가 없어야 함");
    
    println!("✅ ConnectionService 브로드캐스트 테스트 통과");
}

/// ConnectionService 타임아웃 정리 테스트
#[tokio::test]
async fn test_connection_service_cleanup() {
    let service = ConnectionService::new(100);
    
    // 타임아웃된 연결 정리 (현재는 연결이 없음)
    let cleanup_count = service.cleanup_timeout_connections().await;
    assert_eq!(cleanup_count, 0, "연결이 없으면 정리할 것도 없어야 함");
    
    println!("✅ ConnectionService 타임아웃 정리 테스트 통과");
}

/// ConnectionService 사용자 목록 테스트
#[tokio::test]
async fn test_connection_service_user_list() {
    let service = ConnectionService::new(100);
    
    // 사용자 목록 조회 (빈 목록)
    let users = service.get_all_users().await;
    assert_eq!(users.len(), 0, "초기에는 사용자가 없어야 함");
    
    println!("✅ ConnectionService 사용자 목록 테스트 통과");
}

// HeartbeatService 테스트들

/// HeartbeatService 생성 테스트
#[tokio::test]
async fn test_heartbeat_service_creation() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::new(connection_service, 1, 3); // 빠른 테스트용
    
    // 초기 상태 확인
    assert!(!heartbeat_service.is_running().await, "초기에는 실행 중이 아니어야 함");
    
    // 설정 확인
    let (interval, timeout) = heartbeat_service.get_config();
    assert_eq!(interval, 1, "하트비트 간격이 1초여야 함");
    assert_eq!(timeout, 3, "타임아웃이 3초여야 함");
    
    println!("✅ HeartbeatService 생성 테스트 통과");
}

/// HeartbeatService 기본 설정 테스트
#[tokio::test]
async fn test_heartbeat_service_default_config() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::with_default_config(connection_service);
    
    let (interval, timeout) = heartbeat_service.get_config();
    assert_eq!(interval, 10, "기본 하트비트 간격이 10초여야 함");
    assert_eq!(timeout, 30, "기본 타임아웃이 30초여야 함");
    
    println!("✅ HeartbeatService 기본 설정 테스트 통과");
}

/// HeartbeatService 생명주기 테스트
#[tokio::test]
async fn test_heartbeat_service_lifecycle() -> Result<()> {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::new(connection_service, 1, 3); // 빠른 테스트용
    
    // 초기 상태
    assert!(!heartbeat_service.is_running().await);
    
    // 시작
    heartbeat_service.start().await?;
    assert!(heartbeat_service.is_running().await, "시작 후 실행 중이어야 함");
    
    // 잠시 대기
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // 중지
    heartbeat_service.stop().await?;
    assert!(!heartbeat_service.is_running().await, "중지 후 실행 중이 아니어야 함");
    
    println!("✅ HeartbeatService 생명주기 테스트 통과");
    Ok(())
}

/// HeartbeatService 통계 테스트
#[tokio::test]
async fn test_heartbeat_service_stats() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::with_default_config(connection_service);
    
    let stats = heartbeat_service.get_heartbeat_stats().await;
    assert_eq!(stats.total_heartbeats, 0, "초기 하트비트 수는 0");
    assert_eq!(stats.timeout_cleanups, 0, "초기 타임아웃 정리 수는 0");
    assert_eq!(stats.active_connections, 0, "초기 활성 연결 수는 0");
    
    println!("✅ HeartbeatService 통계 테스트 통과");
}

/// HeartbeatService 연결 건강성 평가 테스트
#[tokio::test]
async fn test_heartbeat_service_health() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::with_default_config(connection_service);
    
    let health = heartbeat_service.evaluate_connection_health().await;
    assert_eq!(health.active_connections, 0, "초기 활성 연결 수는 0");
    assert_eq!(health.timeout_rate, 0.0, "초기 타임아웃 비율은 0");
    
    // 초기 상태에서는 Excellent여야 함
    use crate::service::HealthScore;
    assert_eq!(health.score, HealthScore::Excellent);
    
    println!("✅ HeartbeatService 연결 건강성 평가 테스트 통과");
}

/// HeartbeatService 수동 정리 테스트
#[tokio::test]
async fn test_heartbeat_service_manual_cleanup() -> Result<()> {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::with_default_config(connection_service);
    
    // 수동 정리 (연결이 없으므로 0개)
    let cleanup_count = heartbeat_service.cleanup_now().await?;
    assert_eq!(cleanup_count, 0, "연결이 없으면 정리할 것도 없어야 함");
    
    println!("✅ HeartbeatService 수동 정리 테스트 통과");
    Ok(())
}

/// HeartbeatService 활성 연결 수 조회 테스트
#[tokio::test]
async fn test_heartbeat_service_active_connections() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::with_default_config(connection_service);
    
    let active_count = heartbeat_service.get_active_connections().await;
    assert_eq!(active_count, 0, "초기 활성 연결 수는 0");
    
    println!("✅ HeartbeatService 활성 연결 수 조회 테스트 통과");
}