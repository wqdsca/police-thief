use tonic::transport::Channel;
use grpcserver::room::room_service_client::RoomServiceClient;
use grpcserver::user::user_service_client::UserServiceClient;
use grpcserver::room::{MakeRoomRequest, GetRoomListRequest};
use grpcserver::user::{LoginRequest, RegisterRequest};

#[tokio::test]
async fn test_grpc_connection() -> Result<(), Box<dyn std::error::Error>> {
    // gRPC 서버 주소 설정
    let server_addr = "http://127.0.0.1:50051";
    
    // Channel 생성
    let channel = Channel::from_shared(server_addr.to_string())?
        .connect()
        .await?;
    
    // Room 서비스 클라이언트 생성
    let mut room_client = RoomServiceClient::new(channel.clone());
    
    // User 서비스 클라이언트 생성
    let mut user_client = UserServiceClient::new(channel);
    
    println!("✅ gRPC 서버에 연결되었습니다: {}", server_addr);
    
    // Room 서비스 테스트
    test_room_service(&mut room_client).await?;
    
    // User 서비스 테스트
    test_user_service(&mut user_client).await?;
    
    Ok(())
}

async fn test_room_service(
    client: &mut RoomServiceClient<Channel>
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Room 서비스 테스트 시작...");
    
    // 방 생성 테스트
    let make_room_request = tonic::Request::new(MakeRoomRequest {
        user_id: 1,
        nick_name: "test_user".to_string(),
        room_name: "테스트 방".to_string(),
        max_player_num: 4,
    });
    
    match client.make_room(make_room_request).await {
        Ok(response) => {
            println!("✅ 방 생성 성공: room_id = {}", response.get_ref().room_id);
        }
        Err(e) => {
            println!("❌ 방 생성 실패: {}", e);
        }
    }
    
    // 방 리스트 조회 테스트
    let get_room_list_request = tonic::Request::new(GetRoomListRequest {
        last_room_id: 0,
    });
    
    match client.get_room_list(get_room_list_request).await {
        Ok(response) => {
            println!("✅ 방 리스트 조회 성공: {}개 방", response.get_ref().rooms.len());
        }
        Err(e) => {
            println!("❌ 방 리스트 조회 실패: {}", e);
        }
    }
    
    Ok(())
}

async fn test_user_service(
    client: &mut UserServiceClient<Channel>
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 User 서비스 테스트 시작...");
    
    // 로그인 테스트
    let login_request = tonic::Request::new(LoginRequest {
        login_type: "test".to_string(),
        login_token: "test_token".to_string(),
    });
    
    match client.login_user(login_request).await {
        Ok(response) => {
            let user_info = response.get_ref();
            println!("✅ 로그인 성공: user_id = {}, nick = {}", 
                user_info.user_id, user_info.nick_name);
        }
        Err(e) => {
            println!("❌ 로그인 실패: {}", e);
        }
    }
    
    // 회원가입 테스트
    let register_request = tonic::Request::new(RegisterRequest {
        login_type: "test".to_string(),
        login_token: "test_token".to_string(),
        nick_name: "새로운_사용자".to_string(),
    });
    
    match client.register_user(register_request).await {
        Ok(_) => {
            println!("✅ 회원가입 성공");
        }
        Err(e) => {
            println!("❌ 회원가입 실패: {}", e);
        }
    }
    
    Ok(())
} 