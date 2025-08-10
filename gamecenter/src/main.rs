//! 통합 게임센터 서버
//! 
//! grpcserver, tcpserver, rudpserver를 하나의 통합된 서버로 관리합니다.
//! Redis 인스턴스 관리와 함께 모든 게임 서버를 단일 명령으로 실행할 수 있습니다.

use shared::config::redis_config::RedisConfig;
use anyhow::{Context, Result};
use tracing::{info, error, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::signal;
use tokio::process::Command;

mod tests;
mod unified_server;

use unified_server::{UnifiedGameServer, UnifiedServerConfigBuilder};

/// 게임센터 서버 상태
pub struct GameCenterServer {
    pub is_running: Arc<AtomicBool>,
    pub redis_config: Option<RedisConfig>,
    pub redis_process: Option<tokio::process::Child>,
    pub unified_server: Option<UnifiedGameServer>,
}

impl Default for GameCenterServer {
    fn default() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            redis_config: None,
            redis_process: None,
            unified_server: None,
        }
    }
}

impl GameCenterServer {
    /// 새로운 게임센터 서버 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// Redis 서버 시작
    async fn start_redis_server(&mut self) -> Result<()> {
        info!("🔴 Redis 서버 시작 중...");
        
        // Redis 서버가 이미 실행 중인지 확인
        let redis_check = Command::new("redis-cli")
            .arg("ping")
            .output()
            .await;
        
        if redis_check.is_ok() {
            info!("✅ Redis 서버가 이미 실행 중입니다.");
            return Ok(());
        }
        
        // Redis 서버 시작
        let redis_process = Command::new("redis-server")
            .spawn()
            .context("Redis 서버 시작 실패")?;
        
        self.redis_process = Some(redis_process);
        
        // Redis 서버가 완전히 시작될 때까지 대기
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        // Redis 연결 테스트
        let mut retry_count = 0;
        while retry_count < 5 {
            let ping_result = Command::new("redis-cli")
                .arg("ping")
                .output()
                .await;
            
            if ping_result.is_ok() {
                info!("✅ Redis 서버가 성공적으로 시작되었습니다!");
                return Ok(());
            }
            
            retry_count += 1;
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
        
        Err(anyhow::anyhow!("Redis 서버 시작 실패"))
    }

    /// Redis 서버 중지
    async fn stop_redis_server(&mut self) -> Result<()> {
        info!("🔴 Redis 서버 중지 중...");
        
        // Redis 서버 종료 명령 전송
        let _ = Command::new("redis-cli")
            .arg("SHUTDOWN")
            .output()
            .await;
        
        // Redis 프로세스가 있다면 종료
        if let Some(mut process) = self.redis_process.take() {
            let _ = process.kill().await;
        }
        
        info!("✅ Redis 서버가 성공적으로 중지되었습니다!");
        Ok(())
    }

    /// 통합 서버 시작
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 통합 게임센터 서버 시작 중...");
        
        // 환경변수 로드
        dotenv::dotenv().ok();
        
        // Redis 서버 시작
        self.start_redis_server().await?;
        
        // Redis 연결 설정
        let redis_config = RedisConfig::new()
            .await
            .context("RedisConfig 생성 실패")?;
        
        self.redis_config = Some(redis_config.clone());
        info!("✅ Redis 연결 성공: {}:{}", redis_config.host, redis_config.port);
        
        // 통합 서버 생성 및 시작
        let unified_server = UnifiedGameServer::from_env()
            .context("통합 서버 설정 생성 실패")?;
        
        unified_server.start().await.context("통합 서버 시작 실패")?;
        self.unified_server = Some(unified_server);
        
        // 서버 상태를 실행 중으로 설정
        self.is_running.store(true, Ordering::SeqCst);
        
        info!("✅ 통합 게임센터 서버가 성공적으로 시작되었습니다!");
        Ok(())
    }

    /// 서버 중지
    pub async fn stop(&mut self) -> Result<()> {
        info!("🛑 통합 게임센터 서버 중지 중...");
        
        // 서버 상태를 중지로 설정
        self.is_running.store(false, Ordering::SeqCst);
        
        // 통합 서버 중지
        if let Some(server) = &self.unified_server {
            server.stop().await?;
        }
        self.unified_server = None;
        
        // Redis 서버 중지
        self.stop_redis_server().await?;
        
        // Redis 연결 정리
        self.redis_config = None;
        
        info!("✅ 통합 게임센터 서버가 성공적으로 중지되었습니다!");
        Ok(())
    }

