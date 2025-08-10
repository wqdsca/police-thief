//! 채팅방 시스템 통합 테스트
//! 
//! DashMap 기반 RoomConnectionService와 ChatRoomHandler의 통합 기능을 테스트합니다.
//! 방 입장/퇴장, 채팅 메시지, 빈 방 정리 등의 핵심 기능을 검증합니다.

use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;

use crate::service::room_connection_service::RoomConnectionService;
use crate::handler::chat_room_handler::ChatRoomHandler;

/// 채팅방 통합 테스트 환경
struct ChatRoomTestEnv {
    room_service: Arc<RoomConnectionService>,
    chat_handler: Arc<ChatRoomHandler>,
}

impl ChatRoomTestEnv {
    /// 테스트 환경 생성
    async fn new() -> Self {
        let room_service = Arc::new(RoomConnectionService::new("test_server".to_string()));
        let chat_handler = Arc::new(ChatRoomHandler::new(room_service.clone()));

        Self {
            room_service,
            chat_handler,
        }
    }

    /// 모의 TCP writer 생성 (비동기 버전)
    async fn create_mock_writer() -> Arc<Mutex<tokio::io::BufWriter<tokio::net::tcp::OwnedWriteHalf>>> {
        // 테스트용 TCP 연결 생성
        use tokio::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        
        // 클라이언트와 서버 연결 생성
        let client_task = tokio::spawn(async move {
            TcpStream::connect(addr).await.unwrap()
        });
        
        let (server_stream, _) = listener.accept().await.unwrap();
        let _client_stream = client_task.await.unwrap();
        
        // 서버 쪽 스트림을 분리하여 writer 생성
        let (_, writer) = server_stream.into_split();
        let buf_writer = tokio::io::BufWriter::new(writer);
        
        Arc::new(Mutex::new(buf_writer))
    }

    /// 테스트용 사용자 추가
    async fn add_test_user(&self, room_id: u32, user_id: u32, nickname: &str) -> Result<()> {
        let writer = Self::create_mock_writer().await;
        let addr = format!("127.0.0.1:{}", 10000 + user_id);
        
        self.room_service
            .add_user_to_room(room_id, user_id, addr, nickname.to_string(), writer)
            .await
    }
}

/// 기본적인 방 입장/퇴장 테스트
#[tokio::test]
async fn test_room_join_leave() {
    let env = ChatRoomTestEnv::new().await;
    
    // 초기 상태 확인
    assert_eq!(env.chat_handler.get_all_rooms_status().len(), 0);
    
    // 사용자 방 입장
    let writer = ChatRoomTestEnv::create_mock_writer().await;
    let result = env.chat_handler.handle_room_join(
        1, 100, "테스터1".to_string(), "127.0.0.1:10001".to_string(), writer
    ).await;
    
    assert!(result.is_ok());
    let user_count = result.unwrap();
    assert_eq!(user_count, 1);
    
    // 방 상태 확인
    let (room_user_count, users) = env.chat_handler.get_room_status(100);
    assert_eq!(room_user_count, 1);
    assert_eq!(users.len(), 1);
    assert_eq!(users[0], (1, "테스터1".to_string()));
    
    // 방 퇴장
    let leave_result = env.chat_handler.handle_room_leave(1, 100).await;
    assert!(leave_result.is_ok());
    let room_deleted = leave_result.unwrap();
    assert!(room_deleted); // 마지막 사용자가 나가면 방 삭제
    
    // 방이 삭제되었는지 확인
    assert_eq!(env.chat_handler.get_all_rooms_status().len(), 0);
}

/// 여러 사용자 방 입장 테스트
#[tokio::test] 
async fn test_multiple_users_room() {
    let env = ChatRoomTestEnv::new().await;
    
    // 첫 번째 사용자 입장
    let writer1 = ChatRoomTestEnv::create_mock_writer().await;
    let result1 = env.chat_handler.handle_room_join(
        1, 200, "사용자1".to_string(), "127.0.0.1:10001".to_string(), writer1
    ).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap(), 1);
    
    // 두 번째 사용자 입장
    let writer2 = ChatRoomTestEnv::create_mock_writer().await;
    let result2 = env.chat_handler.handle_room_join(
        2, 200, "사용자2".to_string(), "127.0.0.1:10002".to_string(), writer2
    ).await;
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap(), 2);
    
    // 세 번째 사용자 입장
    let writer3 = ChatRoomTestEnv::create_mock_writer().await;
    let result3 = env.chat_handler.handle_room_join(
        3, 200, "사용자3".to_string(), "127.0.0.1:10003".to_string(), writer3
    ).await;
    assert!(result3.is_ok());
    assert_eq!(result3.unwrap(), 3);
    
    // 방 상태 확인
    let (room_user_count, users) = env.chat_handler.get_room_status(200);
    assert_eq!(room_user_count, 3);
    assert_eq!(users.len(), 3);
    
    // 사용자 2만 퇴장
    let leave_result = env.chat_handler.handle_room_leave(2, 200).await;
    assert!(leave_result.is_ok());
    let room_deleted = leave_result.unwrap();
    assert!(!room_deleted); // 아직 다른 사용자가 있으므로 방 유지
    
    // 방 상태 재확인
    let (room_user_count, users) = env.chat_handler.get_room_status(200);
    assert_eq!(room_user_count, 2);
    assert_eq!(users.len(), 2);
    
    // 나머지 사용자들 퇴장
    env.chat_handler.handle_room_leave(1, 200).await.unwrap();
    let leave_result = env.chat_handler.handle_room_leave(3, 200).await;
    assert!(leave_result.is_ok());
    let room_deleted = leave_result.unwrap();
    assert!(room_deleted); // 마지막 사용자가 나가면 방 삭제
}

