// 🔒 Phase 1: unwrap/expect 완전 제거 전략
// anyhow, thiserror 패턴 적용

use anyhow::{Context, Result, bail};
use thiserror::Error;
use tracing::{error, warn, info};

// ✅ 1. 커스텀 에러 타입 정의 (thiserror 스타일)
#[derive(Error, Debug)]
pub enum GameError {
    #[error("Network error: {message}")]
    Network { 
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Validation failed: {field} - {reason}")]
    Validation { field: String, reason: String },
    
    #[error("Authentication failed")]
    AuthenticationFailed,
    
    #[error("Resource not found: {resource}")]
    NotFound { resource: String },
    
    #[error("Operation timed out after {seconds} seconds")]
    Timeout { seconds: u64 },
    
    #[error("Internal server error")]
    Internal,
}

// ✅ 2. Result 타입 별칭
pub type GameResult<T> = Result<T, GameError>;

// ✅ 3. Option을 Result로 변환하는 확장 트레이트
pub trait OptionExt<T> {
    fn ok_or_not_found(self, resource: &str) -> GameResult<T>;
    fn ok_or_validation(self, field: &str, reason: &str) -> GameResult<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_or_not_found(self, resource: &str) -> GameResult<T> {
        self.ok_or_else(|| GameError::NotFound { 
            resource: resource.to_string() 
        })
    }
    
    fn ok_or_validation(self, field: &str, reason: &str) -> GameResult<T> {
        self.ok_or_else(|| GameError::Validation {
            field: field.to_string(),
            reason: reason.to_string(),
        })
    }
}

// ✅ 4. 안전한 파싱 함수들
pub mod safe_parse {
    use super::*;
    
    pub fn parse_u64(s: &str, field_name: &str) -> GameResult<u64> {
        s.parse::<u64>()
            .map_err(|e| GameError::Validation {
                field: field_name.to_string(),
                reason: format!("Invalid number: {}", e),
            })
    }
    
    pub fn parse_json<T: serde::de::DeserializeOwned>(data: &str) -> GameResult<T> {
        serde_json::from_str(data)
            .map_err(|e| GameError::Serialization(e))
    }
    
    pub fn parse_url(url_str: &str) -> GameResult<url::Url> {
        url::Url::parse(url_str)
            .map_err(|e| GameError::Validation {
                field: "url".to_string(),
                reason: e.to_string(),
            })
    }
}

// ✅ 5. 안전한 컬렉션 접근
pub trait SafeIndexing<T> {
    fn safe_get(&self, index: usize) -> GameResult<&T>;
    fn safe_get_mut(&mut self, index: usize) -> GameResult<&mut T>;
}

impl<T> SafeIndexing<T> for Vec<T> {
    fn safe_get(&self, index: usize) -> GameResult<&T> {
        self.get(index)
            .ok_or_else(|| GameError::Validation {
                field: "index".to_string(),
                reason: format!("Index {} out of bounds (len: {})", index, self.len()),
            })
    }
    
    fn safe_get_mut(&mut self, index: usize) -> GameResult<&mut T> {
        let len = self.len();
        self.get_mut(index)
            .ok_or_else(|| GameError::Validation {
                field: "index".to_string(),
                reason: format!("Index {} out of bounds (len: {})", index, len),
            })
    }
}

// ✅ 6. 안전한 HashMap 작업
use std::collections::HashMap;

pub trait SafeHashMap<K, V> {
    fn safe_get(&self, key: &K) -> GameResult<&V>;
    fn safe_remove(&mut self, key: &K) -> GameResult<V>;
}

impl<K: Eq + std::hash::Hash + std::fmt::Display, V> SafeHashMap<K, V> for HashMap<K, V> {
    fn safe_get(&self, key: &K) -> GameResult<&V> {
        self.get(key)
            .ok_or_else(|| GameError::NotFound {
                resource: format!("Key: {}", key),
            })
    }
    
    fn safe_remove(&mut self, key: &K) -> GameResult<V> {
        self.remove(key)
            .ok_or_else(|| GameError::NotFound {
                resource: format!("Key: {}", key),
            })
    }
}

// ✅ 7. 에러 체인 및 컨텍스트 추가
pub trait ErrorContext<T> {
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ErrorContext<T> for Result<T, E>
where
    E: Into<anyhow::Error>,
{
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| e.into()).context(f())
    }
}

// ✅ 8. 안전한 비동기 작업
use tokio::time::{timeout, Duration};

pub async fn safe_timeout<F, T>(
    duration: Duration,
    future: F,
) -> GameResult<T>
where
    F: std::future::Future<Output = T>,
{
    timeout(duration, future)
        .await
        .map_err(|_| GameError::Timeout {
            seconds: duration.as_secs(),
        })
}

// ✅ 9. 안전한 파일 작업
use tokio::fs;
use tokio::io::AsyncReadExt;

pub async fn safe_read_file(path: &str) -> GameResult<String> {
    let mut file = fs::File::open(path)
        .await
        .map_err(|e| GameError::Network {
            message: format!("Failed to open file: {}", path),
            source: Some(Box::new(e)),
        })?;
    
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .await
        .map_err(|e| GameError::Network {
            message: format!("Failed to read file: {}", path),
            source: Some(Box::new(e)),
        })?;
    
    Ok(contents)
}

// ✅ 10. 에러 복구 패턴
pub struct RetryPolicy {
    max_attempts: u32,
    base_delay_ms: u64,
    max_delay_ms: u64,
}

impl RetryPolicy {
    pub async fn execute<F, T, Fut>(&self, operation: F) -> GameResult<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = GameResult<T>>,
    {
        let mut attempt = 0;
        let mut delay = self.base_delay_ms;
        
        loop {
            attempt += 1;
            
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) if attempt >= self.max_attempts => {
                    error!("Operation failed after {} attempts: {:?}", attempt, e);
                    return Err(e);
                }
                Err(e) => {
                    warn!("Attempt {} failed: {:?}, retrying in {}ms", attempt, e, delay);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    delay = (delay * 2).min(self.max_delay_ms);
                }
            }
        }
    }
}

// ✅ 11. 로깅과 모니터링 통합
#[macro_export]
macro_rules! log_error {
    ($result:expr) => {
        if let Err(ref e) = $result {
            error!("Error occurred: {:?}", e);
            // 메트릭 수집
            metrics::increment_counter!("errors_total", "type" => std::any::type_name_of_val(e));
        }
        $result
    };
}

// ✅ 12. 실제 코드 변환 예시
pub mod examples {
    use super::*;
    
    // Before: panic이 발생하는 코드
    pub fn old_get_user(id: u64, users: &HashMap<u64, String>) -> String {
        users.get(&id).ok().clone() // panic!
    }
    
    // After: 안전한 에러 처리
    pub fn new_get_user(id: u64, users: &HashMap<u64, String>) -> GameResult<String> {
        users.get(&id)
            .ok_or_not_found(&format!("user:{}", id))
            .map(|s| s.clone())
    }
    
    // Before: expect 사용
    pub async fn old_connect() -> tokio::net::TcpStream {
        tokio::net::TcpStream::connect("127.0.0.1:8080")
            .await
            .expect("Failed to connect") // panic!
    }
    
    // After: Result 반환
    pub async fn new_connect() -> GameResult<tokio::net::TcpStream> {
        tokio::net::TcpStream::connect("127.0.0.1:8080")
            .await
            .map_err(|e| GameError::Network {
                message: "Failed to connect to server".to_string(),
                source: Some(Box::new(e)),
            })
    }
}