    /// 서버 상태 확인
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// 서버 상태 출력
    pub fn print_status(&self) {
        let status = if self.is_running() { "실행 중" } else { "중지됨" };
        info!("📊 통합 게임센터 서버 상태: {}", status);
        
        if let Some(ref redis_config) = self.redis_config {
            info!("📊 Redis 연결: {}:{}", redis_config.host, redis_config.port);
        } else {
            info!("📊 Redis 연결: 연결되지 않음");
        }

        if let Some(ref server) = self.unified_server {
            server.print_status();
        }
    }

    /// 서버가 종료될 때까지 대기
    pub async fn wait_for_shutdown(&self) -> Result<()> {
        if let Some(ref server) = self.unified_server {
            server.wait_for_shutdown().await?;
        }
        Ok(())
    }
}

/// 통합 게임센터의 모든 기능을 실행하는 메인 함수
pub async fn run_gamecenter() -> Result<()> {
    info!("🎮 통합 게임센터 시작 중...");
    
    // 게임센터 서버 생성
    let mut server = GameCenterServer::new();
    
    // 서버 시작
    server.start().await?;
    
    // 서버 상태 출력
    server.print_status();
    
    // 종료 시그널까지 대기
    info!("🎮 통합 게임센터 모든 기능 실행 완료! Ctrl+C로 중지할 수 있습니다.");
    
    // Ctrl+C 시그널 대기
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("🛑 종료 시그널을 받았습니다. 서버를 중지합니다...");
        }
        result = server.wait_for_shutdown() => {
            if let Err(e) = result {
                error!("서버 실행 중 오류: {}", e);
            }
        }
    }
    
    server.stop().await?;
    Ok(())
}

/// 테스트 모드를 실행하는 함수
pub async fn run_tests() -> Result<()> {
    info!("🧪 테스트 모드 시작...");
    
    // Redis 연결 설정
    let redis_config = RedisConfig::new()
        .await
        .context("RedisConfig 생성 실패")?;
    
    // 모든 테스트 실행
    tests::run_all_tests(&redis_config).await?;
    
    Ok(())
}

/// 서버를 백그라운드에서 실행하는 함수
pub async fn run_server_background() -> Result<()> {
    info!("🔄 백그라운드 서버 모드 시작...");
    
    let mut server = GameCenterServer::new();
    server.start().await?;
    
    info!("🔄 서버가 백그라운드에서 실행 중입니다. Ctrl+C로 중지할 수 있습니다.");
    
    // Ctrl+C 시그널 대기
    if let Err(e) = signal::ctrl_c().await {
        error!("시그널 대기 중 오류: {}", e);
    }
    
    info!("🛑 종료 시그널을 받았습니다. 서버를 중지합니다...");
    server.stop().await?;
    
    Ok(())
}

/// 게임센터를 중지하는 함수
pub async fn stop_gamecenter() -> Result<()> {
    info!("🛑 게임센터 중지 중...");
    
    // 게임센터 서버 생성
    let mut server = GameCenterServer::new();
    
    // 서버가 실행 중인지 확인
    if server.is_running() {
        info!("📊 서버가 실행 중입니다. 중지합니다...");
        server.stop().await?;
        info!("✅ 게임센터가 성공적으로 중지되었습니다!");
    } else {
        info!("📊 서버가 이미 중지된 상태입니다.");
    }
    
    // 서버 상태 출력
    server.print_status();
    
    info!("🛑 게임센터 중지 완료!");
    Ok(())
}

