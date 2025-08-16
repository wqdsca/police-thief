//! 핸들러 레이어 테스트
//! 
//! Handler 모듈들의 메시지 처리 로직 테스트

use crate::handler::{ServerMessageHandler, RoomHandler, FriendHandler};
use crate::service::{ConnectionService, HeartbeatService, MessageService};
use crate::protocol::GameMessage;
use std::sync::Arc;
use anyhow::Result;

/// ServerMessageHandler 생성 테스트
#[tokio::test]
async fn test_server_message_handler_creation() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let message_service = Arc::new(MessageService::new(connection_service.clone()));
    let heartbeat_service = Arc::new(HeartbeatService::with_default_config(connection_service.clone()));
    
    let handler = ServerMessageHandler::new(
        connection_service,
        heartbeat_service,
        message_service,
    );
    
    // 핸들러가 정상적으로 생성되었는지 확인
    println!("✅ ServerMessageHandler 생성 테스트 통과");
}

/// ServerMessageHandler 핸들러 등록 테스트
#[tokio::test]
async fn test_message_handler_registration() -> Result<()> {
    let connection_service = Arc::new(ConnectionService::new(100));
    let message_service = Arc::new(MessageService::new(connection_service.clone()));
    let heartbeat_service = Arc::new(HeartbeatService::with_default_config(connection_service.clone()));
    
    let handler = ServerMessageHandler::new(
        connection_service,
        heartbeat_service,
        message_service,
    );
    
    // 모든 핸들러 등록 테스트
    let result = handler.register_all_handlers().await;
    assert!(result.is_ok(), "핸들러 등록이 성공해야 함");
    
    println!("✅ ServerMessageHandler 핸들러 등록 테스트 통과");
    Ok(())
}

/// RoomHandler 생성 및 기본 기능 테스트
#[tokio::test]
async fn test_room_handler() -> Result<()> {
    let connection_service = Arc::new(ConnectionService::new(100));
    let message_service = Arc::new(MessageService::new(connection_service.clone()));
    let room_handler = RoomHandler::new(connection_service, message_service);
    
    // 방 목록 조회 테스트 (빈 목록)
    let rooms = room_handler.get_room_list().await;
    assert_eq!(rooms.len(), 0, "초기에는 방이 없어야 함");
    
    // 존재하지 않는 방에 입장 시도
    let join_result = room_handler.join_room(1, 999, "User1".to_string()).await;
    assert!(join_result.is_err(), "존재하지 않는 방 입장은 실패해야 함");
    
    // 사용자 방 조회 테스트 (없음)
    let user_room = room_handler.get_user_room(1).await;
    assert_eq!(user_room, None, "방에 속하지 않은 사용자는 None이어야 함");
    
    // 방 통계 조회
    let stats = room_handler.get_room_stats().await;
    assert_eq!(stats.total_rooms, 0, "방 수는 0이어야 함");
    assert_eq!(stats.total_users, 0, "총 사용자 수는 0이어야 함");
    
    println!("✅ RoomHandler 테스트 통과");
    Ok(())
}

/// FriendHandler 생성 및 기본 기능 테스트
#[tokio::test]
async fn test_friend_handler() -> Result<()> {
    let connection_service = Arc::new(ConnectionService::new(100));
    let message_service = Arc::new(MessageService::new(connection_service.clone()));
    let friend_handler = FriendHandler::new(connection_service, message_service);
    
    // 사용자 등록
    friend_handler.register_user(1, "User1".to_string()).await;
    friend_handler.register_user(2, "User2".to_string()).await;
    
    // 친구 추가 테스트
    let add_result = friend_handler.add_friend(1, 2, "User2".to_string()).await;
    assert!(add_result.is_ok(), "친구 추가가 성공해야 함");
    
    // 친구 관계 확인 테스트
    let is_friend = friend_handler.is_friend(1, 2).await;
    assert!(is_friend, "친구 관계가 확인되어야 함");
    
    // 친구 목록 조회 테스트
    let friends = friend_handler.get_friend_list(1).await;
    assert_eq!(friends.len(), 1, "친구가 1명 있어야 함");
    assert_eq!(friends[0].user_id, 2, "친구 ID가 일치해야 함");
    
    // 친구 수 조회 테스트
    let friend_count = friend_handler.get_friend_count(1).await;
    assert_eq!(friend_count, 1, "친구 수가 1명이어야 함");
    
    // 친구 삭제 테스트
    let remove_result = friend_handler.remove_friend(1, 2).await;
    assert!(remove_result.is_ok(), "친구 삭제가 성공해야 함");
    
    // 삭제 후 확인
    let is_friend_after = friend_handler.is_friend(1, 2).await;
    assert!(!is_friend_after, "친구 관계가 해제되어야 함");
    
    println!("✅ FriendHandler 테스트 통과");
    Ok(())
}

