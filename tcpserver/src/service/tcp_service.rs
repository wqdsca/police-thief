//! TCP 서버 메인 서비스
//!
//! TCP 서버의 생명주기와 전반적인 관리를 담당합니다.

use anyhow::{anyhow, Context, Result};
use shared::config::redis_config::RedisConfig;
use shared::tool::high_performance::async_task_scheduler::SchedulerConfig;
use shared::tool::high_performance::{
    AlignedBuffer, AsyncTaskScheduler, EnhancedMemoryPool, EnhancedPoolConfig, TaskPriority,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::handler::ChatRoomMessageHandler;
use crate::service::{ConnectionService, HeartbeatService, RoomConnectionService};

/// TCP 서버 설정
#[derive(Debug, Clone)]
pub struct TcpServerConfig {
    pub bind_address: String,
    pub max_connections: u32,
    pub heartbeat_interval_secs: u64,
    pub connection_timeout_secs: u64,
    pub enable_compression: bool,
    pub enable_logging: bool,
    pub enable_enhanced_memory_pool: bool,
    pub memory_pool_config: Option<EnhancedPoolConfig>,
    pub enable_async_scheduler: bool,
    pub scheduler_config: Option<SchedulerConfig>,
}

impl Default for TcpServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:4000".to_string(),
            max_connections: 1000,
            heartbeat_interval_secs: 10,
            connection_timeout_secs: 30,
            enable_compression: false,
            enable_logging: true,
            enable_enhanced_memory_pool: true,
            memory_pool_config: None, // 기본 설정 사용
            enable_async_scheduler: true,
            scheduler_config: None, // 기본 설정 사용
        }
    }
}

/// TCP 게임 서버 서비스
pub struct TcpGameService {
    config: TcpServerConfig,
    connection_service: Arc<ConnectionService>,
    heartbeat_service: Arc<HeartbeatService>,
    #[allow(dead_code)]
    room_connection_service: Arc<RoomConnectionService>,
    chat_room_handler: Arc<ChatRoomMessageHandler>,
    redis_config: Arc<Mutex<Option<RedisConfig>>>,
    is_running: Arc<Mutex<bool>>,
    enhanced_memory_pool: Option<Arc<EnhancedMemoryPool>>,
    async_scheduler: Option<Arc<AsyncTaskScheduler>>,
}

impl TcpGameService {
    /// 새로운 TCP 게임 서비스 생성
    pub fn new(config: TcpServerConfig) -> Self {
        let connection_service = Arc::new(ConnectionService::new(config.max_connections));
        let heartbeat_service = Arc::new(HeartbeatService::new(
            connection_service.clone(),
            config.heartbeat_interval_secs,
            config.connection_timeout_secs,
        ));

        // 새로운 DashMap 기반 방 연결 서비스 생성
        let room_connection_service =
            Arc::new(RoomConnectionService::new("tcp_server".to_string()));
        let chat_room_handler =
            Arc::new(ChatRoomMessageHandler::new(room_connection_service.clone()));

        // 향상된 메모리 풀 초기화
        let enhanced_memory_pool = if config.enable_enhanced_memory_pool {
            let pool_config = config.memory_pool_config.clone().unwrap_or_default();

            info!("🚀 향상된 메모리 풀 활성화됨 - 할당 속도 30% 향상 목표");
            Some(Arc::new(EnhancedMemoryPool::new(pool_config)))
        } else {
            None
        };

        // 비동기 작업 스케줄러 초기화
        let async_scheduler = if config.enable_async_scheduler {
            let scheduler_config =
                config
                    .scheduler_config
                    .clone()
                    .unwrap_or_else(|| SchedulerConfig {
                        worker_count: num_cpus::get().max(4),
                        ..Default::default()
                    });

            info!("⚡ 고성능 비동기 스케줄러 활성화됨 - 작업 처리 속도 40% 향상 목표");
            Some(Arc::new(AsyncTaskScheduler::new(scheduler_config)))
        } else {
            None
        };

        Self {
            config,
            connection_service,
            heartbeat_service,
            room_connection_service,
            chat_room_handler,
            redis_config: Arc::new(Mutex::new(None)),
            is_running: Arc::new(Mutex::new(false)),
            enhanced_memory_pool,
            async_scheduler,
        }
    }

    /// 기본 설정으로 서비스 생성
    pub fn with_default_config() -> Self {
        Self::new(TcpServerConfig::default())
    }

    /// 사용자 정의 설정으로 서비스 생성
    pub fn with_config(config: TcpServerConfig) -> Self {
        Self::new(config)
    }

