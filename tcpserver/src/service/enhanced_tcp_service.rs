//! 향상된 TCP 게임 서비스
//! 
//! 모든 최적화를 통합한 고성능 TCP 서버
//! 목표: 20,000+ msg/sec, <0.5ms p99 지연시간

use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Semaphore};
use tracing::{info, error, warn, debug};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, AtomicU32, AtomicBool, Ordering};
use std::time::{Duration, Instant};
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::service::optimized_connection_service::OptimizedConnectionService;
use crate::service::optimized_async_io::{IOScheduler, IOMetrics};
use crate::protocol::GameMessage;
use crate::handler::ChatRoomMessageHandler;
use shared::config::redis_config::RedisConfig;

/// 향상된 TCP 서버 설정
#[derive(Debug, Clone)]
pub struct EnhancedTcpConfig {
    pub bind_address: String,
    pub max_connections: u32,
    pub heartbeat_interval_secs: u64,
    pub connection_timeout_secs: u64,
    
    // 성능 최적화 설정
    pub io_buffer_size: usize,
    pub message_batch_size: usize,
    pub flush_interval_ms: u64,
    pub worker_threads: usize,
    pub connection_pool_size: usize,
    
    // DashMap 샤드 설정
    pub dashmap_shard_count: usize,
    
    // 백프레셔 설정
    pub max_pending_messages: usize,
    pub backpressure_threshold: f64,
    
    // 메트릭 설정
    pub enable_metrics: bool,
    pub metrics_interval_secs: u64,
}

impl Default for EnhancedTcpConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:4000".to_string(),
            max_connections: 2000,
            heartbeat_interval_secs: 10,
            connection_timeout_secs: 30,
            
            // 최적화된 기본값
            io_buffer_size: 65536,        // 64KB
            message_batch_size: 100,      // 100개씩 배치
            flush_interval_ms: 5,         // 5ms마다 플러시
            worker_threads: num_cpus::get(),
            connection_pool_size: 100,
            
            dashmap_shard_count: 32,      // 32개 샤드로 경합 감소
            
            max_pending_messages: 10000,
            backpressure_threshold: 0.8,
            
            enable_metrics: true,
            metrics_interval_secs: 10,
        }
    }
}

/// 성능 통계
#[derive(Debug)]
pub struct PerformanceStats {
    pub messages_processed: AtomicU64,
    pub bytes_transferred: AtomicU64,
    pub active_connections: AtomicU32,
    pub total_connections: AtomicU64,
    pub errors: AtomicU64,
    pub avg_latency_us: AtomicU64,
    pub p99_latency_us: AtomicU64,
    pub throughput_msg_sec: AtomicU64,
    pub cpu_usage_percent: AtomicU32,
    pub memory_usage_mb: AtomicU32,
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            messages_processed: AtomicU64::new(0),
            bytes_transferred: AtomicU64::new(0),
            active_connections: AtomicU32::new(0),
            total_connections: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            avg_latency_us: AtomicU64::new(0),
            p99_latency_us: AtomicU64::new(0),
            throughput_msg_sec: AtomicU64::new(0),
            cpu_usage_percent: AtomicU32::new(0),
            memory_usage_mb: AtomicU32::new(0),
        }
    }
}

/// 향상된 TCP 게임 서비스
pub struct EnhancedTcpGameService {
    config: EnhancedTcpConfig,
    connection_service: Arc<OptimizedConnectionService>,
    
    // 메시지 처리 파이프라인
    message_pipeline: Arc<MessagePipeline>,
    
    // IO 스케줄러
    io_scheduler: Arc<IOScheduler>,
    
    // 성능 통계
    stats: Arc<PerformanceStats>,
    
    // 백프레셔 제어
    backpressure_semaphore: Arc<Semaphore>,
    
    // 서버 상태
    is_running: AtomicBool,
    server_start_time: Instant,
    
    // Redis 설정
    redis_config: Arc<RwLock<Option<RedisConfig>>>,
    
    // 채팅 핸들러
    chat_room_handler: Arc<ChatRoomMessageHandler>,
}

/// 메시지 처리 파이프라인
struct MessagePipeline {
    // 수신 파이프라인
    receive_queue: Arc<DashMap<u32, mpsc::UnboundedSender<BytesMut>>>,
    
    // 송신 파이프라인
    send_queue: Arc<DashMap<u32, mpsc::UnboundedSender<GameMessage>>>,
    
