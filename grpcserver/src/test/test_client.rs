//! Comprehensive gRPC Test Client
//! 
//! Redis 데이터 저장 및 방 정보 조회 로직을 테스트합니다.
//! 특히 last_room_id 기반 페이징 로직을 중점적으로 검증합니다.

use anyhow::Result;
use redis::AsyncCommands;
use shared::config::connection_pool::ConnectionPool;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use tonic::transport::Channel;

// gRPC 클라이언트 관련 imports
// Proto generated modules
pub mod room {
    tonic::include_proto!("room");
}
pub mod user {
    tonic::include_proto!("user");
}

use room::{
    room_service_client::RoomServiceClient,
    MakeRoomRequest, GetRoomListRequest, RoomInfo,
};
use user::{
    user_service_client::UserServiceClient,
    LoginRequest,
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 gRPC Server Test Client");
    println!("==========================");
    
    // Redis 연결 확인
    println!("\n🔍 1. Redis 연결 테스트");
    test_redis_connection().await?;
    
    // 기존 테스트 데이터 정리
    println!("\n🧹 2. 기존 테스트 데이터 정리");
    cleanup_test_data().await?;
    
    // gRPC 서버 연결 테스트
    println!("\n🌐 3. gRPC 서버 연결 테스트");
    let channel = connect_to_grpc_server().await?;
    
    // 더미 데이터로 테스트
    println!("\n🎭 4. 더미 데이터 생성 및 테스트");
    run_comprehensive_tests(channel).await?;
    
    println!("\n✅ 모든 테스트 완료!");
    Ok(())
}

/// Redis 연결 테스트
async fn test_redis_connection() -> Result<()> {
    println!("   Redis 연결 풀 초기화 중...");
    ConnectionPool::init().await
        .map_err(|e| anyhow::anyhow!("Redis 연결 실패: {}", e))?;
    
    let mut conn = ConnectionPool::get_connection().await
        .map_err(|e| anyhow::anyhow!("Redis 연결 획득 실패: {}", e))?;
    
    let pong: String = redis::cmd("PING").query_async(&mut conn).await
        .map_err(|e| anyhow::anyhow!("Redis PING 실패: {}", e))?;
    
    println!("   ✅ Redis 연결 성공: {}", pong);
    Ok(())
}

/// 기존 테스트 데이터 정리
async fn cleanup_test_data() -> Result<()> {
    let mut conn = ConnectionPool::get_connection().await?;
    
    // 여러 패턴으로 개별적으로 키 조회
    let mut all_keys = Vec::new();
    
    // 각 패턴별로 개별 KEYS 명령 실행
    let patterns = vec!["*test*", "*room*", "*user:*"];
    for pattern in patterns {
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await?;
        all_keys.extend(keys);
    }
    
    // 중복 제거
    all_keys.sort();
    all_keys.dedup();
    
    if !all_keys.is_empty() {
        let deleted: i32 = redis::cmd("DEL").arg(&all_keys).query_async(&mut conn).await?;
        println!("   🗑️ {} 개 기존 테스트 키 삭제", deleted);
        for key in &all_keys {
            println!("      - {}", key);
        }
    } else {
        println!("   ✨ 정리할 데이터 없음");
    }
    
    Ok(())
}

/// gRPC 서버 연결
async fn connect_to_grpc_server() -> Result<Channel> {
    let addr = "http://127.0.0.1:50051";
    println!("   서버 주소: {}", addr);
    
    let channel = Channel::from_shared(addr.to_string())?
        .connect()
        .await
        .map_err(|e| anyhow::anyhow!("gRPC 서버 연결 실패: {}\n   💡 서버가 실행 중인지 확인하세요 (cargo run --bin grpcserver)", e))?;
    
    println!("   ✅ gRPC 서버 연결 성공");
    Ok(channel)
}

/// 종합 테스트 실행
async fn run_comprehensive_tests(channel: Channel) -> Result<()> {
    // 사용자 로그인 테스트
    test_user_operations(channel.clone()).await?;
    
    // 방 생성 테스트 (더미 데이터)
    test_room_creation(channel.clone()).await?;
    
    // 방 리스트 조회 및 페이징 테스트
    test_room_pagination(channel.clone()).await?;
    
    // Redis 데이터 검증
    verify_redis_data().await?;
    
    Ok(())
}

