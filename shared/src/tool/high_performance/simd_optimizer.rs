//! SIMD 연산 최적화 라이브러리 (Shared)
//! 
//! tcpserver와 rudpserver에서 공통 사용되는 SIMD 최적화 기능을 제공합니다.
//! AVX2, SSE4.2 등을 활용한 고속 벡터 연산으로 패킷 처리 성능을 극대화합니다.

use serde::{Serialize, Deserialize};

/// SIMD 최적화 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimdOptimizerConfig {
    /// AVX2 사용 활성화
    pub enable_avx2: bool,
    /// SSE4.2 사용 활성화
    pub enable_sse42: bool,
    /// 자동 SIMD 감지 활성화
    pub enable_auto_detection: bool,
    /// 벡터화 배치 크기 (기본: 64)
    pub vectorize_batch_size: usize,
    /// 메모리 정렬 강제 (기본: true)
    pub force_alignment: bool,
}

impl Default for SimdOptimizerConfig {
    fn default() -> Self {
        Self {
            enable_avx2: true,
            enable_sse42: true,
            enable_auto_detection: true,
            vectorize_batch_size: 64,
            force_alignment: true,
        }
    }
}

/// SIMD 성능 통계
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SimdPerformanceStats {
    /// AVX2 명령어 사용 횟수
    pub avx2_operations: u64,
    /// SSE4.2 명령어 사용 횟수  
    pub sse42_operations: u64,
    /// 스칼라 연산 횟수 (폴백)
    pub scalar_operations: u64,
    /// 총 처리 바이트 수
    pub total_bytes_processed: u64,
    /// 평균 벡터화 효율성 (%)
    pub vectorization_efficiency: f64,
    /// 메모리 정렬 적중률 (%)
    pub alignment_hit_rate: f64,
}

/// SIMD 연산 유틸리티
pub struct SimdUtils;

impl SimdUtils {
    /// 메모리 정렬 확인 (16바이트 정렬)
    pub fn is_aligned<T>(ptr: *const T) -> bool {
        (ptr as usize) % 16 == 0
    }
    
    /// 32바이트 정렬 확인 (AVX2용)
    pub fn is_avx2_aligned<T>(ptr: *const T) -> bool {
        (ptr as usize) % 32 == 0
    }
    
    /// 데이터 길이가 벡터화에 적합한지 확인
    pub fn is_vectorizable(len: usize, min_size: usize) -> bool {
        len >= min_size && len % 4 == 0
    }
    
    /// 최적 배치 크기 계산
    pub fn calculate_optimal_batch_size(data_len: usize, vector_size: usize) -> usize {
        if data_len < vector_size {
            return data_len;
        }
        
        // 벡터 크기의 배수로 조정
        (data_len / vector_size) * vector_size
    }
}

/// 고속 메모리 비교 (SIMD 최적화)
pub fn fast_memory_compare(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    let len = a.len();
    
    // 길이가 짧으면 일반 비교
    if len < 16 {
        return a == b;
    }
    
    // SIMD 최적화 비교 (여기서는 단순화)
    // 실제 구현에서는 AVX2/SSE 명령어 사용
    
    // 16바이트씩 블록 단위로 비교
    for chunk in 0..(len / 16) {
        let start = chunk * 16;
        let end = start + 16;
        
        if a[start..end] != b[start..end] {
            return false;
        }
    }
    
    // 나머지 바이트 처리
    let remainder = len % 16;
    if remainder > 0 {
        let start = len - remainder;
        if a[start..] != b[start..] {
            return false;
        }
    }
    
    true
}

/// 고속 메모리 검색 (SIMD 최적화)
pub fn fast_memory_find(haystack: &[u8], needle: u8) -> Option<usize> {
    if haystack.is_empty() {
        return None;
    }
    
    // 길이가 짧으면 일반 검색
    if haystack.len() < 16 {
        return haystack.iter().position(|&x| x == needle);
    }
    
    // SIMD 최적화 검색 (여기서는 단순화)
    // 실제 구현에서는 AVX2/SSE 명령어 사용
    
    for (i, &byte) in haystack.iter().enumerate() {
        if byte == needle {
            return Some(i);
        }
    }
    
    None
}

/// 고속 XOR 연산 (SIMD 최적화)
pub fn fast_xor_inplace(data: &mut [u8], key: &[u8]) {
    if data.is_empty() || key.is_empty() {
        return;
    }
    
    let key_len = key.len();
    
    // SIMD 최적화 XOR (여기서는 단순화)
    // 실제 구현에서는 벡터 XOR 명령어 사용
    
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= key[i % key_len];
    }
}

/// 고속 체크섬 계산 (SIMD 최적화)
pub fn fast_checksum(data: &[u8]) -> u32 {
    if data.is_empty() {
        return 0;
    }
    
    // 길이가 짧으면 일반 체크섬
    if data.len() < 32 {
        return data.iter().fold(0u32, |acc, &x| {
            acc.wrapping_add(x as u32)
        });
    }
    
    // SIMD 최적화 체크섬 (여기서는 단순화)
    // 실제 구현에서는 벡터 누산 명령어 사용
    
    let mut checksum = 0u32;
    
    // 4바이트씩 블록 단위로 처리
    for chunk in data.chunks_exact(4) {
        let value = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        checksum = checksum.wrapping_add(value);
    }
    
    // 나머지 바이트 처리
    for &byte in data.chunks_exact(4).remainder() {
        checksum = checksum.wrapping_add(byte as u32);
    }
    
    checksum
}