    // 브로드캐스트 채널
    broadcast_tx: broadcast::Sender<(Option<u32>, GameMessage)>,
    
    // 워커 풀
    worker_pool: Arc<rayon::ThreadPool>,
}

impl MessagePipeline {
    fn new(worker_threads: usize) -> Self {
        let (broadcast_tx, _) = broadcast::channel(10000);
        
        let worker_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(worker_threads)
            .thread_name(|i| format!("tcp-worker-{}", i))
            .build()
            .unwrap();
        
        Self {
            receive_queue: Arc::new(DashMap::with_shard_amount(32)),
            send_queue: Arc::new(DashMap::with_shard_amount(32)),
            broadcast_tx,
            worker_pool: Arc::new(worker_pool),
        }
    }
    
    /// 병렬 메시지 처리
    fn process_message(&self, user_id: u32, message: GameMessage) {
        let send_queue = self.send_queue.clone();
        
        self.worker_pool.spawn(move || {
            // 메시지 처리 로직
            debug!("Processing message from user {}: {:?}", user_id, message);
            
            // 응답 생성 및 전송
            if let Some(entry) = send_queue.get(&user_id) {
                let response = GameMessage::Heartbeat;
                let _ = entry.send(response);
            }
        });
    }
}

impl EnhancedTcpGameService {
    /// 새로운 향상된 TCP 서비스 생성
    pub fn new(config: EnhancedTcpConfig) -> Self {
        let connection_service = Arc::new(OptimizedConnectionService::new(config.max_connections));
        
        let message_pipeline = Arc::new(MessagePipeline::new(config.worker_threads));
        
        let io_scheduler = Arc::new(IOScheduler::new(
            config.io_buffer_size,
            config.message_batch_size,
            config.flush_interval_ms,
        ));
        
        let backpressure_semaphore = Arc::new(Semaphore::new(config.max_pending_messages));
        
        // DashMap 기반 룸 서비스
        let room_connection_service = Arc::new(
            crate::service::RoomConnectionService::new("enhanced_tcp".to_string())
        );
        let chat_room_handler = Arc::new(
            ChatRoomMessageHandler::new(room_connection_service)
        );
        
        Self {
            config,
            connection_service,
            message_pipeline,
            io_scheduler,
            stats: Arc::new(PerformanceStats::default()),
            backpressure_semaphore,
            is_running: AtomicBool::new(false),
            server_start_time: Instant::now(),
            redis_config: Arc::new(RwLock::new(None)),
            chat_room_handler,
        }
    }
    
    /// 서버 시작
    pub async fn start(&self) -> Result<()> {
        if self.is_running.swap(true, Ordering::SeqCst) {
            warn!("서버가 이미 실행 중입니다");
            return Ok(());
        }
        
        info!("🚀 향상된 TCP 게임 서버 시작 중... ({})", self.config.bind_address);
        info!("⚙️  설정: {} 워커 스레드, {} 샤드, {}KB 버퍼", 
            self.config.worker_threads,
            self.config.dashmap_shard_count,
            self.config.io_buffer_size / 1024
        );
        
        // Redis 연결
        if let Ok(redis_config) = RedisConfig::new().await {
            *self.redis_config.write() = Some(redis_config);
            info!("✅ Redis 연결 완료");
        } else {
            warn!("⚠️  Redis 연결 실패 - 메모리 모드로 실행");
        }
        
        // TCP 리스너 바인딩
        let listener = TcpListener::bind(&self.config.bind_address)
            .await
            .context("TCP 리스너 바인드 실패")?;
        
        info!("✅ 향상된 TCP 서버가 {}에서 실행 중", self.config.bind_address);
        
        // 메트릭 수집 태스크 시작
        if self.config.enable_metrics {
            self.start_metrics_collector();
        }
        
        // 연결 수락 루프
        while self.is_running.load(Ordering::Relaxed) {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    // 백프레셔 체크
                    let permit = self.backpressure_semaphore.clone().acquire_owned().await?;
                    
                    let connection_service = self.connection_service.clone();
                    let message_pipeline = self.message_pipeline.clone();
                    let io_scheduler = self.io_scheduler.clone();
                    let stats = self.stats.clone();
                    let chat_handler = self.chat_room_handler.clone();
                    
                    // 비동기 연결 처리
                    tokio::spawn(async move {
                        let start = Instant::now();
                        
                        match Self::handle_optimized_connection(
                            stream,
                            addr.to_string(),
                            connection_service,
                            message_pipeline,
                            io_scheduler,
                            stats.clone(),
                            chat_handler,
                        ).await {
                            Ok(user_id) => {
                                let latency = start.elapsed().as_micros() as u64;
                                stats.avg_latency_us.fetch_add(latency, Ordering::Relaxed);
                                debug!("사용자 {} 연결 처리 완료 ({}μs)", user_id, latency);
                            }
                            Err(e) => {
                                stats.errors.fetch_add(1, Ordering::Relaxed);
                                error!("연결 처리 실패: {}", e);
                            }
                        }
                        
                        drop(permit); // 백프레셔 해제
                    });
                    
                    self.stats.total_connections.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    self.stats.errors.fetch_add(1, Ordering::Relaxed);
                    error!("연결 수락 실패: {}", e);
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }
        
        Ok(())
    }
    
