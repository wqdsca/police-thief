# ID 생성 전략 상세 가이드

Police Thief 게임 서버의 4가지 ID 생성 전략을 언제, 어떻게 사용하는지 상세히 설명합니다.

## ID 전략 비교표

| 전략 | 형식 | 길이 | 정렬 가능 | 분산 환경 | 사용 사례 |
|------|------|------|-----------|-----------|-----------|
| **UUID** | `550e8400-e29b-41d4-a716-446655440000` | 36자 | ❌ | ✅ | 외부 API, 보안 토큰 |
| **Snowflake** | `7184811009659318273` | 64비트 정수 | ✅ | ✅ | 대규모 분산 시스템 |
| **PrefixedSequence** | `USER_000042` | 가변 | ✅ | ❌ | 읽기 쉬운 ID |
| **TimestampBased** | `ORD_1705312845123_42` | 가변 | ✅ | ⚠️ | 로그, 임시 ID |
| **AutoIncrement** | `42` | 정수 | ✅ | ❌ | 단일 DB 시스템 |

## 1. UUID (Universally Unique Identifier)

### 특징
- **형식**: 8-4-4-4-12 형태의 36자 문자열
- **충돌 확률**: 2^122 중 1 (사실상 0)
- **표준**: RFC 4122 준수

### 사용 코드
```rust
use shared::service::db::IdStrategy;

// UUID v4 생성 (랜덤)
let uuid = db_service.generate_id(&IdStrategy::Uuid).await?;
// 예: "550e8400-e29b-41d4-a716-446655440000"
```

### 언제 사용할까?
✅ **추천하는 경우**:
- 외부 시스템과 통합할 때
- API 키나 토큰 생성
- 보안이 중요한 식별자
- ID 예측을 방지해야 할 때

❌ **피해야 할 경우**:
- URL에 사용 (너무 김)
- 사용자가 입력해야 하는 경우
- 정렬이 중요한 경우

### 실제 사용 예제
```rust
// API 키 생성
let api_key = db_service.generate_id(&IdStrategy::Uuid).await?;

let mut api_token = HashMap::new();
api_token.insert("key".to_string(), serde_json::json!(api_key));
api_token.insert("user_id".to_string(), serde_json::json!(user_id));
api_token.insert("expires_at".to_string(), serde_json::json!(expires));

db_service.insert("api_tokens", api_token).await?;
```

## 2. Snowflake ID

### 구조 (64비트)
```
|1bit|  41bit  |  10bit  |  12bit  |
| 0  |timestamp|machine  |sequence |
```
- **1비트**: 항상 0 (부호 비트)
- **41비트**: 타임스탬프 (밀리초, ~69년)
- **10비트**: 머신 ID (1024대)
- **12비트**: 시퀀스 번호 (밀리초당 4096개)

### 사용 코드
```rust
// 초기화 시 설정
let db_service = EnhancedBaseDbServiceImpl::new(pool)
    .with_snowflake(
        1,  // machine_id (0-1023)
        1   // datacenter_id (0-1023)
    )
    .await?;

// ID 생성
let snowflake_id = db_service.generate_id(&IdStrategy::Snowflake {
    machine_id: 1,
    datacenter_id: 1,
}).await?;
// 예: "7184811009659318273"
```

### 언제 사용할까?
✅ **추천하는 경우**:
- 분산 시스템 (여러 서버)
- 시간순 정렬이 필요한 경우
- 대량의 ID 생성 (초당 수백만 개)
- 채팅 메시지, 이벤트 로그

❌ **피해야 할 경우**:
- 단일 서버 환경
- ID 크기가 중요한 경우

### 실제 사용 예제
```rust
// 게임 이벤트 로깅
let event_id = db_service.generate_id(&IdStrategy::Snowflake {
    machine_id: server_id,
    datacenter_id: region_id,
}).await?;

let mut event = HashMap::new();
event.insert("id".to_string(), serde_json::json!(event_id));
event.insert("type".to_string(), serde_json::json!("player_action"));
event.insert("player_id".to_string(), serde_json::json!(player_id));
event.insert("timestamp".to_string(), serde_json::json!(timestamp));

db_service.insert("game_events", event).await?;

// 시간순 조회 가능
let events = db_service.select(
    "game_events",
    Some("id > ?"),  // Snowflake ID는 시간순
    Some(params)
).await?;
```

### Snowflake 설정 가이드
```rust
// 서버별 설정 예제
match server_type {
    "game" => (1, 1),      // 게임 서버
    "chat" => (2, 1),      // 채팅 서버
    "api" => (3, 1),       // API 서버
    "worker" => (4, 1),    // 백그라운드 워커
    _ => (0, 0),
}
```

