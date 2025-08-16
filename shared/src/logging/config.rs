//! 로깅 설정 관리
//!
//! 로깅 시스템의 설정 파라미터와 서비스 타입 정의를 담당합니다.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 서비스 타입 열거형
///
/// Police Thief 게임 서버의 각 서비스 컴포넌트를 구분합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceType {
    /// gRPC API 서버
    GrpcServer,
    /// TCP 게임 서버
    TcpServer,
    /// RUDP 게임 서버
    RudpServer,
    /// 게임 센터
    GameCenter,
    /// 공유 라이브러리
    Shared,
}

impl ServiceType {
    /// 서비스 타입을 문자열로 변환
    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceType::GrpcServer => "grpcserver",
            ServiceType::TcpServer => "tcpserver",
            ServiceType::RudpServer => "rudpserver",
            ServiceType::GameCenter => "gamecenter",
            ServiceType::Shared => "shared",
        }
    }

    /// 로그 파일 접두사 반환
    pub fn log_prefix(&self) -> &'static str {
        match self {
            ServiceType::GrpcServer => "grpc",
            ServiceType::TcpServer => "tcp",
            ServiceType::RudpServer => "rudp",
            ServiceType::GameCenter => "game",
            ServiceType::Shared => "shared",
        }
    }
}

/// 로깅 시스템 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// 로그 보관 일수 (기본값: 7일)
    pub retention_days: u32,

    /// 최대 로그 파일 크기 (바이트 단위, 기본값: 100MB)
    pub max_file_size: u64,

    /// 로그 플러시 간격 (기본값: 5초)
    pub flush_interval: Duration,

    /// 비동기 큐 크기 (기본값: 10000)
    pub async_queue_size: usize,

    /// JSON 형식 여부 (기본값: true)
    pub json_format: bool,

    /// 타임스탬프 UTC 사용 여부 (기본값: true)
    pub use_utc: bool,

    /// 디버그 모드 (기본값: false)
    pub debug_mode: bool,

    /// 로그 압축 여부 (기본값: true)
    pub enable_compression: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            retention_days: 7,
            max_file_size: 100 * 1024 * 1024, // 100MB
            flush_interval: Duration::from_secs(5),
            async_queue_size: 10_000,
            json_format: true,
            use_utc: true,
            debug_mode: false,
            enable_compression: true,
        }
    }
}

impl LoggingConfig {
    /// 환경변수에서 설정 로드
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(val) = std::env::var("LOG_RETENTION_DAYS") {
            if let Ok(days) = val.parse() {
                config.retention_days = days;
            }
        }

        if let Ok(val) = std::env::var("LOG_MAX_FILE_SIZE") {
            if let Ok(size) = val.parse() {
                config.max_file_size = size;
            }
        }

        if let Ok(val) = std::env::var("LOG_FLUSH_INTERVAL") {
            if let Ok(secs) = val.parse::<u64>() {
                config.flush_interval = Duration::from_secs(secs);
            }
        }

        if let Ok(val) = std::env::var("LOG_QUEUE_SIZE") {
            if let Ok(size) = val.parse() {
                config.async_queue_size = size;
            }
        }

        if let Ok(val) = std::env::var("LOG_JSON_FORMAT") {
            config.json_format = val.to_lowercase() == "true";
        }

        if let Ok(val) = std::env::var("LOG_USE_UTC") {
            config.use_utc = val.to_lowercase() == "true";
        }

        if let Ok(val) = std::env::var("LOG_DEBUG_MODE") {
            config.debug_mode = val.to_lowercase() == "true";
        }

        if let Ok(val) = std::env::var("LOG_ENABLE_COMPRESSION") {
            config.enable_compression = val.to_lowercase() == "true";
        }

        config
    }

    /// 설정 유효성 검증
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.retention_days == 0 {
            return Err(anyhow::anyhow!("retention_days must be greater than 0"));
        }

        if self.max_file_size == 0 {
            return Err(anyhow::anyhow!("max_file_size must be greater than 0"));
        }

        if self.async_queue_size == 0 {
            return Err(anyhow::anyhow!("async_queue_size must be greater than 0"));
        }

        Ok(())
    }
}

mod tests {

    #[test]
    fn test_service_type_as_str() {
        assert_eq!(ServiceType::GrpcServer.as_str(), "grpcserver");
        assert_eq!(ServiceType::TcpServer.as_str(), "tcpserver");
        assert_eq!(ServiceType::RudpServer.as_str(), "rudpserver");
        assert_eq!(ServiceType::GameCenter.as_str(), "gamecenter");
        assert_eq!(ServiceType::Shared.as_str(), "shared");
    }

    #[test]
    fn test_service_type_log_prefix() {
        assert_eq!(ServiceType::GrpcServer.log_prefix(), "grpc");
        assert_eq!(ServiceType::TcpServer.log_prefix(), "tcp");
        assert_eq!(ServiceType::RudpServer.log_prefix(), "rudp");
        assert_eq!(ServiceType::GameCenter.log_prefix(), "game");
        assert_eq!(ServiceType::Shared.log_prefix(), "shared");
    }

    #[test]
    fn test_default_config() {
        let config = LoggingConfig::default();
        assert_eq!(config.retention_days, 7);
        assert_eq!(config.max_file_size, 100 * 1024 * 1024);
        assert_eq!(config.flush_interval, Duration::from_secs(5));
        assert_eq!(config.async_queue_size, 10_000);
        assert!(config.json_format);
        assert!(config.use_utc);
        assert!(!config.debug_mode);
        assert!(config.enable_compression);
    }

    #[test]
    fn test_config_validation() {
        let mut config = LoggingConfig::default();
        assert!(config.validate().is_ok());

        config.retention_days = 0;
        assert!(config.validate().is_err());

        config.retention_days = 7;
        config.max_file_size = 0;
        assert!(config.validate().is_err());

        config.max_file_size = 1024;
        config.async_queue_size = 0;
        assert!(config.validate().is_err());
    }
}
