//! 공통 에러 처리 시스템
//!
//! TCP 서버에서 발생하는 모든 에러를 체계적으로 관리합니다.

use anyhow::Result;
use std::error::Error as StdError;
use std::fmt;
use tracing::{error, info, warn};

/// TCP 서버 에러 타입
///
/// 서버에서 발생할 수 있는 모든 에러를 체계적으로 분류합니다.
#[derive(Debug, Clone)]
pub enum TcpServerError {
    /// 연결 관련 에러
    Connection {
        client_id: Option<u32>,
        addr: Option<String>,
        message: String,
    },

    /// 프로토콜 관련 에러
    Protocol {
        message_type: Option<String>,
        raw_data: Option<Vec<u8>>,
        message: String,
    },

    /// 하트비트 관련 에러
    Heartbeat {
        client_id: Option<u32>,
        operation: String,
        message: String,
    },

    /// 서비스 관련 에러
    Service {
        service_name: String,
        operation: String,
        message: String,
    },

    /// 네트워크 관련 에러
    Network {
        addr: Option<String>,
        operation: String,
        message: String,
    },

    /// 직렬화/역직렬화 에러
    Serialization { data_type: String, message: String },

    /// 설정 관련 에러
    Configuration { key: String, message: String },

    /// 내부 시스템 에러
    Internal { component: String, message: String },
}

impl fmt::Display for TcpServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TcpServerError::Connection {
                client_id,
                addr,
                message,
            } => {
                write!(f, "연결 에러")?;
                if let Some(id) = client_id {
                    write!(f, " [클라이언트 {}]", id)?;
                }
                if let Some(address) = addr {
                    write!(f, " [{}]", address)?;
                }
                write!(f, ": {}", message)
            }
            TcpServerError::Protocol {
                message_type,
                raw_data,
                message,
            } => {
                write!(f, "프로토콜 에러")?;
                if let Some(msg_type) = message_type {
                    write!(f, " [타입: {}]", msg_type)?;
                }
                if let Some(data) = raw_data {
                    write!(f, " [데이터 크기: {}바이트]", data.len())?;
                }
                write!(f, ": {}", message)
            }
            TcpServerError::Heartbeat {
                client_id,
                operation,
                message,
            } => {
                write!(f, "하트비트 에러")?;
                if let Some(id) = client_id {
                    write!(f, " [클라이언트 {}]", id)?;
                }
                write!(f, " [작업: {}]: {}", operation, message)
            }
            TcpServerError::Service {
                service_name,
                operation,
                message,
            } => {
                write!(
                    f,
                    "서비스 에러 [{}] [작업: {}]: {}",
                    service_name, operation, message
                )
            }
            TcpServerError::Network {
                addr,
                operation,
                message,
            } => {
                write!(f, "네트워크 에러")?;
                if let Some(address) = addr {
                    write!(f, " [{}]", address)?;
                }
                write!(f, " [작업: {}]: {}", operation, message)
            }
            TcpServerError::Serialization { data_type, message } => {
                write!(f, "직렬화 에러 [타입: {}]: {}", data_type, message)
            }
            TcpServerError::Configuration { key, message } => {
                write!(f, "설정 에러 [키: {}]: {}", key, message)
            }
            TcpServerError::Internal { component, message } => {
                write!(f, "내부 에러 [컴포넌트: {}]: {}", component, message)
            }
        }
    }
}

impl StdError for TcpServerError {}

/// 에러 심각도 레벨
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorSeverity {
    /// 정보성 - 정상 동작 중 발생하는 예상 가능한 상황
    Info,
    /// 경고 - 주의가 필요하지만 서비스는 계속 가능
    Warning,
    /// 에러 - 기능에 영향을 주지만 복구 가능
    Error,
    /// 치명적 - 서비스 중단이 필요한 심각한 문제
    Critical,
}

/// 에러 컨텍스트 정보
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub error: TcpServerError,
    pub severity: ErrorSeverity,
    pub timestamp: i64,
    pub component: String,
    pub operation: String,
    pub additional_info: Option<String>,
}

