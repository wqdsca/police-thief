//! 프로토콜 테스트
//! 
//! GameMessage 직렬화/역직렬화, 바이너리 변환 테스트

use crate::protocol::GameMessage;

/// 기본 하트비트 메시지 테스트
#[tokio::test]
async fn test_heartbeat_message() {
    let message = GameMessage::HeartBeat;
    
    // 직렬화 테스트
    let bytes = message.to_bytes().expect("Test assertion failed");
    assert!(bytes.len() > 4, "메시지 크기가 헤더보다 커야 함");
    
    // 역직렬화 테스트
    let decoded = GameMessage::from_bytes(&bytes).expect("Test assertion failed");
    assert!(matches!(decoded, GameMessage::HeartBeat));
    
    println!("✅ HeartBeat 메시지 테스트 통과");
}

/// 하트비트 응답 메시지 테스트
#[tokio::test]
async fn test_heartbeat_response_message() {
    let timestamp = chrono::Utc::now().timestamp();
    let message = GameMessage::HeartBeatResponse { timestamp };
    
    // 직렬화/역직렬화 테스트
    let bytes = message.to_bytes().expect("Test assertion failed");
    let decoded = GameMessage::from_bytes(&bytes).expect("Test assertion failed");
    
    if let GameMessage::HeartBeatResponse { timestamp: t } = decoded {
        assert_eq!(t, timestamp, "타임스탬프가 일치해야 함");
    } else {
        panic!("메시지 타입이 HeartBeatResponse가 아님");
    }
    
    println!("✅ HeartBeatResponse 메시지 테스트 통과");
}

/// 연결 확인 메시지 테스트
#[tokio::test]
async fn test_connection_ack_message() {
    let user_id = 12345;
    let message = GameMessage::ConnectionAck { user_id };
    
    let bytes = message.to_bytes().expect("Test assertion failed");
    let decoded = GameMessage::from_bytes(&bytes).expect("Test assertion failed");
    
    if let GameMessage::ConnectionAck { user_id: id } = decoded {
        assert_eq!(id, user_id, "사용자 ID가 일치해야 함");
    } else {
        panic!("메시지 타입이 ConnectionAck가 아님");
    }
    
    println!("✅ ConnectionAck 메시지 테스트 통과");
}

/// 에러 메시지 테스트
#[tokio::test]
async fn test_error_message() {
    let code = 404;
    let msg = "Resource not found".to_string();
    let message = GameMessage::Error { 
        code, 
        message: msg.clone() 
    };
    
    let bytes = message.to_bytes().expect("Test assertion failed");
    let decoded = GameMessage::from_bytes(&bytes).expect("Test assertion failed");
    
    if let GameMessage::Error { code: c, message: m } = decoded {
        assert_eq!(c, code, "에러 코드가 일치해야 함");
        assert_eq!(m, msg, "에러 메시지가 일치해야 함");
    } else {
        panic!("메시지 타입이 Error가 아님");
    }
    
    println!("✅ Error 메시지 테스트 통과");
}

/// 방 입장 메시지 테스트
#[tokio::test]
async fn test_room_join_message() {
    let user_id = 123;
    let room_id = 456;
    let nickname = "TestUser".to_string();
    let message = GameMessage::RoomJoin { 
        user_id, 
        room_id, 
        nickname: nickname.clone() 
    };
    
    let bytes = message.to_bytes().expect("Test assertion failed");
    let decoded = GameMessage::from_bytes(&bytes).expect("Test assertion failed");
    
    if let GameMessage::RoomJoin { user_id: uid, room_id: rid, nickname: nick } = decoded {
        assert_eq!(uid, user_id, "사용자 ID가 일치해야 함");
        assert_eq!(rid, room_id, "방 ID가 일치해야 함");
        assert_eq!(nick, nickname, "닉네임이 일치해야 함");
    } else {
        panic!("메시지 타입이 RoomJoin이 아님");
    }
    
    println!("✅ RoomJoin 메시지 테스트 통과");
}

/// 채팅 메시지 테스트
#[tokio::test]
async fn test_chat_message() {
    let user_id = 123;
    let room_id = 456;
    let content = "안녕하세요!".to_string();
    let timestamp = chrono::Utc::now().timestamp();
    let message = GameMessage::ChatMessage { 
        user_id, 
        room_id, 
        content: content.clone(),
        timestamp
    };
    
    let bytes = message.to_bytes().expect("Test assertion failed");
    let decoded = GameMessage::from_bytes(&bytes).expect("Test assertion failed");
    
    if let GameMessage::ChatMessage { user_id: uid, room_id: rid, content: msg, timestamp: ts } = decoded {
        assert_eq!(uid, user_id, "사용자 ID가 일치해야 함");
        assert_eq!(rid, room_id, "방 ID가 일치해야 함");
        assert_eq!(msg, content, "채팅 내용이 일치해야 함");
        assert_eq!(ts, timestamp, "타임스탬프가 일치해야 함");
    } else {
        panic!("메시지 타입이 ChatMessage가 아님");
    }
    
    println!("✅ ChatMessage 메시지 테스트 통과");
}

/// 친구 추가 메시지 테스트
#[tokio::test]
async fn test_friend_add_message() {
    let user_id = 123;
    let friend_user_id = 456;
    let nickname = "Friend".to_string();
    let message = GameMessage::FriendAdd { 
        user_id, 
        friend_user_id, 
        nickname: nickname.clone() 
    };
    
    let bytes = message.to_bytes().expect("Test assertion failed");
    let decoded = GameMessage::from_bytes(&bytes).expect("Test assertion failed");
    
    if let GameMessage::FriendAdd { user_id: uid, friend_user_id: fuid, nickname: nick } = decoded {
        assert_eq!(uid, user_id, "사용자 ID가 일치해야 함");
        assert_eq!(fuid, friend_user_id, "친구 ID가 일치해야 함");
        assert_eq!(nick, nickname, "닉네임이 일치해야 함");
    } else {
        panic!("메시지 타입이 FriendAdd가 아님");
    }
    
    println!("✅ FriendAdd 메시지 테스트 통과");
}

/// 친구 삭제 메시지 테스트
#[tokio::test]
async fn test_friend_remove_message() {
    let user_id = 123;
    let friend_user_id = 456;
    let message = GameMessage::FriendRemove { 
        user_id, 
        friend_user_id
    };
    
    let bytes = message.to_bytes().expect("Test assertion failed");
    let decoded = GameMessage::from_bytes(&bytes).expect("Test assertion failed");
    
    if let GameMessage::FriendRemove { user_id: uid, friend_user_id: fuid } = decoded {
        assert_eq!(uid, user_id, "사용자 ID가 일치해야 함");
        assert_eq!(fuid, friend_user_id, "친구 ID가 일치해야 함");
    } else {
        panic!("메시지 타입이 FriendRemove가 아님");
    }
    
    println!("✅ FriendRemove 메시지 테스트 통과");
}

/// 잘못된 데이터 처리 테스트
#[tokio::test]
async fn test_invalid_data_handling() {
    // 너무 짧은 데이터
    let invalid_data = vec![0, 0];
    assert!(GameMessage::from_bytes(&invalid_data).is_err());
    
    // 길이가 맞지 않는 데이터
    let invalid_data = vec![0, 0, 0, 10, 1, 2, 3]; // 길이 10이라고 했지만 실제로는 3바이트만
    assert!(GameMessage::from_bytes(&invalid_data).is_err());
    
    println!("✅ 잘못된 데이터 처리 테스트 통과");
}