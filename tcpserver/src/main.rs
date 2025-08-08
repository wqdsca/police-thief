//! TCP 서버 - 4가지 핵심 기능
//! 
//! 1. 방 입장 (Room Entry)
//! 2. 채팅 (Chat)  
//! 3. 친구 추가 (Friend Add)
//! 4. 친구 삭제 (Friend Remove)

use anyhow::{Context, Result};
use tracing::{info, error};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

mod config;
mod protocol;
mod service;
mod handler;
mod tool;

use config::{TcpServerConfig, validate_config};
use service::{ConnectionService, HeartbeatService, MessageService};
use handler::{RoomHandler, FriendHandler, ServerMessageHandler, ConnectionHandler};

/// 간단한 TCP 서버 - 4개 핵심 기능만 제공
pub struct SimpleTcpServer {
    connection_service: Arc<ConnectionService>,
    heartbeat_service: Arc<HeartbeatService>,
    message_service: Arc<MessageService>,
    room_handler: Arc<RoomHandler>,
    friend_handler: Arc<FriendHandler>,
    message_handler: Arc<ServerMessageHandler>,
    connection_handler: Arc<ConnectionHandler>,
    is_running: Arc<Mutex<bool>>,
}

impl SimpleTcpServer {
    /// 새로운 간단한 TCP 서버 생성
    pub fn new() -> Self {
        let connection_service = Arc::new(ConnectionService::new(1000));
        let heartbeat_service = Arc::new(HeartbeatService::with_default_config(connection_service.clone()));
        let message_service = Arc::new(MessageService::new(connection_service.clone()));
        let room_handler = Arc::new(RoomHandler::new(connection_service.clone(), message_service.clone()));
        let friend_handler = Arc::new(FriendHandler::new(connection_service.clone(), message_service.clone()));
        let message_handler = Arc::new(ServerMessageHandler::new(
            connection_service.clone(),
            heartbeat_service.clone(),
            message_service.clone(),
        ));
        let connection_handler = Arc::new(ConnectionHandler::new(
            connection_service.clone(),
            heartbeat_service.clone(),
            message_service.clone(),
        ));
        
        Self {
            connection_service,
            heartbeat_service,
            message_service,
            room_handler,
            friend_handler,
            message_handler,
            connection_handler,
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// 서버 시작
    pub async fn start(&mut self, bind_addr: &str) -> Result<()> {
        info!("🚀 TCP 서버 시작 중... ({})", bind_addr);
        
        // TCP 리스너 시작
        let listener = TcpListener::bind(bind_addr)
            .await
            .context("TCP 리스너 바인드 실패")?;
        
        info!("✅ TCP 서버가 {}에서 실행 중입니다", bind_addr);
        
        // 서버 상태 설정
        *self.is_running.lock().await = true;
        
        // 하트비트 시스템 시작
        self.heartbeat_service.start().await?;
        
        // 메시지 핸들러 등록
        self.message_handler.register_all_handlers().await?;
        
        // 클라이언트 연결 처리 루프
        while *self.is_running.lock().await {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("새 사용자 연결: {}", addr);
                    let connection_handler = self.connection_handler.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = connection_handler.handle_new_connection(stream, addr.to_string()).await {
                            error!("사용자 연결 처리 오류: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("사용자 연결 수락 실패: {}", e);
                }
            }
        }
        
        Ok(())
    }

    /// 서버 중지
    pub async fn stop(&mut self) -> Result<()> {
        info!("🛑 TCP 서버 중지 중...");
        
        *self.is_running.lock().await = false;
        
        // 하트비트 시스템 중지
        self.heartbeat_service.stop().await?;
        
        info!("✅ TCP 서버가 성공적으로 중지되었습니다");
        Ok(())
    }
}

/// TCP 서버 메인 진입점
/// 
/// 환경 설정은 Backend/.env 파일에서 로드됩니다.
/// 
/// 환경변수:
/// - tcp_host: TCP 서버 호스트 (기본값: "127.0.0.1")
/// - tcp_port: TCP 서버 포트 (기본값: "4000")
/// - redis_host: Redis 서버 호스트 (기본값: "127.0.0.1")
/// - redis_port: Redis 서버 포트 (기본값: "6379")
/// - grpc_host: gRPC 서버 호스트 (기본값: "127.0.0.1")
/// - grpc_port: gRPC 서버 포트 (기본값: "50051")
#[tokio::main]
async fn main() -> Result<()> {
    // 로깅 설정
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    // 환경 설정 로드
    let config = TcpServerConfig::from_env()?;
    
    // 설정 검증
    validate_config(&config)?;
    
    info!("=== TCP 서버 설정 ===");
    info!("TCP 서버: {}", config.bind_address());
    info!("Redis 서버: {}", config.redis_address());
    info!("gRPC 서버: {}", config.grpc_address());
    info!("====================");
    
    info!("=== TCP 서버 - 4가지 핵심 기능 ===");
    info!("1. 방 입장 (Room Entry)");
    info!("2. 채팅 (Chat)");  
    info!("3. 친구 추가 (Friend Add)");
    info!("4. 친구 삭제 (Friend Remove)");
    info!("====================================");
    
    // TCP 서버 시작
    let server = SimpleTcpServer::new();
    
    // Ctrl+C 시그널 처리
    let server_ref = Arc::new(Mutex::new(server));
    let server_clone = server_ref.clone();
    
    let bind_addr = config.bind_address();
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server_clone.lock().await.start(&bind_addr).await {
            error!("TCP 서버 실행 오류: {}", e);
        }
    });
    
    // 종료 시그널 대기
    tokio::signal::ctrl_c().await?;
    info!("종료 시그널 수신, 서버를 중지합니다...");
    
    server_handle.abort();
    
    if let Ok(mut server) = server_ref.try_lock() {
        server.stop().await?;
    }
    
    Ok(())
}