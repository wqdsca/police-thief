//! TCP Game Server
//! 
//! Police Thief 게임을 위한 실시간 TCP 서버입니다.
//! 하트비트, 채팅, 게임 상태 동기화를 담당합니다.
//! 
//! # 주요 기능
//! 
//! - **실시간 연결 관리**: 클라이언트 연결 상태 모니터링
//! - **하트비트 시스템**: 자동 연결 상태 확인 및 타임아웃 처리
//! - **메시지 브로드캐스트**: 효율적인 다중 클라이언트 통신
//! - **프로토콜 처리**: 바이너리 기반 고성능 메시지 직렬화
//! 
//! # 아키텍처
//! 
//! ```
//! TcpGameServer
//! ├── ConnectionService (연결 관리)
//! ├── HeartbeatService (하트비트 관리)
//! ├── Protocol (메시지 프로토콜)
//! └── Services (비즈니스 로직)
//! ```
//! 
//! # 사용 예시
//! 
//! ```rust
//! let mut server = TcpGameServer::new();
//! server.start("127.0.0.1:8080").await?;
//! ```

use shared::config::redis_config::RedisConfig;
use anyhow::{Context, Result};
use tracing::{info, error};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::Duration;

mod protocol;
mod service;
mod handler;
mod tool;

use service::{ConnectionService, HeartbeatService};

/// TCP 게임 서버 메인 구조체
/// 
/// 클라이언트 연결, 하트비트 관리, 메시지 처리를 담당하는
/// 실시간 게임 서버의 핵심 구조체입니다.
/// 
/// # 주요 구성 요소
/// 
/// - **ConnectionService**: 클라이언트 연결 상태 관리
/// - **HeartbeatService**: 하트비트 기반 연결 상태 모니터링
/// - **RedisConfig**: Redis 연결 설정 (선택적)
/// - **is_running**: 서버 실행 상태 관리
/// 
/// # 예시
/// 
/// ```rust
/// let mut server = TcpGameServer::new();
/// server.start("127.0.0.1:8080").await?;
/// ```
pub struct TcpGameServer {
    /// 클라이언트 연결 관리자
    /// 
    /// 모든 활성 클라이언트 연결을 관리하고,
    /// 메시지 송수신을 담당합니다.
    connection_service: Arc<ConnectionService>,
    
    /// 하트비트 관리자  
    /// 
    /// 클라이언트 연결 상태를 주기적으로 모니터링하고,
    /// 타임아웃된 연결을 자동으로 정리합니다.
    heartbeat_service: Arc<HeartbeatService>,
    
    /// Redis 설정
    /// 
    /// 게임 상태 저장 및 캐싱을 위한 Redis 연결 설정입니다.
    /// None인 경우 Redis 기능이 비활성화됩니다.
    redis_config: Option<RedisConfig>,
    
    /// 서버 실행 상태
    /// 
    /// 서버의 시작/중지 상태를 관리하는 플래그입니다.
    /// Arc<Mutex<bool>>로 여러 스레드에서 안전하게 접근할 수 있습니다.
    is_running: Arc<Mutex<bool>>,
}