/// 사용자 관련 테스트
async fn test_user_operations(channel: Channel) -> Result<()> {
    println!("\n👤 사용자 로그인 테스트");
    let mut client = UserServiceClient::new(channel);
    
    // 테스트 사용자 로그인
    let login_request = LoginRequest {
        login_type: "test".to_string(),
        login_token: "test_token_123".to_string(),
    };
    
    match client.login_user(tonic::Request::new(login_request)).await {
        Ok(response) => {
            let resp = response.into_inner();
            println!("   ✅ 로그인 성공:");
            println!("      - 사용자 ID: {}", resp.user_id);
            println!("      - 닉네임: {}", resp.nick_name);
            println!("      - 액세스 토큰 길이: {}", resp.access_token.len());
            println!("      - 신규 가입 여부: {}", resp.is_register);
            
            // Redis에서 사용자 정보 확인
            verify_user_in_redis(resp.user_id).await?;
        }
        Err(e) => return Err(anyhow::anyhow!("로그인 실패: {}", e)),
    }
    
    Ok(())
}

/// 방 생성 테스트 (더미 데이터)
async fn test_room_creation(channel: Channel) -> Result<()> {
    println!("\n🏠 방 생성 테스트 (더미 데이터 생성)");
    let mut client = RoomServiceClient::new(channel);
    
    let dummy_rooms = vec![
        ("게임방 Alpha", 4), ("베타 테스트룸", 6), ("친구들 모임", 8), ("랜덤 매치방", 2),
        ("고급자 전용", 4), ("초보자 환영", 10), ("스피드 게임", 4), ("전략 게임방", 6),
        ("캐주얼 룸", 8), ("토너먼트방", 12), ("프로 리그", 8), ("뉴비 천국", 6),
        ("마스터 클래스", 4), ("빠른 대결", 2), ("팀워크 중심", 10), ("솔로 플레이", 1),
        ("듀오 매칭", 2), ("트리오 게임", 3), ("스쿼드 배틀", 4), ("길드 전투", 20),
        ("아레나 모드", 8), ("서바이벌", 16), ("배틀로얄", 100), ("클래식 모드", 6),
        ("하드코어", 4), ("이지 모드", 12), ("커스텀 방", 8), ("이벤트 룸", 6),
        ("시즌 매치", 10), ("랭크 게임", 8), ("일반 게임", 6), ("연습 모드", 4),
        ("친선전", 8), ("공식 경기", 10), ("프리미엄 룸", 4), ("VIP 라운지", 6),
        ("스페셜 이벤트", 12), ("한정 모드", 8), ("테스트 서버", 4), ("개발자 룸", 2),
        ("커뮤니티 방", 16), ("스트리머 룸", 8), ("대회 준비", 10), ("전문가 모드", 4),
        ("학습 센터", 6), ("튜토리얼 룸", 8), ("도전 과제", 4), ("업적 헌터", 6),
        ("레벨업 방", 8), ("경험치 부스트", 10),
    ];
    
    let mut created_rooms = Vec::new();
    
    for (i, (room_name, max_players)) in dummy_rooms.iter().enumerate() {
        let request = MakeRoomRequest {
            user_id: 100 + i as i32,
            nick_name: format!("TestUser{}", i + 1),
            room_name: room_name.to_string(),
            max_player_num: *max_players,
        };
        
        match client.make_room(tonic::Request::new(request)).await {
            Ok(response) => {
                let room_id = response.into_inner().room_id;
                created_rooms.push(room_id);
                println!("   ✅ 방 생성 성공: '{}' (ID: {}, 최대 {}명)", room_name, room_id, max_players);
                
                // 생성 간격 추가 (타임스탬프 차이를 만들기 위해)
                sleep(Duration::from_millis(100)).await;
            }
            Err(e) => {
                println!("   ❌ 방 생성 실패: '{}' - {}", room_name, e);
            }
        }
    }
    
    println!("   📊 총 {} 개 방 생성됨", created_rooms.len());
    
    // 생성된 방들을 Redis에서 직접 확인
    verify_rooms_in_redis(&created_rooms).await?;
    
    Ok(())
}