/// 채팅 메시지 테스트
#[tokio::test]
async fn test_chat_messaging() {
    let env = ChatRoomTestEnv::new().await;
    
    // 사용자들을 방에 추가 (직접 room_service 사용)
    env.add_test_user(300, 1, "채터1").await.unwrap();
    env.add_test_user(300, 2, "채터2").await.unwrap();
    env.add_test_user(300, 3, "채터3").await.unwrap();
    
    // 방 상태 확인
    let (user_count, _) = env.chat_handler.get_room_status(300);
    assert_eq!(user_count, 3);
    
    // 채팅 메시지 전송 테스트
    let chat_result = env.chat_handler.handle_chat_message(
        1, 300, "안녕하세요!".to_string()
    ).await;
    
    assert!(chat_result.is_ok());
    let sent_count = chat_result.unwrap();
    assert_eq!(sent_count, 3); // 방에 있는 모든 사용자(3명)에게 전송
    
    // 빈 메시지 테스트
    let empty_chat_result = env.chat_handler.handle_chat_message(
        1, 300, "".to_string()
    ).await;
    assert!(empty_chat_result.is_err());
    
    // 존재하지 않는 방에 메시지 전송 테스트
    let invalid_room_result = env.chat_handler.handle_chat_message(
        1, 999, "존재하지 않는 방".to_string()
    ).await;
    assert!(invalid_room_result.is_err());
}

/// 사용자 연결 해제 테스트
#[tokio::test]
async fn test_user_disconnect() {
    let env = ChatRoomTestEnv::new().await;
    
    // 사용자들을 여러 방에 추가
    env.add_test_user(400, 1, "연결자1").await.unwrap();
    env.add_test_user(400, 2, "연결자2").await.unwrap();
    env.add_test_user(401, 3, "연결자3").await.unwrap();
    
    // 초기 상태 확인
    assert_eq!(env.chat_handler.get_all_rooms_status().len(), 2);
    let (count_400, _) = env.chat_handler.get_room_status(400);
    let (count_401, _) = env.chat_handler.get_room_status(401);
    assert_eq!(count_400, 2);
    assert_eq!(count_401, 1);
    
    // 사용자 1 연결 해제 처리
    let disconnect_result = env.chat_handler.handle_user_disconnect(1).await;
    assert!(disconnect_result.is_ok());
    let cleaned_rooms = disconnect_result.unwrap();
    assert_eq!(cleaned_rooms, 0); // 방 400에 아직 사용자 2가 있으므로 방은 삭제되지 않음
    
    // 상태 확인
    let (count_400, _) = env.chat_handler.get_room_status(400);
    let (count_401, _) = env.chat_handler.get_room_status(401);
    assert_eq!(count_400, 1); // 사용자 1이 제거됨
    assert_eq!(count_401, 1); // 변화 없음
    
    // 사용자 3 연결 해제 처리 (방 401의 유일한 사용자)
    let disconnect_result = env.chat_handler.handle_user_disconnect(3).await;
    assert!(disconnect_result.is_ok());
    let cleaned_rooms = disconnect_result.unwrap();
    assert_eq!(cleaned_rooms, 1); // 방 401이 삭제됨
    
    // 전체 방 수 확인
    assert_eq!(env.chat_handler.get_all_rooms_status().len(), 1); // 방 400만 남음
}