impl TcpGameServer {
    /// 새로운 TCP 게임 서버 인스턴스를 생성합니다.
    /// 
    /// # Returns
    /// 
    /// 초기화된 `TcpGameServer` 인스턴스
    /// 
    /// # 예시
    /// 
    /// ```rust
    /// let server = TcpGameServer::new();
    /// ```
    pub fn new() -> Self {
        let connection_service = Arc::new(ConnectionService::new(1000)); // 최대 1000개 연결
        let heartbeat_service = Arc::new(HeartbeatService::with_default_config(connection_service.clone()));
        
        Self {
            connection_service,
            heartbeat_service,
            redis_config: None,
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// TCP 게임 서버를 시작합니다.
    /// 
    /// 지정된 주소에서 TCP 리스너를 시작하고,
    /// 클라이언트 연결을 수락하며, 하트비트 시스템을 활성화합니다.
    /// 
    /// # Arguments
    /// 
    /// * `bind_addr` - 서버가 바인딩할 주소 (예: "127.0.0.1:8080")
    /// 
    /// # Returns
    /// 
    /// * `Result<()>` - 서버 시작 성공 여부
    /// 
    /// # Errors
    /// 
    /// * Redis 연결 실패 시
    /// * TCP 리스너 바인드 실패 시
    /// * 하트비트 시스템 시작 실패 시
    /// 
    /// # 예시
    /// 
    /// ```rust
    /// let mut server = TcpGameServer::new();
    /// server.start("127.0.0.1:8080").await?;
    /// ```
    pub async fn start(&mut self, bind_addr: &str) -> Result<()> {
        info!("🚀 TCP 게임 서버 시작 중... ({})", bind_addr);
        
        // Redis 연결 설정
        let redis_config = RedisConfig::new()
            .await
            .context("Redis 연결 실패")?;
        self.redis_config = Some(redis_config);
        
        // TCP 리스너 시작
        let listener = TcpListener::bind(bind_addr)
            .await
            .context("TCP 리스너 바인드 실패")?;
        
        info!("✅ TCP 서버가 {}에서 실행 중입니다", bind_addr);
        
        // 서버 상태 설정
        *self.is_running.lock().await = true;
        
        // 하트비트 시스템 시작
        self.heartbeat_service.start().await?;
        
        // 클라이언트 연결 처리 루프
        while *self.is_running.lock().await {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("새 클라이언트 연결: {}", addr);
                    let connection_service = self.connection_service.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(connection_service, stream, addr.to_string()).await {
                            error!("클라이언트 처리 오류: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("클라이언트 연결 수락 실패: {}", e);
                }
            }
        }
        
        Ok(())
    }

    /// 개별 클라이언트 연결을 처리합니다.
    /// 
    /// 새로운 클라이언트 연결을 받아서 연결 관리자에 등록하고,
    /// 메시지 수신 처리를 시작합니다.
    /// 
    /// # Arguments
    /// 
    /// * `connection_service` - 연결 서비스 참조
    /// * `stream` - 클라이언트 TCP 스트림
    /// * `addr` - 클라이언트 주소 문자열
    /// 
    /// # Returns
    /// 
    /// * `Result<()>` - 클라이언트 처리 성공 여부
    /// 
    /// # 예시
    /// 
    /// ```rust
    /// let connection_service = Arc::new(ConnectionService::new(1000));
    /// Self::handle_client(connection_service, stream, "127.0.0.1:12345".to_string()).await?;
    /// ```
    async fn handle_client(
        connection_service: Arc<ConnectionService>,
        stream: TcpStream, 
        addr: String
    ) -> Result<()> {
        info!("클라이언트 처리 시작: {}", addr);
        
        // 연결 등록
        let _client_id = connection_service.handle_new_connection(stream, addr.clone()).await?;
        
        // 연결 해제 시 정리
        tokio::spawn(async move {
            // 실제 메시지 처리는 ConnectionService에서 담당
            tokio::time::sleep(Duration::from_secs(1)).await;
            info!("클라이언트 연결 종료: {}", addr);
        });
        
        Ok(())
    }

    /// TCP 게임 서버를 안전하게 중지합니다.
    /// 
    /// 하트비트 시스템을 중지하고, 모든 클라이언트 연결을 정리하며,
    /// 서버 상태를 안전하게 종료합니다.
    /// 
    /// # Returns
    /// 
    /// * `Result<()>` - 서버 중지 성공 여부
    /// 
    /// # 예시
    /// 
    /// ```rust
    /// let mut server = TcpGameServer::new();
    /// server.start("127.0.0.1:8080").await?;
    /// server.stop().await?;
    /// ```
    pub async fn stop(&mut self) -> Result<()> {
        info!("🛑 TCP 게임 서버 중지 중...");
        
        *self.is_running.lock().await = false;
        
        // 하트비트 시스템 중지
        self.heartbeat_service.stop().await?;
        
        // 모든 연결 종료
        self.connection_service.close_all_connections().await;
        
        info!("✅ TCP 게임 서버가 성공적으로 중지되었습니다");
        Ok(())
    }
}

/// TCP 게임 서버 애플리케이션 진입점
/// 
/// 환경변수를 로드하고, 로깅을 초기화하며,
/// TCP 서버를 시작하고 Ctrl+C 시그널을 처리합니다.
/// 
/// # 환경변수
/// 
/// * `tcp_host` - TCP 서버 호스트 (기본값: "127.0.0.1")
/// * `tcp_port` - TCP 서버 포트 (기본값: "8080")
/// 
/// # 예시
/// 
/// ```bash
/// tcp_host=0.0.0.0 tcp_port=8080 cargo run
/// ```
#[tokio::main]
async fn main() -> Result<()> {
    // 로깅 설정
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    // 환경변수 로드
    dotenv::dotenv().ok();
    
    let tcp_host = std::env::var("tcp_host").unwrap_or_else(|_| "127.0.0.1".to_string());
    let tcp_port = std::env::var("tcp_port").unwrap_or_else(|_| "8080".to_string());
    let bind_addr = format!("{}:{}", tcp_host, tcp_port);
    
    // TCP 서버 시작
    let mut server = TcpGameServer::new();
    
    // Ctrl+C 시그널 처리
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.start(&bind_addr).await {
            error!("TCP 서버 실행 오류: {}", e);
        }
    });
    
    // 종료 시그널 대기
    tokio::signal::ctrl_c().await?;
    info!("종료 시그널 수신, 서버를 중지합니다...");
    
    server_handle.abort();
    Ok(())
}