/// 에러 핸들러
///
/// 모든 에러를 중앙에서 처리하고 로깅하는 핵심 구조체입니다.
pub struct ErrorHandler;

impl ErrorHandler {
    /// 에러를 처리하고 로깅합니다.
    ///
    /// # Arguments
    ///
    /// * `error` - 처리할 에러
    /// * `severity` - 에러 심각도
    /// * `component` - 에러가 발생한 컴포넌트
    /// * `operation` - 에러가 발생한 작업
    ///
    /// # Examples
    ///
    /// ```rust
    /// let error = TcpServerError::Connection {
    ///     client_id: Some(123),
    ///     addr: Some("127.0.0.1:12345".to_string()),
    ///     message: "연결 타임아웃".to_string(),
    /// };
    ///
    /// ErrorHandler::handle_error(
    ///     error,
    ///     ErrorSeverity::Warning,
    ///     "ConnectionManager",
    ///     "cleanup_timeout_connections",
    /// );
    /// ```
    pub fn handle_error(
        error: TcpServerError,
        severity: ErrorSeverity,
        component: &str,
        operation: &str,
    ) {
        let context = ErrorContext {
            error: error.clone(),
            severity,
            timestamp: crate::tool::SimpleUtils::current_timestamp(),
            component: component.to_string(),
            operation: operation.to_string(),
            additional_info: None,
        };

        Self::log_error(&context);
        Self::maybe_recover(&context);
    }

    /// 에러를 적절한 로그 레벨로 출력합니다.
    fn log_error(context: &ErrorContext) {
        let log_message = format!(
            "[{}] [{}] {}",
            context.component, context.operation, context.error
        );

        match context.severity {
            ErrorSeverity::Info => info!("{}", log_message),
            ErrorSeverity::Warning => warn!("{}", log_message),
            ErrorSeverity::Error => error!("{}", log_message),
            ErrorSeverity::Critical => {
                error!("🚨 CRITICAL: {}", log_message);
                error!("시스템 안정성에 영향을 줄 수 있는 심각한 문제입니다!");
            }
        }
    }

    /// 에러에 따른 복구 작업을 시도합니다.
    fn maybe_recover(context: &ErrorContext) {
        match &context.error {
            TcpServerError::Connection { client_id, .. } => {
                if let Some(id) = client_id {
                    info!("클라이언트 {} 연결 정리 중...", id);
                }
            }
            TcpServerError::Heartbeat { client_id, .. } => {
                if let Some(id) = client_id {
                    info!("클라이언트 {} 하트비트 상태 리셋 시도", id);
                }
            }
            TcpServerError::Service { service_name, .. } => {
                info!("서비스 {} 재시작 검토 필요", service_name);
            }
            _ => {} // 기타 에러는 복구 작업 없음
        }
    }

    /// anyhow::Error를 TcpServerError로 변환합니다.
    pub fn from_anyhow(err: anyhow::Error, error_type: &str, component: &str) -> TcpServerError {
        match error_type {
            "connection" => TcpServerError::Connection {
                client_id: None,
                addr: None,
                message: err.to_string(),
            },
            "protocol" => TcpServerError::Protocol {
                message_type: None,
                raw_data: None,
                message: err.to_string(),
            },
            "heartbeat" => TcpServerError::Heartbeat {
                client_id: None,
                operation: "unknown".to_string(),
                message: err.to_string(),
            },
            "service" => TcpServerError::Service {
                service_name: component.to_string(),
                operation: "unknown".to_string(),
                message: err.to_string(),
            },
            "network" => TcpServerError::Network {
                addr: None,
                operation: "unknown".to_string(),
                message: err.to_string(),
            },
            _ => TcpServerError::Internal {
                component: component.to_string(),
                message: err.to_string(),
            },
        }
    }
}

/// 에러 처리 매크로
///
/// 간편한 에러 처리를 위한 매크로입니다.
#[macro_export]
macro_rules! handle_tcp_error {
    ($error:expr, $severity:expr, $component:expr, $operation:expr) => {
        $crate::tool::error::ErrorHandler::handle_error($error, $severity, $component, $operation)
    };
}

