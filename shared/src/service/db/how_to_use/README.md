# Enhanced Database Service 사용 가이드

Police Thief 게임 서버의 향상된 데이터베이스 서비스 완벽 가이드입니다.

## 📚 목차

1. [기본 설정](#기본-설정)
2. [ID 관리 시스템](#id-관리-시스템)
3. [기본 쿼리 작업](#기본-쿼리-작업)
4. [페이지네이션](#페이지네이션)
5. [검색 기능](#검색-기능)
6. [고급 작업](#고급-작업)
7. [집계 함수](#집계-함수)
8. [유틸리티 함수](#유틸리티-함수)
9. [트랜잭션 처리](#트랜잭션-처리)

## 기본 설정

### 서비스 초기화

```rust
use shared::config::db::DbConfig;
use shared::service::db::{EnhancedBaseDbService, EnhancedBaseDbServiceImpl};
use std::sync::Arc;

// 데이터베이스 연결 설정
let db_config = DbConfig::from_env()?;
let pool = Arc::new(db_config.create_pool().await?);

// Enhanced DB 서비스 생성
let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
    .with_snowflake(1, 1)  // Machine ID: 1, Datacenter ID: 1
    .await?;
```

### 환경 변수 설정

`.env` 파일에 다음 설정 필요:
```env
db_host=localhost
db_port=3306
db_id=root
db_password=your_password
db_name=police
```

## ID 관리 시스템

### 4가지 ID 생성 전략

#### 1. UUID (범용 고유 식별자)
```rust
use shared::service::db::IdStrategy;

// UUID 생성 (36자 문자열)
let uuid = db_service.generate_id(&IdStrategy::Uuid).await?;
// 예: "550e8400-e29b-41d4-a716-446655440000"
```

#### 2. Snowflake ID (분산 시스템용)
```rust
// Twitter의 Snowflake 알고리즘 사용 (64비트 정수)
let snowflake_id = db_service.generate_id(&IdStrategy::Snowflake {
    machine_id: 1,      // 머신 ID (0-1023)
    datacenter_id: 1,   // 데이터센터 ID (0-1023)
}).await?;
// 예: "7184811009659318273"

// 특징:
// - 시간순 정렬 가능
// - 분산 환경에서 충돌 없음
// - 밀리초당 4096개 생성 가능
```

#### 3. 접두사 시퀀스
```rust
// 커스텀 접두사와 패딩
let prefixed_id = db_service.generate_id(&IdStrategy::PrefixedSequence {
    prefix: "USER_".to_string(),
    padding: 6,  // 0으로 패딩
}).await?;
// 예: "USER_000042"
```

#### 4. 타임스탬프 기반
```rust
// 타임스탬프 + 선택적 접두사
let timestamp_id = db_service.generate_id(&IdStrategy::TimestampBased {
    prefix: Some("ORD_".to_string()),
}).await?;
// 예: "ORD_1705312845123_42"
```

### ID 정보 조회

```rust
// 테이블의 ID 정보 가져오기
let id_info = db_service.get_id_info("users").await?;
println!("ID 컬럼: {}", id_info.column);        // "id"
println!("ID 전략: {:?}", id_info.strategy);    // AutoIncrement
println!("마지막 값: {:?}", id_info.last_value); // Some(42)

// 다음 AUTO_INCREMENT 값 예측
let next_id = db_service.get_next_auto_increment_id("users").await?;
println!("다음 ID: {}", next_id);

// 마지막 삽입 ID 가져오기
let last_id = db_service.get_last_insert_id().await?;
```

## 기본 쿼리 작업

### 레코드 존재 확인

```rust
use std::collections::HashMap;

// 파라미터 준비
let mut params = HashMap::new();
params.insert("email".to_string(), serde_json::json!("user@example.com"));

// 존재 여부 확인
let exists = db_service.exists(
    "users",           // 테이블명
    "email = ?",       // WHERE 조건
    Some(params)       // 파라미터
).await?;

if exists {
    println!("사용자가 존재합니다");
}
```

### 단일 레코드 조회

```rust
// 조건으로 조회
let user = db_service.get_one(
    "users",
    "email = ?",
    Some(params)
).await?;

if let Some(user_data) = user {
    println!("사용자: {:?}", user_data);
}

// ID로 조회
let user_by_id = db_service.get_by_id(
    "users",
    "id",      // ID 컬럼명
    "42"       // ID 값
).await?;
```

### 레코드 개수 세기

```rust
// 전체 개수
let total = db_service.count("users", None, None).await?;

// 조건부 개수
let active_count = db_service.count(
    "users",
    Some("status = 'active' AND created_at > ?"),
    Some(params)
).await?;
```

## 페이지네이션

### 기본 페이지네이션

```rust
use shared::service::db::{PaginationParams, SortOrder};

let pagination = PaginationParams {
    page: 1,                              // 페이지 번호 (1부터 시작)
    page_size: 20,                        // 페이지당 항목 수
    sort_by: Some("created_at".to_string()),  // 정렬 컬럼
    sort_order: Some(SortOrder::Desc),        // 정렬 방향
};

let result = db_service.paginate(
    "users",
    pagination,
    Some("status = 'active'"),  // WHERE 조건
    None                         // 파라미터
).await?;

// 결과 사용
println!("현재 페이지: {}/{}", result.page, result.total_pages);
println!("전체 항목: {}", result.total);
println!("이 페이지 항목 수: {}", result.data.len());
println!("다음 페이지 존재: {}", result.has_next);
println!("이전 페이지 존재: {}", result.has_prev);

for item in result.data {
    println!("항목: {:?}", item);
}
```

## 검색 기능

### 다중 필드 검색

```rust
use shared::service::db::{SearchParams, SearchMatchType};

let search = SearchParams {
    keyword: "john".to_string(),
    fields: vec![
        "username".to_string(),
        "email".to_string(),
        "full_name".to_string(),
    ],
    match_type: SearchMatchType::Contains,  // Contains, StartsWith, EndsWith, Exact
    case_sensitive: false,
};

// 검색 + 페이지네이션
let search_result = db_service.search(
    "users",
    search,
    Some(PaginationParams {
        page: 1,
        page_size: 50,
        sort_by: Some("relevance".to_string()),
        sort_order: Some(SortOrder::Desc),
    })
).await?;

println!("검색 결과: {} 건", search_result.total);
```

### 검색 매칭 타입

```rust
// Contains: 부분 일치 (LIKE '%keyword%')
SearchMatchType::Contains

// StartsWith: 접두사 일치 (LIKE 'keyword%')
SearchMatchType::StartsWith  

// EndsWith: 접미사 일치 (LIKE '%keyword')
SearchMatchType::EndsWith

// Exact: 정확히 일치 (= 'keyword')
SearchMatchType::Exact
```

## 고급 작업

### Upsert (INSERT or UPDATE)

```rust
let mut user_data = HashMap::new();
user_data.insert("email".to_string(), serde_json::json!("user@example.com"));
user_data.insert("username".to_string(), serde_json::json!("johndoe"));
user_data.insert("status".to_string(), serde_json::json!("active"));

// 단일 upsert
let affected = db_service.upsert(
    "users",
    user_data,
    vec!["email".to_string()]  // 유니크 키 (충돌 시 UPDATE)
).await?;
```

### 대량 Upsert

```rust
let users = vec![
    {
        let mut data = HashMap::new();
        data.insert("email".to_string(), serde_json::json!("user1@example.com"));
        data.insert("username".to_string(), serde_json::json!("user1"));
        data
    },
    {
        let mut data = HashMap::new();
        data.insert("email".to_string(), serde_json::json!("user2@example.com"));
        data.insert("username".to_string(), serde_json::json!("user2"));
        data
    },
];

let result = db_service.bulk_upsert(
    "users",
    users,
    vec!["email".to_string()]
).await?;

println!("전체: {}, 성공: {}, 실패: {}", 
    result.total, result.success, result.failed);

// 실패한 항목 확인
for error in result.errors {
    println!("에러: {} - {}", error.index, error.message);
}
```

### INSERT 후 ID 반환

```rust
let new_user = HashMap::new();
// ... 데이터 설정 ...

let new_id = db_service.insert_returning_id(
    "users",
    new_user,
    "id"  // ID 컬럼명
).await?;

println!("새로 생성된 ID: {}", new_id);
```

### Soft Delete & Restore

```rust
// Soft Delete (논리적 삭제)
let deleted_count = db_service.soft_delete(
    "users",
    "status = 'inactive'",  // 삭제 조건
    None,                    // 파라미터
    "deleted_at"            // 삭제 시간 컬럼
).await?;

// Restore (복원)
let restored_count = db_service.restore(
    "users",
    "id IN (1, 2, 3)",
    None,
    "deleted_at"
).await?;
```

## 집계 함수

### SUM (합계)

```rust
// 완료된 주문 총액
let total = db_service.sum(
    "orders",
    "amount",                        // 합계 컬럼
    Some("status = 'completed'"),    // 조건
    None                             // 파라미터
).await?;
println!("총 매출: ${:.2}", total);
```

### AVG (평균)

```rust
// 평균 주문 금액
let average = db_service.avg(
    "orders",
    "amount",
    Some("created_at > DATE_SUB(NOW(), INTERVAL 30 DAY)"),
    None
).await?;
println!("최근 30일 평균: ${:.2}", average);
```

### MIN/MAX (최소/최대)

```rust
// 최소값
let min_val: Option<f64> = db_service.min(
    "products",
    "price",
    Some("in_stock = true"),
    None
).await?;

// 최대값
let max_val: Option<f64> = db_service.max(
    "products",
    "price",
    None,
    None
).await?;
```

## 유틸리티 함수

### 테이블 관리

```rust
// 테이블 크기 확인 (바이트)
let size = db_service.get_table_size("users").await?;
println!("테이블 크기: {} MB", size / 1024 / 1024);

// 테이블 최적화
db_service.optimize_table("users").await?;

// 테이블 분석 (인덱스 통계 업데이트)
db_service.analyze_table("users").await?;

// 테이블 잠금
use shared::service::db::LockType;

db_service.lock_tables(vec![
    ("users".to_string(), LockType::Write),
    ("orders".to_string(), LockType::Read),
]).await?;

// 작업 수행...

// 잠금 해제
db_service.unlock_tables().await?;
```

### 테이블 백업

```rust
// 테이블 백업
let row_count = db_service.backup_table(
    "users",                    // 원본 테이블
    "users_backup_20240115"     // 백업 테이블명
).await?;
println!("{} 개 행 백업됨", row_count);

// 테이블 비우기 (TRUNCATE)
db_service.truncate("temp_data").await?;
```

## 트랜잭션 처리

### 기본 트랜잭션

```rust
let result = db_service.with_transaction(|tx| {
    Box::pin(async move {
        // 사용자 생성
        sqlx::query("INSERT INTO users (email, username) VALUES (?, ?)")
            .bind("new@example.com")
            .bind("newuser")
            .execute(&mut **tx)
            .await?;
        
        // 생성된 ID 가져오기
        let row = sqlx::query("SELECT LAST_INSERT_ID() as id")
            .fetch_one(&mut **tx)
            .await?;
        let user_id: u64 = row.get("id");
        
        // 관련 데이터 생성
        sqlx::query("INSERT INTO user_profiles (user_id, bio) VALUES (?, ?)")
            .bind(user_id)
            .bind("New user bio")
            .execute(&mut **tx)
            .await?;
        
        Ok(user_id)
    })
}).await?;

// 트랜잭션 성공 시 result에 user_id 반환
// 실패 시 자동 롤백
```

### 복잡한 트랜잭션 예제

```rust
// 주문 처리 트랜잭션
let order_result = db_service.with_transaction(|tx| {
    Box::pin(async move {
        // 1. 재고 확인 및 차감
        let stock = sqlx::query("SELECT quantity FROM inventory WHERE product_id = ? FOR UPDATE")
            .bind(product_id)
            .fetch_one(&mut **tx)
            .await?;
        
        let available: i32 = stock.get("quantity");
        if available < requested_quantity {
            return Err(AppError::InvalidInput("재고 부족".to_string()));
        }
        
        sqlx::query("UPDATE inventory SET quantity = quantity - ? WHERE product_id = ?")
            .bind(requested_quantity)
            .bind(product_id)
            .execute(&mut **tx)
            .await?;
        
        // 2. 주문 생성
        sqlx::query("INSERT INTO orders (user_id, product_id, quantity, amount) VALUES (?, ?, ?, ?)")
            .bind(user_id)
            .bind(product_id)
            .bind(requested_quantity)
            .bind(total_amount)
            .execute(&mut **tx)
            .await?;
        
        // 3. 결제 기록
        sqlx::query("INSERT INTO payments (order_id, amount, status) VALUES (LAST_INSERT_ID(), ?, 'pending')")
            .bind(total_amount)
            .execute(&mut **tx)
            .await?;
        
        Ok("주문 완료")
    })
}).await?;
```

## 성능 최적화 팁

### 1. 인덱스 활용
```rust
// 검색/정렬에 사용되는 컬럼에 인덱스 생성
// CREATE INDEX idx_created_at ON users(created_at);
// CREATE INDEX idx_status_created ON users(status, created_at);
```

### 2. 배치 처리
```rust
// 개별 처리 대신 bulk 작업 사용
// ❌ 나쁜 예: 루프에서 개별 INSERT
for user in users {
    db_service.insert("users", user).await?;
}

// ✅ 좋은 예: bulk_upsert 사용
db_service.bulk_upsert("users", users, vec!["email".to_string()]).await?;
```

### 3. 적절한 페이지 크기
```rust
// 메모리와 성능 균형 맞추기
let pagination = PaginationParams {
    page: 1,
    page_size: 100,  // 10-100 사이 권장
    // ...
};
```

### 4. 연결 풀 관리
```rust
// .env에서 적절한 풀 크기 설정
// db_max_connections=100
// db_min_connections=10
```

## 에러 처리

```rust
use shared::tool::error::AppError;

match db_service.get_by_id("users", "id", "1").await {
    Ok(Some(user)) => {
        // 사용자 찾음
    },
    Ok(None) => {
        // 사용자 없음
    },
    Err(AppError::DatabaseQuery(msg)) => {
        // 쿼리 에러
        eprintln!("쿼리 실패: {}", msg);
    },
    Err(AppError::DatabaseConnection(msg)) => {
        // 연결 에러
        eprintln!("연결 실패: {}", msg);
    },
    Err(e) => {
        // 기타 에러
        eprintln!("에러: {}", e);
    }
}
```

## 전체 사용 예제

완전한 예제는 다음 파일들을 참조하세요:
- `shared/examples/enhanced_db_usage.rs` - 전체 기능 예제
- `shared/tests/enhanced_db_service_test.rs` - 통합 테스트

## 문제 해결

### 일반적인 문제들

1. **"Connection refused"**
   - MariaDB/MySQL이 실행 중인지 확인
   - 포트 3306이 열려있는지 확인

2. **"Too many connections"**
   - 연결 풀 크기 조정
   - 오래된 연결 정리

3. **"Lock wait timeout"**
   - 트랜잭션 시간 단축
   - 데드락 확인

4. **성능 이슈**
   - 인덱스 확인
   - 쿼리 최적화
   - 배치 처리 사용