/// 개별 서버 모드 실행
async fn run_individual_server(server_type: &str) -> Result<()> {
    dotenv::dotenv().ok();

    match server_type {
        "grpc" => {
            info!("📡 gRPC 서버만 실행 중...");
            let config = UnifiedServerConfigBuilder::new()
                .enable_grpc(true)
                .enable_tcp(false)
                .enable_rudp(false)
                .build()?;
            let server = UnifiedGameServer::new(config);
            server.start().await?;
            server.wait_for_shutdown().await?;
        }
        "tcp" => {
            info!("🔌 TCP 서버만 실행 중...");
            let config = UnifiedServerConfigBuilder::new()
                .enable_grpc(false)
                .enable_tcp(true)
                .enable_rudp(false)
                .build()?;
            let server = UnifiedGameServer::new(config);
            server.start().await?;
            server.wait_for_shutdown().await?;
        }
        "rudp" | "udp" => {
            info!("📶 RUDP 서버만 실행 중...");
            let config = UnifiedServerConfigBuilder::new()
                .enable_grpc(false)
                .enable_tcp(false)
                .enable_rudp(true)
                .build()?;
            let server = UnifiedGameServer::new(config);
            server.start().await?;
            server.wait_for_shutdown().await?;
        }
        _ => {
            return Err(anyhow::anyhow!("알 수 없는 서버 타입: {}", server_type));
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // 로깅 설정
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();
    
    // 명령행 인수 확인
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("start");
    
    let result = match command {
        "start" => {
            // 통합 서버 시작 모드
            run_gamecenter().await
        }
        "stop" => {
            // 서버 중지 모드
            stop_gamecenter().await
        }
        "test" => {
            // 테스트 모드
            run_tests().await
        }
        "server" => {
            // 백그라운드 서버 모드
            run_server_background().await
        }
        "grpc" | "tcp" | "rudp" | "udp" => {
            // 개별 서버 모드
            run_individual_server(command).await
        }
        "status" => {
            // 상태 확인 모드
            info!("📊 게임센터 서버 상태 확인 중...");
            let server = GameCenterServer::new();
            server.print_status();
            
            info!("📊 Redis 서버 상태 확인 중...");
            let redis_status = Command::new("redis-cli")
                .arg("ping")
                .output()
                .await;
            
            match redis_status {
                Ok(output) => {
                    if output.status.success() {
                        info!("✅ Redis 서버: 실행 중");
                    } else {
                        warn!("❌ Redis 서버: 응답 없음");
                    }
                }
                Err(_) => warn!("❌ Redis 서버: 중지됨 또는 redis-cli 없음"),
            }
            Ok(())
        }
        "--help" | "-h" | "help" => {
            println!("🎮 Police Thief 통합 게임센터 서버");
            println!();
            println!("사용법: cargo run -p gamecenter [COMMAND]");
            println!();
            println!("COMMANDS:");
            println!("  start     통합 게임센터 시작 (기본값) - 모든 서버 실행");
            println!("  stop      게임센터 중지");
            println!("  test      테스트 실행");
            println!("  server    백그라운드 서버 모드");
            println!("  grpc      gRPC 서버만 실행");
            println!("  tcp       TCP 서버만 실행");
            println!("  rudp      RUDP 서버만 실행");
            println!("  status    서버 상태 확인");
            println!("  help      이 도움말 표시");
            println!();
            println!("환경변수:");
            println!("  grpc_host=127.0.0.1    gRPC 서버 호스트");
            println!("  grpc_port=50051        gRPC 서버 포트");
            println!("  tcp_host=127.0.0.1     TCP 서버 호스트");
            println!("  tcp_port=4000          TCP 서버 포트");
            println!("  udp_host=127.0.0.1     RUDP 서버 호스트");
            println!("  udp_port=5000          RUDP 서버 포트");
            println!("  ENABLE_GRPC=true       gRPC 서버 활성화");
            println!("  ENABLE_TCP=true        TCP 서버 활성화");
            println!("  ENABLE_RUDP=true       RUDP 서버 활성화");
            println!("  ENABLE_MONITORING=true 성능 모니터링 활성화");
            Ok(())
        }
        _ => {
            error!("알 수 없는 명령어: {}", command);
            println!("사용 가능한 명령어: start, stop, test, server, grpc, tcp, rudp, status, help");
            println!("자세한 도움말: cargo run -p gamecenter help");
            std::process::exit(1);
        }
    };
    
    if let Err(e) = result {
        error!("실행 중 오류 발생: {}", e);
        std::process::exit(1);
    }
    
    Ok(())
}