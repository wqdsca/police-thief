//! 간단한 공통 유틸리티 (컴파일 안정화용)

use std::time::{SystemTime, UNIX_EPOCH};

/// 간단한 데이터 유틸리티
pub struct SimpleUtils;

impl SimpleUtils {
    /// 현재 타임스탬프 (초)
    pub fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }
    
    /// 바이트를 16진수로 변환
    pub fn bytes_to_hex(bytes: &[u8]) -> String {
        bytes.iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
    
    /// 16진수를 바이트로 변환
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
}