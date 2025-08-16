# 빠른 시작 가이드

5분 안에 Enhanced Database Service 시작하기!

## 1. 환경 설정 (30초)

`.env` 파일 생성:
```env
db_host=localhost
db_port=3306
db_id=root
db_password=your_password
db_name=police
```

## 2. 의존성 추가 (Cargo.toml)

```toml
[dependencies]
shared = { path = "../shared" }
tokio = { version = "1.0", features = ["full"] }
sqlx = { version = "0.7", features = ["mysql", "runtime-tokio-native-tls"] }
serde_json = "1.0"
```

## 3. 기본 코드 (복사해서 사용)

```rust
use shared::config::db::DbConfig;
use shared::service::db::{
    EnhancedBaseDbService, EnhancedBaseDbServiceImpl,
    IdStrategy, PaginationParams, SortOrder
};
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. DB 서비스 초기화
    let db_config = DbConfig::from_env()?;
    let pool = Arc::new(db_config.create_pool().await?);
    let db = EnhancedBaseDbServiceImpl::new(pool).await?;
    
    // 2. ID 생성
    let new_id = db.generate_id(&IdStrategy::Uuid).await?;
    println!("새 ID: {}", new_id);
    
    // 3. 데이터 삽입
    let mut user = HashMap::new();
    user.insert("email".to_string(), serde_json::json!("test@example.com"));
    user.insert("username".to_string(), serde_json::json!("testuser"));
    
    let id = db.insert_returning_id("users", user, "id").await?;
    println!("삽입된 ID: {}", id);
    
    // 4. 데이터 조회
    if let Some(data) = db.get_by_id("users", "id", &id).await? {
        println!("조회 결과: {:?}", data);
    }
    
    // 5. 페이지네이션
    let page = db.paginate(
        "users",
        PaginationParams {
            page: 1,
            page_size: 10,
            sort_by: Some("created_at".to_string()),
            sort_order: Some(SortOrder::Desc),
        },
        None,
        None
    ).await?;
    
    println!("페이지 {}/{}, 항목 수: {}", 
        page.page, page.total_pages, page.data.len());
    
    Ok(())
}
```

## 4. 실행

```bash
cargo run
```

## 자주 사용하는 패턴

### 패턴 1: 사용자 생성 및 조회

```rust
// 사용자 생성
let mut user = HashMap::new();
user.insert("email".to_string(), serde_json::json!(email));
user.insert("username".to_string(), serde_json::json!(username));
user.insert("password_hash".to_string(), serde_json::json!(hash));

let user_id = db.insert_returning_id("users", user, "id").await?;

// 이메일로 조회
let mut params = HashMap::new();
params.insert("email".to_string(), serde_json::json!(email));

if let Some(user) = db.get_one("users", "email = ?", Some(params)).await? {
    println!("사용자 찾음: {:?}", user);
}
```

### 패턴 2: 검색과 페이지네이션

```rust
use shared::service::db::{SearchParams, SearchMatchType};

// 검색
let search = SearchParams {
    keyword: "john".to_string(),
    fields: vec!["username".to_string(), "email".to_string()],
    match_type: SearchMatchType::Contains,
    case_sensitive: false,
};

let results = db.search("users", search, None).await?;
println!("검색 결과: {} 건", results.total);
```

### 패턴 3: 대량 데이터 처리

```rust
// 여러 사용자 한번에 생성/업데이트
let users = vec![
    // HashMap 데이터들...
];

let result = db.bulk_upsert(
    "users",
    users,
    vec!["email".to_string()]  // 중복 체크 키
).await?;

println!("처리 완료: 성공 {}, 실패 {}", result.success, result.failed);
```

### 패턴 4: 트랜잭션 처리

```rust
// 복잡한 작업을 트랜잭션으로
let result = db.with_transaction(|tx| {
    Box::pin(async move {
        // 여러 DB 작업...
        // 에러 발생 시 자동 롤백
        Ok(())
    })
}).await?;
```

### 패턴 5: Soft Delete

```rust
// 논리적 삭제
db.soft_delete("users", "status = 'inactive'", None, "deleted_at").await?;

// 복원
db.restore("users", "id = ?", Some(params), "deleted_at").await?;
```

## 다음 단계

1. **전체 가이드 읽기**: [README.md](README.md)
2. **예제 코드 실행**: `cargo run --example enhanced_db_usage`
3. **테스트 확인**: `cargo test -p shared enhanced_db_service_test`
4. **API 문서**: `cargo doc --open -p shared`

## 도움이 필요하면

- 전체 예제: `shared/examples/enhanced_db_usage.rs`
- 통합 테스트: `shared/tests/enhanced_db_service_test.rs`
- 소스 코드: `shared/src/service/db/enhanced_base_db_service.rs`