# TCP 서버 아키텍처 및 확장 가이드

## 📋 목차
1. [서버 아키텍처 개요](#서버-아키텍처-개요)
2. [핵심 컴포넌트](#핵심-컴포넌트)
3. [확장 방법](#확장-방법)
4. [성능 최적화](#성능-최적화)
5. [모니터링 및 디버깅](#모니터링-및-디버깅)

## 🏗️ 서버 아키텍처 개요

```
TCP Server Architecture
├── TcpGameService (메인 서버)
├── ConnectionService (연결 관리)
├── HeartbeatService (생존 확인)
├── MessageService (메시지 처리)
├── RoomConnectionService (방 기반 연결)
├── ChatRoomMessageHandler (채팅방 핸들러)
└── Performance Services (성능 최적화)
    ├── EnhancedMemoryPool (메모리 풀)
    ├── AsyncTaskScheduler (비동기 스케줄러)
    ├── SIMD Optimizer (하드웨어 가속)
    └── Performance Monitor (성능 모니터링)
```

### 현재 성능 지표
- **처리량**: 12,991+ msg/sec (검증된)
- **동시 연결**: 500+ 사용자
- **메모리 효율**: 22KB/연결
- **지연시간**: <1ms p99

## 🔧 핵심 컴포넌트

### 1. TcpGameService
**역할**: TCP 서버의 생명주기 관리 및 전체 서비스 조율

```rust
// 서버 시작
let server = TcpGameService::with_default_config();
server.start().await?;

// 커스텀 설정
let config = TcpServerConfig {
    bind_address: "0.0.0.0:8080".to_string(),
    max_connections: 10000,
    enable_enhanced_memory_pool: true,
    enable_async_scheduler: true,
    ..Default::default()
};
let server = TcpGameService::with_config(config);
```

### 2. ConnectionService
**역할**: 클라이언트 연결 상태 관리 및 메시지 라우팅

```rust
// 연결 정보 조회
let connection_count = server.get_connection_count().await;
let stats = connection_service.get_connection_stats().await;

// 특정 사용자에게 메시지 전송
connection_service.send_to_user(user_id, &message).await?;
```

### 3. ChatRoomMessageHandler
**역할**: 실시간 채팅방 시스템 관리

```rust
// 방 상태 조회
let (user_count, users) = server.get_room_status(room_id);
let all_rooms = server.get_all_rooms_status();

// 빈 방 정리
let cleaned = server.cleanup_empty_rooms().await;
```

## 🚀 확장 방법

### 1. 새로운 메시지 타입 추가

#### Step 1: 프로토콜 정의 (protocol.rs)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameMessage {
    // ... 기존 메시지들
    
    // 새로운 메시지 타입 추가
    CustomCommand {
        command_type: String,
        parameters: HashMap<String, String>,
        timestamp: u64,
    },
    
    CustomResponse {
        command_type: String,
        success: bool,
        data: Option<String>,
        error_message: Option<String>,
    },
}
```

#### Step 2: 메시지 핸들러 생성
```rust
// handlers/custom_handler.rs
use crate::protocol::GameMessage;
use anyhow::Result;

pub struct CustomHandler {
    // 필요한 서비스들
}

impl CustomHandler {
    pub fn new() -> Self {
        Self {}
    }
    
    pub async fn handle_custom_command(
        &self,
        user_id: u32,
        command_type: &str,
        parameters: &HashMap<String, String>
    ) -> Result<GameMessage> {
        match command_type {
            "ping" => Ok(GameMessage::CustomResponse {
                command_type: "pong".to_string(),
                success: true,
                data: Some("pong".to_string()),
                error_message: None,
            }),
            _ => Ok(GameMessage::CustomResponse {
                command_type: command_type.to_string(),
                success: false,
                data: None,
                error_message: Some("Unknown command".to_string()),
            }),
        }
    }
}
```

### 2. 새로운 서비스 컴포넌트 추가

#### Step 1: 서비스 구조 정의
```rust
// service/my_custom_service.rs
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::Result;

pub struct MyCustomService {
    data_store: Arc<RwLock<HashMap<String, String>>>,
    config: CustomServiceConfig,
}

#[derive(Debug, Clone)]
pub struct CustomServiceConfig {
    pub cache_size: usize,
    pub cleanup_interval: u64,
}

impl MyCustomService {
    pub fn new(config: CustomServiceConfig) -> Self {
        Self {
            data_store: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }
    
    pub async fn process_data(&self, key: &str, value: &str) -> Result<()> {
        let mut store = self.data_store.write().await;
        store.insert(key.to_string(), value.to_string());
        Ok(())
    }
    
    pub async fn get_data(&self, key: &str) -> Option<String> {
        let store = self.data_store.read().await;
        store.get(key).cloned()
    }
}
```

#### Step 2: TcpGameService에 통합
```rust
// service/tcp_service.rs에서
pub struct TcpGameService {
    // ... 기존 필드들
    my_custom_service: Arc<MyCustomService>,
}

impl TcpGameService {
    pub fn new(config: TcpServerConfig) -> Self {
        // ... 기존 초기화
        
        let custom_service = Arc::new(MyCustomService::new(
            CustomServiceConfig {
                cache_size: 1000,
                cleanup_interval: 300,
            }
        ));
        
        Self {
            // ... 기존 필드들
            my_custom_service: custom_service,
        }
    }
    
    // 새로운 API 메서드 추가
    pub async fn custom_operation(&self, key: &str, value: &str) -> Result<()> {
        self.my_custom_service.process_data(key, value).await
    }
}
```

### 3. 미들웨어 패턴으로 기능 확장

#### Step 1: 미들웨어 트레이트 정의
```rust
// middleware/mod.rs
use async_trait::async_trait;
use crate::protocol::GameMessage;

#[async_trait]
pub trait MessageMiddleware: Send + Sync {
    async fn before_process(
        &self,
        user_id: u32,
        message: &mut GameMessage
    ) -> Result<bool>; // false면 처리 중단
    
    async fn after_process(
        &self,
        user_id: u32,
        message: &GameMessage,
        result: &Result<GameMessage>
    ) -> Result<()>;
}

// 로깅 미들웨어 예시
pub struct LoggingMiddleware {
    enabled: bool,
}

#[async_trait]
impl MessageMiddleware for LoggingMiddleware {
    async fn before_process(&self, user_id: u32, message: &mut GameMessage) -> Result<bool> {
        if self.enabled {
            tracing::info!("Processing message from user {}: {:?}", user_id, message);
        }
        Ok(true) // 계속 처리
    }
    
    async fn after_process(
        &self,
        user_id: u32,
        message: &GameMessage,
        result: &Result<GameMessage>
    ) -> Result<()> {
        if self.enabled {
            match result {
                Ok(response) => tracing::info!("Response sent to {}: {:?}", user_id, response),
                Err(e) => tracing::error!("Error processing message for {}: {}", user_id, e),
            }
        }
        Ok(())
    }
}
```

## ⚡ 성능 최적화

### 1. 메모리 풀 활용
```rust
// 고성능 버퍼 할당
if let Some(buffer) = server.allocate_buffer(4096) {
    // 버퍼 사용
    let mut data = buffer.get_buffer();
    data.extend_from_slice(b"test data");
    
    // 사용 완료 후 반환
    server.deallocate_buffer(buffer);
}
```

### 2. 비동기 스케줄러 활용
```rust
// 우선순위 기반 작업 스케줄링
server.schedule_message_processing(async {
    // 중요한 메시지 처리
    process_critical_message().await;
}, true).await?; // true = critical priority

// 백그라운드 정리 작업
server.schedule_background_cleanup(async {
    cleanup_old_data().await;
}).await?;
```

### 3. SIMD 최적화 활용
```rust
use shared::tool::high_performance::simd_optimizer::*;

// 대용량 데이터 처리시 SIMD 활용
let data = vec![1u8; 10000];
let checksum = fast_checksum(&data); // SIMD 최적화된 체크섬

// 메모리 비교
if fast_memory_compare(&data1, &data2) {
    // SIMD 최적화된 비교
}
```

## 📊 모니터링 및 디버깅

### 1. 서버 상태 모니터링
```rust
// 실시간 서버 통계
let stats = server.get_server_stats().await;
println!("Connection count: {}", stats.connection_count);
println!("Memory pool enabled: {}", stats.enhanced_memory_pool_enabled);
println!("Scheduler enabled: {}", stats.async_scheduler_enabled);

// 메모리 풀 성능 확인
if let Some(pool_report) = server.get_memory_pool_status().await {
    println!("Memory Pool Performance:\n{}", pool_report);
}

// 스케줄러 성능 확인
if let Some(scheduler_report) = server.get_scheduler_performance_report().await {
    println!("Scheduler Performance:\n{}", scheduler_report);
}
```

### 2. 성능 벤치마크
```rust
// service/performance_benchmark.rs 활용
use crate::service::performance_benchmark::*;

// 메시지 처리 성능 테스트
let benchmark_result = run_message_processing_benchmark(
    1000,  // 메시지 수
    100,   // 동시 연결 수
    Duration::from_secs(30), // 테스트 시간
).await;

println!("Messages/sec: {}", benchmark_result.messages_per_second);
println!("Average latency: {}ms", benchmark_result.avg_latency_ms);
```

## 🔧 확장 모범 사례

### 1. 에러 처리
- `anyhow::Result`를 일관되게 사용
- 의미있는 에러 메시지 제공
- 에러 로깅과 복구 전략 구현

### 2. 비동기 프로그래밍
- `tokio::spawn` 대신 스케줄러 활용
- `Arc<Mutex<T>>` 대신 `Arc<RwLock<T>>` 선호
- 데드락 방지를 위한 lock 순서 일관성

### 3. 메모리 관리
- 메모리 풀을 적극 활용
- 큰 데이터는 스트리밍 처리
- 주기적인 메모리 정리 작업

### 4. 테스트
- 단위 테스트와 통합 테스트 작성
- 성능 테스트 포함
- 로드 테스트로 확장성 검증