    /// 서버 시작
    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.lock().await;

        if *is_running {
            warn!("TCP 서버가 이미 실행 중입니다");
            return Ok(());
        }

        info!("🚀 TCP 게임 서버 시작 중... ({})", self.config.bind_address);

        // 바인드 주소 사용
        let bind_addr = &self.config.bind_address;

        // Redis 연결 설정
        if let Ok(redis_config) = RedisConfig::new().await {
            *self.redis_config.lock().await = Some(redis_config);
            info!("✅ Redis 연결 완료");
        } else {
            warn!("⚠️ Redis 연결 실패 - Redis 없이 실행");
        }

        // TCP 리스너 시작
        let listener = TcpListener::bind(bind_addr)
            .await
            .context("TCP 리스너 바인드 실패")?;

        info!("✅ TCP 서버가 {}에서 실행 중입니다", bind_addr);

        // 서버 상태 설정
        *is_running = true;
        drop(is_running);

        // 하트비트 시스템 시작
        self.heartbeat_service
            .start()
            .await
            .context("하트비트 시스템 시작 실패")?;

        // 비동기 스케줄러 시작
        if let Some(scheduler) = &self.async_scheduler {
            scheduler.start().await;
            info!("✅ 고성능 비동기 스케줄러 시작됨");
        }