## 3. PrefixedSequence (접두사 시퀀스)

### 특징
- 읽기 쉬운 형식
- 비즈니스 의미 포함
- 순차 번호 보장

### 사용 코드
```rust
// 사용자 ID: USER_000001
let user_id = db_service.generate_id(&IdStrategy::PrefixedSequence {
    prefix: "USER_".to_string(),
    padding: 6,  // 6자리 0 패딩
}).await?;

// 주문 번호: ORD2024_00042
let order_id = db_service.generate_id(&IdStrategy::PrefixedSequence {
    prefix: format!("ORD{}_", current_year),
    padding: 5,
}).await?;

// 티켓 번호: TKT-1234
let ticket_id = db_service.generate_id(&IdStrategy::PrefixedSequence {
    prefix: "TKT-".to_string(),
    padding: 4,
}).await?;
```

### 언제 사용할까?
✅ **추천하는 경우**:
- 고객 서비스 (주문번호, 티켓번호)
- 사람이 읽어야 하는 ID
- 업무 프로세스 추적
- 인보이스, 영수증

❌ **피해야 할 경우**:
- 분산 환경
- 높은 동시성
- 보안이 중요한 경우 (예측 가능)

### 실제 사용 예제
```rust
// 주문 시스템
async fn create_order(db: &impl EnhancedBaseDbService) -> Result<String, AppError> {
    // 연도별 주문 번호 생성
    let year = chrono::Utc::now().year();
    let order_id = db.generate_id(&IdStrategy::PrefixedSequence {
        prefix: format!("ORD{}-", year),
        padding: 6,
    }).await?;
    // 예: "ORD2024-000042"
    
    let mut order = HashMap::new();
    order.insert("order_id".to_string(), serde_json::json!(order_id.clone()));
    order.insert("status".to_string(), serde_json::json!("pending"));
    
    db.insert("orders", order).await?;
    
    // 고객에게 보여줄 수 있는 친숙한 ID
    println!("주문 번호: {}", order_id);
    Ok(order_id)
}
```

## 4. TimestampBased (타임스탬프 기반)

### 형식
```
[접두사]_[타임스탬프]_[랜덤/시퀀스]
예: LOG_1705312845123_42
```

### 사용 코드
```rust
// 기본 타임스탬프 ID
let id = db_service.generate_id(&IdStrategy::TimestampBased {
    prefix: None,
}).await?;
// 예: "1705312845123_42"

// 접두사 포함
let log_id = db_service.generate_id(&IdStrategy::TimestampBased {
    prefix: Some("LOG_".to_string()),
}).await?;
// 예: "LOG_1705312845123_42"

// 세션 ID
let session_id = db_service.generate_id(&IdStrategy::TimestampBased {
    prefix: Some("SESSION_".to_string()),
}).await?;
```

### 언제 사용할까?
✅ **추천하는 경우**:
- 로그 항목
- 임시 데이터
- 세션 ID
- 디버깅 용도

❌ **피해야 할 경우**:
- 영구 데이터
- 높은 동시성 (같은 밀리초 충돌 가능)
- 외부 공개 ID

### 실제 사용 예제
```rust
// 감사 로그
async fn audit_log(db: &impl EnhancedBaseDbService, action: &str) -> Result<(), AppError> {
    let audit_id = db.generate_id(&IdStrategy::TimestampBased {
        prefix: Some("AUDIT_".to_string()),
    }).await?;
    
    let mut log = HashMap::new();
    log.insert("id".to_string(), serde_json::json!(audit_id));
    log.insert("action".to_string(), serde_json::json!(action));
    log.insert("timestamp".to_string(), serde_json::json!(chrono::Utc::now()));
    
    db.insert("audit_logs", log).await?;
    
    // 타임스탬프가 포함되어 있어 정렬/필터링 쉬움
    Ok(())
}
```

## 5. AutoIncrement (자동 증가)

### 특징
- 데이터베이스 내장 기능
- 가장 간단하고 효율적
- 단일 서버 전용

### 사용 코드
```rust
// 다음 AUTO_INCREMENT 값 예측
let next_id = db_service.get_next_auto_increment_id("users").await?;

// INSERT 후 ID 가져오기
let mut data = HashMap::new();
data.insert("name".to_string(), serde_json::json!("John"));

let new_id = db_service.insert_returning_id(
    "users",
    data,
    "id"  // AUTO_INCREMENT 컬럼
).await?;
```

