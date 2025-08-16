//! 연결 관리 테스트
//! 
//! ConnectionManager 기능 테스트

use crate::service::ConnectionService;
use crate::protocol::GameMessage;

/// 연결 서비스 기본 생성 테스트
#[tokio::test]
async fn test_connection_service_creation() {
    let service = ConnectionService::new(100);
    let count = service.get_connection_count().await;
    assert_eq!(count, 0, "새 연결 서비스는 연결이 0개여야 함");
    
    println!("✅ 연결 서비스 생성 테스트 통과");
}

/// 연결 카운트 테스트
#[tokio::test]
async fn test_connection_count() {
    let service = ConnectionService::new(100);
    
    // 초기 상태
    assert_eq!(service.get_connection_count().await, 0);
    
    println!("✅ 연결 카운트 테스트 통과");
}

/// 브로드캐스트 구독 테스트
#[tokio::test]
async fn test_broadcast_subscription() {
    let service = ConnectionService::new(100);
    
    // 브로드캐스트 구독자 생성
    let mut receiver = service.subscribe_broadcast();
    
    // 논블로킹 체크 (메시지가 없어야 함)
    let result = receiver.try_recv();
    assert!(result.is_err(), "초기 상태에서는 메시지가 없어야 함");
    
    println!("✅ 브로드캐스트 구독 테스트 통과");
}

/// 연결 타임아웃 테스트
#[tokio::test]
async fn test_connection_timeout() {
    let service = ConnectionService::new(100);
    
    // 타임아웃 정리 테스트 (실제 연결 없이)
    let cleanup_count = service.cleanup_timeout_connections().await;
    assert_eq!(cleanup_count, 0, "연결이 없을 때 정리 수는 0이어야 함");
    
    println!("✅ 연결 타임아웃 테스트 통과");
}

/// 모든 연결 해제 테스트
#[tokio::test]
async fn test_close_all_connections() {
    let service = ConnectionService::new(100);
    
    // 모든 연결 해제 (실제 연결 없이)
    service.close_all_connections().await;
    
    let count = service.get_connection_count().await;
    assert_eq!(count, 0, "모든 연결 해제 후에는 연결 수가 0이어야 함");
    
    println!("✅ 모든 연결 해제 테스트 통과");
}

/// 존재하지 않는 클라이언트 전송 테스트
#[tokio::test]
async fn test_send_to_nonexistent_client() {
    let service = ConnectionService::new(100);
    let message = GameMessage::HeartBeat;
    
    // 존재하지 않는 클라이언트에게 메시지 전송
    let result = service.send_to_user(999, &message).await;
    assert!(result.is_err(), "존재하지 않는 클라이언트에게 전송은 실패해야 함");
    
    println!("✅ 존재하지 않는 클라이언트 전송 테스트 통과");
}

/// 브로드캐스트 기능 테스트
#[tokio::test]
async fn test_broadcast_functionality() {
    let service = ConnectionService::new(100);
    let message = GameMessage::HeartBeat;
    
    // 연결이 없는 상태에서 브로드캐스트
    let result = service.broadcast_message(&message).await;
    assert!(result.is_ok(), "연결이 없어도 브로드캐스트는 성공해야 함");
    
    println!("✅ 브로드캐스트 기능 테스트 통과");
}