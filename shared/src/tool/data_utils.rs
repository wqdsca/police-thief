//! 데이터 전송 및 처리 유틸리티
//!
//! 바이너리 데이터 전송, 압축, 검증 등의 공통 기능을 제공합니다.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 데이터 전송 결과
#[derive(Debug, Clone)]
pub struct TransferResult {
    pub bytes_transferred: usize,
    pub duration_ms: u64,
    pub success: bool,
}

/// 바이너리 데이터 유틸리티
pub struct DataUtils;

impl DataUtils {
    /// 길이 헤더와 함께 데이터 직렬화 (4바이트 BE + 데이터)
    pub fn serialize_with_length<T: Serialize>(data: &T) -> Result<Vec<u8>> {
        let json = serde_json::to_string(data)?;
        let payload = json.as_bytes();
        let length = payload.len() as u32;

        let mut result = Vec::with_capacity(4 + payload.len());
        result.extend_from_slice(&length.to_be_bytes());
        result.extend_from_slice(payload);

        Ok(result)
    }

    /// 길이 헤더로 데이터 역직렬화
    pub fn deserialize_with_length<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<T> {
        if data.len() < 4 {
            return Err(anyhow!("데이터가 너무 짧습니다: {} < 4", data.len()));
        }

        let length = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;

        if data.len() < 4 + length {
            return Err(anyhow!(
                "메시지 길이 불일치: {} < {}",
                data.len(),
                4 + length
            ));
        }

        let json_data = &data[4..4 + length];
        let json_str = std::str::from_utf8(json_data)?;
        let result: T = serde_json::from_str(json_str)?;

        Ok(result)
    }

    /// 스트림에서 길이 헤더 읽기
    pub async fn read_length_header<R: AsyncReadExt + Unpin>(stream: &mut R) -> Result<u32> {
        let mut length_bytes = [0u8; 4];
        stream.read_exact(&mut length_bytes).await?;
        Ok(u32::from_be_bytes(length_bytes))
    }

    /// 스트림에서 정확한 길이만큼 데이터 읽기
    pub async fn read_exact_data<R: AsyncReadExt + Unpin>(
        stream: &mut R,
        length: usize,
    ) -> Result<Vec<u8>> {
        let mut buffer = vec![0u8; length];
        stream.read_exact(&mut buffer).await?;
        Ok(buffer)
    }

    /// 스트림에 데이터 전송 (플러시 포함)
    pub async fn write_data_flush<W: AsyncWriteExt + Unpin>(
        stream: &mut W,
        data: &[u8],
    ) -> Result<TransferResult> {
        let start_time = SystemTime::now();

        stream.write_all(data).await?;
        stream.flush().await?;

        let duration_ms = start_time.elapsed().unwrap_or_default().as_millis() as u64;

        Ok(TransferResult {
            bytes_transferred: data.len(),
            duration_ms,
            success: true,
        })
    }

    /// 현재 유닉스 타임스탬프 (밀리초)
    pub fn current_timestamp_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    /// 현재 유닉스 타임스탬프 (초)
    pub fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    /// 데이터 압축 (간단한 gzip)
    pub fn compress_data(data: &[u8]) -> Result<Vec<u8>> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        Ok(encoder.finish()?)
    }

    /// 데이터 압축 해제
    pub fn decompress_data(compressed: &[u8]) -> Result<Vec<u8>> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(compressed);
        let mut result = Vec::new();
        decoder.read_to_end(&mut result)?;
        Ok(result)
    }

    /// 바이트 배열을 16진수 문자열로 변환
    pub fn bytes_to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    }

    /// 체크섬 계산 (CRC32)
    pub fn calculate_checksum(data: &[u8]) -> u32 {
        crc32fast::hash(data)
    }

    /// 데이터 검증 (체크섬 포함)
    pub fn validate_data_with_checksum(data: &[u8], expected_checksum: u32) -> bool {
        Self::calculate_checksum(data) == expected_checksum
    }
}

mod tests {

    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        id: u32,
        message: String,
    }

    #[test]
    fn test_serialize_deserialize_with_length() {
        let test_data = TestData {
            id: 123,
            message: "테스트 메시지".to_string(),
        };

        let serialized =
            DataUtils::serialize_with_length(&test_data).expect("Test assertion failed");
        let deserialized: TestData =
            DataUtils::deserialize_with_length(&serialized).expect("Test assertion failed");

        assert_eq!(test_data, deserialized);
    }

    #[test]
    fn test_bytes_to_hex() {
        let bytes = vec![0x12, 0x34, 0xab, 0xcd];
        let hex = DataUtils::bytes_to_hex(&bytes);
        assert_eq!(hex, "1234abcd");
    }

    #[test]
    fn test_checksum() {
        let data = b"test data";
        let checksum = DataUtils::calculate_checksum(data);
        assert!(DataUtils::validate_data_with_checksum(data, checksum));
        assert!(!DataUtils::validate_data_with_checksum(data, checksum + 1));
    }

    #[test]
    fn test_timestamp() {
        let ts = DataUtils::current_timestamp();
        let ts_ms = DataUtils::current_timestamp_ms();

        assert!(ts > 0);
        assert!(ts_ms > ts * 1000);
    }
}