### 언제 사용할까?
✅ **추천하는 경우**:
- 단일 데이터베이스
- 간단한 시스템
- 레거시 시스템
- 성능이 중요한 경우

❌ **피해야 할 경우**:
- 분산 시스템
- 마이크로서비스
- 보안 중요 (예측 가능)

## ID 전략 선택 가이드

### 의사결정 트리
```
분산 시스템인가?
├─ YES → 시간순 정렬 필요?
│        ├─ YES → Snowflake
│        └─ NO → UUID
└─ NO → 사람이 읽어야 하는가?
         ├─ YES → PrefixedSequence
         └─ NO → 보안 중요?
                  ├─ YES → UUID
                  └─ NO → AutoIncrement
```

### 시스템별 추천
| 시스템 유형 | 추천 전략 | 이유 |
|------------|----------|------|
| 게임 이벤트 | Snowflake | 대량 생성, 시간순 정렬 |
| 사용자 계정 | UUID | 보안, 예측 불가 |
| 주문 시스템 | PrefixedSequence | 고객 친화적 |
| 채팅 메시지 | Snowflake | 시간순, 대량 |
| API 토큰 | UUID | 보안, 표준 |
| 로그 | TimestampBased | 디버깅 용이 |
| 내부 테이블 | AutoIncrement | 단순, 효율적 |

## 마이그레이션 가이드

### AutoIncrement → UUID 마이그레이션
```rust
// 1. 새 컬럼 추가
sqlx::query("ALTER TABLE users ADD COLUMN uuid VARCHAR(36)")
    .execute(pool).await?;

// 2. UUID 생성 및 업데이트
let users = db.select("users", None, None).await?;
for user in users {
    let uuid = db.generate_id(&IdStrategy::Uuid).await?;
    let mut params = HashMap::new();
    params.insert("uuid".to_string(), serde_json::json!(uuid));
    params.insert("id".to_string(), user.get("id").unwrap().clone());
    
    db.execute_non_query(
        "UPDATE users SET uuid = ? WHERE id = ?",
        Some(params)
    ).await?;
}

// 3. 새 컬럼을 기본키로 변경
```

### 단일 서버 → 분산 시스템
```rust
// 기존: AutoIncrement
let id = db.insert_returning_id("events", data, "id").await?;

// 변경: Snowflake로 전환
let id = db.generate_id(&IdStrategy::Snowflake {
    machine_id: server_config.machine_id,
    datacenter_id: server_config.datacenter_id,
}).await?;
data.insert("id".to_string(), serde_json::json!(id));
db.insert("events", data).await?;
```

## 성능 비교

| 전략 | 생성 속도 | 저장 공간 | DB 인덱스 성능 |
|------|-----------|-----------|----------------|
| AutoIncrement | ⚡⚡⚡⚡⚡ | 4-8 bytes | 최고 |
| Snowflake | ⚡⚡⚡⚡ | 8 bytes | 우수 |
| UUID | ⚡⚡⚡ | 36 bytes | 보통 |
| PrefixedSequence | ⚡⚡ | 가변 | 보통 |
| TimestampBased | ⚡⚡⚡ | 가변 | 보통 |

## 모범 사례

### 1. 일관성 유지
```rust
// 한 테이블에는 하나의 ID 전략만 사용
struct UserService {
    id_strategy: IdStrategy,  // 서비스 초기화 시 결정
}
```

### 2. 적절한 인덱스
```sql
-- UUID는 인덱스 크기가 큼
CREATE INDEX idx_uuid ON users(uuid) USING HASH;

-- Snowflake는 B-Tree 인덱스 효율적
CREATE INDEX idx_snowflake ON events(id);
```

### 3. ID 캐싱
```rust
// 자주 사용하는 ID는 캐싱
let cached_id = redis_client
    .get(format!("user:id:{}", username))
    .await?;
```

## 트러블슈팅

### 문제: Snowflake ID 시간 역전
```rust
// 해결: NTP 동기화 확인
// 또는 에러 처리 추가
if new_timestamp < last_timestamp {
    tokio::time::sleep(Duration::from_millis(1)).await;
    return self.generate().await;
}
```

### 문제: UUID 인덱스 성능
```sql
-- 해결: 바이너리 저장으로 변경
ALTER TABLE users 
  MODIFY uuid BINARY(16);

-- 애플리케이션에서 변환
let uuid_bytes = uuid.as_bytes();
```

### 문제: PrefixedSequence 동시성
```rust
// 해결: 분산 잠금 사용
let lock = redis_client.lock("sequence_lock").await?;
let id = db.generate_id(&IdStrategy::PrefixedSequence { ... }).await?;
lock.release().await?;
```