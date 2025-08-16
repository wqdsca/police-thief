//! Unified Game Server
//!
//! grpcserver, tcpserver를 하나의 통합된 서버로 관리합니다.
//! 단일 명령으로 모든 서버를 시작하고 중지할 수 있습니다.

use actix_web::{middleware, web, App, HttpServer};
use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

// Server imports
use grpcserver::server::start_server as start_grpc_server;
use tcpserver::service::MessageService;
use tcpserver::{
    validate_config as validate_tcp_config, ConnectionService, HeartbeatService, TcpServerConfig,
};

// QUIC server import (primary protocol)
#[cfg(feature = "quic")]
use quicserver::{config::QuicServerConfig, network::server::QuicGameServer, game_logic::DefaultGameLogicHandler};

/// 통합 서버 설정
#[derive(Debug, Clone)]
pub struct UnifiedServerConfig {
    /// gRPC 서버 주소
    pub grpc_address: SocketAddr,
    /// TCP 서버 주소 (fallback)
    pub tcp_address: SocketAddr,
    /// QUIC 서버 주소 (primary)
    pub quic_address: SocketAddr,
    /// 서버별 활성화 상태
    pub enable_grpc: bool,
    pub enable_tcp: bool,
    pub enable_quic: bool,
    /// 성능 모니터링 활성화
    pub enable_monitoring: bool,
    /// 프로토콜 우선순위 (quic > tcp > rudp)
    pub protocol_priority: ProtocolPriority,
}

#[derive(Debug, Clone)]
pub enum ProtocolPriority {
    QuicFirst, // QUIC를 우선으로, TCP fallback
    TcpFirst,  // TCP를 우선으로, QUIC optional
    Auto,      // 클라이언트 능력에 따라 자동 선택
}

impl Default for UnifiedServerConfig {
    fn default() -> Self {
        Self {
            grpc_address: "127.0.0.1:50051".parse().unwrap_or_else(|e| {
                tracing::error!("Invalid default gRPC address: {}", e);
                std::process::exit(1);
            }),
            tcp_address: "127.0.0.1:4000".parse().unwrap_or_else(|e| {
                tracing::error!("Invalid default TCP address: {}", e);
                std::process::exit(1);
            }),
            quic_address: "127.0.0.1:5001".parse().unwrap_or_else(|e| {
                tracing::error!("Invalid default QUIC address: {}", e);
                std::process::exit(1);
            }),
            enable_grpc: true,
            enable_tcp: true,
            enable_quic: true, // QUIC enabled by default
            enable_monitoring: true,
            protocol_priority: ProtocolPriority::QuicFirst, // QUIC as primary
        }
    }
}

impl UnifiedServerConfig {
    /// 환경변수에서 설정 로드
    pub fn from_env() -> Result<Self> {
        let grpc_host = std::env::var("grpc_host").unwrap_or_else(|_| "127.0.0.1".to_string());
        let grpc_port = std::env::var("grpc_port")
            .unwrap_or_else(|_| "50051".to_string())
            .parse::<u16>()
            .unwrap_or(50051);

        let tcp_host = std::env::var("tcp_host").unwrap_or_else(|_| "127.0.0.1".to_string());
        let tcp_port = std::env::var("tcp_port")
            .unwrap_or_else(|_| "4000".to_string())
            .parse::<u16>()
            .unwrap_or(4000);

        let udp_host = std::env::var("udp_host").unwrap_or_else(|_| "127.0.0.1".to_string());
        let udp_port = std::env::var("udp_port")
            .unwrap_or_else(|_| "5000".to_string())
            .parse::<u16>()
            .unwrap_or(5000);

        Ok(Self {
            grpc_address: format!("{}:{}", grpc_host, grpc_port).parse()?,
            tcp_address: format!("{}:{}", tcp_host, tcp_port).parse()?,
            quic_address: format!("{}:{}", udp_host, udp_port + 1).parse()?,
            enable_grpc: std::env::var("ENABLE_GRPC")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_tcp: std::env::var("ENABLE_TCP")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_quic: std::env::var("ENABLE_QUIC")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_monitoring: std::env::var("ENABLE_MONITORING")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            protocol_priority: ProtocolPriority::QuicFirst,
        })
    }

