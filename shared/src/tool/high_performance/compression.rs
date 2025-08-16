//! 메시지 압축 및 배칭 라이브러리 (Shared)
//!
//! tcpserver와 rudpserver에서 공통 사용되는 압축 최적화 기능을 제공합니다.
//! LZ4, Zstd 등 고속 압축 알고리즘과 지능형 메시지 배칭을 지원합니다.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// 압축 알고리즘 타입
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CompressionAlgorithm {
    /// 압축 없음
    None,
    /// LZ4 압축 (고속)
    LZ4,
    /// Zstd 압축 (고압축률)
    Zstd,
    /// 적응형 압축 (자동 선택)
    Adaptive,
}

impl Default for CompressionAlgorithm {
    fn default() -> Self {
        Self::Adaptive
    }
}

/// 메시지 압축 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageCompressionConfig {
    /// 사용할 압축 알고리즘
    pub algorithm: CompressionAlgorithm,
    /// 압축 임계값 (바이트) - 이보다 작으면 압축하지 않음
    pub compression_threshold: usize,
    /// 압축 레벨 (1-9, 높을수록 압축률 좋지만 느림)
    pub compression_level: i32,
    /// 배칭 활성화 여부
    pub enable_batching: bool,
    /// 배치 크기 (메시지 개수)
    pub batch_size: usize,
    /// 배치 타임아웃 (밀리초)
    pub batch_timeout_ms: u64,
    /// 최대 배치 바이트 크기
    pub max_batch_bytes: usize,
    /// 압축 캐시 활성화
    pub enable_compression_cache: bool,
    /// 캐시 TTL (초)
    pub cache_ttl_secs: u64,
}

impl Default for MessageCompressionConfig {
    fn default() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Adaptive,
            compression_threshold: 128, // 128바이트 이상부터 압축
            compression_level: 1,       // 빠른 압축
            enable_batching: true,
            batch_size: 10,        // 10개 메시지씩 배치
            batch_timeout_ms: 5,   // 5ms 타임아웃
            max_batch_bytes: 8192, // 8KB 최대 배치
            enable_compression_cache: true,
            cache_ttl_secs: 300, // 5분 캐시 TTL
        }
    }
}

/// 압축 성능 통계
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CompressionPerformanceReport {
    /// 총 압축 횟수
    pub total_compressions: u64,
    /// 총 압축 해제 횟수
    pub total_decompressions: u64,
    /// 원본 바이트 수
    pub total_original_bytes: u64,
    /// 압축 후 바이트 수
    pub total_compressed_bytes: u64,
    /// 평균 압축률 (%)
    pub average_compression_ratio: f64,
    /// 평균 압축 시간 (마이크로초)
    pub average_compression_time_us: f64,
    /// 평균 압축 해제 시간 (마이크로초)
    pub average_decompression_time_us: f64,
    /// 배치 처리 통계
    pub batch_stats: BatchProcessingStats,
    /// 캐시 적중률 (%)
    pub cache_hit_rate: f64,
    /// 총 대역폭 절약 (바이트)
    pub bandwidth_saved_bytes: u64,
}

/// 배치 처리 통계
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BatchProcessingStats {
    /// 총 배치 수
    pub total_batches: u64,
    /// 배치된 총 메시지 수
    pub total_batched_messages: u64,
    /// 평균 배치 크기
    pub average_batch_size: f64,
    /// 평균 배치 시간 (마이크로초)
    pub average_batch_time_us: f64,
    /// 배치 타임아웃 발생 횟수
    pub batch_timeouts: u64,
    /// 배치로 인한 지연 감소 (%)
    pub latency_reduction: f64,
}

/// 메시지 배치
#[derive(Debug)]
pub struct MessageBatch {
    messages: VecDeque<Vec<u8>>,
    total_bytes: usize,
    created_at: Instant,
    max_size: usize,
    max_bytes: usize,
    timeout: Duration,
}

impl MessageBatch {
    pub fn new(max_size: usize, max_bytes: usize, timeout: Duration) -> Self {
        Self {
            messages: VecDeque::with_capacity(max_size),
            total_bytes: 0,
            created_at: Instant::now(),
            max_size,
            max_bytes,
            timeout,
        }
    }

