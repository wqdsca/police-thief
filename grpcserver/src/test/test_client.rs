use tonic::transport::Channel;
use crate::room::room_service_client::RoomServiceClient;
use crate::user::user_service_client::UserServiceClient;
use crate::room::{MakeRoomRequest, GetRoomListRequest};
use crate::user::{LoginRequest, RegisterRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = Channel::from_shared("http://127.0.0.1:50051".to_string())?
        .connect()
        .await?;

    let mut room_client = RoomServiceClient::new(channel.clone());
    let mut user_client = UserServiceClient::new(channel);

    println!("=== gRPC 서버 테스트 ===");

    // 1. 사용자 로그인 테스트
    println!("\n1. 사용자 로그인 테스트");
    let login_request = tonic::Request::new(LoginRequest {
        login_type: "test".to_string(),
        login_token: "test_token".to_string(),
    });

    match user_client.login_user(login_request).await {
        Ok(response) => {
            let user_info = response.get_ref();
            println!("로그인 성공: user_id = {}, nick = {}", 
                user_info.user_id, user_info.nick_name);
        }
        Err(e) => {
            println!("로그인 실패: {}", e);
        }
    }

    // 2. 방 생성 테스트
    println!("\n2. 방 생성 테스트");
    let make_room_request = tonic::Request::new(MakeRoomRequest {
        user_id: 1,
        nick_name: "test_user".to_string(),
        room_name: "테스트 방".to_string(),
        max_player_num: 4,
    });

    match room_client.make_room(make_room_request).await {
        Ok(response) => {
            println!("방 생성 성공: room_id = {}", response.get_ref().room_id);
        }
        Err(e) => {
            println!("방 생성 실패: {}", e);
        }
    }

    // 3. 방 리스트 조회 테스트
    println!("\n3. 방 리스트 조회 테스트");
    let get_room_list_request = tonic::Request::new(GetRoomListRequest {
        last_room_id: 0,
    });

    match room_client.get_room_list(get_room_list_request).await {
        Ok(response) => {
            println!("방 리스트 조회 성공: {}개 방", response.get_ref().rooms.len());
        }
        Err(e) => {
            println!("방 리스트 조회 실패: {}", e);
        }
    }

    // 4. 회원가입 테스트
    println!("\n4. 회원가입 테스트");
    let register_request = tonic::Request::new(RegisterRequest {
        login_type: "test".to_string(),
        login_token: "new_user_token".to_string(),
        nick_name: "새로운_사용자".to_string(),
    });

    match user_client.register_user(register_request).await {
        Ok(_) => {
            println!("회원가입 성공");
        }
        Err(e) => {
            println!("회원가입 실패: {}", e);
        }
    }

    println!("\n=== 테스트 완료 ===");
    Ok(())
} 