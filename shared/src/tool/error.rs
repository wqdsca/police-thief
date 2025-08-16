//! gRPC Error Management System
//!
//! Police Thief gRPC 서버의 모든 에러를 체계적으로 관리합니다.
//! 비즈니스 로직 에러를 gRPC Status로 변환하고, 로깅과 모니터링을 지원합니다.

use thiserror::Error;
use tonic::Status;
use tracing::{error, info, warn};

/// 공통 애플리케이션 에러 정의
///
/// 모든 비즈니스 로직에서 발생할 수 있는 에러를 정의합니다.
/// 각 에러는 적절한 gRPC Status 코드로 변환됩니다.
#[derive(Error, Debug, Clone)]
pub enum AppError {
    // 인증 관련 에러
    #[error("인증 실패: {0}")]
    AuthError(String),

    #[error("토큰 만료: {0}")]
    TokenExpired(String),

    #[error("권한 없음: {0}")]
    PermissionDenied(String),

    // 사용자 관련 에러
    #[error("사용자를 찾을 수 없습니다: {0}")]
    UserNotFound(String),

    #[error("닉네임 중복: {0}")]
    NicknameExists(String),

    #[error("잘못된 로그인 타입: {0}")]
    InvalidLoginType(String),

    // 방 관련 에러
    #[error("방을 찾을 수 없습니다: {0}")]
    RoomNotFound(String),

    #[error("방이 가득 찼습니다: {0}")]
    RoomFull(String),

    #[error("방 이름이 너무 깁니다: {0}")]
    RoomNameTooLong(String),

    #[error("최대 플레이어 수가 잘못되었습니다: {0}")]
    InvalidMaxPlayers(String),

    #[error("방 생성 실패: {0}")]
    RoomCreation(String),

    // 입력값 검증 에러
    #[error("입력값 오류: {0}")]
    InvalidInput(String),

    #[error("필수 필드 누락: {0}")]
    MissingField(String),

    #[error("잘못된 형식: {0}")]
    InvalidFormat(String),

    // 데이터베이스 관련 에러
    #[error("데이터베이스 연결 실패: {0}")]
    DatabaseConnection(String),

    #[error("데이터베이스 쿼리 실패: {0}")]
    DatabaseQuery(String),

    #[error("트랜잭션 실패: {0}")]
    TransactionFailed(String),

    #[error("중복된 데이터: {0}")]
    DuplicateEntry(String),

    // 외부 서비스 에러
    #[error("외부 API 호출 실패: {0}")]
    ExternalApiError(String),

    #[error("Redis 연결 실패: {0}")]
    RedisConnection(String),

    // 시스템 에러
    #[error("내부 서버 에러: {0}")]
    InternalError(String),

    #[error("서비스 일시적 사용 불가: {0}")]
    ServiceUnavailable(String),

    #[error("타임아웃: {0}")]
    Timeout(String),

    #[error("Redis 에러: {0}")]
    RedisError(String),

    #[error("데이터베이스 URL 오류")]
    InvalidDatabaseUrl,
    
    #[error("Not Found: {0}")]
    NotFound(String),
    
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
}

impl AppError {
    /// 에러의 심각도를 반환합니다.
    ///
    /// # Returns
    /// * `ErrorSeverity` - 에러의 심각도 레벨
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: 시스템 장애
            AppError::DatabaseConnection(_)
            | AppError::RedisConnection(_)
            | AppError::ServiceUnavailable(_) => ErrorSeverity::Critical,

            // High: 비즈니스 로직 실패
            AppError::AuthError(_)
            | AppError::UserNotFound(_)
            | AppError::RoomNotFound(_)
            | AppError::DatabaseQuery(_)
            | AppError::TransactionFailed(_)
            | AppError::DuplicateEntry(_) => ErrorSeverity::High,

            // Medium: 사용자 입력 오류
            AppError::InvalidInput(_)
            | AppError::MissingField(_)
            | AppError::InvalidFormat(_)
            | AppError::RoomNameTooLong(_)
            | AppError::InvalidMaxPlayers(_) => ErrorSeverity::Medium,

            // Low: 일반적인 경고
            AppError::NicknameExists(_) | AppError::RoomFull(_) => ErrorSeverity::Low,

