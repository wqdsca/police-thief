# Police Thief 게임 서버 로깅 시스템

## 📝 개요

Police Thief 게임 서버를 위한 종합적인 로깅 시스템입니다. TDD 방식으로 개발되었으며, 고성능 비동기 로깅, 자동 파일 순환, 기능별 로그 분류 등의 기능을 제공합니다.

## 🎯 주요 기능

### ⚡ 고성능 비동기 로깅
- **논블로킹 I/O**: 게임 서버 성능에 영향을 최소화
- **배치 처리**: 다수의 로그를 효율적으로 묶어서 처리
- **메모리 버퍼링**: 적응형 버퍼 크기로 메모리 사용량 최적화

### 📁 기능별 로그 분류
```
logs/
├── grpcserver/     # gRPC API 서버 로그
├── tcpserver/      # TCP 게임 서버 로그  
├── rudpserver/     # RUDP 게임 서버 로그
├── gamecenter/     # 게임 센터 로그
└── shared/         # 공유 라이브러리 로그
```

### 📅 자동 파일 관리
- **날짜별 파일 생성**: `grpc_2024-01-15.log` 형식
- **자동 순환**: 파일 크기 제한 (기본 100MB) 시 자동 순환
- **보관 정책**: 7일 경과 후 자동 삭제
- **압축 지원**: 디스크 공간 절약

### 🎨 다양한 출력 형식
- **JSON 형식**: 구조화된 로그 분석 용이
- **텍스트 형식**: 사람이 읽기 쉬운 형태
- **색상 지원**: 개발 환경에서 가독성 향상

## 🚀 빠른 시작

### 1. 기본 사용법

```rust
use shared::logging::{init_logging, ServiceType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 로깅 시스템 초기화
    let logger = init_logging(ServiceType::GrpcServer, None).await?;
    
    // 다양한 레벨의 로그 작성
    logger.info("서버 시작됨", &[("port", "50051")]).await;
    logger.warn("메모리 사용량 높음", &[("usage", "85%")]).await;
    logger.error("데이터베이스 연결 실패", &[("error", "timeout")]).await;
    
    Ok(())
}
```

### 2. 환경변수 설정

```bash
# 로그 보관 일수 (기본값: 7일)
export LOG_RETENTION_DAYS=14

# 최대 파일 크기 (기본값: 100MB)
export LOG_MAX_FILE_SIZE=52428800

# JSON 형식 사용 여부 (기본값: true)
export LOG_JSON_FORMAT=true

# 플러시 간격 (기본값: 5초)
export LOG_FLUSH_INTERVAL=5

# 비동기 큐 크기 (기본값: 10000)
export LOG_QUEUE_SIZE=10000
```

### 3. 서비스별 초기화

```rust
// gRPC 서버
let grpc_logger = init_logging(ServiceType::GrpcServer, Some("./logs")).await?;

// TCP 서버  
let tcp_logger = init_logging(ServiceType::TcpServer, Some("./logs")).await?;

// RUDP 서버
let rudp_logger = init_logging(ServiceType::RudpServer, Some("./logs")).await?;

// 게임 센터
let game_logger = init_logging(ServiceType::GameCenter, Some("./logs")).await?;
```

## 📊 로그 레벨

| 레벨 | 용도 | 권장 환경 |
|------|------|-----------|
| `TRACE` | 상세한 디버깅 정보 | 개발 |
| `DEBUG` | 개발 정보 | 개발/스테이징 |
| `INFO` | 일반 정보 | 모든 환경 |
| `WARN` | 경고 (복구 가능) | 모든 환경 |
| `ERROR` | 오류 (복구 불가) | 모든 환경 |
| `FATAL` | 시스템 중단 수준 | 모든 환경 |

## 🔧 고급 사용법

### 1. 커스텀 설정으로 시스템 생성

```rust
use shared::logging::{LoggingSystem, LoggingConfig, ServiceType};

let mut config = LoggingConfig::default();
config.retention_days = 14;
config.max_file_size = 50 * 1024 * 1024; // 50MB
config.json_format = false;

let mut system = LoggingSystem::new("./custom_logs").await?;
system.init(ServiceType::GrpcServer).await?;
```

### 2. 구조화된 컨텍스트 데이터

```rust
// 사용자 인증 로그
logger.info("사용자 로그인 성공", &[
    ("user_id", "12345"),
    ("ip_address", "192.168.1.100"),
    ("session_id", "sess_abc123"),
    ("login_method", "oauth"),
    ("duration_ms", "250")
]).await;

// 게임 이벤트 로그
logger.info("플레이어 방 입장", &[
    ("player_id", "67890"),
    ("room_id", "room_456"),
    ("room_type", "normal"),
    ("current_players", "3"),
    ("max_players", "4")
]).await;
```

### 3. 에러 상황 로깅