    /// 메시지를 배치에 추가
    pub fn add_message(&mut self, message: Vec<u8>) -> bool {
        if self.is_full() {
            return false;
        }

        let message_size = message.len();
        if self.total_bytes + message_size > self.max_bytes {
            return false;
        }

        self.total_bytes += message_size;
        self.messages.push_back(message);
        true
    }

    /// 배치가 가득 찼는지 확인
    pub fn is_full(&self) -> bool {
        self.messages.len() >= self.max_size
    }

    /// 배치가 타임아웃되었는지 확인
    pub fn is_timeout(&self) -> bool {
        self.created_at.elapsed() >= self.timeout
    }

    /// 배치를 처리할 준비가 되었는지 확인
    pub fn is_ready(&self) -> bool {
        !self.messages.is_empty() && (self.is_full() || self.is_timeout())
    }

    /// 배치에서 모든 메시지를 추출
    pub fn extract_messages(&mut self) -> Vec<Vec<u8>> {
        let messages = self.messages.drain(..).collect();
        self.total_bytes = 0;
        self.created_at = Instant::now();
        messages
    }

    /// 배치 크기 (메시지 개수)
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// 배치가 비어있는지 확인
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// 총 바이트 크기
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// 배치 생성 이후 경과 시간
    pub fn elapsed(&self) -> Duration {
        self.created_at.elapsed()
    }
}

/// 압축 결과
#[derive(Debug)]
pub struct CompressionResult {
    /// 압축된 데이터
    pub compressed_data: Vec<u8>,
    /// 원본 크기
    pub original_size: usize,
    /// 압축 후 크기
    pub compressed_size: usize,
    /// 압축률 (0.0 ~ 1.0)
    pub compression_ratio: f64,
    /// 압축 시간
    pub compression_time: Duration,
    /// 사용된 알고리즘
    pub algorithm_used: CompressionAlgorithm,
}

impl CompressionResult {
    pub fn new(
        compressed_data: Vec<u8>,
        original_size: usize,
        compression_time: Duration,
        algorithm_used: CompressionAlgorithm,
    ) -> Self {
        let compressed_size = compressed_data.len();
        let compression_ratio = if original_size > 0 {
            compressed_size as f64 / original_size as f64
        } else {
            1.0
        };

        Self {
            compressed_data,
            original_size,
            compressed_size,
            compression_ratio,
            compression_time,
            algorithm_used,
        }
    }

    /// 대역폭 절약량 (바이트)
    pub fn bandwidth_saved(&self) -> usize {
        self.original_size.saturating_sub(self.compressed_size)
    }

    /// 압축률 퍼센트 (%)
    pub fn compression_ratio_percent(&self) -> f64 {
        (1.0 - self.compression_ratio) * 100.0
    }
}

/// 압축 유틸리티
pub struct CompressionUtils;

impl CompressionUtils {
    /// 데이터가 압축할 가치가 있는지 판단
    pub fn should_compress(data: &[u8], threshold: usize) -> bool {
        if data.len() < threshold {
            return false;
        }

        // 간단한 엔트로피 검사 (이미 압축된 데이터인지 확인)
        let entropy = Self::calculate_entropy(data);
        entropy > 0.5 // 엔트로피가 높으면 압축 효과 있음
    }

    /// 데이터의 엔트로피 계산 (단순화된 버전)
    pub fn calculate_entropy(data: &[u8]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        let mut frequency = [0u32; 256];
        for &byte in data {
            frequency[byte as usize] += 1;
        }

        let len = data.len() as f64;
        let mut entropy = 0.0;

        for &freq in &frequency {
            if freq > 0 {
                let p = freq as f64 / len;
                entropy -= p * p.log2();
            }
        }

        entropy / 8.0 // 정규화 (0.0 ~ 1.0)
    }

