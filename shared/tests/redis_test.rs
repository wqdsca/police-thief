use anyhow::Result;
use serde::{Deserialize, Serialize};

use shared::config::redis_config::RedisConfig;
use shared::service::redis::core::redis_get_key::KeyType;
use shared::service::redis::hepler::{
    cash_helper::CacheHelper,
    hash_helper::HashHelper,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestUser {
    id: u32,
    name: String,
    email: String,
    score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestLocation {
    name: String,
    address: String,
    category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestTag {
    name: String,
    color: String,
    priority: u8,
}

#[tokio::test]
async fn test_cache_helper() -> Result<()> {
    let redis_config = RedisConfig::new().await?;
    let cache = CacheHelper::new(redis_config.clone(), KeyType::User, Some(3600), Some(10));

    // 테스트 데이터
    let user1 = TestUser {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        score: 100.5,
    };

    let user2 = TestUser {
        id: 2,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
        score: 200.0,
    };

    // 캐시에 데이터 저장
    cache.set_cache_field(42, &user1).await?;
    cache.set_cache_field(42, &user2).await?;

    // 저장된 데이터 조회
    let retrieved_user1: Option<TestUser> = cache.get_cache_field(42).await?;
    let retrieved_user2: Option<TestUser> = cache.get_cache_field(42).await?;

    assert!(retrieved_user1.is_some());
    assert!(retrieved_user2.is_some());

    // 최근 목록 조회
    let recent_list = cache.get_recent_list().await?;
    assert!(!recent_list.is_empty());

    // 존재 여부 확인
    let exists = cache.exists_cache_field(42).await?;
    assert!(exists);

    // 통계 조회
    let (count, _) = cache.get_cache_stats().await?;
    assert!(count > 0);

    // 정리 (주석 처리 - 데이터 확인용)
    // cache.delete_cache_field(42).await?;

    println!("✅ CacheHelper 테스트 통과");
    Ok(())
}

#[tokio::test]
async fn test_hash_helper() -> Result<()> {
    let redis_config = RedisConfig::new().await?;
    let hash = HashHelper::new(redis_config.clone(), KeyType::User, Some(3600), None);

    // 테스트 데이터
    let user_name = "Alice Johnson".to_string();
    let user_email = "alice@example.com".to_string();
    let user_age = 25;

    // Hash에 필드 추가
    hash.set_hash_field(42, "name", &user_name).await?;
    hash.set_hash_field(42, "email", &user_email).await?;
    hash.set_hash_field(42, "age", &user_age).await?;

    // 개별 필드 조회
    let retrieved_name: Option<String> = hash.get_hash_field(42, "name").await?;
    let retrieved_email: Option<String> = hash.get_hash_field(42, "email").await?;
    let retrieved_age: Option<u32> = hash.get_hash_field(42, "age").await?;

    println!("Debug - retrieved_name: {:?}", retrieved_name);
    println!("Debug - retrieved_email: {:?}", retrieved_email);
    println!("Debug - retrieved_age: {:?}", retrieved_age);

    // 모든 필드 조회
    let all_fields = hash.get_all_hash_fields(42).await?;
    println!("Debug - all_fields: {:?}", all_fields);
    assert!(all_fields.contains_key("name"));
    assert!(all_fields.contains_key("email"));
    assert!(all_fields.contains_key("age"));

    // 필드 존재 여부 확인
    let exists = hash.hexists(42, "name").await?;
    assert!(exists);

    // 점수 증가 (나이 증가)
    let new_age = hash.incr_hash_field(42, "age", 1).await?;
    assert_eq!(new_age, user_age as i64 + 1);

    // 필드 삭제
    let deleted = hash.delete_hash_field(42, "age").await?;
    assert_eq!(deleted, 1);

    // 정리 (주석 처리 - 데이터 확인용)
    // hash.delete_all_hash_fields(42).await?;

    println!("✅ HashHelper 테스트 통과");
    Ok(())
}