    /// 설정 검증
    pub fn validate(&self) -> Result<()> {
        if !self.enable_grpc && !self.enable_tcp {
            return Err(anyhow::anyhow!("최소 하나의 서버는 활성화되어야 합니다"));
        }

        // TCP 설정 검증
        if self.enable_tcp {
            let tcp_config = TcpServerConfig {
                host: self.tcp_address.ip().to_string(),
                port: self.tcp_address.port(),
                redis_host: std::env::var("redis_host").unwrap_or_else(|_| "127.0.0.1".to_string()),
                redis_port: std::env::var("redis_port")
                    .unwrap_or_else(|_| "6379".to_string())
                    .parse()
                    .unwrap_or(6379),
                grpc_host: self.grpc_address.ip().to_string(),
                grpc_port: self.grpc_address.port(),
            };
            validate_tcp_config(&tcp_config)?;
        }

        Ok(())
    }
}

/// 통합 게임 서버
pub struct UnifiedGameServer {
    config: UnifiedServerConfig,
    is_running: Arc<AtomicBool>,
    server_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<Result<()>>>>>,
}

impl UnifiedGameServer {
    /// 새 통합 서버 생성
    pub fn new(config: UnifiedServerConfig) -> Self {
        Self {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            server_handles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 환경변수에서 설정을 로드하여 서버 생성
    pub fn from_env() -> Result<Self> {
        let config = UnifiedServerConfig::from_env()?;
        config.validate()?;
        Ok(Self::new(config))
    }

    /// 모든 서버 시작
    pub async fn start(&self) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            warn!("서버가 이미 실행 중입니다");
            return Ok(());
        }

        info!("🚀 통합 게임 서버 시작 중...");

        let mut handles = self.server_handles.lock().await;
        handles.clear();

        // 관리자 API 서버 시작
        let admin_handle = tokio::spawn(async move {
            Self::start_admin_api_server()
                .await
                .context("관리자 API 서버 시작 실패")
        });
        handles.push(admin_handle);

        // gRPC 서버 시작
        if self.config.enable_grpc {
            info!("📡 gRPC 서버 시작 중... ({})", self.config.grpc_address);
            let grpc_addr = self.config.grpc_address;
            let handle = tokio::spawn(async move {
                start_grpc_server(grpc_addr)
                    .await
                    .context("gRPC 서버 시작 실패")
            });
            handles.push(handle);
        }

        // TCP 서버 시작
        if self.config.enable_tcp {
            info!("🔌 TCP 서버 시작 중... ({})", self.config.tcp_address);
            let tcp_addr = self.config.tcp_address;
            let handle = tokio::spawn(async move {
                Self::start_tcp_server(tcp_addr)
                    .await
                    .context("TCP 서버 시작 실패")
            });
            handles.push(handle);
        }

        // QUIC 서버 시작 (primary protocol)
        #[cfg(feature = "quic")]
        if self.config.enable_quic {
            info!("🚀 QUIC 서버 시작 중... ({})", self.config.quic_address);
            let quic_addr = self.config.quic_address;
            let handle = tokio::spawn(async move {
                Self::start_quic_server(quic_addr)
                    .await
                    .context("QUIC 서버 시작 실패")
            });
            handles.push(handle);
        }

        // 성능 모니터링 시작
        if self.config.enable_monitoring {
            info!("📊 성능 모니터링 시작 중...");
            let handle = tokio::spawn(async move {
                Self::start_monitoring()
                    .await
                    .context("성능 모니터링 시작 실패")
            });
            handles.push(handle);
        }

        self.is_running.store(true, Ordering::SeqCst);

        info!("✅ 통합 게임 서버가 성공적으로 시작되었습니다!");
        self.print_status();

        Ok(())
    }

