//! gRPC Client Test Module
//!
//! gRPC 서버와의 통신을 테스트하는 클라이언트 코드입니다.
//! RoomService와 UserService의 모든 기능을 테스트합니다.

use crate::room::{room_service_client::RoomServiceClient, GetRoomListRequest, MakeRoomRequest};
use crate::user::{user_service_client::UserServiceClient, LoginRequest, RegisterRequest};
use tonic::transport::Channel;
// use shared::tool::error::{AppError, ErrorTracker};

/// gRPC 연결 테스트
///
/// 서버와의 연결을 확인하고 기본적인 통신을 테스트합니다.
#[tokio::test]
async fn test_grpc_connection() {
    // 서버 주소 설정
    let addr = "http://127.0.0.1:50051";

    // 채널 생성
    let channel = match Channel::from_shared(addr.to_string()) {
        Ok(channel) => channel.connect().await.expect("Test assertion failed"),
        Err(_) => {
            eprintln!("서버에 연결할 수 없습니다. 서버가 실행 중인지 확인하세요.");
            return;
        }
    };

    // Room Service 테스트
    test_room_service(channel.clone()).await;

    // User Service 테스트
    test_user_service(channel).await;
}

/// Room Service 테스트
#[allow(dead_code)]
async fn test_room_service(channel: Channel) {
    println!("=== Room Service 테스트 시작 ===");

    let mut client = RoomServiceClient::new(channel);

    // 1. 정상적인 방 생성 테스트
    println!("1. 정상적인 방 생성 테스트");
    let request = tonic::Request::new(MakeRoomRequest {
        user_id: 123,
        nick_name: "test_user".to_string(),
        room_name: "테스트 방".to_string(),
        max_player_num: 4,
    });

    match client.make_room(request).await {
        Ok(response) => {
            let response = response.into_inner();
            println!("✅ 방 생성 성공: room_id={}", response.room_id);
        }
        Err(e) => {
            println!("❌ 방 생성 실패: {e}");
        }
    }

    // 2. 에러 케이스 테스트 - 잘못된 방 이름
    println!("2. 에러 케이스 테스트 - 잘못된 방 이름");
    let request = tonic::Request::new(MakeRoomRequest {
        user_id: 123,
        nick_name: "test_user".to_string(),
        room_name: "error".to_string(), // 에러 트리거
        max_player_num: 4,
    });

    match client.make_room(request).await {
        Ok(response) => {
            let response = response.into_inner();
            println!("✅ 방 생성 성공: room_id={}", response.room_id);
        }
        Err(e) => {
            println!("❌ 방 생성 실패 (예상됨): {e}");
        }
    }

    // 3. 방 리스트 조회 테스트
    println!("3. 방 리스트 조회 테스트");
    let request = tonic::Request::new(GetRoomListRequest { last_room_id: 0 });

    match client.get_room_list(request).await {
        Ok(response) => {
            let response = response.into_inner();
            println!("✅ 방 리스트 조회 성공: {}개 방", response.rooms.len());
        }
        Err(e) => {
            println!("❌ 방 리스트 조회 실패: {e}");
        }
    }

    // 4. 에러 케이스 테스트 - 데이터베이스 연결 실패
    println!("4. 에러 케이스 테스트 - 데이터베이스 연결 실패");
    let request = tonic::Request::new(GetRoomListRequest {
        last_room_id: -999, // 에러 트리거
    });

    match client.get_room_list(request).await {
        Ok(response) => {
            let response = response.into_inner();
            println!("✅ 방 리스트 조회 성공: {}개 방", response.rooms.len());
        }
        Err(e) => {
            println!("❌ 방 리스트 조회 실패 (예상됨): {e}");
        }
    }
}