            // Default: 기타
            _ => ErrorSeverity::Medium,
        }
    }

    /// 에러를 로깅합니다.
    ///
    /// 심각도에 따라 적절한 로깅 레벨을 사용합니다.
    pub fn log(&self, context: &str) {
        let severity = self.severity();
        let error_msg = self.to_string();

        match severity {
            ErrorSeverity::Critical => {
                error!("[CRITICAL] {} - {}", context, error_msg);
            }
            ErrorSeverity::High => {
                error!("[HIGH] {} - {}", context, error_msg);
            }
            ErrorSeverity::Medium => {
                warn!("[MEDIUM] {} - {}", context, error_msg);
            }
            ErrorSeverity::Low => {
                info!("[LOW] {} - {}", context, error_msg);
            }
        }
    }

    /// 에러를 gRPC Status로 변환합니다.
    ///
    /// # Returns
    /// * `Status` - gRPC Status 객체
    pub fn to_status(&self) -> Status {
        let status: Status = self.clone().into();
        self.log("gRPC Status 변환");
        status
    }
}

/// 에러 심각도 레벨
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorSeverity {
    Critical, // 시스템 장애
    High,     // 비즈니스 로직 실패
    Medium,   // 사용자 입력 오류
    Low,      // 일반적인 경고
}

impl From<AppError> for Status {
    fn from(e: AppError) -> Self {
        match e {
            // 인증 관련
            AppError::AuthError(msg) => Status::unauthenticated(msg),
            AppError::TokenExpired(msg) => Status::unauthenticated(format!("Token expired: {msg}")),
            AppError::PermissionDenied(msg) => Status::permission_denied(msg),

            // 리소스 없음
            AppError::UserNotFound(msg) => Status::not_found(format!("User not found: {msg}")),
            AppError::RoomNotFound(msg) => Status::not_found(format!("Room not found: {msg}")),

            // 비즈니스 로직 오류
            AppError::NicknameExists(msg) => {
                Status::already_exists(format!("Nickname exists: {msg}"))
            }
            AppError::RoomFull(msg) => Status::resource_exhausted(format!("Room full: {msg}")),
            AppError::InvalidLoginType(msg) => {
                Status::invalid_argument(format!("Invalid login type: {msg}"))
            }

            // 입력값 오류
            AppError::InvalidInput(msg) => Status::invalid_argument(msg),
            AppError::MissingField(msg) => {
                Status::invalid_argument(format!("Missing field: {msg}"))
            }
            AppError::InvalidFormat(msg) => {
                Status::invalid_argument(format!("Invalid format: {msg}"))
            }
            AppError::RoomNameTooLong(msg) => {
                Status::invalid_argument(format!("Room name too long: {msg}"))
            }
            AppError::InvalidMaxPlayers(msg) => {
                Status::invalid_argument(format!("Invalid max players: {msg}"))
            }

            // 시스템 오류
            AppError::DatabaseConnection(msg) => {
                Status::unavailable(format!("Database connection failed: {msg}"))
            }
            AppError::DatabaseQuery(msg) => {
                Status::internal(format!("Database query failed: {msg}"))
            }
            AppError::TransactionFailed(msg) => {
                Status::internal(format!("Transaction failed: {msg}"))
            }
            AppError::ExternalApiError(msg) => {
                Status::unavailable(format!("External API error: {msg}"))
            }
            AppError::RedisConnection(msg) => {
                Status::unavailable(format!("Redis connection failed: {msg}"))
            }
            AppError::InternalError(msg) => Status::internal(msg),
            AppError::ServiceUnavailable(msg) => Status::unavailable(msg),
            AppError::Timeout(msg) => Status::deadline_exceeded(msg),
            AppError::RoomCreation(msg) => Status::internal(format!("Room creation failed: {msg}")),
            AppError::DuplicateEntry(msg) => {
                Status::already_exists(format!("Duplicate entry: {msg}"))
            }
            AppError::RedisError(msg) => Status::unavailable(format!("Redis error: {msg}")),
            AppError::InvalidDatabaseUrl => Status::invalid_argument("Invalid database URL"),
            AppError::NotFound(msg) => Status::not_found(msg),
            AppError::Unauthorized(msg) => Status::unauthenticated(msg),
            AppError::Configuration(msg) => Status::internal(format!("Configuration error: {msg}")),
        }
    }
}