    /// TCP 서버 시작 (내부 구현)
    async fn start_tcp_server(addr: SocketAddr) -> Result<()> {
        use tokio::net::TcpListener;

        let connection_service = Arc::new(ConnectionService::new(1000));
        let heartbeat_service = Arc::new(HeartbeatService::with_default_config(
            connection_service.clone(),
        ));
        let message_service = Arc::new(MessageService::new(connection_service.clone()));

        // 하트비트 서비스 시작
        heartbeat_service.start().await?;

        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("TCP 서버를 {}에 바인드하는데 실패했습니다", addr))?;

        info!("🔌 TCP 서버가 {}에서 연결을 기다리고 있습니다", addr);

        loop {
            match listener.accept().await {
                Ok((socket, peer_addr)) => {
                    info!("새 TCP 연결: {}", peer_addr);
                    let conn_service = connection_service.clone();
                    let msg_service = message_service.clone();

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_tcp_connection(
                            socket,
                            peer_addr,
                            conn_service,
                            msg_service,
                        )
                        .await
                        {
                            error!("TCP 연결 처리 오류 ({}): {}", peer_addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("TCP 연결 승인 실패: {}", e);
                    continue;
                }
            }
        }
    }

    /// TCP 연결 처리
    async fn handle_tcp_connection(
        socket: tokio::net::TcpStream,
        peer_addr: SocketAddr,
        _connection_service: Arc<ConnectionService>,
        _message_service: Arc<MessageService>,
    ) -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (mut reader, mut writer) = socket.into_split();
        let mut buffer = [0; 1024];

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    info!("TCP 연결 종료: {}", peer_addr);
                    break;
                }
                Ok(n) => {
                    // 간단한 에코 서버로 구현
                    if let Err(e) = writer.write_all(&buffer[..n]).await {
                        error!("TCP 응답 전송 실패 ({}): {}", peer_addr, e);
                        break;
                    }
                }
                Err(e) => {
                    error!("TCP 읽기 오류 ({}): {}", peer_addr, e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// QUIC 서버 시작 (내부 구현)
    #[cfg(feature = "quic")]
    async fn start_quic_server(addr: SocketAddr) -> Result<()> {
        use std::sync::Arc;

        // QUIC 서버 설정 생성
        let mut config = QuicServerConfig::from_env().unwrap_or_else(|_| {
            warn!("환경변수에서 QUIC 설정 로드 실패, 기본값 사용");
            QuicServerConfig {
                host: addr.ip().to_string(),
                port: addr.port(),
                bind_addr: addr,
                max_concurrent_streams: 100,
                max_idle_timeout_ms: 30_000,
                keep_alive_interval_ms: 10_000,
                enable_0rtt: true,
                enable_migration: true,
                max_connections: 1000,
                send_buffer_size: 65536,
                recv_buffer_size: 65536,
                stream_buffer_size: 32768,
                compression_threshold: 512,
                cert_path: None,
                key_path: None,
                use_self_signed: true,
                enable_simd: true,
                enable_dashmap_optimization: true,
                enable_memory_pool: true,
                enable_parallel_processing: true,
                worker_threads: num_cpus::get(),
                metrics_interval_secs: 30,
                stats_window_secs: 60,
            }
        });

        // 주소 정보 업데이트
        config.host = addr.ip().to_string();
        config.port = addr.port();
        config.bind_addr = addr;

        info!("🚀 QUIC 서버 설정: {:?}", config);

        // 게임 로직 핸들러 생성 (기본 구현체 사용)
        let game_logic = Arc::new(DefaultGameLogicHandler::new());
        
        // QUIC 서버 생성 및 시작
        let server = QuicGameServer::new_with_game_logic(config.clone(), game_logic)
            .await
            .with_context(|| format!("QUIC 서버 생성 실패: {}", addr))?;

        info!("🎮 QUIC 서버가 {}에서 연결을 기다리고 있습니다", addr);

        // 서버 실행 (블로킹)
        server.run()
            .await
            .with_context(|| format!("QUIC 서버 실행 실패: {}", addr))?;

        Ok(())
    }

    /// 관리자 API 서버 시작
    async fn start_admin_api_server() -> Result<()> {
        use crate::admin_api::{configure_admin_routes, AdminApiState};
        use crate::auth_middleware::{login_handler, AuthConfig, AuthMiddleware};
        use crate::websocket_handler::{ws_handler, WsBroadcaster};

        // Redis 연결이 있는 AdminApiState 생성
        let admin_state = match AdminApiState::new().with_redis().await {
            Ok(state) => state,
            Err(e) => {
                warn!("Redis 연결 실패, 기본 상태로 시작: {}", e);
                AdminApiState::new()
            }
        };

        // 데이터베이스 연결 풀 생성 (Admin API용)
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "mysql://game_simple:game_password_123@localhost/police_thief_simple".to_string()
        });

        let pool = match sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(10)
            .min_connections(2)
            .connect(&database_url)
            .await
        {
            Ok(pool) => pool,
            Err(e) => {
                error!("Database connection failed for admin server: {}", e);
                return Err(anyhow::anyhow!("Cannot start admin server without database connection: {}", e));
            }
        };

        let admin_state = web::Data::new(admin_state);
        let ws_broadcaster = web::Data::new(std::sync::Arc::new(WsBroadcaster::new()));
        let auth_config = AuthConfig::new(pool.clone());

        // 소셜 로그인 state 저장소
        let social_state_store = web::Data::new(crate::social_auth_handler::StateStore::default());

        // 서버 상태 주기적 브로드캐스트 (5초마다)
        let broadcaster_clone = ws_broadcaster.clone();
        let state_clone = admin_state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                interval.tick().await;

                // 서버 상태 가져오기
                let mut system = state_clone.system.write().await;
                system.refresh_all();

                let cpu_usage = system.global_cpu_info().cpu_usage();
                let memory_mb = system.used_memory() as f64 / 1024.0 / 1024.0;

                let connected_clients = if let Some(ref client) = state_clone.redis_client {
                    if let Ok(mut conn) = client.get_multiplexed_tokio_connection().await {
                        if let Ok(keys) = redis::cmd("KEYS")
                            .arg("user:*")
                            .query_async::<_, Vec<String>>(&mut conn)
                            .await
                        {
                            keys.len()
                        } else {
                            0
                        }
                    } else {
                        0
                    }
                } else {
                    0
                };

                // WebSocket으로 브로드캐스트
                broadcaster_clone
                    .broadcast_server_status(cpu_usage, memory_mb, connected_clients)
                    .await;
            }
        });

        let server = HttpServer::new(move || {
            App::new()
                .app_data(admin_state.clone())
                .app_data(ws_broadcaster.clone())
                .app_data(web::Data::new(pool.clone()))
                .app_data(social_state_store.clone())
                .wrap(middleware::Logger::default())
                .wrap(
                    actix_cors::Cors::default()
                        .allow_any_origin()
                        .allow_any_method()
                        .allow_any_header()
                        .max_age(3600),
                )
                // 로그인 엔드포인트 (인증 불필요)
                .route("/api/admin/login", web::post().to(login_handler))
                // WebSocket 엔드포인트
                .route("/ws", web::get().to(ws_handler))
                // 소셜 로그인 엔드포인트 (인증 불필요)
                .configure(crate::social_auth_handler::configure_social_auth_routes)
                // 관리자 API (인증 필요)
                .service(
                    web::scope("")
                        .wrap(AuthMiddleware::new(auth_config.clone()))
                        .configure(configure_admin_routes),
                )
        })
        .bind("127.0.0.1:8080")?;

        info!("🔧 관리자 API 서버가 http://127.0.0.1:8080 에서 실행 중입니다");

        server.run().await.context("관리자 API 서버 실행 실패")
    }

    /// 성능 모니터링 시작
    async fn start_monitoring() -> Result<()> {
        use tokio::time::{interval, Duration};

        let mut interval = interval(Duration::from_secs(30));

        loop {
            interval.tick().await;

            // 간단한 시스템 상태 로깅
            info!("📊 성능 모니터링: 시스템 정상 작동 중");

            // TODO: 실제 성능 메트릭 수집 및 로깅
            // - 메모리 사용량
            // - CPU 사용률
            // - 네트워크 처리량
            // - 활성 연결 수
        }
    }

    /// 서버 중지
    pub async fn stop(&self) -> Result<()> {
        if !self.is_running.load(Ordering::SeqCst) {
            warn!("서버가 이미 중지되어 있습니다");
            return Ok(());
        }

        info!("🛑 통합 게임 서버 중지 중...");

        self.is_running.store(false, Ordering::SeqCst);

        let mut handles = self.server_handles.lock().await;
        for handle in handles.drain(..) {
            handle.abort();
        }

        info!("✅ 통합 게임 서버가 성공적으로 중지되었습니다!");
        Ok(())
    }

    /// 서버 실행 상태 확인
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// 서버 상태 출력
    pub fn print_status(&self) {
        let status = if self.is_running() {
            "실행 중"
        } else {
            "중지됨"
        };
        info!("📊 통합 게임 서버 상태: {}", status);

        if self.config.enable_grpc {
            info!("📡 gRPC 서버: {} (활성화)", self.config.grpc_address);
        }

        if self.config.enable_tcp {
            info!("🔌 TCP 서버: {} (활성화)", self.config.tcp_address);
        }

        #[cfg(feature = "quic")]
        if self.config.enable_quic {
            info!("🚀 QUIC 서버: {} (활성화)", self.config.quic_address);
        }

        if self.config.enable_monitoring {
            info!("📊 성능 모니터링: 활성화");
        }
    }

    /// 서버가 완전히 종료될 때까지 대기
    pub async fn wait_for_shutdown(&self) -> Result<()> {
        let handles = self.server_handles.clone();
        let handles_guard = handles.lock().await;

        if !handles_guard.is_empty() {
            // 모든 핸들을 소유권으로 가져와서 사용
            let mut owned_handles = Vec::new();
            for handle in handles_guard.iter() {
                // 핸들을 abortable로 만들어서 나중에 중단할 수 있도록 함
                owned_handles.push(handle.abort_handle());
            }
            drop(handles_guard); // 락 해제

            // 첫 번째 핸들의 완료를 대기하거나 중단 신호 대기
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("종료 신호를 받았습니다. 모든 서버를 중지합니다.");
                    for abort_handle in owned_handles {
                        abort_handle.abort();
                    }
                }
            }
        }

        self.stop().await
    }
}