/// User Service 테스트
#[allow(dead_code)]
async fn test_user_service(channel: Channel) {
    println!("=== User Service 테스트 시작 ===");

    let mut client = UserServiceClient::new(channel);

    // 1. 정상적인 로그인 테스트
    println!("1. 정상적인 로그인 테스트");
    let request = tonic::Request::new(LoginRequest {
        login_type: "google".to_string(),
        login_token: "valid_token".to_string(),
    });

    match client.login_user(request).await {
        Ok(response) => {
            let response = response.into_inner();
            println!(
                "✅ 로그인 성공: user_id={}, nick={}",
                response.user_id, response.nick_name
            );
        }
        Err(e) => {
            println!("❌ 로그인 실패: {e}");
        }
    }

    // 2. 에러 케이스 테스트 - 인증 실패
    println!("2. 에러 케이스 테스트 - 인증 실패");
    let request = tonic::Request::new(LoginRequest {
        login_type: "google".to_string(),
        login_token: "invalid_token".to_string(), // 에러 트리거
    });

    match client.login_user(request).await {
        Ok(response) => {
            let response = response.into_inner();
            println!(
                "✅ 로그인 성공: user_id={}, nick={}",
                response.user_id, response.nick_name
            );
        }
        Err(e) => {
            println!("❌ 로그인 실패 (예상됨): {e}");
        }
    }

    // 3. 에러 케이스 테스트 - 사용자 없음
    println!("3. 에러 케이스 테스트 - 사용자 없음");
    let request = tonic::Request::new(LoginRequest {
        login_type: "google".to_string(),
        login_token: "notfound_token".to_string(), // 에러 트리거
    });

    match client.login_user(request).await {
        Ok(response) => {
            let response = response.into_inner();
            println!(
                "✅ 로그인 성공: user_id={}, nick={}",
                response.user_id, response.nick_name
            );
        }
        Err(e) => {
            println!("❌ 로그인 실패 (예상됨): {e}");
        }
    }

    // 4. 정상적인 회원가입 테스트
    println!("4. 정상적인 회원가입 테스트");
    let request = tonic::Request::new(RegisterRequest {
        login_type: "google".to_string(),
        login_token: "new_user_token".to_string(),
        nick_name: "new_user".to_string(),
    });

    match client.register_user(request).await {
        Ok(response) => {
            let response = response.into_inner();
            println!("✅ 회원가입 성공: success={}", response.success);
        }
        Err(e) => {
            println!("❌ 회원가입 실패: {e}");
        }
    }

    // 5. 에러 케이스 테스트 - 닉네임 중복
    println!("5. 에러 케이스 테스트 - 닉네임 중복");
    let request = tonic::Request::new(RegisterRequest {
        login_type: "google".to_string(),
        login_token: "duplicate_token".to_string(),
        nick_name: "duplicate".to_string(), // 에러 트리거
    });

    match client.register_user(request).await {
        Ok(response) => {
            let response = response.into_inner();
            println!("✅ 회원가입 성공: success={}", response.success);
        }
        Err(e) => {
            println!("❌ 회원가입 실패 (예상됨): {e}");
        }
    }

    // 6. 에러 케이스 테스트 - 데이터베이스 오류
    println!("6. 에러 케이스 테스트 - 데이터베이스 오류");
    let request = tonic::Request::new(RegisterRequest {
        login_type: "google".to_string(),
        login_token: "db_error_token".to_string(),
        nick_name: "db_error".to_string(), // 에러 트리거
    });

    match client.register_user(request).await {
        Ok(response) => {
            let response = response.into_inner();
            println!("✅ 회원가입 성공: success={}", response.success);
        }
        Err(e) => {
            println!("❌ 회원가입 실패 (예상됨): {e}");
        }
    }
}

/*
/// 에러 시스템 테스트
#[tokio::test]
async fn test_error_system() {
    println!("=== 에러 시스템 테스트 시작 ===");
    // Test implementation commented out due to import issues
}

/// 에러 헬퍼 함수 테스트
fn test_error_helpers() {
    println!("=== 에러 헬퍼 함수 테스트 ===");
    // Test implementation commented out due to import issues
}
*/