// SqlxError conversion
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("Database record not found".to_string()),
            sqlx::Error::Database(db_err) => {
                if let Some(code) = db_err.code() {
                    if code == "23000" || code.starts_with("1062") {
                        // MySQL duplicate entry error
                        AppError::DuplicateEntry(db_err.to_string())
                    } else {
                        AppError::DatabaseQuery(db_err.to_string())
                    }
                } else {
                    AppError::DatabaseQuery(db_err.to_string())
                }
            }
            sqlx::Error::PoolTimedOut => AppError::Timeout("Database connection pool timeout".to_string()),
            sqlx::Error::PoolClosed => AppError::DatabaseConnection("Database pool is closed".to_string()),
            sqlx::Error::Configuration(_) => AppError::Configuration("Database configuration error".to_string()),
            _ => AppError::DatabaseQuery(err.to_string()),
        }
    }
}

/// 에러 처리 헬퍼 함수들
pub mod helpers {
    use super::*;
    use anyhow::Result;

    /// Result를 AppError로 변환하는 헬퍼 함수
    ///
    /// # Arguments
    /// * `result` - anyhow::Result
    /// * `context` - 에러 컨텍스트
    ///
    /// # Returns
    /// * `Result<T, AppError>` - 변환된 결과
    pub fn map_anyhow_error<T>(result: Result<T>, context: &str) -> Result<T, AppError> {
        result.map_err(|e| {
            let app_error = AppError::InternalError(format!("{context}: {e}"));
            app_error.log(context);
            app_error
        })
    }

    /// Option을 AppError로 변환하는 헬퍼 함수
    ///
    /// # Arguments
    /// * `option` - Option<T>
    /// * `error` - None일 때 반환할 에러
    ///
    /// # Returns
    /// * `Result<T, AppError>` - 변환된 결과
    pub fn map_option_error<T>(option: Option<T>, error: AppError) -> Result<T, AppError> {
        option.ok_or_else(|| {
            error.log("Option to Error");
            error
        })
    }

    /// 문자열 검증 헬퍼 함수
    ///
    /// # Arguments
    /// * `value` - 검증할 문자열
    /// * `field_name` - 필드 이름
    /// * `max_length` - 최대 길이
    ///
    /// # Returns
    /// * `Result<String, AppError>` - 검증 결과
    pub fn validate_string(
        value: String,
        field_name: &str,
        max_length: usize,
    ) -> Result<String, AppError> {
        if value.is_empty() {
            return Err(AppError::MissingField(field_name.to_string()));
        }

        if value.len() > max_length {
            return Err(AppError::InvalidInput(format!(
                "{field_name} too long (max: {max_length})"
            )));
        }

        Ok(value)
    }

    /// 숫자 범위 검증 헬퍼 함수
    ///
    /// # Arguments
    /// * `value` - 검증할 숫자
    /// * `field_name` - 필드 이름
    /// * `min` - 최소값
    /// * `max` - 최대값
    ///
    /// # Returns
    /// * `Result<i32, AppError>` - 검증 결과
    pub fn validate_range(
        value: i32,
        field_name: &str,
        min: i32,
        max: i32,
    ) -> Result<i32, AppError> {
        if value < min || value > max {
            return Err(AppError::InvalidInput(format!(
                "{field_name} out of range ({min}-{max})"
            )));
        }

        Ok(value)
    }
}

/// 에러 통계 추적을 위한 구조체
#[derive(Debug, Default)]
pub struct ErrorTracker {
    critical_count: u64,
    high_count: u64,
    medium_count: u64,
    low_count: u64,
}

impl ErrorTracker {
    /// 새로운 에러를 기록합니다.
    pub fn record_error(&mut self, error: &AppError) {
        match error.severity() {
            ErrorSeverity::Critical => self.critical_count += 1,
            ErrorSeverity::High => self.high_count += 1,
            ErrorSeverity::Medium => self.medium_count += 1,
            ErrorSeverity::Low => self.low_count += 1,
        }
    }

    /// 에러 통계를 반환합니다.
    pub fn get_stats(&self) -> ErrorStats {
        ErrorStats {
            critical: self.critical_count,
            high: self.high_count,
            medium: self.medium_count,
            low: self.low_count,
            total: self.critical_count + self.high_count + self.medium_count + self.low_count,
        }
    }
}

/// 에러 통계 정보
#[derive(Debug, Clone)]
pub struct ErrorStats {
    pub critical: u64,
    pub high: u64,
    pub medium: u64,
    pub low: u64,
    pub total: u64,
}
