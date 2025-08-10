# gRPC 서버 성능 최적화 가이드

## 📋 목차
1. [성능 최적화 개요](#성능-최적화-개요)
2. [HTTP/2 최적화](#http2-최적화)
3. [Protocol Buffers 최적화](#protocol-buffers-최적화)
4. [데이터베이스 최적화](#데이터베이스-최적화)
5. [캐싱 전략](#캐싱-전략)
6. [모니터링 및 프로파일링](#모니터링-및-프로파일링)

## 🎯 성능 최적화 개요

### 현재 성능 목표
```
gRPC Server Performance Targets:
├── RPS: 10,000+ requests/sec
├── 지연시간: <50ms p95, <100ms p99
├── 동시 연결: 5,000+ connections
├── 메모리: <2GB for 1000 concurrent users
├── CPU: <70% utilization under full load
└── 처리량: 100MB/s sustained
```

### 최적화 전략 계층
```
Optimization Layers:
├── Protocol Level (HTTP/2, Protocol Buffers)
├── Application Level (Connection pooling, Async I/O)
├── Data Level (Database, Cache, Serialization)
├── Infrastructure Level (Load balancing, CDN)
└── Monitoring Level (Observability, Alerting)
```

## ⚡ HTTP/2 최적화

### 1. 연결 관리 최적화

```rust
// src/server/connection_pool.rs
use tonic::transport::Server;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use std::time::Duration;

pub struct OptimizedGrpcServer {
    connection_config: ConnectionConfig,
    pool_manager: Arc<ConnectionPoolManager>,
    metrics_collector: Arc<MetricsCollector>,
}

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub max_concurrent_streams: u32,
    pub initial_window_size: u32,
    pub max_frame_size: u32,
    pub keepalive_interval: Duration,
    pub keepalive_timeout: Duration,
    pub tcp_keepalive: Duration,
    pub tcp_nodelay: bool,
    pub http2_adaptive_window: bool,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            max_concurrent_streams: 1000,        // HTTP/2 스트림 수 제한
            initial_window_size: 1 << 20,        // 1MB 초기 윈도우
            max_frame_size: 1 << 14,             // 16KB 최대 프레임
            keepalive_interval: Duration::from_secs(30),
            keepalive_timeout: Duration::from_secs(5),
            tcp_keepalive: Duration::from_secs(60),
            tcp_nodelay: true,                   // Nagle 알고리즘 비활성화
            http2_adaptive_window: true,         // 적응형 윈도우 크기
        }
    }
}

impl OptimizedGrpcServer {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            connection_config: config,
            pool_manager: Arc::new(ConnectionPoolManager::new()),
            metrics_collector: Arc::new(MetricsCollector::new()),
        }
    }
    
    pub async fn build_server(&self) -> Result<Server, Box<dyn std::error::Error + Send + Sync>> {
        let mut server = Server::builder()
            // HTTP/2 설정 최적화
            .http2_keepalive_interval(Some(self.connection_config.keepalive_interval))
            .http2_keepalive_timeout(Some(self.connection_config.keepalive_timeout))
            .http2_adaptive_window(self.connection_config.http2_adaptive_window)
            .initial_stream_window_size(Some(self.connection_config.initial_window_size))
            .initial_connection_window_size(Some(self.connection_config.initial_window_size))
            .max_frame_size(Some(self.connection_config.max_frame_size))
            
            // TCP 설정 최적화
            .tcp_keepalive(Some(self.connection_config.tcp_keepalive))
            .tcp_nodelay(self.connection_config.tcp_nodelay)
            
            // 동시성 제한
            .concurrency_limit_per_connection(self.connection_config.max_concurrent_streams)
            
            // 타임아웃 설정
            .timeout(Duration::from_secs(30))
            
            // 압축 활성화
            .accept_compressed(tonic::codec::CompressionEncoding::Gzip);
        
        // 미들웨어 스택 구성
        let service_stack = ServiceBuilder::new()
            .layer(TraceLayer::new_for_grpc())
            .layer(tower::limit::ConcurrencyLimitLayer::new(10000)) // 전역 동시성 제한
            .layer(tower::timeout::TimeoutLayer::new(Duration::from_secs(30)))
            .layer(tower::load_shed::LoadShedLayer::new())
            .layer(self.build_metrics_layer())
            .layer(self.build_auth_layer());
        
        Ok(server)
    }
    
    /// 메트릭 수집 레이어
    fn build_metrics_layer(&self) -> MetricsLayer {
        let collector = self.metrics_collector.clone();
        
        MetricsLayer::new(move |req| {
            let start_time = std::time::Instant::now();
            let collector_clone = collector.clone();
            
            async move {
                let result = req.await;
                let duration = start_time.elapsed();
                
                collector_clone.record_request_duration(duration);
                match &result {
                    Ok(_) => collector_clone.increment_success_counter(),
                    Err(_) => collector_clone.increment_error_counter(),
                }
                
                result
            }
        })
    }
    
    /// 인증 레이어
    fn build_auth_layer(&self) -> AuthLayer {
        AuthLayer::new(move |req| {
            // JWT 토큰 검증을 비동기적으로 처리
            async move {
                // 캐시된 토큰 검증으로 성능 최적화
                if let Some(auth_header) = req.headers().get("authorization") {
                    if let Ok(token) = auth_header.to_str() {
                        // Redis에 캐시된 토큰 검증 결과 사용
                        return validate_cached_token(token).await;
                    }
                }
                
                // 공개 엔드포인트는 통과
                if is_public_endpoint(req.uri().path()) {
                    return Ok(req);
                }
                
                Err(tonic::Status::unauthenticated("Token required"))
            }
        })
    }
}
```

### 2. 스트리밍 최적화

```rust
// src/streaming/optimized_stream.rs
use tonic::{Request, Response, Status, Streaming};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use std::pin::Pin;

pub struct OptimizedStreamHandler {
    buffer_size: usize,
    batch_size: usize,
    flush_interval: Duration,
    compression_enabled: bool,
}

impl OptimizedStreamHandler {
    pub fn new() -> Self {
        Self {
            buffer_size: 8192,                    // 8KB 버퍼
            batch_size: 100,                      // 100개 메시지 배치
            flush_interval: Duration::from_millis(10), // 10ms 플러시 간격
            compression_enabled: true,
        }
    }
    
    /// 클라이언트 스트리밍 최적화
    pub async fn handle_client_stream<T>(
        &self,
        mut stream: Streaming<T>,
    ) -> Result<Vec<T>, Status>
    where
        T: prost::Message + Default + Clone,
    {
        let mut messages = Vec::with_capacity(self.batch_size);
        let mut buffer = Vec::with_capacity(self.buffer_size);
        
        // 스트림을 배치 단위로 처리
        while let Some(message) = stream.next().await {
            match message {
                Ok(msg) => {
                    messages.push(msg);
                    
                    // 배치 크기에 도달하면 처리
                    if messages.len() >= self.batch_size {
                        self.process_message_batch(&mut messages, &mut buffer).await?;
                        messages.clear();
                    }
                }
                Err(e) => {
                    tracing::error!("Stream error: {:?}", e);
                    return Err(e);
                }
            }
        }
        
        // 남은 메시지 처리
        if !messages.is_empty() {
            self.process_message_batch(&mut messages, &mut buffer).await?;
        }
        
        Ok(buffer)
    }
    
    /// 서버 스트리밍 최적화
    pub async fn create_server_stream<T>(
        &self,
        data: Vec<T>,
    ) -> Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>>
    where
        T: Send + Clone + 'static,
    {
        let (tx, rx) = tokio::sync::mpsc::channel(self.buffer_size);
        let batch_size = self.batch_size;
        let flush_interval = self.flush_interval;
        
        // 백그라운드에서 데이터 전송
        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(batch_size);
            let mut flush_timer = tokio::time::interval(flush_interval);
            
            for item in data {
                batch.push(item);
                
                // 배치가 가득 차거나 타이머가 만료되면 전송
                if batch.len() >= batch_size {
                    Self::send_batch(&tx, &mut batch).await;
                }
                
                // 주기적으로 플러시
                tokio::select! {
                    _ = flush_timer.tick() => {
                        if !batch.is_empty() {
                            Self::send_batch(&tx, &mut batch).await;
                        }
                    }
                    _ = tokio::task::yield_now() => {}
                }
            }
            
            // 남은 배치 전송
            if !batch.is_empty() {
                Self::send_batch(&tx, &mut batch).await;
            }
        });
        
        Box::pin(ReceiverStream::new(rx))
    }
    
    /// 배치 전송
    async fn send_batch<T>(
        sender: &tokio::sync::mpsc::Sender<Result<T, Status>>,
        batch: &mut Vec<T>,
    ) where
        T: Clone,
    {
        for item in batch.drain(..) {
            if sender.send(Ok(item)).await.is_err() {
                tracing::warn!("Failed to send stream item: receiver dropped");
                break;
            }
        }
    }
    
    /// 메시지 배치 처리
    async fn process_message_batch<T>(
        &self,
        messages: &mut Vec<T>,
        buffer: &mut Vec<T>,
    ) -> Result<(), Status>
    where
        T: Clone,
    {
        // 병렬 처리
        let tasks: Vec<_> = messages.chunks(10).map(|chunk| {
            let chunk = chunk.to_vec();
            tokio::spawn(async move {
                // 각 청크를 비동기적으로 처리
                Self::process_chunk(chunk).await
            })
        }).collect();
        
        // 모든 작업 완료 대기
        for task in tasks {
            match task.await {
                Ok(Ok(processed)) => buffer.extend(processed),
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(Status::internal(format!("Task error: {:?}", e))),
            }
        }
        
        Ok(())
    }
    
    /// 청크 처리 (병렬 처리용)
    async fn process_chunk<T>(chunk: Vec<T>) -> Result<Vec<T>, Status>
    where
        T: Clone,
    {
        // 실제 비즈니스 로직 처리
        tokio::time::sleep(Duration::from_micros(100)).await; // 시뮬레이션
        Ok(chunk)
    }
}
```

## 🔧 Protocol Buffers 최적화

### 1. 메시지 설계 최적화

```protobuf
// proto/optimized_messages.proto
syntax = "proto3";

package optimized;

// 최적화된 메시지 설계 원칙:
// 1. 자주 사용되는 필드는 낮은 번호 (1-15: 1바이트 인코딩)
// 2. 옵션 필드는 높은 번호
// 3. 중첩 메시지보다는 평면 구조 선호
// 4. 반복 필드는 packed=true 사용

message OptimizedUserInfo {
  // 자주 사용되는 핵심 필드들 (1-15)
  int32 user_id = 1;           // 1바이트 인코딩
  string username = 2;         // 1바이트 인코딩
  string nickname = 3;         // 1바이트 인코딩
  int32 level = 4;             // 1바이트 인코딩
  int64 last_login = 5;        // 1바이트 인코딩
  
  // 덜 중요한 필드들 (16+)
  string email = 16;           // 2바이트 인코딩
  string profile_image = 17;
  repeated string tags = 18;   // packed 적용 불가 (string)
  
  // 숫자 배열은 packed 최적화
  repeated int32 achievements = 19 [packed=true];  // 공간 절약
  repeated int32 friend_ids = 20 [packed=true];
  
  // 큰 데이터는 별도 요청으로 분리 고려
  // bytes profile_data = 21;  // 큰 데이터는 별도 API로
}

message OptimizedGameState {
  // 필수 필드들
  int32 room_id = 1;
  int32 game_phase = 2;
  int32 round_number = 3;
  int32 remaining_time = 4;
  
  // 플레이어 정보 (경량화)
  repeated LightPlayerInfo players = 5;
  
  // 점수 정보
  GameScore score = 6;
  
  // 최근 이벤트만 (전체 이벤트는 별도 스트림)
  repeated GameEvent recent_events = 7; // 최대 10개로 제한
}

message LightPlayerInfo {
  int32 user_id = 1;
  string nickname = 2;
  int32 role = 3;
  int32 status = 4;
  
  // 위치 정보 (float → int32로 정밀도 조정)
  int32 pos_x = 5;  // 실제 좌표 * 1000 (밀리미터 정밀도)
  int32 pos_y = 6;
  int32 pos_z = 7;
  int32 rotation = 8;
  
  // 통계 (핵심만)
  int32 kills = 9;
  int32 deaths = 10;
  int32 score = 11;
}

// 대용량 데이터를 위한 청크 메시지
message DataChunk {
  string chunk_id = 1;
  int32 sequence = 2;
  int32 total_chunks = 3;
  bytes data = 4;
  string checksum = 5; // 데이터 무결성 검증
}

// 압축된 배치 요청
message BatchRequest {
  string request_id = 1;
  repeated google.protobuf.Any requests = 2; // 여러 요청을 배치로
  bool enable_compression = 3;
  int32 priority = 4; // 0=높음, 1=보통, 2=낮음
}

message BatchResponse {
  string request_id = 1;
  repeated google.protobuf.Any responses = 2;
  repeated Error errors = 3; // 부분 실패 처리
  int64 processing_time_ms = 4;
}
```

### 2. 직렬화/역직렬화 최적화

```rust
// src/serialization/optimized_proto.rs
use prost::{Message, DecodeError, EncodeError};
use std::sync::Arc;

pub struct OptimizedProtoHandler {
    buffer_pool: Arc<BufferPool>,
    compression_enabled: bool,
    cache: Arc<ProtoCache>,
}

struct BufferPool {
    small_buffers: crossbeam_queue::SegQueue<Vec<u8>>,  // < 1KB
    medium_buffers: crossbeam_queue::SegQueue<Vec<u8>>, // 1KB - 10KB
    large_buffers: crossbeam_queue::SegQueue<Vec<u8>>,  // > 10KB
}

impl BufferPool {
    fn new() -> Self {
        let pool = Self {
            small_buffers: crossbeam_queue::SegQueue::new(),
            medium_buffers: crossbeam_queue::SegQueue::new(),
            large_buffers: crossbeam_queue::SegQueue::new(),
        };
        
        // 버퍼 미리 할당
        for _ in 0..100 {
            pool.small_buffers.push(Vec::with_capacity(1024));
            pool.medium_buffers.push(Vec::with_capacity(10 * 1024));
            pool.large_buffers.push(Vec::with_capacity(100 * 1024));
        }
        
        pool
    }
    
    fn get_buffer(&self, size_hint: usize) -> Vec<u8> {
        let buffer = if size_hint < 1024 {
            self.small_buffers.pop()
        } else if size_hint < 10 * 1024 {
            self.medium_buffers.pop()
        } else {
            self.large_buffers.pop()
        };
        
        buffer.unwrap_or_else(|| Vec::with_capacity(size_hint.max(1024)))
    }
    
    fn return_buffer(&self, mut buffer: Vec<u8>) {
        buffer.clear();
        let capacity = buffer.capacity();
        
        if capacity < 1024 {
            let _ = self.small_buffers.push(buffer);
        } else if capacity < 10 * 1024 {
            let _ = self.medium_buffers.push(buffer);
        } else if capacity < 100 * 1024 {
            let _ = self.large_buffers.push(buffer);
        }
        // 너무 큰 버퍼는 버림
    }
}

impl OptimizedProtoHandler {
    pub fn new() -> Self {
        Self {
            buffer_pool: Arc::new(BufferPool::new()),
            compression_enabled: true,
            cache: Arc::new(ProtoCache::new()),
        }
    }
    
    /// 최적화된 직렬화
    pub fn encode_optimized<T: Message>(&self, message: &T) -> Result<Vec<u8>, EncodeError> {
        let size_hint = message.encoded_len();
        let mut buffer = self.buffer_pool.get_buffer(size_hint);
        
        // 직접 버퍼에 인코딩
        message.encode(&mut buffer)?;
        
        // 압축 적용 (큰 메시지만)
        if self.compression_enabled && buffer.len() > 1024 {
            let compressed = self.compress_data(&buffer)?;
            self.buffer_pool.return_buffer(buffer);
            Ok(compressed)
        } else {
            Ok(buffer)
        }
    }
    
    /// 최적화된 역직렬화
    pub fn decode_optimized<T: Message + Default>(
        &self, 
        data: &[u8]
    ) -> Result<T, DecodeError> {
        // 압축 해제 확인
        let decompressed_data = if self.is_compressed(data) {
            self.decompress_data(data)?
        } else {
            data.to_vec()
        };
        
        // 캐시에서 확인 (자주 사용되는 메시지)
        if let Some(cached) = self.cache.get::<T>(&decompressed_data) {
            return Ok(cached);
        }
        
        // 역직렬화
        let message = T::decode(&*decompressed_data)?;
        
        // 캐시에 저장 (작은 메시지만)
        if decompressed_data.len() < 1024 {
            self.cache.put(&decompressed_data, message.clone());
        }
        
        Ok(message)
    }
    
    /// 배치 직렬화
    pub fn encode_batch<T: Message>(
        &self, 
        messages: &[T]
    ) -> Result<Vec<u8>, EncodeError> {
        let total_size: usize = messages.iter().map(|m| m.encoded_len()).sum();
        let mut buffer = self.buffer_pool.get_buffer(total_size + messages.len() * 4); // 헤더 공간
        
        // 메시지 개수 인코딩
        (messages.len() as u32).encode(&mut buffer)?;
        
        // 각 메시지 길이 + 데이터 인코딩
        for message in messages {
            let message_size = message.encoded_len() as u32;
            message_size.encode(&mut buffer)?;
            message.encode(&mut buffer)?;
        }
        
        Ok(buffer)
    }
    
    /// 배치 역직렬화
    pub fn decode_batch<T: Message + Default>(
        &self, 
        data: &[u8]
    ) -> Result<Vec<T>, DecodeError> {
        let mut cursor = std::io::Cursor::new(data);
        
        // 메시지 개수 읽기
        let count = u32::decode(&mut cursor)? as usize;
        let mut messages = Vec::with_capacity(count);
        
        // 각 메시지 읽기
        for _ in 0..count {
            let message_size = u32::decode(&mut cursor)? as usize;
            let start_pos = cursor.position() as usize;
            let end_pos = start_pos + message_size;
            
            if end_pos > data.len() {
                return Err(DecodeError::new("Unexpected end of input"));
            }
            
            let message_data = &data[start_pos..end_pos];
            let message = T::decode(message_data)?;
            messages.push(message);
            
            cursor.set_position(end_pos as u64);
        }
        
        Ok(messages)
    }
    
    /// 데이터 압축
    fn compress_data(&self, data: &[u8]) -> Result<Vec<u8>, EncodeError> {
        // LZ4 고속 압축 사용
        let compressed = lz4_flex::compress_prepend_size(data);
        Ok(compressed)
    }
    
    /// 데이터 압축 해제
    fn decompress_data(&self, data: &[u8]) -> Result<Vec<u8>, DecodeError> {
        lz4_flex::decompress_size_prepended(data)
            .map_err(|e| DecodeError::new(format!("Decompression failed: {}", e)))
    }
    
    /// 압축 여부 확인
    fn is_compressed(&self, data: &[u8]) -> bool {
        data.len() > 4 && &data[0..4] == b"LZ4\x01" // LZ4 매직 바이트
    }
}

/// Proto 메시지 캐시
struct ProtoCache {
    cache: Arc<std::sync::RwLock<lru::LruCache<Vec<u8>, CachedMessage>>>,
}

#[derive(Clone)]
struct CachedMessage {
    data: Vec<u8>,
    last_access: std::time::Instant,
}

impl ProtoCache {
    fn new() -> Self {
        Self {
            cache: Arc::new(std::sync::RwLock::new(lru::LruCache::new(1000.try_into().unwrap()))),
        }
    }
    
    fn get<T: Message + Default>(&self, key: &[u8]) -> Option<T> {
        let mut cache = self.cache.write().unwrap();
        
        if let Some(cached) = cache.get_mut(key) {
            cached.last_access = std::time::Instant::now();
            T::decode(&*cached.data).ok()
        } else {
            None
        }
    }
    
    fn put<T: Message>(&self, key: &[u8], message: T) {
        let mut buffer = Vec::new();
        if message.encode(&mut buffer).is_ok() {
            let mut cache = self.cache.write().unwrap();
            cache.put(
                key.to_vec(),
                CachedMessage {
                    data: buffer,
                    last_access: std::time::Instant::now(),
                },
            );
        }
    }
}
```

## 💾 데이터베이스 최적화

### 1. 연결 풀 최적화

```rust
// src/database/optimized_pool.rs
use sqlx::{Pool, MySql, MySqlPool, Row};
use std::time::Duration;

pub struct OptimizedDatabasePool {
    read_pool: MySqlPool,
    write_pool: MySqlPool,
    cache: Arc<QueryCache>,
    metrics: Arc<DatabaseMetrics>,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub read_url: String,
    pub write_url: String,
    pub max_read_connections: u32,
    pub max_write_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
    pub enable_query_cache: bool,
    pub cache_size: usize,
}

impl OptimizedDatabasePool {
    pub async fn new(config: DatabaseConfig) -> Result<Self, sqlx::Error> {
        // 읽기 전용 풀 설정
        let read_pool = MySqlPool::connect_with(
            sqlx::mysql::MySqlConnectOptions::from_url(&config.read_url.parse().unwrap())?
                .statement_cache_capacity(100)  // 준비된 문장 캐시
        )
        .max_connections(config.max_read_connections)
        .acquire_timeout(config.connection_timeout)
        .idle_timeout(Some(config.idle_timeout))
        .max_lifetime(Some(config.max_lifetime))
        .build()
        .await?;
        
        // 쓰기 전용 풀 설정
        let write_pool = MySqlPool::connect_with(
            sqlx::mysql::MySqlConnectOptions::from_url(&config.write_url.parse().unwrap())?
                .statement_cache_capacity(50)
        )
        .max_connections(config.max_write_connections)
        .acquire_timeout(config.connection_timeout)
        .idle_timeout(Some(config.idle_timeout))
        .max_lifetime(Some(config.max_lifetime))
        .build()
        .await?;
        
        Ok(Self {
            read_pool,
            write_pool,
            cache: Arc::new(QueryCache::new(config.cache_size)),
            metrics: Arc::new(DatabaseMetrics::new()),
        })
    }
    
    /// 최적화된 SELECT 쿼리
    pub async fn execute_read_query<T>(
        &self,
        query: &str,
        params: &[&(dyn sqlx::Encode<'_, MySql> + sqlx::Type<MySql> + Sync)],
    ) -> Result<Vec<T>, sqlx::Error>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        let start_time = std::time::Instant::now();
        
        // 캐시 확인
        let cache_key = self.generate_cache_key(query, params);
        if let Some(cached_result) = self.cache.get::<Vec<T>>(&cache_key) {
            self.metrics.record_cache_hit(start_time.elapsed());
            return Ok(cached_result);
        }
        
        // 데이터베이스 쿼리 실행
        let mut query_builder = sqlx::query_as::<_, T>(query);
        for param in params {
            query_builder = query_builder.bind(*param);
        }
        
        let result = query_builder
            .fetch_all(&self.read_pool)
            .await;
        
        match result {
            Ok(data) => {
                // 캐시에 저장 (작은 결과만)
                if data.len() < 1000 {
                    self.cache.put(cache_key, data.clone(), Duration::from_secs(300));
                }
                
                self.metrics.record_query_success(start_time.elapsed());
                Ok(data)
            }
            Err(e) => {
                self.metrics.record_query_error(start_time.elapsed());
                Err(e)
            }
        }
    }
    
    /// 최적화된 INSERT/UPDATE/DELETE 쿼리
    pub async fn execute_write_query(
        &self,
        query: &str,
        params: &[&(dyn sqlx::Encode<'_, MySql> + sqlx::Type<MySql> + Sync)],
    ) -> Result<sqlx::mysql::MySqlQueryResult, sqlx::Error> {
        let start_time = std::time::Instant::now();
        
        let mut query_builder = sqlx::query(query);
        for param in params {
            query_builder = query_builder.bind(*param);
        }
        
        let result = query_builder
            .execute(&self.write_pool)
            .await;
        
        match result {
            Ok(result) => {
                // 관련 캐시 무효화
                self.invalidate_related_cache(query);
                
                self.metrics.record_write_success(start_time.elapsed());
                Ok(result)
            }
            Err(e) => {
                self.metrics.record_write_error(start_time.elapsed());
                Err(e)
            }
        }
    }
    
    /// 배치 삽입 최적화
    pub async fn batch_insert<T>(
        &self,
        table: &str,
        columns: &[&str],
        data: &[T],
    ) -> Result<u64, sqlx::Error>
    where
        T: BatchInsertable,
    {
        let start_time = std::time::Instant::now();
        
        if data.is_empty() {
            return Ok(0);
        }
        
        // VALUES 절을 동적으로 생성
        let placeholders: Vec<String> = data.iter()
            .map(|_| format!("({})", columns.iter().map(|_| "?").collect::<Vec<_>>().join(", ")))
            .collect();
        
        let query = format!(
            "INSERT INTO {} ({}) VALUES {}",
            table,
            columns.join(", "),
            placeholders.join(", ")
        );
        
        let mut query_builder = sqlx::query(&query);
        
        // 모든 파라미터 바인딩
        for item in data {
            let values = item.get_values();
            for value in values {
                query_builder = query_builder.bind(value);
            }
        }
        
        let result = query_builder
            .execute(&self.write_pool)
            .await?;
        
        self.metrics.record_batch_insert(data.len(), start_time.elapsed());
        Ok(result.rows_affected())
    }
    
    /// 트랜잭션 최적화
    pub async fn execute_transaction<F, R>(&self, callback: F) -> Result<R, sqlx::Error>
    where
        F: for<'c> FnOnce(&'c mut sqlx::Transaction<'_, MySql>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<R, sqlx::Error>> + Send + 'c>> + Send,
        R: Send,
    {
        let start_time = std::time::Instant::now();
        let mut tx = self.write_pool.begin().await?;
        
        match callback(&mut tx).await {
            Ok(result) => {
                tx.commit().await?;
                self.metrics.record_transaction_success(start_time.elapsed());
                Ok(result)
            }
            Err(e) => {
                tx.rollback().await?;
                self.metrics.record_transaction_error(start_time.elapsed());
                Err(e)
            }
        }
    }
    
    /// 캐시 키 생성
    fn generate_cache_key(
        &self,
        query: &str,
        params: &[&(dyn sqlx::Encode<'_, MySql> + sqlx::Type<MySql> + Sync)],
    ) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        query.hash(&mut hasher);
        // 파라미터는 해시하기 복잡하므로 쿼리만 사용
        format!("query:{:x}", hasher.finish())
    }
    
    /// 관련 캐시 무효화
    fn invalidate_related_cache(&self, query: &str) {
        // INSERT/UPDATE/DELETE에 영향받는 테이블 추출
        if let Some(table) = self.extract_table_name(query) {
            self.cache.invalidate_by_pattern(&format!("table:{}", table));
        }
    }
    
    /// 쿼리에서 테이블명 추출
    fn extract_table_name(&self, query: &str) -> Option<String> {
        let query = query.to_uppercase();
        
        if let Some(pos) = query.find("FROM ") {
            let after_from = &query[pos + 5..];
            if let Some(end) = after_from.find(' ') {
                Some(after_from[..end].trim().to_string())
            } else {
                Some(after_from.trim().to_string())
            }
        } else if let Some(pos) = query.find("UPDATE ") {
            let after_update = &query[pos + 7..];
            if let Some(end) = after_update.find(' ') {
                Some(after_update[..end].trim().to_string())
            } else {
                Some(after_update.trim().to_string())
            }
        } else if let Some(pos) = query.find("INSERT INTO ") {
            let after_insert = &query[pos + 12..];
            if let Some(end) = after_insert.find(' ') {
                Some(after_insert[..end].trim().to_string())
            } else {
                Some(after_insert.trim().to_string())
            }
        } else {
            None
        }
    }
    
    /// 연결 풀 상태 조회
    pub fn get_pool_status(&self) -> PoolStatus {
        PoolStatus {
            read_pool_size: self.read_pool.size(),
            read_pool_idle: self.read_pool.num_idle(),
            write_pool_size: self.write_pool.size(),
            write_pool_idle: self.write_pool.num_idle(),
            cache_hit_rate: self.cache.get_hit_rate(),
        }
    }
}

pub trait BatchInsertable {
    fn get_values(&self) -> Vec<&(dyn sqlx::Encode<'_, MySql> + sqlx::Type<MySql> + Sync)>;
}

#[derive(Debug)]
pub struct PoolStatus {
    pub read_pool_size: u32,
    pub read_pool_idle: u32,
    pub write_pool_size: u32,
    pub write_pool_idle: u32,
    pub cache_hit_rate: f64,
}
```

## 📊 모니터링 및 프로파일링

### 1. 성능 메트릭 수집

```rust
// src/monitoring/performance_monitor.rs
use prometheus::{Counter, Histogram, Gauge, Registry};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct PerformanceMonitor {
    registry: Registry,
    
    // gRPC 메트릭
    grpc_requests_total: Counter,
    grpc_request_duration: Histogram,
    grpc_active_connections: Gauge,
    
    // 데이터베이스 메트릭
    db_queries_total: Counter,
    db_query_duration: Histogram,
    db_connections_active: Gauge,
    
    // 캐시 메트릭
    cache_hits_total: Counter,
    cache_misses_total: Counter,
    cache_evictions_total: Counter,
    
    // 시스템 메트릭
    memory_usage_bytes: Gauge,
    cpu_usage_percent: Gauge,
    goroutines_count: Gauge,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        let registry = Registry::new();
        
        let grpc_requests_total = Counter::new(
            "grpc_requests_total", 
            "Total number of gRPC requests"
        ).unwrap();
        
        let grpc_request_duration = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "grpc_request_duration_seconds",
                "Duration of gRPC requests in seconds"
            ).buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0])
        ).unwrap();
        
        let grpc_active_connections = Gauge::new(
            "grpc_active_connections",
            "Number of active gRPC connections"
        ).unwrap();
        
        let db_queries_total = Counter::new(
            "db_queries_total",
            "Total number of database queries"
        ).unwrap();
        
        let db_query_duration = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "db_query_duration_seconds",
                "Duration of database queries in seconds"
            ).buckets(vec![0.0001, 0.001, 0.01, 0.1, 1.0])
        ).unwrap();
        
        let db_connections_active = Gauge::new(
            "db_connections_active",
            "Number of active database connections"
        ).unwrap();
        
        let cache_hits_total = Counter::new(
            "cache_hits_total",
            "Total number of cache hits"
        ).unwrap();
        
        let cache_misses_total = Counter::new(
            "cache_misses_total",
            "Total number of cache misses"
        ).unwrap();
        
        let cache_evictions_total = Counter::new(
            "cache_evictions_total",
            "Total number of cache evictions"
        ).unwrap();
        
        let memory_usage_bytes = Gauge::new(
            "memory_usage_bytes",
            "Memory usage in bytes"
        ).unwrap();
        
        let cpu_usage_percent = Gauge::new(
            "cpu_usage_percent",
            "CPU usage percentage"
        ).unwrap();
        
        let goroutines_count = Gauge::new(
            "goroutines_count",
            "Number of goroutines"
        ).unwrap();
        
        // 메트릭 등록
        registry.register(Box::new(grpc_requests_total.clone())).unwrap();
        registry.register(Box::new(grpc_request_duration.clone())).unwrap();
        registry.register(Box::new(grpc_active_connections.clone())).unwrap();
        registry.register(Box::new(db_queries_total.clone())).unwrap();
        registry.register(Box::new(db_query_duration.clone())).unwrap();
        registry.register(Box::new(db_connections_active.clone())).unwrap();
        registry.register(Box::new(cache_hits_total.clone())).unwrap();
        registry.register(Box::new(cache_misses_total.clone())).unwrap();
        registry.register(Box::new(cache_evictions_total.clone())).unwrap();
        registry.register(Box::new(memory_usage_bytes.clone())).unwrap();
        registry.register(Box::new(cpu_usage_percent.clone())).unwrap();
        registry.register(Box::new(goroutines_count.clone())).unwrap();
        
        Self {
            registry,
            grpc_requests_total,
            grpc_request_duration,
            grpc_active_connections,
            db_queries_total,
            db_query_duration,
            db_connections_active,
            cache_hits_total,
            cache_misses_total,
            cache_evictions_total,
            memory_usage_bytes,
            cpu_usage_percent,
            goroutines_count,
        }
    }
    
    /// gRPC 요청 기록
    pub fn record_grpc_request(&self, method: &str, duration: Duration, success: bool) {
        self.grpc_requests_total.inc();
        self.grpc_request_duration.observe(duration.as_secs_f64());
        
        tracing::info!(
            method = method,
            duration_ms = duration.as_millis(),
            success = success,
            "gRPC request completed"
        );
    }
    
    /// 데이터베이스 쿼리 기록
    pub fn record_db_query(&self, query_type: &str, duration: Duration, success: bool) {
        self.db_queries_total.inc();
        self.db_query_duration.observe(duration.as_secs_f64());
        
        if !success {
            tracing::warn!(
                query_type = query_type,
                duration_ms = duration.as_millis(),
                "Database query failed"
            );
        }
    }
    
    /// 캐시 히트/미스 기록
    pub fn record_cache_hit(&self) {
        self.cache_hits_total.inc();
    }
    
    pub fn record_cache_miss(&self) {
        self.cache_misses_total.inc();
    }
    
    pub fn record_cache_eviction(&self) {
        self.cache_evictions_total.inc();
    }
    
    /// 시스템 메트릭 업데이트
    pub async fn update_system_metrics(&self) {
        // 메모리 사용량 측정
        let memory_usage = self.get_memory_usage().await;
        self.memory_usage_bytes.set(memory_usage);
        
        // CPU 사용률 측정
        let cpu_usage = self.get_cpu_usage().await;
        self.cpu_usage_percent.set(cpu_usage);
        
        // 고루틴 수 (Tokio 태스크 수로 대체)
        let task_count = self.get_active_task_count().await;
        self.goroutines_count.set(task_count);
    }
    
    /// Prometheus 메트릭 익스포트
    pub fn export_metrics(&self) -> String {
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode_to_string(&metric_families).unwrap_or_default()
    }
    
    /// 성능 보고서 생성
    pub fn generate_performance_report(&self) -> PerformanceReport {
        PerformanceReport {
            total_grpc_requests: self.grpc_requests_total.get(),
            avg_grpc_duration_ms: self.get_avg_grpc_duration(),
            cache_hit_rate: self.calculate_cache_hit_rate(),
            active_connections: self.grpc_active_connections.get() as u32,
            memory_usage_mb: self.memory_usage_bytes.get() / 1024.0 / 1024.0,
            cpu_usage_percent: self.cpu_usage_percent.get(),
            generated_at: chrono::Utc::now(),
        }
    }
    
    async fn get_memory_usage(&self) -> f64 {
        // 플랫폼별 메모리 사용량 측정
        #[cfg(target_os = "linux")]
        {
            if let Ok(contents) = tokio::fs::read_to_string("/proc/self/status").await {
                for line in contents.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Ok(kb) = line.split_whitespace().nth(1).unwrap_or("0").parse::<f64>() {
                            return kb * 1024.0; // KB to bytes
                        }
                    }
                }
            }
        }
        
        0.0 // 측정 실패시 기본값
    }
    
    async fn get_cpu_usage(&self) -> f64 {
        // 간단한 CPU 사용률 측정 (실제로는 더 정확한 측정 필요)
        50.0 // 임시값
    }
    
    async fn get_active_task_count(&self) -> f64 {
        // Tokio 런타임의 활성 태스크 수 (실제 구현 필요)
        100.0 // 임시값
    }
    
    fn get_avg_grpc_duration(&self) -> f64 {
        // Histogram에서 평균 계산
        let sum = self.grpc_request_duration.get_sample_sum();
        let count = self.grpc_request_duration.get_sample_count();
        
        if count > 0 {
            (sum / count as f64) * 1000.0 // seconds to milliseconds
        } else {
            0.0
        }
    }
    
    fn calculate_cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hits_total.get();
        let misses = self.cache_misses_total.get();
        let total = hits + misses;
        
        if total > 0 {
            hits / total
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub total_grpc_requests: f64,
    pub avg_grpc_duration_ms: f64,
    pub cache_hit_rate: f64,
    pub active_connections: u32,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

impl PerformanceReport {
    pub fn to_string(&self) -> String {
        format!(
            "gRPC 서버 성능 보고서 ({})\n\
            ═══════════════════════════════════\n\
            📊 요청 통계:\n\
            • 총 요청 수: {:.0}\n\
            • 평균 응답 시간: {:.2}ms\n\
            • 활성 연결 수: {}\n\
            \n\
            💾 캐시 성능:\n\
            • 캐시 히트율: {:.2}%\n\
            \n\
            🖥️ 시스템 리소스:\n\
            • 메모리 사용량: {:.1}MB\n\
            • CPU 사용률: {:.1}%\n\
            \n\
            📅 생성 시간: {}",
            self.generated_at.format("%Y-%m-%d %H:%M:%S UTC"),
            self.total_grpc_requests,
            self.avg_grpc_duration_ms,
            self.active_connections,
            self.cache_hit_rate * 100.0,
            self.memory_usage_mb,
            self.cpu_usage_percent,
            self.generated_at.format("%Y-%m-%d %H:%M:%S UTC")
        )
    }
}
```

이 성능 최적화 가이드는 gRPC 서버의 모든 계층에서 최적화를 적용하는 방법을 제시합니다. HTTP/2 멀티플렉싱 활용, Protocol Buffers 최적화, 데이터베이스 연결 풀링, 지능형 캐싱, 그리고 포괄적인 모니터링을 통해 높은 성능과 확장성을 달성할 수 있습니다.