/// 메시지 검증 테스트
#[tokio::test]
async fn test_message_validation() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let message_service = Arc::new(MessageService::new(connection_service.clone()));
    let heartbeat_service = Arc::new(HeartbeatService::with_default_config(connection_service.clone()));
    
    let handler = ServerMessageHandler::new(
        connection_service,
        heartbeat_service,
        message_service,
    );
    
    // 정상적인 하트비트 메시지
    let heartbeat_msg = GameMessage::HeartBeat;
    assert!(handler.validate_message(1, &heartbeat_msg).is_ok());
    
    // 정상적인 채팅 메시지
    let chat_msg = GameMessage::ChatMessage {
        user_id: 1,
        room_id: 1,
        content: "안녕하세요!".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    };
    assert!(handler.validate_message(1, &chat_msg).is_ok());
    
    // 잘못된 사용자 ID (채팅)
    let invalid_chat = GameMessage::ChatMessage {
        user_id: 2, // 다른 사용자 ID
        room_id: 1,
        content: "안녕하세요!".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    };
    assert!(handler.validate_message(1, &invalid_chat).is_err());
    
    // 빈 채팅 내용
    let empty_chat = GameMessage::ChatMessage {
        user_id: 1,
        room_id: 1,
        content: "".to_string(), // 빈 내용
        timestamp: chrono::Utc::now().timestamp(),
    };
    assert!(handler.validate_message(1, &empty_chat).is_err());
    
    // 너무 긴 채팅 내용
    let long_content = "a".repeat(1001); // 1000자 초과
    let long_chat = GameMessage::ChatMessage {
        user_id: 1,
        room_id: 1,
        content: long_content,
        timestamp: chrono::Utc::now().timestamp(),
    };
    assert!(handler.validate_message(1, &long_chat).is_err());
    
    // 자기 자신을 친구로 추가
    let self_friend = GameMessage::FriendAdd {
        user_id: 1,
        friend_user_id: 1, // 자기 자신
        nickname: "Me".to_string(),
    };
    assert!(handler.validate_message(1, &self_friend).is_err());
    
    println!("✅ 메시지 검증 테스트 통과");
}

/// 핸들러 에러 처리 테스트
#[tokio::test]
async fn test_handler_error_handling() -> Result<()> {
    let connection_service = Arc::new(ConnectionService::new(100));
    let message_service = Arc::new(MessageService::new(connection_service.clone()));
    let room_handler = RoomHandler::new(connection_service.clone(), message_service.clone());
    let friend_handler = FriendHandler::new(connection_service, message_service);
    
    // RoomHandler 에러 처리 테스트
    // 존재하지 않는 방에 입장 시도
    let join_invalid_room = room_handler.join_room(1, 999, "User1".to_string()).await;
    assert!(join_invalid_room.is_err(), "존재하지 않는 방 입장은 실패해야 함");
    
    // 존재하지 않는 방에서 퇴장 시도
    let leave_invalid_room = room_handler.leave_room(1, 999).await;
    assert!(leave_invalid_room.is_err(), "존재하지 않는 방 퇴장은 실패해야 함");
    
    // FriendHandler 에러 처리 테스트
    // 자기 자신을 친구로 추가
    let self_friend = friend_handler.add_friend(1, 1, "Self".to_string()).await;
    assert!(self_friend.is_err(), "자기 자신을 친구로 추가하는 것은 실패해야 함");
    
    // 존재하지 않는 친구 삭제
    let remove_nonexistent = friend_handler.remove_friend(1, 999).await;
    assert!(remove_nonexistent.is_err(), "존재하지 않는 친구 삭제는 실패해야 함");
    
    println!("✅ 핸들러 에러 처리 테스트 통과");
    Ok(())
}

/// 채팅 기록 관리 테스트
#[tokio::test]
async fn test_chat_history_management() {
    let connection_service = Arc::new(ConnectionService::new(100));
    let message_service = Arc::new(MessageService::new(connection_service.clone()));
    let heartbeat_service = Arc::new(HeartbeatService::with_default_config(connection_service.clone()));
    
    let handler = ServerMessageHandler::new(
        connection_service,
        heartbeat_service,
        message_service,
    );
    
    let room_id = 1;
    
    // 초기 채팅 기록 확인 (빈 목록)
    let initial_history = handler.get_chat_history(room_id).await;
    assert_eq!(initial_history.len(), 0, "초기 채팅 기록은 비어있어야 함");
    
    // 채팅 기록 정리 테스트
    handler.cleanup_chat_history(room_id).await;
    let cleaned_history = handler.get_chat_history(room_id).await;
    assert_eq!(cleaned_history.len(), 0, "정리 후 채팅 기록은 비어있어야 함");
    
    println!("✅ 채팅 기록 관리 테스트 통과");
}

/// 핸들러 성능 기본 테스트
#[tokio::test]
async fn test_handler_performance_basic() -> Result<()> {
    let connection_service = Arc::new(ConnectionService::new(100));
    let message_service = Arc::new(MessageService::new(connection_service.clone()));
    let room_handler = RoomHandler::new(connection_service, message_service);
    
    let start_time = std::time::Instant::now();
    
    // 방 목록 조회 성능 테스트 (빈 목록)
    let list_start = std::time::Instant::now();
    let rooms = room_handler.get_room_list().await;
    let list_time = list_start.elapsed();
    
    assert_eq!(rooms.len(), 0, "초기에는 방이 없어야 함");
    println!("빈 방 목록 조회 시간: {:?}", list_time);
    
    // 방 정리 성능 테스트 (빈 상태)
    let cleanup_start = std::time::Instant::now();
    let cleanup_count = room_handler.cleanup_rooms().await;
    let cleanup_time = cleanup_start.elapsed();
    
    assert_eq!(cleanup_count, 0, "정리할 방이 없어야 함");
    println!("빈 상태 방 정리 시간: {:?}, 정리된 방 수: {}", cleanup_time, cleanup_count);
    
    // 방 통계 조회 성능 테스트
    let stats_start = std::time::Instant::now();
    let stats = room_handler.get_room_stats().await;
    let stats_time = stats_start.elapsed();
    
    assert_eq!(stats.total_rooms, 0);
    assert_eq!(stats.total_users, 0);
    println!("방 통계 조회 시간: {:?}", stats_time);
    
    let total_time = start_time.elapsed();
    println!("전체 테스트 시간: {:?}", total_time);
    
    println!("✅ 핸들러 성능 기본 테스트 통과");
    Ok(())
}