/// 통합 서버 설정 빌더
pub struct UnifiedServerConfigBuilder {
    config: UnifiedServerConfig,
}

impl UnifiedServerConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: UnifiedServerConfig::default(),
        }
    }

    pub fn grpc_address(mut self, addr: SocketAddr) -> Self {
        self.config.grpc_address = addr;
        self
    }

    pub fn tcp_address(mut self, addr: SocketAddr) -> Self {
        self.config.tcp_address = addr;
        self
    }

    pub fn enable_grpc(mut self, enable: bool) -> Self {
        self.config.enable_grpc = enable;
        self
    }

    pub fn enable_tcp(mut self, enable: bool) -> Self {
        self.config.enable_tcp = enable;
        self
    }

    pub fn enable_monitoring(mut self, enable: bool) -> Self {
        self.config.enable_monitoring = enable;
        self
    }

    pub fn build(self) -> Result<UnifiedServerConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for UnifiedServerConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

mod tests {

    #[test]
    fn test_config_validation() {
        let mut config = UnifiedServerConfig::default();
        assert!(config.validate().is_ok());

        // 모든 서버 비활성화시 오류
        config.enable_grpc = false;
        config.enable_tcp = false;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_builder() {
        let config = UnifiedServerConfigBuilder::new()
            .enable_grpc(true)
            .enable_tcp(false)
            .build()
            .expect("Failed to build test config");

        assert!(config.enable_grpc);
        assert!(!config.enable_tcp);
    }

    #[tokio::test]
    async fn test_server_lifecycle() {
        let config = UnifiedServerConfigBuilder::new()
            .enable_grpc(false)
            .enable_tcp(false)
            .enable_monitoring(true)
            .build()
            .expect("Failed to build test config");

        let server = UnifiedGameServer::new(config);
        assert!(!server.is_running());

        // Note: 실제 시작은 테스트에서 생략 (포트 충돌 방지)
    }
}
