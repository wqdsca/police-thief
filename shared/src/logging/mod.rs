//! 통합 로깅 시스템
//!
//! Police Thief 게임 서버를 위한 종합 로깅 시스템입니다.
//! 
//! # 주요 기능
//! - **기능별 로그 분류**: gRPC, TCP, RUDP, Game Center별 로그 분리
//! - **날짜별 파일 관리**: 매일 새로운 로그 파일 생성
//! - **자동 보관 정책**: 7일 후 자동 삭제
//! - **비동기 처리**: 성능 영향 최소화
//! - **구조화된 로그**: JSON 형태로 분석 용이
//!
//! # 사용 예시
//! ```rust
//! use shared::logging::{LoggingSystem, ServiceType, LogLevel};
//! 
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let logger = LoggingSystem::new("./logs").await?;
//!     logger.init(ServiceType::GrpcServer).await?;
//!     
//!     logger.info("서버 시작", &[("port", "50051")]).await;
//!     logger.error("연결 실패", &[("error", "timeout")]).await;
//!     
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod formatter;
pub mod rotation;
pub mod system;
pub mod writer;

pub use config::{LoggingConfig, ServiceType};
pub use formatter::{LogFormatter, LogLevel, LogEntry};
pub use rotation::LogRotationManager;
pub use system::LoggingSystem;
pub use writer::AsyncLogWriter;

use anyhow::Result;
use std::path::Path;

/// 로깅 시스템 초기화 함수
/// 
/// 각 서비스에서 간편하게 로깅 시스템을 초기화할 수 있도록 제공하는 헬퍼 함수입니다.
///
/// # Arguments
/// * `service_type` - 서비스 타입 (GrpcServer, TcpServer, RudpServer, GameCenter)
/// * `log_dir` - 로그 디렉토리 경로 (기본값: "./logs")
///
/// # Returns
/// 초기화된 LoggingSystem 인스턴스
///
/// # Examples
/// ```rust
/// use shared::logging::{init_logging, ServiceType};
///
/// #[tokio::main] 
/// async fn main() -> anyhow::Result<()> {
///     let logger = init_logging(ServiceType::GrpcServer, None).await?;
///     logger.info("gRPC 서버 시작됨", &[]).await;
///     Ok(())
/// }
/// ```
pub async fn init_logging<P: AsRef<Path>>(
    service_type: ServiceType,
    log_dir: Option<P>,
) -> Result<LoggingSystem> {
    let log_path = log_dir
        .as_ref()
        .map(|p| p.as_ref())
        .unwrap_or_else(|| Path::new("./logs"));
        
    let mut logging_system = LoggingSystem::new(log_path).await?;
    logging_system.init(service_type).await?;
    Ok(logging_system)
}