    /// 최적 압축 알고리즘 선택 (적응형)
    pub fn select_optimal_algorithm(data: &[u8], target_ratio: f64) -> CompressionAlgorithm {
        let entropy = Self::calculate_entropy(data);
        let size = data.len();

        if entropy < 0.3 {
            // 낮은 엔트로피 → 압축 효과 높음 → Zstd
            return CompressionAlgorithm::Zstd;
        }

        if size > 8192 && entropy > 0.7 {
            // 큰 크기 + 높은 엔트로피 → 빠른 압축 → LZ4
            return CompressionAlgorithm::LZ4;
        }

        if target_ratio < 0.5 {
            // 높은 압축률 요구 → Zstd
            return CompressionAlgorithm::Zstd;
        }

        // 기본적으로 LZ4 (속도 우선)
        CompressionAlgorithm::LZ4
    }

    /// 압축 캐시 키 생성
    pub fn generate_cache_key(data: &[u8], algorithm: CompressionAlgorithm) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        algorithm.hash(&mut hasher);
        hasher.finish()
    }
}

/// 모의 압축 함수들 (실제 구현에서는 flate2, lz4, zstd 크레이트 사용)
pub mod mock_compression {
    use super::*;

    /// LZ4 모의 압축
    pub fn mock_lz4_compress(data: &[u8]) -> CompressionResult {
        let start = Instant::now();

        // 간단한 런-길이 인코딩으로 시뮬레이션
        let mut compressed = Vec::new();
        let mut i = 0;

        while i < data.len() {
            let byte = data[i];
            let mut count = 1;

            while i + count < data.len() && data[i + count] == byte && count < 255 {
                count += 1;
            }

            if count > 3 {
                compressed.extend([0xFF, count as u8, byte]);
                i += count;
            } else {
                for _ in 0..count {
                    compressed.push(byte);
                }
                i += count;
            }
        }

        let compression_time = start.elapsed();
        CompressionResult::new(
            compressed,
            data.len(),
            compression_time,
            CompressionAlgorithm::LZ4,
        )
    }

    /// LZ4 모의 압축 해제
    pub fn mock_lz4_decompress(compressed: &[u8]) -> Result<Vec<u8>, String> {
        let mut decompressed = Vec::new();
        let mut i = 0;

        while i < compressed.len() {
            if compressed[i] == 0xFF && i + 2 < compressed.len() {
                let count = compressed[i + 1];
                let byte = compressed[i + 2];
                decompressed.extend(vec![byte; count as usize]);
                i += 3;
            } else {
                decompressed.push(compressed[i]);
                i += 1;
            }
        }

        Ok(decompressed)
    }

    /// Zstd 모의 압축 (LZ4보다 더 높은 압축률로 시뮬레이션)
    pub fn mock_zstd_compress(data: &[u8]) -> CompressionResult {
        let start = Instant::now();

        // Zstd는 더 복잡한 압축을 시뮬레이션하기 위해 추가 시간 소요
        std::thread::sleep(Duration::from_micros(10)); // 시뮬레이션

        let mut result = mock_lz4_compress(data);

        // Zstd는 일반적으로 더 높은 압축률을 제공
        if result.compressed_size > 10 {
            result
                .compressed_data
                .truncate(result.compressed_size * 8 / 10);
        }

        result.compression_time = start.elapsed();
        result.algorithm_used = CompressionAlgorithm::Zstd;
        result.compressed_size = result.compressed_data.len();
        result.compression_ratio = result.compressed_size as f64 / result.original_size as f64;

        result
    }
}

mod tests {

    #[test]
    fn test_message_batch() {
        let mut batch = MessageBatch::new(3, 100, Duration::from_millis(10));

        assert!(batch.is_empty());
        assert!(!batch.is_ready());

        // 메시지 추가
        assert!(batch.add_message(b"message1".to_vec()));
        assert!(batch.add_message(b"message2".to_vec()));
        assert_eq!(batch.len(), 2);

        // 배치 가득 찰 때까지 추가
        assert!(batch.add_message(b"message3".to_vec()));
        assert!(batch.is_full());
        assert!(batch.is_ready());

        // 메시지 추출
        let messages = batch.extract_messages();
        assert_eq!(messages.len(), 3);
        assert!(batch.is_empty());
    }

