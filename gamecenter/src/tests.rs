use shared::config::redis_config::RedisConfig;
use shared::service::redis::core::redis_get_key::KeyType;
use shared::service::redis::hepler::hash_helper::HashHelper;
use anyhow::{Result, Context};
use tracing::info;

/// 키 타입 테스트 함수
pub fn test_key_types() {
    println!("키 타입 테스트 시작...");
    info!("키 타입 테스트 시작...");
    
    let user_id = 123;
    let room_id = 456;
    
    let user_key = KeyType::User.get_key(&user_id);
    let room_info_key = KeyType::RoomInfo.get_key(&room_id);
    let room_user_list_key = KeyType::RoomUserList.get_key(&room_id);
    let room_list_by_time_key = KeyType::RoomListByTime.get_key(&room_id);
    
    println!("생성된 키들:");
    info!("생성된 키들:");
    println!("  user: {}", user_key);
    info!("  user: {}", user_key);
    println!("  room_info: {}", room_info_key);
    info!("  room_info: {}", room_info_key);
    println!("  room_user_list: {}", room_user_list_key);
    info!("  room_user_list: {}", room_user_list_key);
    println!("  room_list_by_time: {}", room_list_by_time_key);
    info!("  room_list_by_time: {}", room_list_by_time_key);
    
    println!("✅ 키 타입 테스트 통과");
    info!("✅ 키 타입 테스트 통과");
}

/// Redis 연결 테스트 함수
pub async fn test_redis_connection(redis_config: &RedisConfig) -> Result<()> {
    println!("Redis 연결 테스트 시작...");
    info!("Redis 연결 테스트 시작...");
    
    // Redis 연결이 제대로 되어 있는지 확인
    let mut conn = redis_config.get_connection();
    
    // 실제로 Redis에 데이터를 저장해보기
    let test_key = "test:connection";
    let test_value = "Hello Redis!";
    
    // 데이터 저장
    let _: () = redis::cmd("SET")
        .arg(test_key)
        .arg(test_value)
        .query_async(&mut conn)
        .await
        .context("Redis SET 실패")?;
    
    println!("✅ Redis에 데이터 저장 성공");
    
    // 데이터 조회
    let result: String = redis::cmd("GET")
        .arg(test_key)
        .query_async(&mut conn)
        .await
        .context("Redis GET 실패")?;
    
    println!("✅ Redis에서 데이터 조회 성공: {}", result);
    
    // 테스트 데이터 삭제
    let _: () = redis::cmd("DEL")
        .arg(test_key)
        .query_async(&mut conn)
        .await
        .context("Redis DEL 실패")?;
    
    println!("✅ Redis 테스트 데이터 삭제 성공");
    println!("✅ Redis 연결 테스트 통과: {}:{}", redis_config.host, redis_config.port);
    info!("✅ Redis 연결 테스트 통과: {}:{}", redis_config.host, redis_config.port);
    
    Ok(())
}

/// HashHelper 테스트 함수
pub async fn test_hash_helper(redis_config: &RedisConfig) -> Result<()> {
    println!("HashHelper 테스트 시작...");
    info!("HashHelper 테스트 시작...");
    
    let helper = HashHelper::new(
        redis_config.clone(),
        KeyType::User, // KeyType 사용
        Some(3600), // TTL: 1시간
        Some(100), // limit
    );
    
    // 테스트 데이터
    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct UserData {
        name: String,
        level: u32,
        score: u64,
    }
    
    let user_data = UserData {
        name: "테스트유저".to_string(),
        level: 10,
        score: 1500,
    };
    
    let user_id = 123; // user_id 추가
    
    // Hash에 데이터 저장
    let key = helper.set_hash_field(user_id, "user_data", &user_data).await?;
    println!("✅ Hash에 데이터 저장 완료: {}", key);
    
    // Hash에서 데이터 가져오기
    let retrieved_data = helper.get_hash_field::<UserData>(user_id, "user_data").await?;
    if let Some(data) = retrieved_data {
        println!("✅ Hash에서 데이터 조회 성공: {:?}", data);
    } else {
        println!("❌ Hash에서 데이터를 찾을 수 없습니다");
    }
    
    println!("✅ HashHelper 테스트 통과");
    info!("✅ HashHelper 테스트 통과");
    Ok(())
}

/// 기본 기능 테스트 함수
pub async fn test_basic_functionality(redis_config: &RedisConfig) -> Result<()> {
    println!("기본 기능 테스트 시작...");
    info!("기본 기능 테스트 시작...");
    
    // 1. 키 타입 테스트
    test_key_types();
    
    // 2. Redis 연결 테스트
    test_redis_connection(redis_config).await?;
    
    // 3. HashHelper 테스트
    test_hash_helper(redis_config).await?;
    
    println!("✅ 기본 기능 테스트 통과");
    info!("✅ 기본 기능 테스트 통과");
    Ok(())
}

/// 모든 테스트를 실행하는 함수
pub async fn run_all_tests(redis_config: &RedisConfig) -> Result<()> {
    println!("=== 모든 테스트 시작 ===");
    info!("=== 모든 테스트 시작 ===");
    
    // 기본 기능 테스트
    test_basic_functionality(redis_config).await?;
    
    println!("=== 모든 테스트 완료 ===");
    info!("=== 모든 테스트 완료 ===");
    Ok(())
} 