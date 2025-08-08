//! 16진수 변환 유틸리티
//! 
//! 바이트 배열과 16진수 문자열 간 변환 기능을 제공합니다.

use anyhow::{Result, anyhow};

/// 16진수 변환 유틸리티
pub struct HexUtils;

impl HexUtils {
    /// 바이트 배열을 16진수 문자열로 변환
    pub fn bytes_to_hex(bytes: &[u8]) -> String {
        bytes.iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
    
    /// 바이트 배열을 대문자 16진수 문자열로 변환
    pub fn bytes_to_hex_upper(bytes: &[u8]) -> String {
        bytes.iter()
            .map(|b| format!("{:02X}", b))
            .collect()
    }
    
    /// 16진수 문자열을 바이트 배열로 변환
    pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>> {
        let hex = hex.trim().replace(" ", "").replace("0x", "");
        
        if hex.len() % 2 != 0 {
            return Err(anyhow!("16진수 문자열 길이가 홀수입니다: {}", hex.len()));
        }
        
        let mut result = Vec::with_capacity(hex.len() / 2);
        
        for chunk in hex.as_bytes().chunks(2) {
            let hex_str = std::str::from_utf8(chunk)?;
            let byte = u8::from_str_radix(hex_str, 16)
                .map_err(|e| anyhow!("잘못된 16진수 문자: {} ({})", hex_str, e))?;
            result.push(byte);
        }
        
        Ok(result)
    }
    
    /// u32를 16진수 문자열로 변환
    pub fn u32_to_hex(value: u32) -> String {
        format!("{:08x}", value)
    }
    
    /// u64를 16진수 문자열로 변환
    pub fn u64_to_hex(value: u64) -> String {
        format!("{:016x}", value)
    }
    
    /// 16진수 문자열을 u32로 변환
    pub fn hex_to_u32(hex: &str) -> Result<u32> {
        let hex = hex.trim().replace("0x", "");
        u32::from_str_radix(&hex, 16)
            .map_err(|e| anyhow!("u32 변환 실패: {} ({})", hex, e))
    }
    
    /// 16진수 문자열을 u64로 변환
    pub fn hex_to_u64(hex: &str) -> Result<u64> {
        let hex = hex.trim().replace("0x", "");
        u64::from_str_radix(&hex, 16)
            .map_err(|e| anyhow!("u64 변환 실패: {} ({})", hex, e))
    }
    
    /// 바이트 배열을 스페이스로 구분된 16진수 문자열로 변환
    pub fn bytes_to_hex_spaced(bytes: &[u8]) -> String {
        bytes.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }
    
    /// 디버그용 바이트 배열 출력 (16바이트씩 줄바꿈)
    pub fn bytes_to_hex_debug(bytes: &[u8]) -> String {
        let hex_chars: Vec<String> = bytes.iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        
        let mut result = String::new();
        for (i, hex) in hex_chars.iter().enumerate() {
            if i > 0 && i % 16 == 0 {
                result.push('\n');
            } else if i > 0 && i % 8 == 0 {
                result.push_str("  ");
            } else if i > 0 {
                result.push(' ');
            }
            result.push_str(hex);
        }
        
        result
    }
    
    /// 바이트 배열에서 지정된 오프셋의 값 추출
    pub fn extract_u32_be(bytes: &[u8], offset: usize) -> Result<u32> {
        if bytes.len() < offset + 4 {
            return Err(anyhow!("바이트 배열이 너무 짧습니다: {} < {}", bytes.len(), offset + 4));
        }
        
        Ok(u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1], 
            bytes[offset + 2],
            bytes[offset + 3]
        ]))
    }
    
    /// 바이트 배열에서 지정된 오프셋의 값 추출 (리틀 엔디안)
    pub fn extract_u32_le(bytes: &[u8], offset: usize) -> Result<u32> {
        if bytes.len() < offset + 4 {
            return Err(anyhow!("바이트 배열이 너무 짧습니다: {} < {}", bytes.len(), offset + 4));
        }
        
        Ok(u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2], 
            bytes[offset + 3]
        ]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bytes_hex_conversion() {
        let bytes = vec![0x12, 0x34, 0xab, 0xcd, 0xef];
        let hex = HexUtils::bytes_to_hex(&bytes);
        let converted_back = HexUtils::hex_to_bytes(&hex).unwrap();
        
        assert_eq!(hex, "1234abcdef");
        assert_eq!(bytes, converted_back);
    }
    
    #[test]
    fn test_hex_with_prefix() {
        let hex_with_prefix = "0x1234abcd";
        let bytes = HexUtils::hex_to_bytes(hex_with_prefix).unwrap();
        assert_eq!(bytes, vec![0x12, 0x34, 0xab, 0xcd]);
    }
    
    #[test]
    fn test_u32_conversion() {
        let value = 0x12345678u32;
        let hex = HexUtils::u32_to_hex(value);
        let converted_back = HexUtils::hex_to_u32(&hex).unwrap();
        
        assert_eq!(hex, "12345678");
        assert_eq!(value, converted_back);
    }
    
    #[test]
    fn test_extract_u32() {
        let bytes = vec![0x12, 0x34, 0x56, 0x78, 0xab, 0xcd];
        
        let be_value = HexUtils::extract_u32_be(&bytes, 0).unwrap();
        let le_value = HexUtils::extract_u32_le(&bytes, 0).unwrap();
        
        assert_eq!(be_value, 0x12345678);
        assert_eq!(le_value, 0x78563412);
    }
    
    #[test]
    fn test_hex_debug_format() {
        let bytes: Vec<u8> = (0..32).collect();
        let debug_output = HexUtils::bytes_to_hex_debug(&bytes);
        
        println!("Debug output:\n{}", debug_output);
        assert!(debug_output.contains('\n')); // 줄바꿈 포함 확인
    }
}