    #[test]
    fn test_batch_timeout() {
        let mut batch = MessageBatch::new(10, 1000, Duration::from_millis(1));
        batch.add_message(b"test".to_vec());

        // 타임아웃 대기
        std::thread::sleep(Duration::from_millis(2));

        assert!(batch.is_timeout());
        assert!(batch.is_ready());
    }

    #[test]
    fn test_compression_utils() {
        let data = b"aaaaaaaaaaaaaaaa"; // 낮은 엔트로피
        let random_data = b"abcdefghijk12345"; // 높은 엔트로피

        assert!(CompressionUtils::should_compress(data, 10));
        assert!(!CompressionUtils::should_compress(b"ab", 10)); // 너무 짧음

        let entropy1 = CompressionUtils::calculate_entropy(data);
        let entropy2 = CompressionUtils::calculate_entropy(random_data);

        assert!(entropy1 < entropy2); // 반복 데이터는 엔트로피 낮음
    }

    #[test]
    fn test_optimal_algorithm_selection() {
        let low_entropy = vec![b'A'; 1000]; // 반복 데이터
        let high_entropy: Vec<u8> = (0..1000).map(|i| i as u8).collect();

        let algo1 = CompressionUtils::select_optimal_algorithm(&low_entropy, 0.5);
        let algo2 = CompressionUtils::select_optimal_algorithm(&high_entropy, 0.8);

        // 낮은 엔트로피는 Zstd 선호 (높은 압축률)
        assert_eq!(algo1, CompressionAlgorithm::Zstd);
        // 높은 엔트로피는 LZ4 선호 (빠른 속도)
        assert_eq!(algo2, CompressionAlgorithm::LZ4);
    }

    #[test]
    fn test_mock_compression() {
        let test_data = b"aaaaaaaaaaaabbbbbbbbbbbbcccccccccccc";

        let result = mock_lz4_compress(test_data);
        assert!(result.compressed_size <= result.original_size);
        assert!(result.compression_time.as_micros() > 0);
        assert_eq!(result.algorithm_used, CompressionAlgorithm::LZ4);

        // 압축 해제 테스트
        let decompressed =
            mock_lz4_decompress(&result.compressed_data).expect("Test assertion failed");
        assert_eq!(&decompressed, test_data);
    }

    #[test]
    fn test_zstd_compression() {
        let test_data = vec![b'X'; 1000];

        let lz4_result = mock_lz4_compress(&test_data);
        let zstd_result = mock_zstd_compress(&test_data);

        // Zstd가 더 높은 압축률을 제공해야 함
        assert!(zstd_result.compression_ratio <= lz4_result.compression_ratio);
        assert_eq!(zstd_result.algorithm_used, CompressionAlgorithm::Zstd);
    }

    #[test]
    fn test_compression_result() {
        let compressed_data = vec![1, 2, 3];
        let original_size = 10;
        let result = CompressionResult::new(
            compressed_data,
            original_size,
            Duration::from_millis(1),
            CompressionAlgorithm::LZ4,
        );

        assert_eq!(result.bandwidth_saved(), 7); // 10 - 3
        assert_eq!(result.compression_ratio_percent(), 70.0); // (1 - 3/10) * 100
    }

    #[test]
    fn test_cache_key_generation() {
        let data1 = b"test data";
        let data2 = b"test data";
        let data3 = b"different";

        let key1 = CompressionUtils::generate_cache_key(data1, CompressionAlgorithm::LZ4);
        let key2 = CompressionUtils::generate_cache_key(data2, CompressionAlgorithm::LZ4);
        let key3 = CompressionUtils::generate_cache_key(data3, CompressionAlgorithm::LZ4);
        let key4 = CompressionUtils::generate_cache_key(data1, CompressionAlgorithm::Zstd);

        assert_eq!(key1, key2); // 같은 데이터, 같은 알고리즘
        assert_ne!(key1, key3); // 다른 데이터
        assert_ne!(key1, key4); // 다른 알고리즘
    }
}
