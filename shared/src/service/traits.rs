//! 공통 서비스 트레이트 정의
//!
//! 서비스 중복을 제거하고 통일된 인터페이스를 제공합니다.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// 서비스 상태
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceStatus {
    /// 중지됨
    Stopped,
    /// 시작 중
    Starting,
    /// 실행 중
    Running,
    /// 중지 중
    Stopping,
    /// 오류 상태
    Error(String),
}

impl Default for ServiceStatus {
    fn default() -> Self {
        Self::Stopped
    }
}

/// 서비스 메트릭스
#[derive(Debug, Default)]
pub struct ServiceMetrics {
    pub requests_processed: AtomicU64,
    pub requests_failed: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub active_connections: AtomicU64,
    pub uptime_seconds: AtomicU64,
}

impl ServiceMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn increment_processed(&self) {
        self.requests_processed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_failed(&self) {
        self.requests_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_bytes_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_bytes_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn set_active_connections(&self, count: u64) {
        self.active_connections.store(count, Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> (u64, u64, u64, u64, u64, u64) {
        (
            self.requests_processed.load(Ordering::Relaxed),
            self.requests_failed.load(Ordering::Relaxed),
            self.bytes_sent.load(Ordering::Relaxed),
            self.bytes_received.load(Ordering::Relaxed),
            self.active_connections.load(Ordering::Relaxed),
            self.uptime_seconds.load(Ordering::Relaxed),
        )
    }
}

/// 기본 서비스 트레이트
#[async_trait]
pub trait GameService: Send + Sync {
    /// 서비스 이름
    fn name(&self) -> &'static str;

    /// 서비스 시작2
    async fn start(&self) -> Result<()>;

    /// 서비스 중지
    async fn stop(&self) -> Result<()>;

    /// 서비스 상태 조회
    fn status(&self) -> ServiceStatus;

    /// 서비스 메트릭스 조회
    fn metrics(&self) -> &ServiceMetrics;

    /// 헬스체크
    async fn health_check(&self) -> Result<bool> {
        Ok(self.status() == ServiceStatus::Running)
    }
}

/// 연결 기반 서비스 트레이트
#[async_trait]
pub trait ConnectionService: GameService {
    /// 바인드 주소
    fn bind_address(&self) -> SocketAddr;

    /// 최대 연결 수
    fn max_connections(&self) -> usize;

    /// 현재 연결 수
    fn current_connections(&self) -> usize;

    /// 연결 처리
    async fn handle_connection(&self, connection: Box<dyn std::any::Any + Send>) -> Result<()>;
}

/// 메시지 기반 서비스 트레이트  
#[async_trait]
pub trait MessageService: GameService {
    type Message: Send + Sync;
    type Response: Send + Sync;

    /// 메시지 처리
    async fn process_message(&self, message: Self::Message) -> Result<Self::Response>;

    /// 브로드캐스트 메시지 처리
    async fn broadcast_message(&self, message: Self::Message) -> Result<usize>;
}

/// 서비스 팩토리
pub struct ServiceFactory;

impl ServiceFactory {
    /// TCP 서비스 생성
    pub fn create_tcp_service(
        bind_addr: SocketAddr,
        max_connections: usize,
    ) -> Arc<dyn ConnectionService> {
        Arc::new(UnifiedTcpService::new(bind_addr, max_connections))
    }

    /// gRPC 서비스 생성  
    pub fn create_grpc_service(bind_addr: SocketAddr) -> Arc<dyn GameService> {
        Arc::new(UnifiedGrpcService::new(bind_addr))
    }

    /// RUDP 서비스 생성
    pub fn create_rudp_service(bind_addr: SocketAddr) -> Arc<dyn ConnectionService> {
        Arc::new(UnifiedRudpService::new(bind_addr))
    }
}

/// 통합 TCP 서비스 구현
pub struct UnifiedTcpService {
    bind_addr: SocketAddr,
    max_connections: usize,
    metrics: ServiceMetrics,
    status: Arc<tokio::sync::RwLock<ServiceStatus>>,
    shutdown_signal: Arc<AtomicBool>,
}

impl UnifiedTcpService {
    pub fn new(bind_addr: SocketAddr, max_connections: usize) -> Self {
        Self {
            bind_addr,
            max_connections,
            metrics: ServiceMetrics::new(),
            status: Arc::new(tokio::sync::RwLock::new(ServiceStatus::Stopped)),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl GameService for UnifiedTcpService {
    fn name(&self) -> &'static str {
        "UnifiedTcpService"
    }

    async fn start(&self) -> Result<()> {
        let mut status = self.status.write().await;
        *status = ServiceStatus::Starting;

        // 실제 TCP 서버 시작 로직은 여기에 구현
        tracing::info!("Starting TCP service on {}", self.bind_addr);

        *status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut status = self.status.write().await;
        *status = ServiceStatus::Stopping;

        self.shutdown_signal.store(true, Ordering::Relaxed);
        tracing::info!("Stopping TCP service");

        *status = ServiceStatus::Stopped;
        Ok(())
    }

    fn status(&self) -> ServiceStatus {
        // 동기 접근을 위해 try_read 사용
        match self.status.try_read() {
            Ok(status) => status.clone(),
            Err(_) => ServiceStatus::Error("Status lock contention".to_string()),
        }
    }

    fn metrics(&self) -> &ServiceMetrics {
        &self.metrics
    }
}

#[async_trait]
impl ConnectionService for UnifiedTcpService {
    fn bind_address(&self) -> SocketAddr {
        self.bind_addr
    }

    fn max_connections(&self) -> usize {
        self.max_connections
    }

    fn current_connections(&self) -> usize {
        self.metrics.active_connections.load(Ordering::Relaxed) as usize
    }

    async fn handle_connection(&self, _connection: Box<dyn std::any::Any + Send>) -> Result<()> {
        // TCP 연결 처리 로직
        self.metrics.increment_processed();
        Ok(())
    }
}

/// 통합 gRPC 서비스 구현
pub struct UnifiedGrpcService {
    bind_addr: SocketAddr,
    metrics: ServiceMetrics,
    status: Arc<tokio::sync::RwLock<ServiceStatus>>,
}

impl UnifiedGrpcService {
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self {
            bind_addr,
            metrics: ServiceMetrics::new(),
            status: Arc::new(tokio::sync::RwLock::new(ServiceStatus::Stopped)),
        }
    }
}

#[async_trait]
impl GameService for UnifiedGrpcService {
    fn name(&self) -> &'static str {
        "UnifiedGrpcService"
    }

    async fn start(&self) -> Result<()> {
        let mut status = self.status.write().await;
        *status = ServiceStatus::Starting;

        tracing::info!("Starting gRPC service on {}", self.bind_addr);

        *status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut status = self.status.write().await;
        *status = ServiceStatus::Stopping;

        tracing::info!("Stopping gRPC service");

        *status = ServiceStatus::Stopped;
        Ok(())
    }

    fn status(&self) -> ServiceStatus {
        match self.status.try_read() {
            Ok(status) => status.clone(),
            Err(_) => ServiceStatus::Error("Status lock contention".to_string()),
        }
    }

    fn metrics(&self) -> &ServiceMetrics {
        &self.metrics
    }
}

/// 통합 RUDP 서비스 구현
pub struct UnifiedRudpService {
    bind_addr: SocketAddr,
    max_connections: usize,
    metrics: ServiceMetrics,
    status: Arc<tokio::sync::RwLock<ServiceStatus>>,
}

impl UnifiedRudpService {
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self {
            bind_addr,
            max_connections: 1000,
            metrics: ServiceMetrics::new(),
            status: Arc::new(tokio::sync::RwLock::new(ServiceStatus::Stopped)),
        }
    }
}

#[async_trait]
impl GameService for UnifiedRudpService {
    fn name(&self) -> &'static str {
        "UnifiedRudpService"
    }

    async fn start(&self) -> Result<()> {
        let mut status = self.status.write().await;
        *status = ServiceStatus::Starting;

        tracing::info!("Starting RUDP service on {}", self.bind_addr);

        *status = ServiceStatus::Running;
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let mut status = self.status.write().await;
        *status = ServiceStatus::Stopping;

        tracing::info!("Stopping RUDP service");

        *status = ServiceStatus::Stopped;
        Ok(())
    }

    fn status(&self) -> ServiceStatus {
        match self.status.try_read() {
            Ok(status) => status.clone(),
            Err(_) => ServiceStatus::Error("Status lock contention".to_string()),
        }
    }

    fn metrics(&self) -> &ServiceMetrics {
        &self.metrics
    }
}

#[async_trait]
impl ConnectionService for UnifiedRudpService {
    fn bind_address(&self) -> SocketAddr {
        self.bind_addr
    }

    fn max_connections(&self) -> usize {
        self.max_connections
    }

    fn current_connections(&self) -> usize {
        self.metrics.active_connections.load(Ordering::Relaxed) as usize
    }

    async fn handle_connection(&self, _connection: Box<dyn std::any::Any + Send>) -> Result<()> {
        // RUDP 연결 처리 로직
        self.metrics.increment_processed();
        Ok(())
    }
}
