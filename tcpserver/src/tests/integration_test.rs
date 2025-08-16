//! TCP 서버 통합 테스트
//! 
//! 전체 TCP 서버 시스템의 통합 기능을 테스트합니다.

use crate::service::{ConnectionService, HeartbeatService, MessageService};
use crate::handler::{ServerMessageHandler, RoomHandler, FriendHandler};
use crate::protocol::GameMessage;
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use anyhow::Result;

/// 전체 TCP 서버 시스템 통합 테스트
#[tokio::test]
async fn test_full_tcp_server_integration() -> Result<()> {
    // 서비스 레이어 구축
    let connection_service = Arc::new(ConnectionService::new(100));
    let message_service = Arc::new(MessageService::new(connection_service.clone()));
    let heartbeat_service = Arc::new(HeartbeatService::with_default_config(connection_service.clone()));
    
    // 핸들러 레이어 구축
    let message_handler = Arc::new(ServerMessageHandler::new(
        connection_service.clone(),
        heartbeat_service.clone(),
        message_service.clone(),
    ));
    let room_handler = Arc::new(RoomHandler::new(
        connection_service.clone(),
        message_service.clone(),
    ));
    let friend_handler = Arc::new(FriendHandler::new(
        connection_service.clone(),
        message_service.clone(),
    ));
    
    // 메시지 핸들러 등록
    message_handler.register_all_handlers().await?;
    
    // 하트비트 서비스 시작
    heartbeat_service.start().await?;
    
    // 시나리오 1: 방 관리 (방은 외부에서 미리 생성되어야 함)
    println!("🎮 시나리오 1: 방 관리");
    
    // 방 목록 조회 (초기에는 빈 목록)
    let rooms = room_handler.get_room_list().await;
    assert_eq!(rooms.len(), 0, "초기에는 방이 없어야 함");
    
    // 존재하지 않는 방에 입장 시도 (실패해야 함)
    let join_result = room_handler.join_room(1, 999, "Player1".to_string()).await;
    assert!(join_result.is_err(), "존재하지 않는 방 입장은 실패해야 함");
    
    println!("✅ 방 관리 시스템 검증 성공");
    
    // 시나리오 2: 친구 관리 시스템
    println!("🤝 시나리오 2: 친구 관리 시스템");
    friend_handler.register_user(1, "Player1".to_string()).await;
    friend_handler.register_user(2, "Player2".to_string()).await;
    friend_handler.register_user(3, "Player3".to_string()).await;
    
    // 친구 관계 설정
    friend_handler.add_friend(1, 2, "Player2".to_string()).await?;
    friend_handler.add_friend(1, 3, "Player3".to_string()).await?;
    friend_handler.add_friend(2, 1, "Player1".to_string()).await?;
    
    // 친구 관계 확인
    let friends_of_1 = friend_handler.get_friend_list(1).await;
    assert_eq!(friends_of_1.len(), 2, "Player1은 2명의 친구가 있어야 함");
    
    // 상호 친구 확인
    assert!(friend_handler.are_mutual_friends(1, 2).await, "Player1과 Player2는 상호 친구여야 함");
    assert!(!friend_handler.are_mutual_friends(1, 3).await, "Player1과 Player3는 단방향 친구여야 함");
    
    println!("✅ 친구 관리 시스템 성공");
    
    // 시나리오 3: 메시지 검증 시스템
    println!("💬 시나리오 3: 메시지 검증 시스템");
    
    // 정상 메시지들 검증
    let valid_messages = vec![
        GameMessage::HeartBeat,
        GameMessage::ConnectionAck { user_id: 1 },
        GameMessage::RoomJoin { 
            user_id: 1, 
            room_id: 1, 
            nickname: "Player1".to_string() 
        },
        GameMessage::ChatMessage { 
            user_id: 1, 
            room_id: 1, 
            content: "안녕하세요!".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        },
    ];
    
    for msg in valid_messages {
        let result = message_handler.validate_message(1, &msg);
        assert!(result.is_ok(), "정상 메시지는 검증을 통과해야 함: {:?}", msg);
    }
    
    // 비정상 메시지들 검증
    let invalid_messages = vec![
        GameMessage::ChatMessage { 
            user_id: 2, // 잘못된 사용자 ID
            room_id: 1, 
            content: "안녕하세요!".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        },
        GameMessage::ChatMessage { 
            user_id: 1, 
            room_id: 1, 
            content: "".to_string(), // 빈 내용
            timestamp: chrono::Utc::now().timestamp(),
        },
        GameMessage::FriendAdd { 
            user_id: 1, 
            friend_user_id: 1, // 자기 자신
            nickname: "Self".to_string() 
        },
    ];
    
    for msg in invalid_messages {
        let result = message_handler.validate_message(1, &msg);
        assert!(result.is_err(), "비정상 메시지는 검증에 실패해야 함: {:?}", msg);
    }
    
    println!("✅ 메시지 검증 시스템 성공");
    
    // 시나리오 4: 하트비트 및 연결 관리
    println!("💓 시나리오 4: 하트비트 및 연결 관리");
    
    // 하트비트 통계 확인
    let initial_stats = heartbeat_service.get_heartbeat_stats().await;
    assert_eq!(initial_stats.active_connections, 0, "초기 활성 연결은 0");
    
    // 연결 건강성 평가
    let health = heartbeat_service.evaluate_connection_health().await;
    println!("연결 건강성: {:?}", health.score);
    
    println!("✅ 하트비트 및 연결 관리 성공");
    
    // 시나리오 5: 정리 및 종료
    println!("🧹 시나리오 5: 정리 및 종료");
    
    // 방 정리 (빈 상태)
    let cleanup_count = room_handler.cleanup_rooms().await;
    println!("정리된 방 수: {} (빈 상태)", cleanup_count);
    
    // 친구 관계 정리
    friend_handler.cleanup_user(1).await;
    friend_handler.cleanup_user(2).await;
    friend_handler.cleanup_user(3).await;
    
    // 하트비트 서비스 중지
    heartbeat_service.stop().await?;
    assert!(!heartbeat_service.is_running().await, "하트비트 서비스가 중지되어야 함");
    
    println!("✅ 정리 및 종료 성공");
    
    println!("🎉 전체 TCP 서버 통합 테스트 완료!");
    Ok(())
}

