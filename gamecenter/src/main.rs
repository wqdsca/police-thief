use shared::config::redis_config::RedisConfig;
use anyhow::{Context, Result};
use tracing::{info, error};
use tracing_subscriber;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::signal;
use tokio::process::Command;

mod tests;

/// 게임센터 서버 상태
pub struct GameCenterServer {
    pub is_running: Arc<AtomicBool>,
    pub redis_config: Option<RedisConfig>,
    pub redis_process: Option<tokio::process::Child>,
}

impl GameCenterServer {
    /// 새로운 게임센터 서버 생성
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            redis_config: None,
            redis_process: None,
        }
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

    /// 서버 시작
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 게임센터 서버 시작 중...");
        
        // Redis 서버 시작
        self.start_redis_server().await?;
        
        // Redis 연결 설정
        let redis_config = RedisConfig::new()
            .await
            .context("RedisConfig 생성 실패")?;
        
        self.redis_config = Some(redis_config.clone());
        info!("✅ Redis 연결 성공: {}:{}", redis_config.host, redis_config.port);
        
        // 서버 상태를 실행 중으로 설정
        self.is_running.store(true, Ordering::SeqCst);
        
        info!("✅ 게임센터 서버가 성공적으로 시작되었습니다!");
        Ok(())
    }

    /// 서버 중지
    pub async fn stop(&mut self) -> Result<()> {
        info!("🛑 게임센터 서버 중지 중...");
        
        // 서버 상태를 중지로 설정
        self.is_running.store(false, Ordering::SeqCst);
        
        // Redis 서버 중지
        self.stop_redis_server().await?;
        
        // Redis 연결 정리
        self.redis_config = None;
        
        info!("✅ 게임센터 서버가 성공적으로 중지되었습니다!");
        Ok(())
    }

    /// 서버 상태 확인
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// 서버 상태 출력
    pub fn print_status(&self) {
        let status = if self.is_running() { "실행 중" } else { "중지됨" };
        info!("📊 게임센터 서버 상태: {}", status);
        
        if let Some(ref redis_config) = self.redis_config {
            info!("📊 Redis 연결: {}:{}", redis_config.host, redis_config.port);
        } else {
            info!("📊 Redis 연결: 연결되지 않음");
        }
    }
}

/// 게임센터의 모든 기능을 실행하는 메인 함수
pub async fn run_gamecenter() -> Result<()> {
    info!("🎮 게임센터 시작 중...");
    
    // 게임센터 서버 생성
    let mut server = GameCenterServer::new();
    
    // 서버 시작
    server.start().await?;
    
    // 서버 상태 출력
    server.print_status();
    
    info!("🎮 게임센터 모든 기능 실행 완료!");
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

#[tokio::main]
async fn main() -> Result<()> {
    // 로깅 설정
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    // 명령행 인수 확인
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("start");
    
    match command {
        "start" => {
            // 일반 시작 모드
            if let Err(e) = run_gamecenter().await {
                error!("게임센터 실행 중 오류 발생: {}", e);
                std::process::exit(1);
            }
        }
        "stop" => {
            // 서버 중지 모드
            if let Err(e) = stop_gamecenter().await {
                error!("게임센터 중지 중 오류 발생: {}", e);
                std::process::exit(1);
            }
        }
        "test" => {
            // 테스트 모드
            if let Err(e) = run_tests().await {
                error!("테스트 실행 중 오류 발생: {}", e);
                std::process::exit(1);
            }
        }
        "server" => {
            // 백그라운드 서버 모드
            if let Err(e) = run_server_background().await {
                error!("서버 실행 중 오류 발생: {}", e);
                std::process::exit(1);
            }
        }
        "status" => {
            // 상태 확인 모드
            println!("📊 게임센터 서버 상태 확인 중...");
            let server = GameCenterServer::new();
            server.print_status();
            println!("📊 Redis 서버 상태 확인 중...");
            let redis_status = Command::new("redis-cli")
                .arg("ping")
                .output()
                .await;
            
            match redis_status {
                Ok(_) => println!("✅ Redis 서버: 실행 중"),
                Err(_) => println!("❌ Redis 서버: 중지됨"),
            }
        }
        _ => {
            println!("사용법:");
            println!("  cargo run start   - 게임센터 시작");
            println!("  cargo run stop    - 게임센터 중지");
            println!("  cargo run test    - 테스트 실행");
            println!("  cargo run server  - 백그라운드 서버 모드");
            println!("  cargo run status  - 서버 상태 확인");
        }
    }
    
    Ok(())
}