/// 빈 방 정리 테스트
#[tokio::test]
async fn test_cleanup_empty_rooms() {
    let env = ChatRoomTestEnv::new().await;
    
    // 초기에는 빈 방이 없음
    let cleaned = env.chat_handler.cleanup_empty_rooms().await;
    assert_eq!(cleaned, 0);
    
    // 사용자를 추가한 후 제거하여 빈 방 생성
    env.add_test_user(500, 1, "임시사용자").await.unwrap();
    assert_eq!(env.chat_handler.get_all_rooms_status().len(), 1);
    
    // 사용자 제거 (방이 빈 상태가 됨)
    env.room_service.remove_user_from_room(500, 1).await.unwrap();
    
    // 빈 방 정리
    let cleaned = env.chat_handler.cleanup_empty_rooms().await;
    assert_eq!(cleaned, 1);
    assert_eq!(env.chat_handler.get_all_rooms_status().len(), 0);
}

/// 방 이동 테스트
#[tokio::test]
async fn test_room_movement() {
    let env = ChatRoomTestEnv::new().await;
    
    // 사용자를 첫 번째 방에 추가
    let writer1 = ChatRoomTestEnv::create_mock_writer().await;
    env.chat_handler.handle_room_join(
        1, 600, "이동자".to_string(), "127.0.0.1:10001".to_string(), writer1
    ).await.unwrap();
    
    // 첫 번째 방 상태 확인
    let (count_600, _) = env.chat_handler.get_room_status(600);
    assert_eq!(count_600, 1);
    
    // 같은 사용자가 다른 방에 입장 (자동으로 기존 방에서 퇴장)
    let writer2 = ChatRoomTestEnv::create_mock_writer().await;
    env.chat_handler.handle_room_join(
        1, 601, "이동자".to_string(), "127.0.0.1:10001".to_string(), writer2
    ).await.unwrap();
    
    // 방 상태 확인
    let (count_600, _) = env.chat_handler.get_room_status(600);
    let (count_601, _) = env.chat_handler.get_room_status(601);
    assert_eq!(count_600, 0); // 기존 방에서 자동 퇴장
    assert_eq!(count_601, 1); // 새 방으로 이동
    
    // 전체 활성 방 수 확인 (빈 방은 자동 정리됨)
    let active_rooms = env.chat_handler.get_all_rooms_status();
    assert_eq!(active_rooms.len(), 1);
    assert_eq!(active_rooms[0], (601, 1));
}

/// 동시성 테스트 - 여러 사용자가 동시에 방 입장/퇴장
#[tokio::test]
async fn test_concurrent_room_operations() {
    let env = Arc::new(ChatRoomTestEnv::new().await);
    
    let mut handles = Vec::new();
    
    // 10명의 사용자가 동시에 방 700에 입장
    for user_id in 1..=10 {
        let env_clone = env.clone();
        let handle = tokio::spawn(async move {
            let writer = ChatRoomTestEnv::create_mock_writer().await;
            env_clone.chat_handler.handle_room_join(
                user_id,
                700,
                format!("동시사용자{}", user_id),
                format!("127.0.0.1:{}", 10000 + user_id),
                writer,
            ).await
        });
        handles.push(handle);
    }
    
    // 모든 작업 완료 대기
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
    
    // 최종 상태 확인
    let (final_count, users) = env.chat_handler.get_room_status(700);
    assert_eq!(final_count, 10);
    assert_eq!(users.len(), 10);
    
    // 모든 사용자 동시 퇴장
    let mut leave_handles = Vec::new();
    for user_id in 1..=10 {
        let env_clone = env.clone();
        let handle = tokio::spawn(async move {
            env_clone.chat_handler.handle_room_leave(user_id, 700).await
        });
        leave_handles.push(handle);
    }
    
    let mut room_deletions = 0;
    for handle in leave_handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        if result.unwrap() {
            room_deletions += 1;
        }
    }
    
    // 정확히 한 번만 방이 삭제되어야 함 (마지막으로 나가는 사용자가 삭제)
    assert_eq!(room_deletions, 1);
    assert_eq!(env.chat_handler.get_all_rooms_status().len(), 0);
}

/// 에러 처리 테스트
#[tokio::test]
async fn test_error_handling() {
    let env = ChatRoomTestEnv::new().await;
    
    // 존재하지 않는 방에서 퇴장 시도
    let leave_result = env.chat_handler.handle_room_leave(999, 999).await;
    assert!(leave_result.is_err());
    
    // 방에 없는 사용자의 채팅 시도
    let chat_result = env.chat_handler.handle_chat_message(
        999, 999, "존재하지 않는 방".to_string()
    ).await;
    assert!(chat_result.is_err());
    
    // 존재하지 않는 사용자 연결 해제
    let disconnect_result = env.chat_handler.handle_user_disconnect(999).await;
    assert!(disconnect_result.is_ok()); // 연결 해제는 에러가 발생하지 않음 (이미 해제된 상태)
    let cleaned_rooms = disconnect_result.unwrap();
    assert_eq!(cleaned_rooms, 0);
}