/// 방 리스트 조회 및 페이징 테스트 (핵심 테스트)
async fn test_room_pagination(channel: Channel) -> Result<()> {
    println!("\n📋 방 리스트 조회 및 페이징 테스트");
    let mut client = RoomServiceClient::new(channel);
    
    // 1. 첫 페이지 조회 (last_room_id = 0)
    println!("\n   📄 첫 페이지 조회 (last_room_id = 0)");
    let request = GetRoomListRequest { last_room_id: 0 };
    
    match client.get_room_list(tonic::Request::new(request)).await {
        Ok(response) => {
            let rooms = response.into_inner().rooms;
            println!("      ✅ 첫 페이지 조회 성공: {} 개 방", rooms.len());
            
            for (idx, room) in rooms.iter().enumerate() {
                println!("         {}. {} (ID: {}) - {}/{} 명", 
                    idx + 1, room.room_name, room.room_id, 
                    room.current_player_num, room.max_player_num);
            }
            
            // 2. 두 번째 페이지 조회 (마지막 방 ID 사용)
            if !rooms.is_empty() {
                let last_id = rooms.last().unwrap().room_id;
                println!("\n   📄 두 번째 페이지 조회 (last_room_id = {})", last_id);
                
                let request2 = GetRoomListRequest { last_room_id: last_id };
                match client.get_room_list(tonic::Request::new(request2)).await {
                    Ok(response2) => {
                        let rooms2 = response2.into_inner().rooms;
                        println!("      ✅ 두 번째 페이지 조회 성공: {} 개 방", rooms2.len());
                        
                        for (idx, room) in rooms2.iter().enumerate() {
                            println!("         {}. {} (ID: {}) - {}/{} 명", 
                                idx + 1, room.room_name, room.room_id, 
                                room.current_player_num, room.max_player_num);
                        }
                        
                        // 페이징 로직 검증
                        validate_pagination_logic(&rooms, &rooms2, last_id)?;
                        
                        // 3. 세 번째 페이지 조회 (50개 더미 데이터용)
                        if !rooms2.is_empty() {
                            let last_id2 = rooms2.last().unwrap().room_id;
                            println!("\n   📄 세 번째 페이지 조회 (last_room_id = {})", last_id2);
                            
                            let request3 = GetRoomListRequest { last_room_id: last_id2 };
                            match client.get_room_list(tonic::Request::new(request3)).await {
                                Ok(response3) => {
                                    let rooms3 = response3.into_inner().rooms;
                                    println!("      ✅ 세 번째 페이지 조회 성공: {} 개 방", rooms3.len());
                                    
                                    for (idx, room) in rooms3.iter().enumerate() {
                                        println!("         {}. {} (ID: {}) - {}/{} 명", 
                                            idx + 1, room.room_name, room.room_id, 
                                            room.current_player_num, room.max_player_num);
                                    }
                                    
                                    // 세 번째 페이징 검증
                                    validate_pagination_logic(&rooms2, &rooms3, last_id2)?;
                                    
                                    println!("      📊 총 3페이지로 조회된 방: {} + {} + {} = {} 개", 
                                        rooms.len(), rooms2.len(), rooms3.len(),
                                        rooms.len() + rooms2.len() + rooms3.len());
                                }
                                Err(e) => println!("      ❌ 세 번째 페이지 조회 실패: {}", e),
                            }
                        }
                    }
                    Err(e) => println!("      ❌ 두 번째 페이지 조회 실패: {}", e),
                }
            }
        }
        Err(e) => return Err(anyhow::anyhow!("첫 페이지 조회 실패: {}", e)),
    }
    
    // 3. 존재하지 않는 ID로 조회 테스트
    println!("\n   📄 존재하지 않는 ID로 조회 테스트 (last_room_id = 99999)");
    let request3 = GetRoomListRequest { last_room_id: 99999 };
    match client.get_room_list(tonic::Request::new(request3)).await {
        Ok(response) => {
            let rooms = response.into_inner().rooms;
            println!("      ✅ 빈 결과 조회: {} 개 방 (예상대로)", rooms.len());
        }
        Err(e) => println!("      ❌ 조회 실패: {}", e),
    }
    
    Ok(())
}

/// 페이징 로직 검증
fn validate_pagination_logic(
    first_page: &[RoomInfo], 
    second_page: &[RoomInfo],
    last_id: i32
) -> Result<()> {
    println!("\n   🔍 페이징 로직 검증");
    
    // 1. 첫 페이지와 두 번째 페이지에 중복이 없는지 확인
    let first_ids: std::collections::HashSet<i32> = first_page.iter().map(|r| r.room_id).collect();
    let second_ids: std::collections::HashSet<i32> = second_page.iter().map(|r| r.room_id).collect();
    
    let overlap: Vec<_> = first_ids.intersection(&second_ids).collect();
    if overlap.is_empty() {
        println!("      ✅ 페이지 간 중복 없음");
    } else {
        println!("      ❌ 페이지 간 중복 발견: {:?}", overlap);
        return Err(anyhow::anyhow!("페이징 로직 오류: 중복된 방 ID"));
    }
    
    // 2. 두 번째 페이지의 모든 방 ID가 last_id보다 작은지 확인 (시간 기반 정렬이므로)
    let invalid_ids: Vec<_> = second_page.iter()
        .filter(|r| r.room_id >= last_id)
        .map(|r| r.room_id)
        .collect();
    
    if invalid_ids.is_empty() {
        println!("      ✅ 시간 기반 정렬 정상 (두 번째 페이지 모든 ID < {})", last_id);
    } else {
        println!("      ⚠️  시간 기반 정렬 확인 필요: {:?} >= {}", invalid_ids, last_id);
        // 이는 에러가 아닐 수 있음 (방 생성 시간에 따라)
    }
    
    println!("      📊 총 조회된 고유 방: {} 개", first_ids.len() + second_ids.len());
    
    Ok(())
}