```rust
// 구체적인 에러 정보와 함께 로깅
logger.error("Redis 연결 실패", &[
    ("redis_host", "localhost:6379"),
    ("error_type", "connection_timeout"),
    ("retry_count", "3"),
    ("last_error", "Connection refused")
]).await;
```

## 📈 성능 특성

### 벤치마크 결과
- **처리량**: 1,000개 로그 작성 < 10ms
- **메모리 사용량**: 베이스라인 대비 < 5MB 추가
- **지연시간**: 평균 < 1ms (99% < 5ms)
- **동시성**: 100개 동시 작성 스레드 지원

### 성능 최적화 팁

```rust
// 1. 배치 로그 작성 (권장)
for i in 0..1000 {
    logger.info(&format!("Batch log {}", i), &[("batch_id", "1")]).await;
}
// 자동으로 100개씩 배치 처리됨

// 2. 명시적 플러시 (필요시만)
logger.flush().await?; // 즉시 디스크에 기록

// 3. 컨텍스트 데이터 최적화
let user_id = "12345";
logger.info("User action", &[("user_id", user_id)]).await; // ✅ 좋음
// logger.info("User action", &[("user_id", &expensive_computation())]).await; // ❌ 피하세요
```

## 🧪 테스트

### 단위 테스트 실행
```bash
# 전체 테스트
cargo test --lib

# 로깅 시스템만
cargo test --test logging_integration_test

# 특정 테스트
cargo test test_async_logging_performance -- --nocapture
```

### 테스트 모드 사용
```rust
let mut system = LoggingSystem::new_test_mode().await?;
system.init(ServiceType::GrpcServer).await?;

system.info("Test message", &[]).await;

// 메모리에서 로그 확인
let logs = system.get_memory_logs().await.unwrap();
assert!(logs[0].contains("Test message"));
```

## 🔍 로그 분석

### JSON 형식 로그 분석 예시

```json
{
  "timestamp": "2024-01-15T10:30:45.123Z",
  "level": "Info", 
  "service": "grpcserver",
  "message": "사용자 인증 성공",
  "context": {
    "user_id": "12345",
    "ip_address": "192.168.1.100",
    "session_id": "sess_abc123"
  },
  "thread_id": "ThreadId(2)"
}
```

### 로그 검색 쿼리 예시

```bash
# 특정 사용자의 모든 로그
grep "user_id.*12345" logs/grpcserver/*.log

# 오류 로그만 필터링  
grep "\"level\":\"Error\"" logs/*/*.log

# 특정 시간대 로그
grep "2024-01-15T10:" logs/grpcserver/*.log
```

## ⚙️ 설정 참고

### LoggingConfig 구조체

```rust
pub struct LoggingConfig {
    /// 로그 보관 일수 (기본값: 7일)
    pub retention_days: u32,
    
    /// 최대 로그 파일 크기 (바이트 단위, 기본값: 100MB)  
    pub max_file_size: u64,
    
    /// 로그 플러시 간격 (기본값: 5초)
    pub flush_interval: Duration,
    
    /// 비동기 큐 크기 (기본값: 10000)
    pub async_queue_size: usize,
    
    /// JSON 형식 여부 (기본값: true)
    pub json_format: bool,
    
    /// 타임스탬프 UTC 사용 여부 (기본값: true)
    pub use_utc: bool,
    
    /// 디버그 모드 (기본값: false)  
    pub debug_mode: bool,
    
    /// 로그 압축 여부 (기본값: true)
    pub enable_compression: bool,
}
```

## 🛠️ 트러블슈팅

### 일반적인 문제들

**Q: 로그 파일이 생성되지 않아요**
```rust
// 권한 확인
let logger = init_logging(ServiceType::GrpcServer, Some("/var/log/app")).await?;
// 디렉토리 권한이 없을 수 있습니다.

// 해결책: 상대 경로 사용
let logger = init_logging(ServiceType::GrpcServer, Some("./logs")).await?;
```

**Q: 성능이 느려요**
```rust
// 설정 최적화
let mut config = LoggingConfig::default();
config.flush_interval = Duration::from_secs(10); // 플러시 간격 늘리기
config.async_queue_size = 50_000; // 큐 크기 늘리기
```

**Q: 로그 파일이 너무 많아요**
```bash
# 보관 일수 줄이기
export LOG_RETENTION_DAYS=3

# 수동 정리
find ./logs -name "*.log" -mtime +7 -delete
```

## 🤝 기여하기

1. **테스트 추가**: 새 기능에는 반드시 테스트 추가
2. **문서 업데이트**: 공개 API 변경 시 문서 수정  
3. **성능 테스트**: 성능에 영향을 주는 변경사항은 벤치마크 실행
4. **TDD 원칙**: 테스트 먼저 작성하고 구현

## 📜 라이선스

이 프로젝트는 Police Thief 게임 서버의 일부로, 프로젝트 라이선스를 따릅니다.

---

**개발팀**: Police Thief Backend Team  
**최종 업데이트**: 2024-01-15  
**버전**: 1.0.0