/// 에러 생성 헬퍼 함수들
impl TcpServerError {
    /// 연결 에러 생성
    pub fn connection_error(client_id: Option<u32>, addr: Option<String>, message: &str) -> Self {
        Self::Connection {
            client_id,
            addr,
            message: message.to_string(),
        }
    }

    /// 프로토콜 에러 생성
    pub fn protocol_error(message_type: Option<String>, message: &str) -> Self {
        Self::Protocol {
            message_type,
            raw_data: None,
            message: message.to_string(),
        }
    }

    /// 하트비트 에러 생성
    pub fn heartbeat_error(client_id: Option<u32>, operation: &str, message: &str) -> Self {
        Self::Heartbeat {
            client_id,
            operation: operation.to_string(),
            message: message.to_string(),
        }
    }

    /// 서비스 에러 생성
    pub fn service_error(service_name: &str, operation: &str, message: &str) -> Self {
        Self::Service {
            service_name: service_name.to_string(),
            operation: operation.to_string(),
            message: message.to_string(),
        }
    }

    /// 네트워크 에러 생성
    pub fn network_error(addr: Option<String>, operation: &str, message: &str) -> Self {
        Self::Network {
            addr,
            operation: operation.to_string(),
            message: message.to_string(),
        }
    }
}

/// 결과 타입 별칭
pub type TcpResult<T> = Result<T, TcpServerError>;

/// 에러 변환 트레이트 구현
impl From<std::io::Error> for TcpServerError {
    fn from(err: std::io::Error) -> Self {
        Self::Network {
            addr: None,
            operation: "io_operation".to_string(),
            message: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for TcpServerError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization {
            data_type: "json".to_string(),
            message: err.to_string(),
        }
    }
}

mod tests {

    /// 에러 생성 테스트
    #[test]
    fn test_error_creation() {
        let conn_error = TcpServerError::connection_error(
            Some(123),
            Some("127.0.0.1:12345".to_string()),
            "연결 타임아웃",
        );

        match conn_error {
            TcpServerError::Connection {
                client_id,
                addr,
                message,
            } => {
                assert_eq!(client_id, Some(123));
                assert_eq!(addr, Some("127.0.0.1:12345".to_string()));
                assert_eq!(message, "연결 타임아웃");
            }
            _ => panic!("잘못된 에러 타입"),
        }

        println!("✅ 에러 생성 테스트 통과");
    }

    /// 에러 표시 테스트
    #[test]
    fn test_error_display() {
        let error = TcpServerError::Protocol {
            message_type: Some("HeartBeat".to_string()),
            raw_data: Some(vec![0x01, 0x02, 0x03]),
            message: "잘못된 형식".to_string(),
        };

        let display_str = error.to_string();
        assert!(display_str.contains("프로토콜 에러"));
        assert!(display_str.contains("HeartBeat"));
        assert!(display_str.contains("3바이트"));

        println!("✅ 에러 표시 테스트 통과: {}", display_str);
    }

    /// 에러 심각도 테스트
    #[test]
    fn test_error_severity() {
        let severities = vec![
            ErrorSeverity::Info,
            ErrorSeverity::Warning,
            ErrorSeverity::Error,
            ErrorSeverity::Critical,
        ];

        for severity in severities {
            let error = TcpServerError::Internal {
                component: "test".to_string(),
                message: format!("테스트 에러 {:?}", severity),
            };

            // 로깅 테스트 (실제 출력은 안 함)
            ErrorHandler::handle_error(error, severity, "test_component", "test_operation");
        }

        println!("✅ 에러 심각도 테스트 통과");
    }

    /// 에러 변환 테스트
    #[test]
    fn test_error_conversion() {
        // std::io::Error 변환
        let io_error = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "연결 거부");
        let tcp_error: TcpServerError = io_error.into();

        match tcp_error {
            TcpServerError::Network { message, .. } => {
                assert!(message.contains("연결 거부"));
            }
            _ => panic!("잘못된 에러 변환"),
        }

        println!("✅ 에러 변환 테스트 통과");
    }
}