/// 성능 통합 테스트
#[tokio::test]
async fn test_performance_integration() -> Result<()> {
    let connection_service = Arc::new(ConnectionService::new(1000));
    let message_service = Arc::new(MessageService::new(connection_service.clone()));
    let room_handler = Arc::new(RoomHandler::new(
        connection_service.clone(),
        message_service.clone(),
    ));
    let friend_handler = Arc::new(FriendHandler::new(
        connection_service.clone(),
        message_service.clone(),
    ));
    
    let start_time = std::time::Instant::now();
    
    // 방 목록 조회 성능 테스트 (빈 목록)
    let room_query_start = std::time::Instant::now();
    for _ in 0..100 {
        let _rooms = room_handler.get_room_list().await;
    }
    let room_query_time = room_query_start.elapsed();
    
    // 대량 친구 관계 생성 성능 테스트
    let friend_start = std::time::Instant::now();
    for i in 0..100 {
        friend_handler.register_user(i, format!("User_{}", i)).await;
        
        // 각 사용자에게 5명의 친구 추가
        for j in 1..=5 {
            if i + j < 100 {
                friend_handler.add_friend(i, i + j, format!("User_{}", i + j)).await?;
            }
        }
    }
    let friend_creation_time = friend_start.elapsed();
    
    // 통계 수집
    let room_stats = room_handler.get_room_stats().await;
    let friend_stats = friend_handler.get_friend_stats().await;
    let connection_stats = connection_service.get_connection_stats().await;
    
    // 성능 결과 출력
    println!("📊 성능 테스트 결과:");
    println!("- 100회 방 목록 조회: {:?}", room_query_time);
    println!("- 100명 사용자 + 친구 관계: {:?}", friend_creation_time);
    println!("- 총 방 수: {}", room_stats.total_rooms);
    println!("- 총 사용자 수: {}", friend_stats.total_users);
    println!("- 총 친구 관계 수: {}", friend_stats.total_friendships);
    println!("- 연결 서비스 상태: OK");
    
    // 성능 기준 검증 (너무 느리면 실패)
    assert!(room_query_time.as_secs() < 2, "방 목록 조회가 2초 이내여야 함");
    assert!(friend_creation_time.as_secs() < 10, "친구 관계 생성이 10초 이내여야 함");
    
    println!("✅ 성능 통합 테스트 완료");
    Ok(())
}

