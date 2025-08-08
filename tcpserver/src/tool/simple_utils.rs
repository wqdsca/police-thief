//! 간단한 공통 유틸리티 (컴파일 안정화용)

use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Instant;

/// 간단한 데이터 유틸리티
pub struct SimpleUtils;

impl SimpleUtils {
    /// 현재 타임스탬프 (초)
    /// 
    /// 현재 시간을 Unix 타임스탬프(초 단위)로 반환합니다.
    /// 시스템 시간을 사용하며, 오류 시 0을 반환합니다.
    /// 
    /// # Returns
    /// 
    /// * `i64` - Unix 타임스탬프 (초 단위)
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let timestamp = SimpleUtils::current_timestamp();
    /// println!("현재 시간: {}", timestamp);
    /// ```
    pub fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }
    
    /// 바이트를 16진수로 변환
    /// 
    /// 바이트 배열을 소문자 16진수 문자열로 변환합니다.
    /// 각 바이트는 2자리 16진수로 표현됩니다.
    /// 
    /// # Arguments
    /// 
    /// * `bytes` - 변환할 바이트 배열
    /// 
    /// # Returns
    /// 
    /// * `String` - 16진수 문자열 (예: "48656c6c6f")
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let bytes = b"Hello";
    /// let hex = SimpleUtils::bytes_to_hex(bytes);
    /// assert_eq!(hex, "48656c6c6f");
    /// ```
    pub fn bytes_to_hex(bytes: &[u8]) -> String {
        bytes.iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
    
    /// 16진수를 바이트로 변환
    /// 
    /// 16진수 문자열을 바이트 배열로 변환합니다.
    /// "0x" 접두사는 자동으로 제거되며, 대소문자를 구분하지 않습니다.
    /// 
    /// # Arguments
    /// 
    /// * `hex` - 변환할 16진수 문자열 (예: "48656c6c6f" 또는 "0x48656c6c6f")
    /// 
    /// # Returns
    /// 
    /// * `Result<Vec<u8>, &'static str>` - 성공 시 바이트 배열, 실패 시 에러 메시지
    /// 
    /// # Errors
    /// 
    /// * "홀수 길이 16진수" - 16진수 길이가 홀수인 경우
    /// * "잘못된 UTF-8" - 유효하지 않은 문자가 포함된 경우
    /// * "잘못된 16진수" - 16진수가 아닌 문자가 포함된 경우
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let bytes = SimpleUtils::hex_to_bytes("48656c6c6f").unwrap();
    /// assert_eq!(bytes, b"Hello");
    /// 
    /// let bytes = SimpleUtils::hex_to_bytes("0x48656c6c6f").unwrap();
    /// assert_eq!(bytes, b"Hello");
    /// ```
    pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, &'static str> {
        let hex = hex.trim().replace("0x", "");
        
        if hex.len() % 2 != 0 {
            return Err("홀수 길이 16진수");
        }
        
        let mut result = Vec::new();
        for chunk in hex.as_bytes().chunks(2) {
            let hex_str = std::str::from_utf8(chunk).map_err(|_| "잘못된 UTF-8")?;
            let byte = u8::from_str_radix(hex_str, 16).map_err(|_| "잘못된 16진수")?;
            result.push(byte);
        }
        
        Ok(result)
    }
    
    /// Instant를 Unix 타임스탬프로 변환 (근사치)
    /// 
    /// 참고: Instant는 시스템 부팅 시점부터의 경과 시간이므로
    /// 정확한 Unix 타임스탬프로 변환할 수 없습니다.
    /// 현재 시간을 기준으로 근사치를 계산합니다.
    pub fn instant_to_timestamp(instant: Instant) -> i64 {
        let now_instant = Instant::now();
        let now_timestamp = Self::current_timestamp();
        
        if instant <= now_instant {
            // 과거 시간 계산
            let elapsed = now_instant.duration_since(instant);
            now_timestamp - elapsed.as_secs() as i64
        } else {
            // 미래 시간 계산 (일반적으로 발생하지 않음)
            let elapsed = instant.duration_since(now_instant);
            now_timestamp + elapsed.as_secs() as i64
        }
    }
}