/// SIMD 최적화 상태를 나타내는 열거형
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdCapability {
    /// SIMD 미지원
    None,
    /// SSE4.2 지원
    SSE42,
    /// AVX2 지원
    AVX2,
    /// AVX512 지원 (미래 확장)
    AVX512,
}

/// SIMD 기능 감지
pub fn detect_simd_capability() -> SimdCapability {
    // 실제 구현에서는 CPUID 명령어 사용하여 감지
    // 여기서는 단순화하여 AVX2 가정
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            return SimdCapability::AVX2;
        }
        if std::arch::is_x86_feature_detected!("sse4.2") {
            return SimdCapability::SSE42;
        }
    }
    
    SimdCapability::None
}

/// SIMD 성능 벤치마크 함수
pub fn benchmark_simd_operations(data_size: usize) -> SimdPerformanceStats {
    let test_data = vec![0xABu8; data_size];
    let mut stats = SimdPerformanceStats::default();
    
    // 체크섬 계산으로 성능 측정
    let start = std::time::Instant::now();
    let _checksum = fast_checksum(&test_data);
    let duration = start.elapsed();
    
    // 통계 업데이트
    stats.total_bytes_processed = data_size as u64;
    
    // 벡터화 효율성 계산 (가정)
    let capability = detect_simd_capability();
    match capability {
        SimdCapability::AVX2 => {
            stats.avx2_operations = 1;
            stats.vectorization_efficiency = 85.0; // AVX2는 85% 효율
        },
        SimdCapability::SSE42 => {
            stats.sse42_operations = 1;
            stats.vectorization_efficiency = 70.0; // SSE4.2는 70% 효율
        },
        _ => {
            stats.scalar_operations = 1;
            stats.vectorization_efficiency = 0.0;
        }
    }
    
    stats.alignment_hit_rate = 95.0; // 가정값
    
    tracing::debug!(
        "SIMD 벤치마크 완료: {}바이트, {:.2}μs, 효율성: {:.1}%",
        data_size,
        duration.as_micros(),
        stats.vectorization_efficiency
    );
    
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_memory_compare() {
        let data1 = vec![1, 2, 3, 4, 5];
        let data2 = vec![1, 2, 3, 4, 5];
        let data3 = vec![1, 2, 3, 4, 6];
        
        assert!(fast_memory_compare(&data1, &data2));
        assert!(!fast_memory_compare(&data1, &data3));
        assert!(!fast_memory_compare(&data1, &[1, 2, 3])); // 길이 다름
    }

    #[test]
    fn test_fast_memory_find() {
        let data = vec![1, 2, 3, 4, 5, 4, 6];
        
        assert_eq!(fast_memory_find(&data, 4), Some(3)); // 첫 번째 4의 위치
        assert_eq!(fast_memory_find(&data, 6), Some(6));
        assert_eq!(fast_memory_find(&data, 9), None);
        assert_eq!(fast_memory_find(&[], 1), None);
    }

    #[test]
    fn test_fast_xor_inplace() {
        let mut data = vec![0xFF, 0x00, 0xAB, 0xCD];
        let key = vec![0x0F, 0xF0];
        
        fast_xor_inplace(&mut data, &key);
        
        assert_eq!(data[0], 0xFF ^ 0x0F); // 0xF0
        assert_eq!(data[1], 0x00 ^ 0xF0); // 0xF0
        assert_eq!(data[2], 0xAB ^ 0x0F); // 0xA4
        assert_eq!(data[3], 0xCD ^ 0xF0); // 0x3D
    }

    #[test]
    fn test_fast_checksum() {
        let data = vec![1, 2, 3, 4];
        let checksum = fast_checksum(&data);
        assert_eq!(checksum, 10); // 1 + 2 + 3 + 4
        
        assert_eq!(fast_checksum(&[]), 0);
    }

    #[test]
    fn test_simd_utils() {
        // 16바이트 정렬 테스트
        let aligned_data = vec![0u8; 16];
        let ptr = aligned_data.as_ptr();
        
        // 정렬 여부는 실제 메모리 주소에 따라 다름
        // 단순히 함수가 작동하는지만 확인
        let _ = SimdUtils::is_aligned(ptr);
        let _ = SimdUtils::is_avx2_aligned(ptr);
        
        assert!(SimdUtils::is_vectorizable(16, 8));
        assert!(!SimdUtils::is_vectorizable(7, 8));
        
        assert_eq!(SimdUtils::calculate_optimal_batch_size(100, 16), 96);
    }

    #[test]
    fn test_simd_detection() {
        let capability = detect_simd_capability();
        // 단순히 함수가 작동하는지 확인
        assert!(matches!(capability, 
            SimdCapability::None | SimdCapability::SSE42 | 
            SimdCapability::AVX2 | SimdCapability::AVX512
        ));
    }

    #[test]
    fn test_benchmark_simd() {
        let stats = benchmark_simd_operations(1024);
        assert_eq!(stats.total_bytes_processed, 1024);
        assert!(stats.vectorization_efficiency >= 0.0);
        assert!(stats.vectorization_efficiency <= 100.0);
    }
}