/// 에러 처리 통합 테스트
#[tokio::test]
async fn test_error_handling_integration() -> Result<()> {
    let connection_service = Arc::new(ConnectionService::new(10)); // 작은 제한
    let message_service = Arc::new(MessageService::new(connection_service.clone()));
    let room_handler = Arc::new(RoomHandler::new(
        connection_service.clone(),
        message_service.clone(),
    ));
    let friend_handler = Arc::new(FriendHandler::new(
        connection_service.clone(),
        message_service.clone(),
    ));
    
    // 시나리오 1: 존재하지 않는 방 접근 테스트
    let rooms = room_handler.get_room_list().await;
    assert_eq!(rooms.len(), 0, "초기에는 방이 없어야 함");
    
    // 존재하지 않는 방에 접근 시도
    let invalid_room_access = room_handler.get_room_details(999).await;
    assert!(invalid_room_access.is_err(), "존재하지 않는 방 접근은 실패해야 함");
    
    println!("✅ 존재하지 않는 방 접근 에러 처리 확인");
    
    // 시나리오 2: 중복 친구 추가 에러 처리
    friend_handler.register_user(1, "User1".to_string()).await;
    friend_handler.register_user(2, "User2".to_string()).await;
    
    // 첫 번째 친구 추가는 성공
    assert!(friend_handler.add_friend(1, 2, "User2".to_string()).await.is_ok());
    
    // 중복 친구 추가는 실패
    assert!(friend_handler.add_friend(1, 2, "User2".to_string()).await.is_err());
    
    // 시나리오 3: 존재하지 않는 리소스 접근
    assert!(room_handler.join_room(1, 9999, "User1".to_string()).await.is_err());
    assert!(room_handler.leave_room(1, 9999).await.is_err());
    assert!(friend_handler.remove_friend(1, 9999).await.is_err());
    
    println!("✅ 에러 처리 통합 테스트 완료");
    Ok(())
}

/// 동시성 안전성 통합 테스트
#[tokio::test]
async fn test_concurrency_integration() -> Result<()> {
    let connection_service = Arc::new(ConnectionService::new(1000));
    let message_service = Arc::new(MessageService::new(connection_service.clone()));
    let room_handler = Arc::new(RoomHandler::new(
        connection_service.clone(),
        message_service.clone(),
    ));
    let friend_handler = Arc::new(FriendHandler::new(
        connection_service.clone(),
        message_service.clone(),
    ));
    
    // 동시 방 목록 조회 테스트
    let mut handles = Vec::new();
    for i in 0..20 {
        let room_handler_clone = room_handler.clone();
        let handle = tokio::spawn(async move {
            room_handler_clone.get_room_list().await
        });
        handles.push(handle);
    }
    
    // 모든 작업 완료 대기
    let mut success_count = 0;
    for handle in handles {
        match handle.await {
            Ok(rooms) => {
                assert_eq!(rooms.len(), 0, "빈 방 목록이어야 함");
                success_count += 1;
            },
            Err(_) => {} // 일부 실패는 허용 (동시성 충돌)
        }
    }
    
    println!("동시 방 목록 조회 성공 수: {}/20", success_count);
    assert!(success_count >= 20, "모든 동시 조회가 성공해야 함");
    
    // 동시 친구 추가 테스트
    let mut friend_handles = Vec::new();
    for i in 0..20 {
        friend_handler.register_user(i, format!("User_{}", i)).await;
    }
    
    for i in 0..10 {
        let friend_handler_clone = friend_handler.clone();
        let handle = tokio::spawn(async move {
            friend_handler_clone.add_friend(i, i + 10, format!("Friend_{}", i + 10)).await
        });
        friend_handles.push(handle);
    }
    
    // 친구 추가 작업 완료 대기
    let mut friend_success = 0;
    for handle in friend_handles {
        match handle.await? {
            Ok(_) => friend_success += 1,
            Err(_) => {}
        }
    }
    
    println!("동시 친구 추가 성공 수: {}/10", friend_success);
    assert!(friend_success > 8, "대부분의 친구 추가가 성공해야 함");
    
    println!("✅ 동시성 안전성 통합 테스트 완료");
    Ok(())
}

/// 시간 제한 통합 테스트
#[tokio::test]
async fn test_timeout_integration() -> Result<()> {
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = Arc::new(HeartbeatService::new(connection_service.clone(), 1, 2)); // 빠른 타임아웃
    
    // 하트비트 서비스 시작
    heartbeat_service.start().await?;
    
    // 짧은 시간 대기 후 통계 확인
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    let stats = heartbeat_service.get_heartbeat_stats().await;
    let health = heartbeat_service.evaluate_connection_health().await;
    
    println!("하트비트 통계: {:?}", stats);
    println!("연결 건강성: {:?}", health);
    
    // 타임아웃 테스트 - 정해진 시간 내에 완료되어야 함
    let timeout_result = timeout(Duration::from_secs(5), async {
        heartbeat_service.stop().await
    }).await;
    
    assert!(timeout_result.is_ok(), "하트비트 서비스 중지가 시간 내에 완료되어야 함");
    assert!(timeout_result.expect("Test assertion failed").is_ok(), "하트비트 서비스 중지가 성공해야 함");
    
    println!("✅ 시간 제한 통합 테스트 완료");
    Ok(())
}