    /// 최적화된 연결 처리
    async fn handle_optimized_connection(
        stream: TcpStream,
        addr: String,
        connection_service: Arc<OptimizedConnectionService>,
        message_pipeline: Arc<MessagePipeline>,
        io_scheduler: Arc<IOScheduler>,
        stats: Arc<PerformanceStats>,
        chat_handler: Arc<ChatRoomMessageHandler>,
    ) -> Result<u32> {
        // TCP 설정 최적화
        stream.set_nodelay(true)?;
        
        // 연결 등록
        let user_id = connection_service.handle_new_connection(stream.try_clone()?, addr.clone()).await?;
        
        stats.active_connections.fetch_add(1, Ordering::Relaxed);
        
        // 스트림 분할
        let (reader, writer) = stream.into_split();
        
        // IO 스케줄러 시작
        io_scheduler.start(reader, writer).await?;
        
        info!("사용자 {} 최적화 연결 완료 (주소: {})", user_id, addr);
        
        Ok(user_id)
    }
    
    /// 메트릭 수집기 시작
    fn start_metrics_collector(&self) {
        let stats = self.stats.clone();
        let interval_secs = self.config.metrics_interval_secs;
        let start_time = self.server_start_time;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
            let mut last_messages = 0u64;
            
            loop {
                interval.tick().await;
                
                let current_messages = stats.messages_processed.load(Ordering::Relaxed);
                let messages_delta = current_messages - last_messages;
                let throughput = messages_delta / interval_secs;
                
                stats.throughput_msg_sec.store(throughput, Ordering::Relaxed);
                
                // CPU 및 메모리 사용량 (sysinfo 사용 시)
                // let sys = System::new_all();
                // stats.cpu_usage_percent.store(sys.global_cpu_info().cpu_usage() as u32, Ordering::Relaxed);
                
                let uptime = start_time.elapsed().as_secs();
                
                info!("📊 성능 메트릭 (업타임: {}초)", uptime);
                info!("  • 처리량: {} msg/sec", throughput);
                info!("  • 활성 연결: {}", stats.active_connections.load(Ordering::Relaxed));
                info!("  • 총 메시지: {}", current_messages);
                info!("  • 평균 지연: {}μs", stats.avg_latency_us.load(Ordering::Relaxed));
                info!("  • P99 지연: {}μs", stats.p99_latency_us.load(Ordering::Relaxed));
                info!("  • 에러: {}", stats.errors.load(Ordering::Relaxed));
                
                last_messages = current_messages;
            }
        });
    }
    
    /// 서버 중지
    pub async fn stop(&self) -> Result<()> {
        if !self.is_running.swap(false, Ordering::SeqCst) {
            warn!("서버가 이미 중지되어 있습니다");
            return Ok(());
        }
        
        info!("TCP 서버 중지 중...");
        
        // 연결 정리
        let active = self.stats.active_connections.load(Ordering::Relaxed);
        info!("활성 연결 {} 개 정리 중...", active);
        
        // 통계 출력
        let total_messages = self.stats.messages_processed.load(Ordering::Relaxed);
        let total_bytes = self.stats.bytes_transferred.load(Ordering::Relaxed);
        let uptime = self.server_start_time.elapsed().as_secs();
        
        info!("📈 최종 통계:");
        info!("  • 총 처리 메시지: {}", total_messages);
        info!("  • 총 전송 바이트: {} MB", total_bytes / 1_000_000);
        info!("  • 평균 처리량: {} msg/sec", total_messages / uptime.max(1));
        
        Ok(())
    }
    
    /// 성능 통계 조회
    pub fn get_stats(&self) -> &PerformanceStats {
        &self.stats
    }
}