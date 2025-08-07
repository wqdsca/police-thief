//! TCP Game Server - 새로운 모듈 구조
//! 
//! service/handler/tool 구조로 리팩토링된 버전

use anyhow::{Context, Result};
use tracing::{info, error};
use std::sync::Arc;

// 모듈 import
use tcpserver::service::{TcpGameService, TcpServerConfig};
use tcpserver::tool::SimpleUtils;

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
    
    info!("🎮 Police Thief TCP 서버 시작");
    info!("시작 시간: {}", SimpleUtils::current_timestamp());
    
    // 서버 설정
    let config = TcpServerConfig {
        bind_address: bind_addr,
        max_connections: 1000,
        heartbeat_interval_secs: 10,
        connection_timeout_secs: 30,
        enable_compression: false,
        enable_logging: true,
    };
    
    // TCP 서버 생성 및 시작
    let service = TcpGameService::with_config(config);
    
    // Ctrl+C 시그널 처리
    let service_handle = tokio::spawn(async move {
        if let Err(e) = service.start().await {
            error!("TCP 서버 실행 오류: {}", e);
        }
    });
    
    // 종료 시그널 대기
    tokio::signal::ctrl_c().await?;
    info!("종료 시그널 수신, 서버를 중지합니다...");
    
    service_handle.abort();
    info!("서버 종료 완료");
    
    Ok(())
}