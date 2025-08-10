//! Unified Game Server
//! 
//! grpcserver, tcpserver, rudpserver를 하나의 통합된 서버로 관리합니다.
//! 단일 명령으로 모든 서버를 시작하고 중지할 수 있습니다.

use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

// Server imports
use grpcserver::server::start_server as start_grpc_server;
use tcpserver::{ConnectionService, HeartbeatService, TcpServerConfig, validate_config as validate_tcp_config};
use tcpserver::service::MessageService;
// use rudpserver::config::RudpServerConfig; // Currently unused

/// 통합 서버 설정
#[derive(Debug, Clone)]
pub struct UnifiedServerConfig {
    /// gRPC 서버 주소
    pub grpc_address: SocketAddr,
    /// TCP 서버 주소  
    pub tcp_address: SocketAddr,
    /// RUDP 서버 주소
    pub rudp_address: SocketAddr,
    /// 서버별 활성화 상태
    pub enable_grpc: bool,
    pub enable_tcp: bool,
    pub enable_rudp: bool,
    /// 성능 모니터링 활성화
    pub enable_monitoring: bool,
}

impl Default for UnifiedServerConfig {
    fn default() -> Self {
        Self {
            grpc_address: "127.0.0.1:50051".parse().unwrap(),
            tcp_address: "127.0.0.1:4000".parse().unwrap(),
            rudp_address: "127.0.0.1:5000".parse().unwrap(),
            enable_grpc: true,
            enable_tcp: true,
            enable_rudp: true,
            enable_monitoring: true,
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
            rudp_address: format!("{}:{}", udp_host, udp_port).parse()?,
            enable_grpc: std::env::var("ENABLE_GRPC").unwrap_or_else(|_| "true".to_string()).parse().unwrap_or(true),
            enable_tcp: std::env::var("ENABLE_TCP").unwrap_or_else(|_| "true".to_string()).parse().unwrap_or(true),
            enable_rudp: std::env::var("ENABLE_RUDP").unwrap_or_else(|_| "true".to_string()).parse().unwrap_or(true),
            enable_monitoring: std::env::var("ENABLE_MONITORING").unwrap_or_else(|_| "true".to_string()).parse().unwrap_or(true),
        })
    }

    /// 설정 검증
    pub fn validate(&self) -> Result<()> {
        if !self.enable_grpc && !self.enable_tcp && !self.enable_rudp {
            return Err(anyhow::anyhow!("최소 하나의 서버는 활성화되어야 합니다"));
        }

        // TCP 설정 검증
        if self.enable_tcp {
            let tcp_config = TcpServerConfig {
                host: self.tcp_address.ip().to_string(),
                port: self.tcp_address.port(),
                redis_host: std::env::var("redis_host").unwrap_or_else(|_| "127.0.0.1".to_string()),
                redis_port: std::env::var("redis_port").unwrap_or_else(|_| "6379".to_string()).parse().unwrap_or(6379),
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

        // gRPC 서버 시작
        if self.config.enable_grpc {
            info!("📡 gRPC 서버 시작 중... ({})", self.config.grpc_address);
            let grpc_addr = self.config.grpc_address;
            let handle = tokio::spawn(async move {
                start_grpc_server(grpc_addr).await.context("gRPC 서버 시작 실패")
            });
            handles.push(handle);
        }

        // TCP 서버 시작
        if self.config.enable_tcp {
            info!("🔌 TCP 서버 시작 중... ({})", self.config.tcp_address);
            let tcp_addr = self.config.tcp_address;
            let handle = tokio::spawn(async move {
                Self::start_tcp_server(tcp_addr).await.context("TCP 서버 시작 실패")
            });
            handles.push(handle);
        }

        // RUDP 서버 시작
        if self.config.enable_rudp {
            info!("📶 RUDP 서버 시작 중... ({})", self.config.rudp_address);
            let rudp_addr = self.config.rudp_address;
            let handle = tokio::spawn(async move {
                Self::start_rudp_server(rudp_addr).await.context("RUDP 서버 시작 실패")
            });
            handles.push(handle);
        }

        // 성능 모니터링 시작
        if self.config.enable_monitoring {
            info!("📊 성능 모니터링 시작 중...");
            let handle = tokio::spawn(async move {
                Self::start_monitoring().await.context("성능 모니터링 시작 실패")
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
        let heartbeat_service = Arc::new(HeartbeatService::with_default_config(connection_service.clone()));
        let message_service = Arc::new(MessageService::new(connection_service.clone()));

        // 하트비트 서비스 시작
        heartbeat_service.start().await?;

        let listener = TcpListener::bind(addr).await
            .with_context(|| format!("TCP 서버를 {}에 바인드하는데 실패했습니다", addr))?;

        info!("🔌 TCP 서버가 {}에서 연결을 기다리고 있습니다", addr);

        loop {
            match listener.accept().await {
                Ok((socket, peer_addr)) => {
                    info!("새 TCP 연결: {}", peer_addr);
                    let conn_service = connection_service.clone();
                    let msg_service = message_service.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_tcp_connection(socket, peer_addr, conn_service, msg_service).await {
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

    /// RUDP 서버 시작 (내부 구현)
    async fn start_rudp_server(addr: SocketAddr) -> Result<()> {
        use tokio::net::UdpSocket;
        
        let socket = UdpSocket::bind(addr).await
            .with_context(|| format!("RUDP 서버를 {}에 바인드하는데 실패했습니다", addr))?;

        info!("📶 RUDP 서버가 {}에서 패킷을 기다리고 있습니다", addr);

        let mut buffer = [0; 65536];
        
        loop {
            match socket.recv_from(&mut buffer).await {
                Ok((size, peer_addr)) => {
                    // 간단한 에코 서버로 구현
                    if let Err(e) = socket.send_to(&buffer[..size], peer_addr).await {
                        error!("RUDP 응답 전송 실패 ({}): {}", peer_addr, e);
                    }
                }
                Err(e) => {
                    error!("RUDP 수신 오류: {}", e);
                    continue;
                }
            }
        }
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
        let status = if self.is_running() { "실행 중" } else { "중지됨" };
        info!("📊 통합 게임 서버 상태: {}", status);
        
        if self.config.enable_grpc {
            info!("📡 gRPC 서버: {} (활성화)", self.config.grpc_address);
        }
        
        if self.config.enable_tcp {
            info!("🔌 TCP 서버: {} (활성화)", self.config.tcp_address);
        }
        
        if self.config.enable_rudp {
            info!("📶 RUDP 서버: {} (활성화)", self.config.rudp_address);
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

    pub fn rudp_address(mut self, addr: SocketAddr) -> Self {
        self.config.rudp_address = addr;
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

    pub fn enable_rudp(mut self, enable: bool) -> Self {
        self.config.enable_rudp = enable;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = UnifiedServerConfig::default();
        assert!(config.validate().is_ok());

        // 모든 서버 비활성화시 오류
        config.enable_grpc = false;
        config.enable_tcp = false;
        config.enable_rudp = false;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_builder() {
        let config = UnifiedServerConfigBuilder::new()
            .enable_grpc(true)
            .enable_tcp(false)
            .enable_rudp(true)
            .build()
            .unwrap();

        assert!(config.enable_grpc);
        assert!(!config.enable_tcp);
        assert!(config.enable_rudp);
    }

    #[tokio::test]
    async fn test_server_lifecycle() {
        let config = UnifiedServerConfigBuilder::new()
            .enable_grpc(false)
            .enable_tcp(false)
            .enable_rudp(false)
            .enable_monitoring(true)
            .build()
            .unwrap();

        let server = UnifiedGameServer::new(config);
        assert!(!server.is_running());

        // Note: 실제 시작은 테스트에서 생략 (포트 충돌 방지)
    }
}