        // 클라이언트 연결 처리 루프
        while *self.is_running.lock().await {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("새 클라이언트 연결: {}", addr);
                    let chat_handler = self.chat_room_handler.clone();

                    tokio::spawn(async move {
                        if let Err(e) =
                            Self::handle_client_connection(stream, addr.to_string(), chat_handler)
                                .await
                        {
                            error!("클라이언트 처리 오류: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("클라이언트 연결 수락 실패: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }

        Ok(())
    }

    /// 서버 중지
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.lock().await;

        if !*is_running {
            warn!("TCP 서버가 이미 중지되어 있습니다");
            return Ok(());
        }

        info!("🛑 TCP 게임 서버 중지 중...");

        *is_running = false;
        drop(is_running);

        // 하트비트 시스템 중지
        self.heartbeat_service
            .stop()
            .await
            .context("하트비트 시스템 중지 실패")?;

        // 비동기 스케줄러 중지
        if let Some(scheduler) = &self.async_scheduler {
            scheduler.shutdown().await;
            info!("✅ 비동기 스케줄러 중지됨");
        }

        // 모든 연결 종료
        self.connection_service.close_all_connections().await;

        info!("✅ TCP 게임 서버가 성공적으로 중지되었습니다");
        Ok(())
    }

    /// 서버 실행 상태 확인
    pub async fn is_running(&self) -> bool {
        *self.is_running.lock().await
    }

    /// 현재 연결 수 조회
    pub async fn get_connection_count(&self) -> usize {
        self.connection_service.get_connection_count().await
    }

    /// 서버 통계 조회
    pub async fn get_server_stats(&self) -> ServerStats {
        let connection_count = self.connection_service.get_connection_count().await;
        let heartbeat_running = self.heartbeat_service.is_running().await;
        let uptime_secs = self.connection_service.get_uptime_seconds().await;

        let memory_pool_performance = if self.enhanced_memory_pool.is_some() {
            self.get_memory_pool_status().await
        } else {
            None
        };

        let scheduler_performance = if self.async_scheduler.is_some() {
            self.get_scheduler_performance_report().await
        } else {
            None
        };

        ServerStats {
            is_running: self.is_running().await,
            connection_count,
            heartbeat_running,
            uptime_seconds: uptime_secs,
            max_connections: self.config.max_connections,
            bind_address: self.config.bind_address.clone(),
            enhanced_memory_pool_enabled: self.config.enable_enhanced_memory_pool,
            memory_pool_performance,
            async_scheduler_enabled: self.config.enable_async_scheduler,
            scheduler_performance,
        }
    }

    /// Redis 연결 상태 확인
    pub async fn is_redis_connected(&self) -> bool {
        self.redis_config.lock().await.is_some()
    }

    /// 설정 조회
    pub fn get_config(&self) -> &TcpServerConfig {
        &self.config
    }

    /// 새로운 클라이언트 연결 처리 (채팅방 시스템 사용)
    ///
    /// 새로운 클라이언트가 연결되면 스트림을 분리하고 첫 Connect 메시지를 읽은 후
    /// ChatRoomMessageHandler로 전달하여 메시지 루프를 시작합니다.
    ///
    /// # Arguments
    ///
    /// * `stream` - 새로운 TCP 스트림
    /// * `addr` - 클라이언트 주소
    /// * `chat_handler` - 채팅방 메시지 핸들러
    async fn handle_client_connection(
        stream: tokio::net::TcpStream,
        addr: String,
        chat_handler: Arc<ChatRoomMessageHandler>,
    ) -> Result<()> {
        use crate::protocol::GameMessage;
        use tokio::io::BufReader;

        info!("클라이언트 연결 처리 시작: {}", addr);

        // 스트림 분리
        let (reader, writer) = stream.into_split();
        let mut buf_reader = BufReader::new(reader);
        let buf_writer = tokio::io::BufWriter::new(writer);

        // 첫 Connect 메시지 읽기
        let connect_message = match GameMessage::read_from_stream(&mut buf_reader).await {
            Ok(msg) => msg,
            Err(e) => {
                error!("Connect 메시지 읽기 실패 ({}): {}", addr, e);
                return Err(anyhow!("Connect 메시지 읽기 실패: {}", e));
            }
        };

        // Connect 메시지 검증
        if !matches!(connect_message, GameMessage::Connect { .. }) {
            error!(
                "첫 메시지가 Connect가 아님 ({}): {:?}",
                addr, connect_message
            );
            return Err(anyhow!("첫 메시지는 Connect 메시지여야 합니다"));
        }

        info!(
            "Connect 메시지 수신 완료: {} -> {:?}",
            addr, connect_message
        );

        // 채팅방 핸들러로 연결 전달
        chat_handler
            .handle_client_connection(buf_reader, buf_writer, addr.clone(), connect_message)
            .await
            .with_context(|| "채팅방 핸들러 처리 실패")?;

        info!("클라이언트 연결 처리 완료: {}", addr);
        Ok(())
    }

    /// 방 상태 조회
    ///
    /// 특정 방의 현재 상태를 조회합니다.
    ///
    /// # Arguments
    ///
    /// * `room_id` - 조회할 방 ID
    pub fn get_room_status(&self, room_id: u32) -> (u32, Vec<(u32, String)>) {
        self.chat_room_handler.get_room_status(room_id)
    }

    /// 전체 방 목록 조회
    ///
    /// 현재 활성화된 모든 방의 정보를 조회합니다.
    pub fn get_all_rooms_status(&self) -> Vec<(u32, u32)> {
        self.chat_room_handler.get_all_rooms_status()
    }

    /// 빈 방 정리
    ///
    /// 사용자가 없는 방들을 자동으로 정리합니다.
    pub async fn cleanup_empty_rooms(&self) -> usize {
        self.chat_room_handler.cleanup_empty_rooms().await
    }

    /// 향상된 메모리 풀에서 버퍼 할당
    ///
    /// 고성능 메모리 풀이 활성화된 경우 최적화된 버퍼를 반환합니다.
    pub fn allocate_buffer(&self, size: usize) -> Option<AlignedBuffer> {
        self.enhanced_memory_pool
            .as_ref()
            .map(|pool| pool.allocate(size))
    }

    /// 향상된 메모리 풀에 버퍼 반환
    pub fn deallocate_buffer(&self, buffer: AlignedBuffer) {
        if let Some(pool) = &self.enhanced_memory_pool {
            pool.deallocate(buffer);
        }
    }

    /// 메모리 풀 상태 확인
    pub async fn get_memory_pool_status(&self) -> Option<String> {
        self.enhanced_memory_pool
            .as_ref()
            .map(|pool| pool.get_performance_report())
    }

    /// 메모리 풀 정리 실행
    pub async fn cleanup_memory_pool(&self) {
        if let Some(pool) = &self.enhanced_memory_pool {
            pool.cleanup().await;
        }
    }

    /// 비동기 작업 스케줄링
    ///
    /// 우선순위가 있는 비동기 작업을 고성능 스케줄러에 제출합니다.
    pub async fn schedule_async_task<F>(
        &self,
        task: F,
        priority: TaskPriority,
    ) -> Result<(), &'static str>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        if let Some(scheduler) = &self.async_scheduler {
            scheduler.schedule(task, priority).await
        } else {
            // 스케줄러가 비활성화된 경우 직접 실행
            tokio::spawn(task);
            Ok(())
        }
    }

