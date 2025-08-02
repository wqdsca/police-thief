# Police Thief Backend

Rust로 작성된 Police Thief 게임의 백엔드 서버입니다.

## 📁 프로젝트 구조

```
Backend/
├── Cargo.toml                 # Rust 의존성 관리
├── Cargo.lock                 # 의존성 잠금 파일
├── GrpcServer/                # gRPC 서버 구현
├── Share/                     # 공유 모듈
│   ├── Comman/               # 공통 유틸리티
│   │   ├── error.rs          # 통합 에러 처리 시스템
│   │   └── ...
│   ├── Config/               # 설정 관리
│   │   ├── redisConfig.rs    # Redis 설정
│   │   └── ...
│   └── Service/              # 서비스 레이어
│       └── Redis/            # Redis 서비스 모듈
│           ├── mod.rs        # 메인 모듈 정의
│           ├── core/         # 핵심 기능
│           │   ├── mod.rs    # 코어 모듈 정의
│           │   ├── retry_operation.rs  # 재시도 로직
│           │   └── redisGetKey.rs      # 키 생성 관리
│           ├── helpers/      # Redis 헬퍼들
│           │   ├── mod.rs    # 헬퍼 모듈 정의
│           │   ├── CacheHelper.rs      # LRU 캐시 헬퍼
│           │   ├── HashHelper.rs       # Hash 데이터 헬퍼
│           │   ├── GeoHelper.rs        # 위치 기반 헬퍼
│           │   ├── ZSetHelper.rs       # 랭킹/정렬 헬퍼
│           │   └── SetHelper.rs        # Set 데이터 헬퍼
│           └── tests/        # 테스트 코드
│               ├── mod.rs    # 테스트 모듈 정의
│               └── RedisTest.rs        # Redis 테스트
└── src/
    └── main.rs               # 애플리케이션 진입점
```

## 🔧 Redis 서비스 모듈

### 개요
Redis 서비스는 체계적으로 구성된 모듈로, 핵심 기능과 헬퍼들이 분리되어 있습니다.

#### 📂 폴더 구조
- **`core/`**: 핵심 기능 (재시도 로직, 키 생성)
- **`helpers/`**: Redis 데이터 타입별 헬퍼
- **`tests/`**: 테스트 코드



## ⚠️ 에러 처리

### 통합 에러 시스템
모든 Redis 작업은 `AppResult<T>` 타입을 반환하며, 구체적인 에러 정보를 제공합니다.

```rust
// 에러 타입들
AppError::Redis { message, operation }
AppError::Serialization { message, format }
AppError::Business { message, code }
AppError::NotFound { message, resource_type, resource_id }
// ... 기타

// 사용 예시
match cache.getItem::<User>(123).await {
    Ok(Some(user)) => println!("User found: {:?}", user),
    Ok(None) => println!("User not found"),
    Err(AppError::Redis { message, operation }) => {
        println!("Redis error in {}: {}", operation.unwrap_or("unknown"), message);
    }
    Err(e) => println!("Other error: {}", e.message()),
}
```

## 🧪 테스트

### 테스트 실행
```bash
cargo test --package police-thief-backend
```

### 테스트 예시
```rust
use Share::Service::Redis::tests::RedisTest;

#[test]
fn test_item_keys() {
    assert_eq!(item_key(KeyType::User, 1).unwrap(), "user:1");
    assert_eq!(item_key(KeyType::RoomInfo, 10).unwrap(), "room:list:10");
}

#[test]
fn test_error_cases() {
    // User key without id should fail
    assert!(try_get_key(KeyType::User, None).is_err());
    
    // RoomListByTime with id should fail
    assert!(try_get_key(KeyType::RoomListByTime, Some(1)).is_err());
}
```

## 🚀 성능 최적화

### 1. Lua 스크립트 활용
- CacheHelper의 LRU 로직은 Lua 스크립트로 원자적 실행
- 네트워크 왕복 최소화

### 2. 재시도 메커니즘
- 지수 백오프를 사용한 자동 재시도
- 네트워크 불안정성 대응

### 3. 연결 풀링
- `ConnectionManager`를 통한 효율적인 연결 관리
- 동시성 안전성 보장

## 📊 모니터링

### 로깅
```rust
use tracing::{info, warn, error};

info!("Cache item added: id={}, key_type={:?}", id, key_type);
warn!("Retry attempt {} for operation", attempt);
error!("Redis operation failed: {}", error);
```

### 메트릭 (추후 추가 예정)
- Redis 작업 성능 측정
- 캐시 히트율 모니터링
- 에러율 추적

## 🔧 설정

### Redis 연결 설정
```rust
// redisConfig.rs
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    pub database: u8,
    pub pool_size: usize,
    pub timeout: Duration,
}
```

### TTL 설정
- CacheHelper: 아이템별 TTL + 리스트 TTL
- HashHelper: Hash 전체 TTL
- GeoHelper: 위치 데이터 TTL
- ZSetHelper: 랭킹 데이터 TTL
- SetHelper: Set 전체 TTL

## 📝 사용 가이드

### 1. 새로운 Redis 헬퍼 추가
```rust
// Share/Service/Redis/helpers/NewHelper.rs
pub struct NewHelper {
    conn: RedisConnection,
    key: String,
    ttl: Option<u64>,
}

impl NewHelper {
    pub fn new(conn: RedisConnection, key: impl Into<String>, ttl: Option<u64>) -> Self {
        Self { conn, key: key.into(), ttl }
    }
    
    // 메서드 구현...
}
```

### 2. 헬퍼 모듈에 추가
```rust
// Share/Service/Redis/helpers/mod.rs
pub mod NewHelper;
```

### 3. 에러 처리 추가
```rust
// Share/Comman/error.rs에 새로운 에러 타입 추가
AppError::NewError { message, details }
```

### 4. 테스트 작성
```rust
// Share/Service/Redis/tests/NewHelperTest.rs
#[test]
fn test_new_helper() {
    // 테스트 구현
}
```

## 🤝 기여 가이드

1. 코드 스타일 준수
2. 테스트 코드 작성
3. 문서화 업데이트
4. 에러 처리 통합

## 📄 라이선스

이 프로젝트는 MIT 라이선스 하에 배포됩니다. 