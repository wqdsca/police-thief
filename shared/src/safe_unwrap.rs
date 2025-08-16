//! 안전한 unwrap 대체 유틸리티
//!
//! unwrap() 사용을 제거하고 안전한 에러 처리를 제공합니다.

use crate::tool::error::AppError;
use std::fmt::Debug;

/// Option 타입을 위한 안전한 unwrap 트레이트
pub trait SafeUnwrapOption<T> {
    /// 안전한 unwrap - 에러 컨텍스트 포함
    fn safe_unwrap_or(self, default: T) -> T;

    /// 안전한 unwrap - 에러 반환
    fn safe_unwrap(self, context: &str) -> Result<T, AppError>;

    /// 안전한 expect - 커스텀 메시지
    fn safe_expect(self, msg: &str) -> Result<T, AppError>;
}

impl<T> SafeUnwrapOption<T> for Option<T> {
    fn safe_unwrap_or(self, default: T) -> T {
        self.unwrap_or(default)
    }

    fn safe_unwrap(self, context: &str) -> Result<T, AppError> {
        self.ok_or_else(|| AppError::InternalError(format!("Unwrap failed: {}", context)))
    }

    fn safe_expect(self, msg: &str) -> Result<T, AppError> {
        self.ok_or_else(|| AppError::InternalError(format!("Expect failed: {}", msg)))
    }
}

/// Result 타입을 위한 안전한 unwrap 트레이트
pub trait SafeUnwrapResult<T, E: Debug> {
    /// 안전한 unwrap - 에러 로깅 후 기본값 반환
    fn safe_unwrap_or(self, default: T) -> T;

    /// 안전한 unwrap - AppError로 변환
    fn safe_unwrap(self, context: &str) -> Result<T, AppError>;

    /// 로깅과 함께 unwrap_or
    fn unwrap_or_log(self, default: T, context: &str) -> T;
}

impl<T, E: Debug> SafeUnwrapResult<T, E> for Result<T, E> {
    fn safe_unwrap_or(self, default: T) -> T {
        match self {
            Ok(val) => val,
            Err(e) => {
                tracing::warn!("Result unwrap failed: {:?}", e);
                default
            }
        }
    }

    fn safe_unwrap(self, context: &str) -> Result<T, AppError> {
        self.map_err(|e| AppError::InternalError(format!("{}: {:?}", context, e)))
    }

    fn unwrap_or_log(self, default: T, context: &str) -> T {
        match self {
            Ok(val) => val,
            Err(e) => {
                tracing::error!("{}: {:?}", context, e);
                default
            }
        }
    }
}

/// 테스트 환경에서만 사용하는 unwrap
#[cfg(test)]
pub fn test_unwrap<T>(opt: Option<T>) -> T {
    opt.expect("Test unwrap failed - this should not happen in tests")
}

/// 테스트 환경에서만 사용하는 Result unwrap
#[cfg(test)]
pub fn test_unwrap_result<T, E: Debug>(res: Result<T, E>) -> T {
    res.expect("Test unwrap failed - this should not happen in tests")
}

/// 초기화 시점에만 사용하는 unwrap (프로그램 시작 시)
pub fn init_unwrap<T>(opt: Option<T>, component: &str) -> T {
    opt.unwrap_or_else(|| {
        panic!(
            "Initialization failed for {}: required value is None",
            component
        );
    })
}

/// 초기화 시점 Result unwrap
pub fn init_unwrap_result<T, E: Debug>(res: Result<T, E>, component: &str) -> T {
    res.unwrap_or_else(|e| {
        panic!("Initialization failed for {}: {:?}", component, e);
    })
}

/// 매크로: 안전한 unwrap_or 제공
#[macro_export]
macro_rules! safe_unwrap {
    ($expr:expr, $default:expr) => {
        $expr.unwrap_or($default)
    };
    ($expr:expr, $default:expr, $context:literal) => {
        match $expr {
            Some(val) => val,
            None => {
                tracing::warn!("Unwrap failed at {}: using default", $context);
                $default
            }
        }
    };
}

/// 매크로: Result를 위한 안전한 처리
#[macro_export]
macro_rules! safe_try {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                tracing::error!("Operation failed: {:?}", e);
                return Err($crate::tool::error::AppError::from(e));
            }
        }
    };
    ($expr:expr, $context:literal) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                tracing::error!("{}: {:?}", $context, e);
                return Err($crate::tool::error::AppError::InternalError(format!(
                    "{}: {:?}",
                    $context, e
                )));
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_unwrap_option() {
        let some_val: Option<i32> = Some(42);
        assert_eq!(some_val.safe_unwrap_or(0), 42);

        let none_val: Option<i32> = None;
        assert_eq!(none_val.safe_unwrap_or(0), 0);
    }

    #[test]
    fn test_safe_unwrap_result() {
        let ok_val: Result<i32, &str> = Ok(42);
        assert_eq!(ok_val.safe_unwrap_or(0), 42);

        let err_val: Result<i32, &str> = Err("error");
        assert_eq!(err_val.safe_unwrap_or(0), 0);
    }

    #[test]
    fn test_safe_unwrap_macro() {
        let val = safe_unwrap!(Some(42), 0);
        assert_eq!(val, 42);

        let val = safe_unwrap!(None::<i32>, 0);
        assert_eq!(val, 0);
    }
}