    /// 데드라인이 있는 비동기 작업 스케줄링
    pub async fn schedule_async_task_with_deadline<F>(
        &self,
        task: F,
        priority: TaskPriority,
        deadline: tokio::time::Duration,
    ) -> Result<(), &'static str>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        if let Some(scheduler) = &self.async_scheduler {
            scheduler
                .schedule_with_deadline(task, priority, deadline)
                .await
        } else {
            // 스케줄러가 비활성화된 경우 직접 실행
            tokio::spawn(task);
            Ok(())
        }
    }

    /// 스케줄러 성능 보고서
    pub async fn get_scheduler_performance_report(&self) -> Option<String> {
        if let Some(scheduler) = &self.async_scheduler {
            Some(scheduler.get_performance_report().await)
        } else {
            None
        }
    }

    /// 고우선순위 메시지 처리 (Critical/High 우선순위 사용)
    pub async fn schedule_message_processing<F>(
        &self,
        task: F,
        is_critical: bool,
    ) -> Result<(), &'static str>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let priority = if is_critical {
            TaskPriority::Critical
        } else {
            TaskPriority::High
        };

        self.schedule_async_task(task, priority).await
    }

    /// 백그라운드 정리 작업 스케줄링
    pub async fn schedule_background_cleanup<F>(&self, task: F) -> Result<(), &'static str>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.schedule_async_task(task, TaskPriority::Low).await
    }
}

/// 서버 통계 정보
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerStats {
    pub is_running: bool,
    pub connection_count: usize,
    pub heartbeat_running: bool,
    pub uptime_seconds: u64,
    pub max_connections: u32,
    pub bind_address: String,
    pub enhanced_memory_pool_enabled: bool,
    pub memory_pool_performance: Option<String>,
    pub async_scheduler_enabled: bool,
    pub scheduler_performance: Option<String>,
}

mod tests {

    #[test]
    fn test_tcp_server_config() {
        let config = TcpServerConfig::default();
        assert_eq!(config.bind_address, "127.0.0.1:4000");
        assert_eq!(config.max_connections, 1000);
        assert_eq!(config.heartbeat_interval_secs, 10);
    }

    #[test]
    fn test_custom_config() {
        let config = TcpServerConfig {
            bind_address: "0.0.0.0:9999".to_string(),
            max_connections: 500,
            heartbeat_interval_secs: 5,
            connection_timeout_secs: 15,
            enable_compression: true,
            enable_logging: false,
            enable_enhanced_memory_pool: false,
            memory_pool_config: None,
            enable_async_scheduler: false,
            scheduler_config: None,
        };

        let service = TcpGameService::with_config(config.clone());
        assert_eq!(service.get_config().bind_address, "0.0.0.0:9999");
        assert_eq!(service.get_config().max_connections, 500);
        assert!(!service.get_config().enable_enhanced_memory_pool);
        assert!(!service.get_config().enable_async_scheduler);
    }

    #[tokio::test]
    async fn test_service_lifecycle() {
        let service = TcpGameService::with_default_config();

        // 초기 상태
        assert!(!service.is_running().await);
        assert_eq!(service.get_connection_count().await, 0);

        // 중지 상태에서 중지 시도 (경고만)
        assert!(service.stop().await.is_ok());

        // 통계 조회
        let stats = service.get_server_stats().await;
        assert!(!stats.is_running);
        assert_eq!(stats.connection_count, 0);
        assert!(stats.enhanced_memory_pool_enabled); // 기본값은 활성화
        assert!(stats.async_scheduler_enabled); // 기본값은 활성화
    }

    #[tokio::test]
    async fn test_enhanced_memory_pool() {
        let service = TcpGameService::with_default_config();

        // 메모리 풀 버퍼 할당 테스트
        let buffer = service.allocate_buffer(4096);
        assert!(buffer.is_some());

        if let Some(buffer) = buffer {
            service.deallocate_buffer(buffer);
        }

        // 메모리 풀 상태 확인
        let status = service.get_memory_pool_status().await;
        assert!(status.is_some());

        // 메모리 풀 정리
        service.cleanup_memory_pool().await;
    }

    #[tokio::test]
    async fn test_async_scheduler() {
        let service = TcpGameService::with_default_config();

        // 비동기 작업 스케줄링 테스트
        let result = service
            .schedule_async_task(
                async {
                    tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                },
                TaskPriority::High,
            )
            .await;

        assert!(result.is_ok());

        // 메시지 처리 스케줄링
        let result = service
            .schedule_message_processing(
                async {
                    // 모의 메시지 처리
                },
                true,
            )
            .await;

        assert!(result.is_ok());

        // 백그라운드 정리 작업 스케줄링
        let result = service
            .schedule_background_cleanup(async {
                // 모의 정리 작업
            })
            .await;

        assert!(result.is_ok());

        // 스케줄러 성능 보고서 확인
        let report = service.get_scheduler_performance_report().await;
        assert!(report.is_some());

        // 잠시 대기하여 작업들이 처리되도록
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
}
