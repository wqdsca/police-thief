//! 안전한 메트릭 초기화 모듈
//!
//! Prometheus 메트릭을 안전하게 초기화하고 에러를 처리합니다.

use std::sync::{Arc, Mutex, Once};
use tracing::{error, info};

static INIT: Once = Once::new();
static INIT_STATE: Mutex<Option<Arc<InitResult>>> = Mutex::new(None);

#[derive(Debug, Clone)]
struct InitResult {
    success: bool,
    error_msg: Option<String>,
}

/// 메트릭 시스템 초기화
///
/// 이 함수는 한 번만 실행되며, 실패하면 에러 메시지를 반환합니다.
pub fn initialize_metrics() -> Result<(), String> {
    INIT.call_once(|| {
        // 메트릭 초기화 시도
        let result = match try_init_metrics() {
            Ok(_) => {
                info!("Prometheus metrics initialized successfully");
                InitResult {
                    success: true,
                    error_msg: None,
                }
            }
            Err(e) => {
                let error_msg = format!("Failed to initialize Prometheus metrics: {}", e);
                error!("{}", error_msg);
                InitResult {
                    success: false,
                    error_msg: Some(error_msg),
                }
            }
        };

        // 결과를 안전하게 저장
        if let Ok(mut state) = INIT_STATE.lock() {
            *state = Some(Arc::new(result));
        }
    });

    // 초기화 결과 반환
    match INIT_STATE.lock() {
        Ok(state) => match &*state {
            Some(result) => {
                if result.success {
                    Ok(())
                } else {
                    Err(result
                        .error_msg
                        .clone()
                        .unwrap_or_else(|| "Unknown initialization error".to_string()))
                }
            }
            None => Err("Initialization state not available".to_string()),
        },
        Err(_) => Err("Failed to access initialization state".to_string()),
    }
}

fn try_init_metrics() -> Result<(), String> {
    // 메트릭은 lazy_static으로 자동 등록됨
    // 여기서는 추가 초기화만 수행

    info!("Metrics system initialized");

    Ok(())
}

/// 메트릭이 초기화되었는지 확인
pub fn is_metrics_initialized() -> bool {
    match INIT_STATE.lock() {
        Ok(state) => match &*state {
            Some(result) => result.success,
            None => false,
        },
        Err(_) => false,
    }
}

/// 메트릭 초기화 에러 가져오기
pub fn get_init_error() -> Option<String> {
    match INIT_STATE.lock() {
        Ok(state) => match &*state {
            Some(result) => result.error_msg.clone(),
            None => None,
        },
        Err(_) => None,
    }
}

/// 메트릭 시스템 헬스체크
pub fn healthcheck() -> Result<(), String> {
    if !is_metrics_initialized() {
        if let Some(error) = get_init_error() {
            return Err(format!("Metrics initialization failed: {}", error));
        } else {
            return Err("Metrics not initialized".to_string());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        // 초기화는 한 번만 실행되어야 함
        let result1 = initialize_metrics();
        let result2 = initialize_metrics();

        // 두 결과가 동일해야 함
        assert_eq!(result1.is_ok(), result2.is_ok());

        // 초기화 상태 확인
        assert!(is_metrics_initialized());
    }

    #[test]
    fn test_healthcheck() {
        initialize_metrics().ok();

        let health = healthcheck();
        assert!(health.is_ok());
    }
}
