//! 프로토콜 테스트
//! 
//! GameMessage 직렬화/역직렬화, 바이너리 변환 테스트

use crate::protocol::GameMessage;

/// 기본 하트비트 메시지 테스트
#[tokio::test]
async fn test_heartbeat_message() {
    let message = GameMessage::HeartBeat;
    
    // 직렬화 테스트
    let bytes = message.to_bytes().unwrap();
    assert!(bytes.len() > 4, "메시지 크기가 헤더보다 커야 함");
    
    // 역직렬화 테스트
    let decoded = GameMessage::from_bytes(&bytes).unwrap();
    assert!(matches!(decoded, GameMessage::HeartBeat));
    
    println!("✅ HeartBeat 메시지 테스트 통과");
}

/// 하트비트 응답 메시지 테스트
#[tokio::test]
async fn test_heartbeat_response_message() {
    let timestamp = chrono::Utc::now().timestamp();
    let message = GameMessage::HeartBeatResponse { timestamp };
    
    // 직렬화/역직렬화 테스트
    let bytes = message.to_bytes().unwrap();
    let decoded = GameMessage::from_bytes(&bytes).unwrap();
    
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
    let client_id = 12345;
    let message = GameMessage::ConnectionAck { client_id };
    
    let bytes = message.to_bytes().unwrap();
    let decoded = GameMessage::from_bytes(&bytes).unwrap();
    
    if let GameMessage::ConnectionAck { client_id: id } = decoded {
        assert_eq!(id, client_id, "클라이언트 ID가 일치해야 함");
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
    
    let bytes = message.to_bytes().unwrap();
    let decoded = GameMessage::from_bytes(&bytes).unwrap();
    
    if let GameMessage::Error { code: c, message: m } = decoded {
        assert_eq!(c, code, "에러 코드가 일치해야 함");
        assert_eq!(m, msg, "에러 메시지가 일치해야 함");
    } else {
        panic!("메시지 타입이 Error가 아님");
    }
    
    println!("✅ Error 메시지 테스트 통과");
}