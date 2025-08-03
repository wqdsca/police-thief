use tonic::transport::Channel;

use game::game_service_client::GameServiceClient;
use game::{ConnectRequest, GetGameStateRequest, PlayerActionRequest, GameEventRequest};

pub mod game {
    tonic::include_proto!("game");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = Channel::from_shared("http://[::1]:50051".to_string())?
        .connect()
        .await?;

    let mut client = GameServiceClient::new(channel);

    println!("=== gRPC 게임 서버 테스트 ===");

    // 1. 플레이어 연결 테스트
    println!("\n1. 플레이어 연결 테스트");
    let connect_request = tonic::Request::new(ConnectRequest {
        player_id: "player_001".to_string(),
        player_name: "Alice".to_string(),
    });

    let connect_response = client.connect_player(connect_request).await?;
    let session_id = connect_response.get_ref().session_id.clone();
    println!("연결 성공: {}", session_id);

    // 2. 게임 상태 조회 테스트
    println!("\n2. 게임 상태 조회 테스트");
    let state_request = tonic::Request::new(GetGameStateRequest {
        session_id: session_id.clone(),
    });

    let state_response = client.get_game_state(state_request).await?;
    let game_state = state_response.get_ref();
    println!("게임 상태: {} 플레이어", game_state.game_state.as_ref().unwrap().players.len());

    // 3. 플레이어 액션 테스트
    println!("\n3. 플레이어 액션 테스트");
    let action_request = tonic::Request::new(PlayerActionRequest {
        session_id: session_id.clone(),
        player_id: "player_001".to_string(),
        action_type: game::ActionType::Move as i32,
        action_data: vec![10, 20], // x=10, y=20
    });

    let action_response = client.player_action(action_request).await?;
    println!("액션 결과: {}", action_response.get_ref().message);

    // 4. 게임 이벤트 스트림 테스트
    println!("\n4. 게임 이벤트 스트림 테스트");
    let event_request = tonic::Request::new(GameEventRequest {
        session_id,
    });

    let mut event_stream = client.game_event_stream(event_request).await?.into_inner();
    
    // 3개의 이벤트만 받기
    for i in 0..3 {
        if let Some(event) = event_stream.message().await? {
            println!("이벤트 {}: {:?} - {}", i + 1, event.event_type, event.player_id);
        }
    }

    println!("\n=== 테스트 완료 ===");
    Ok(())
} 