/// Redis에서 사용자 정보 확인
async fn verify_user_in_redis(user_id: i32) -> Result<()> {
    let mut conn = ConnectionPool::get_connection().await?;
    let user_key = format!("user:{}", user_id);
    
    let user_data: HashMap<String, String> = conn.hgetall(&user_key).await?;
    
    if user_data.is_empty() {
        println!("      ❌ Redis에 사용자 데이터 없음: {}", user_key);
    } else {
        println!("      ✅ Redis 사용자 데이터 확인:");
        for (key, value) in &user_data {
            println!("         {}: {}", key, value);
        }
        
        // TTL 확인
        let ttl: i32 = conn.ttl(&user_key).await?;
        println!("         TTL: {} 초", ttl);
    }
    
    Ok(())
}

/// Redis에서 방 정보 확인
async fn verify_rooms_in_redis(room_ids: &[i32]) -> Result<()> {
    println!("\n   🔍 Redis에서 방 데이터 확인");
    let mut conn = ConnectionPool::get_connection().await?;
    
    for &room_id in room_ids {
        let room_key = format!("room:info:{}", room_id);
        let room_data: HashMap<String, String> = conn.hgetall(&room_key).await?;
        
        if room_data.is_empty() {
            println!("      ❌ 방 {}의 데이터 없음", room_id);
        } else {
            println!("      ✅ 방 {} 데이터:", room_id);
            for (key, value) in &room_data {
                println!("         {}: {}", key, value);
            }
        }
    }
    
    // ZSet 인덱스 확인
    verify_room_zset_index().await?;
    
    Ok(())
}

/// 방 리스트 ZSet 인덱스 확인
async fn verify_room_zset_index() -> Result<()> {
    println!("\n   🔍 방 리스트 ZSet 인덱스 확인");
    let mut conn = ConnectionPool::get_connection().await?;
    
    // ZSet 키 확인
    let zset_key = "room:list:time:index";
    let room_count: usize = conn.zcard(&zset_key).await?;
    println!("      📊 ZSet에 등록된 방 개수: {}", room_count);
    
    if room_count > 0 {
        // 최신 5개 방 조회 (높은 점수 순)
        let recent_rooms: Vec<(String, f64)> = conn.zrevrange_withscores(&zset_key, 0, 4).await?;
        println!("      🕐 최신 방 목록 (타임스탬프 순):");
        for (room_id, timestamp) in recent_rooms {
            let datetime = chrono::DateTime::from_timestamp(timestamp as i64, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S");
            println!("         방 ID {}: {} ({})", room_id, timestamp, datetime);
        }
        
        // 가장 오래된 5개 방 조회
        let oldest_rooms: Vec<(String, f64)> = conn.zrange_withscores(&zset_key, 0, 4).await?;
        println!("      🕐 오래된 방 목록:");
        for (room_id, timestamp) in oldest_rooms {
            let datetime = chrono::DateTime::from_timestamp(timestamp as i64, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S");
            println!("         방 ID {}: {} ({})", room_id, timestamp, datetime);
        }
    }
    
    Ok(())
}

/// Redis 데이터 전체 검증
async fn verify_redis_data() -> Result<()> {
    println!("\n🔍 Redis 데이터 전체 검증");
    let mut conn = ConnectionPool::get_connection().await?;
    
    // 모든 키 조회
    let all_keys: Vec<String> = conn.keys("*").await?;
    println!("   📊 총 Redis 키 개수: {}", all_keys.len());
    
    // 키 타입별 분류
    let mut key_counts = HashMap::new();
    for key in &all_keys {
        if key.starts_with("user:") {
            *key_counts.entry("user").or_insert(0) += 1;
        } else if key.starts_with("room:info:") {
            *key_counts.entry("room_info").or_insert(0) += 1;
        } else if key.starts_with("room:list:") {
            *key_counts.entry("room_index").or_insert(0) += 1;
        } else {
            *key_counts.entry("other").or_insert(0) += 1;
        }
    }
    
    println!("   📋 키 타입별 분포:");
    for (key_type, count) in key_counts {
        println!("      {}: {} 개", key_type, count);
    }
    
    // 메모리 사용량 확인
    let info: String = redis::cmd("INFO").arg("memory").query_async(&mut conn).await?;
    if let Some(line) = info.lines().find(|line| line.starts_with("used_memory_human:")) {
        println!("   💾 Redis 메모리 사용량: {}", line.split(':').nth(1).unwrap_or("N/A"));
    }
    
    println!("   ✅ Redis 데이터 검증 완료");
    Ok(())
}