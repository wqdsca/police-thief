//! TCP 서버 환경 설정 모듈
//!
//! Backend/.env 파일에서 환경변수를 로드하고 관리합니다.

use anyhow::Result;
use std::path::Path;
use tracing::{info, warn};

/// TCP 서버 설정 구조체
#[derive(Debug, Clone)]
pub struct TcpServerConfig {
    /// TCP 서버 호스트 주소
    pub host: String,
    /// TCP 서버 포트 번호
    pub port: u16,
    /// Redis 서버 호스트 주소
    pub redis_host: String,
    /// Redis 서버 포트 번호
    pub redis_port: u16,
    /// gRPC 서버 호스트 주소
    pub grpc_host: String,
    /// gRPC 서버 포트 번호
    pub grpc_port: u16,
}

impl TcpServerConfig {
    /// 환경변수에서 설정을 로드합니다.
    ///
    /// 로드 순서:
    /// 1. 프로젝트 루트의 .env 파일 (Backend/.env)
    /// 2. 현재 디렉토리의 .env 파일
    /// 3. 시스템 환경변수
    /// 4. 기본값
    pub fn from_env() -> Result<Self> {
        // .env 파일 로드 시도
        Self::load_env_file();

        // 환경변수에서 값 읽기 (기본값 포함)
        let config = Self {
            host: std::env::var("tcp_host").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("tcp_port")
                .unwrap_or_else(|_| "4000".to_string())
                .parse()
                .unwrap_or(4000),
            redis_host: std::env::var("redis_host").unwrap_or_else(|_| "127.0.0.1".to_string()),
            redis_port: std::env::var("redis_port")
                .unwrap_or_else(|_| "6379".to_string())
                .parse()
                .unwrap_or(6379),
            grpc_host: std::env::var("grpc_host").unwrap_or_else(|_| "127.0.0.1".to_string()),
            grpc_port: std::env::var("grpc_port")
                .unwrap_or_else(|_| "50051".to_string())
                .parse()
                .unwrap_or(50051),
        };

        info!("TCP 서버 설정 로드 완료: {:?}", config);
        Ok(config)
    }

    /// TCP 서버 바인딩 주소를 반환합니다.
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Redis 연결 주소를 반환합니다.
    pub fn redis_address(&self) -> String {
        format!("redis://{}:{}", self.redis_host, self.redis_port)
    }

    /// gRPC 연결 주소를 반환합니다.
    pub fn grpc_address(&self) -> String {
        format!("http://{}:{}", self.grpc_host, self.grpc_port)
    }

    /// .env 파일을 로드합니다.
    fn load_env_file() {
        // 여러 위치에서 .env 파일 찾기
        let env_paths = vec![
            "../.env",    // 상위 디렉토리 (Backend/.env)
            ".env",       // 현재 디렉토리
            "../../.env", // 상위의 상위 디렉토리 (프로젝트 루트)
        ];

        let mut loaded = false;
        for path in env_paths {
            if Path::new(path).exists() && dotenv::from_filename(path).is_ok() {
                info!(".env 파일 로드 성공: {}", path);
                loaded = true;
                break;
            }
        }

        if !loaded {
            warn!(".env 파일을 찾을 수 없습니다. 기본값과 시스템 환경변수를 사용합니다.");
        }
    }
}

/// 설정 검증 유틸리티
pub fn validate_config(config: &TcpServerConfig) -> Result<()> {
    // 포트 범위 검증 - u16 maximum is 65535, so only check for 0
    if config.port == 0 {
        anyhow::bail!("유효하지 않은 TCP 포트 번호: {}", config.port);
    }

    if config.redis_port == 0 {
        anyhow::bail!("유효하지 않은 Redis 포트 번호: {}", config.redis_port);
    }

    if config.grpc_port == 0 {
        anyhow::bail!("유효하지 않은 gRPC 포트 번호: {}", config.grpc_port);
    }

    // 호스트 주소 기본 검증
    if config.host.is_empty() {
        anyhow::bail!("TCP 호스트 주소가 비어있습니다");
    }

    if config.redis_host.is_empty() {
        anyhow::bail!("Redis 호스트 주소가 비어있습니다");
    }

    if config.grpc_host.is_empty() {
        anyhow::bail!("gRPC 호스트 주소가 비어있습니다");
    }

    Ok(())
}
