use dotenv::dotenv;
use std::env;
use redis::{aio::ConnectionManager, Client, RedisError};

pub type RedisConnection = ConnectionManager;

#[derive(Clone)]
pub struct RedisConfig {
    pub conn: RedisConnection,
    pub host: String,
    pub port: u16,
}

impl RedisConfig {
    pub async fn new() -> Result<Self, RedisError> {
        // .env 파일 로드 - workspace root에서 찾기
        let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let workspace_env = current_dir.join(".env");
        let parent_env = current_dir.parent().map(|p| p.join(".env"));
        
        let mut env_loaded = false;
        
        // 현재 디렉토리의 .env 파일 시도
        if workspace_env.exists() {
            dotenv::from_path(&workspace_env).ok();
            env_loaded = true;
        }
        // 상위 디렉토리의 .env 파일 시도 (서브파크에서 실행되는 경우)
        else if let Some(parent_env) = parent_env {
            if parent_env.exists() {
                dotenv::from_path(&parent_env).ok();
                env_loaded = true;
            }
        }
        
        if !env_loaded {
            dotenv().ok(); // 기본 .env 파일 시도
        }
        
        // 환경변수 로드 확인
        let host = env::var("redis_host").unwrap_or_else(|_| {
            println!("redis_host 환경변수가 없어서 localhost를 사용합니다.");
            "localhost".to_string()
        });
        
        let port_str = env::var("redis_port").unwrap_or_else(|_| {
            println!("redis_port 환경변수가 없어서 6379를 사용합니다.");
            "6379".to_string()
        });
        
        let port = port_str.parse::<u16>().expect("redis_port는 숫자여야 함");

        let client = Client::open(format!("redis://{}:{}", host, port))?;
        let manager = ConnectionManager::new(client).await?;
        Ok(Self { conn: manager, host, port })
    }

    pub fn get_connection(&self) -> RedisConnection {
        self.conn.clone()
    }
}
