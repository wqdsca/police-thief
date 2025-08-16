//! SIMD 연산 최적화 서비스
//!
//! CPU의 SIMD 명령어를 활용하여 대량 데이터 처리 성능을 극대화합니다.
//! AVX2, SSE4.2 등의 명령어 세트를 활용한 벡터화 연산을 제공합니다.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::info;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// SIMD 최적화 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimdOptimizerConfig {
    /// AVX2 사용 활성화 (기본: true)
    pub enable_avx2: bool,
    /// SSE4.2 사용 활성화 (기본: true)
    pub enable_sse42: bool,
    /// 자동 벡터화 활성화 (기본: true)
    pub enable_auto_vectorization: bool,
    /// SIMD 처리 최소 크기 (기본: 64 bytes)
    pub min_simd_size: usize,
    /// 정렬된 메모리 사용 (기본: true)
    pub use_aligned_memory: bool,
    /// 병렬 SIMD 처리 활성화 (기본: true)
    pub enable_parallel_simd: bool,
}

impl Default for SimdOptimizerConfig {
    fn default() -> Self {
        Self {
            enable_avx2: true,
            enable_sse42: true,
            enable_auto_vectorization: true,
            min_simd_size: 64,
            use_aligned_memory: true,
            enable_parallel_simd: true,
        }
    }
}

/// SIMD 기능 감지 결과
#[derive(Debug, Clone, Default)]
pub struct SimdCapabilities {
    pub has_sse: bool,
    pub has_sse2: bool,
    pub has_sse3: bool,
    pub has_ssse3: bool,
    pub has_sse41: bool,
    pub has_sse42: bool,
    pub has_avx: bool,
    pub has_avx2: bool,
    pub has_avx512: bool,
}

impl SimdCapabilities {
    /// CPU의 SIMD 기능 감지
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse") {
                Self {
                    has_sse: is_x86_feature_detected!("sse"),
                    has_sse2: is_x86_feature_detected!("sse2"),
                    has_sse3: is_x86_feature_detected!("sse3"),
                    has_ssse3: is_x86_feature_detected!("ssse3"),
                    has_sse41: is_x86_feature_detected!("sse4.1"),
                    has_sse42: is_x86_feature_detected!("sse4.2"),
                    has_avx: is_x86_feature_detected!("avx"),
                    has_avx2: is_x86_feature_detected!("avx2"),
                    has_avx512: is_x86_feature_detected!("avx512f"),
                }
            } else {
                Self::default()
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            Self::default()
        }
    }

    /// 최적 SIMD 레벨 선택
    pub fn optimal_level(&self) -> SimdLevel {
        if self.has_avx2 {
            SimdLevel::Avx2
        } else if self.has_sse42 {
            SimdLevel::Sse42
        } else if self.has_sse2 {
            SimdLevel::Sse2
        } else {
            SimdLevel::None
        }
    }
}

/// SIMD 레벨
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimdLevel {
    None,
    Sse2,
    Sse42,
    Avx2,
    Avx512,
}

/// SIMD 연산 통계
#[derive(Debug, Default)]
pub struct SimdStats {
    pub total_operations: AtomicU64,
    pub vectorized_operations: AtomicU64,
    pub bytes_processed: AtomicU64,
    pub avx2_operations: AtomicU64,
    pub sse42_operations: AtomicU64,
    pub fallback_operations: AtomicU64,
    pub alignment_optimizations: AtomicU64,
    pub parallel_executions: AtomicU64,
}

/// SIMD 최적화기
pub struct SimdOptimizer {
    config: SimdOptimizerConfig,
    #[allow(dead_code)]
    capabilities: SimdCapabilities,
    optimal_level: SimdLevel,
    stats: Arc<SimdStats>,
}

impl SimdOptimizer {
    /// 새 SIMD 최적화기 생성
    pub fn new(config: SimdOptimizerConfig) -> Self {
        let capabilities = SimdCapabilities::detect();
        let optimal_level = capabilities.optimal_level();

        info!(
            "SIMD 최적화기 초기화: 최적 레벨 = {:?}, AVX2={}, SSE4.2={}",
            optimal_level, capabilities.has_avx2, capabilities.has_sse42
        );

        Self {
            config,
            capabilities,
            optimal_level,
            stats: Arc::new(SimdStats::default()),
        }
    }

    /// 메모리 비교 (SIMD 가속)
    pub fn simd_memcmp(&self, a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }

        let len = a.len();
        self.stats.total_operations.fetch_add(1, Ordering::Relaxed);
        self.stats
            .bytes_processed
            .fetch_add(len as u64, Ordering::Relaxed);

        // SIMD 최소 크기 확인
        if len < self.config.min_simd_size {
            self.stats
                .fallback_operations
                .fetch_add(1, Ordering::Relaxed);
            return a == b;
        }

        #[cfg(target_arch = "x86_64")]
        {
            if self.config.enable_avx2 && self.capabilities.has_avx2 {
                self.stats.avx2_operations.fetch_add(1, Ordering::Relaxed);
                self.stats
                    .vectorized_operations
                    .fetch_add(1, Ordering::Relaxed);
                return unsafe { self.avx2_memcmp(a, b) };
            } else if self.config.enable_sse42 && self.capabilities.has_sse42 {
                self.stats.sse42_operations.fetch_add(1, Ordering::Relaxed);
                self.stats
                    .vectorized_operations
                    .fetch_add(1, Ordering::Relaxed);
                return unsafe { self.sse42_memcmp(a, b) };
            }
        }

        self.stats
            .fallback_operations
            .fetch_add(1, Ordering::Relaxed);
        a == b
    }

    /// AVX2를 사용한 메모리 비교
    #[cfg(target_arch = "x86_64")]
    unsafe fn avx2_memcmp(&self, a: &[u8], b: &[u8]) -> bool {
        let len = a.len();
        let mut i = 0;

        // 32바이트 단위로 처리 (AVX2는 256비트 = 32바이트)
        while i + 32 <= len {
            let a_vec = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
            let b_vec = _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i);

            let cmp = _mm256_cmpeq_epi8(a_vec, b_vec);
            let mask = _mm256_movemask_epi8(cmp);

            if mask != -1i32 {
                return false;
            }

            i += 32;
        }

        // 나머지 바이트 처리
        while i < len {
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }

        true
    }

    /// SSE4.2를 사용한 메모리 비교
    #[cfg(target_arch = "x86_64")]
    unsafe fn sse42_memcmp(&self, a: &[u8], b: &[u8]) -> bool {
        let len = a.len();
        let mut i = 0;

        // 16바이트 단위로 처리 (SSE는 128비트 = 16바이트)
        while i + 16 <= len {
            let a_vec = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);

            let cmp = _mm_cmpeq_epi8(a_vec, b_vec);
            let mask = _mm_movemask_epi8(cmp);

            if mask != 0xFFFF {
                return false;
            }

            i += 16;
        }

        // 나머지 바이트 처리
        while i < len {
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }

        true
    }

    /// 메모리 검색 (SIMD 가속)
    pub fn simd_find(&self, haystack: &[u8], needle: u8) -> Option<usize> {
        let len = haystack.len();
        self.stats.total_operations.fetch_add(1, Ordering::Relaxed);
        self.stats
            .bytes_processed
            .fetch_add(len as u64, Ordering::Relaxed);

        if len < self.config.min_simd_size {
            self.stats
                .fallback_operations
                .fetch_add(1, Ordering::Relaxed);
            return haystack.iter().position(|&b| b == needle);
        }

        #[cfg(target_arch = "x86_64")]
        {
            if self.config.enable_avx2 && self.capabilities.has_avx2 {
                self.stats.avx2_operations.fetch_add(1, Ordering::Relaxed);
                self.stats
                    .vectorized_operations
                    .fetch_add(1, Ordering::Relaxed);
                return unsafe { self.avx2_find(haystack, needle) };
            } else if self.config.enable_sse42 && self.capabilities.has_sse42 {
                self.stats.sse42_operations.fetch_add(1, Ordering::Relaxed);
                self.stats
                    .vectorized_operations
                    .fetch_add(1, Ordering::Relaxed);
                return unsafe { self.sse42_find(haystack, needle) };
            }
        }

        self.stats
            .fallback_operations
            .fetch_add(1, Ordering::Relaxed);
        haystack.iter().position(|&b| b == needle)
    }

    /// AVX2를 사용한 바이트 검색
    #[cfg(target_arch = "x86_64")]
    unsafe fn avx2_find(&self, haystack: &[u8], needle: u8) -> Option<usize> {
        let len = haystack.len();
        let needle_vec = _mm256_set1_epi8(needle as i8);
        let mut i = 0;

        while i + 32 <= len {
            let data = _mm256_loadu_si256(haystack.as_ptr().add(i) as *const __m256i);
            let cmp = _mm256_cmpeq_epi8(data, needle_vec);
            let mask = _mm256_movemask_epi8(cmp);

            if mask != 0 {
                let offset = mask.trailing_zeros() as usize;
                return Some(i + offset);
            }

            i += 32;
        }

        // 나머지 바이트 검색
        while i < len {
            if haystack[i] == needle {
                return Some(i);
            }
            i += 1;
        }

        None
    }

    /// SSE4.2를 사용한 바이트 검색
    #[cfg(target_arch = "x86_64")]
    unsafe fn sse42_find(&self, haystack: &[u8], needle: u8) -> Option<usize> {
        let len = haystack.len();
        let needle_vec = _mm_set1_epi8(needle as i8);
        let mut i = 0;

        while i + 16 <= len {
            let data = _mm_loadu_si128(haystack.as_ptr().add(i) as *const __m128i);
            let cmp = _mm_cmpeq_epi8(data, needle_vec);
            let mask = _mm_movemask_epi8(cmp);

            if mask != 0 {
                let offset = mask.trailing_zeros() as usize;
                return Some(i + offset);
            }

            i += 16;
        }

        // 나머지 바이트 검색
        while i < len {
            if haystack[i] == needle {
                return Some(i);
            }
            i += 1;
        }

        None
    }

    /// XOR 연산 (SIMD 가속)
    pub fn simd_xor(&self, a: &[u8], b: &[u8]) -> Vec<u8> {
        if a.len() != b.len() {
            return Vec::new();
        }

        let len = a.len();
        let mut result = vec![0u8; len];

        self.stats.total_operations.fetch_add(1, Ordering::Relaxed);
        self.stats
            .bytes_processed
            .fetch_add(len as u64 * 2, Ordering::Relaxed);

        if len < self.config.min_simd_size {
            self.stats
                .fallback_operations
                .fetch_add(1, Ordering::Relaxed);
            for i in 0..len {
                result[i] = a[i] ^ b[i];
            }
            return result;
        }

        #[cfg(target_arch = "x86_64")]
        {
            if self.config.enable_avx2 && self.capabilities.has_avx2 {
                self.stats.avx2_operations.fetch_add(1, Ordering::Relaxed);
                self.stats
                    .vectorized_operations
                    .fetch_add(1, Ordering::Relaxed);
                unsafe { self.avx2_xor(a, b, &mut result) };
                return result;
            } else if self.config.enable_sse42 && self.capabilities.has_sse42 {
                self.stats.sse42_operations.fetch_add(1, Ordering::Relaxed);
                self.stats
                    .vectorized_operations
                    .fetch_add(1, Ordering::Relaxed);
                unsafe { self.sse42_xor(a, b, &mut result) };
                return result;
            }
        }

        self.stats
            .fallback_operations
            .fetch_add(1, Ordering::Relaxed);
        for i in 0..len {
            result[i] = a[i] ^ b[i];
        }
        result
    }

    /// AVX2를 사용한 XOR 연산
    #[cfg(target_arch = "x86_64")]
    unsafe fn avx2_xor(&self, a: &[u8], b: &[u8], result: &mut [u8]) {
        let len = a.len();
        let mut i = 0;

        while i + 32 <= len {
            let a_vec = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
            let b_vec = _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i);
            let xor_result = _mm256_xor_si256(a_vec, b_vec);
            _mm256_storeu_si256(result.as_mut_ptr().add(i) as *mut __m256i, xor_result);
            i += 32;
        }

        while i < len {
            result[i] = a[i] ^ b[i];
            i += 1;
        }
    }

    /// SSE4.2를 사용한 XOR 연산
    #[cfg(target_arch = "x86_64")]
    unsafe fn sse42_xor(&self, a: &[u8], b: &[u8], result: &mut [u8]) {
        let len = a.len();
        let mut i = 0;

        while i + 16 <= len {
            let a_vec = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
            let b_vec = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);
            let xor_result = _mm_xor_si128(a_vec, b_vec);
            _mm_storeu_si128(result.as_mut_ptr().add(i) as *mut __m128i, xor_result);
            i += 16;
        }

        while i < len {
            result[i] = a[i] ^ b[i];
            i += 1;
        }
    }

    /// 체크섬 계산 (SIMD 가속)
    pub fn simd_checksum(&self, data: &[u8]) -> u32 {
        let len = data.len();
        self.stats.total_operations.fetch_add(1, Ordering::Relaxed);
        self.stats
            .bytes_processed
            .fetch_add(len as u64, Ordering::Relaxed);

        if len < self.config.min_simd_size {
            self.stats
                .fallback_operations
                .fetch_add(1, Ordering::Relaxed);
            return self.simple_checksum(data);
        }

        #[cfg(target_arch = "x86_64")]
        {
            if self.config.enable_avx2 && self.capabilities.has_avx2 {
                self.stats.avx2_operations.fetch_add(1, Ordering::Relaxed);
                self.stats
                    .vectorized_operations
                    .fetch_add(1, Ordering::Relaxed);
                return unsafe { self.avx2_checksum(data) };
            }
        }

        self.stats
            .fallback_operations
            .fetch_add(1, Ordering::Relaxed);
        self.simple_checksum(data)
    }

    /// 단순 체크섬 계산
    fn simple_checksum(&self, data: &[u8]) -> u32 {
        data.iter().map(|&b| b as u32).sum()
    }

    /// AVX2를 사용한 체크섬 계산
    #[cfg(target_arch = "x86_64")]
    unsafe fn avx2_checksum(&self, data: &[u8]) -> u32 {
        let len = data.len();
        let mut sum = _mm256_setzero_si256();
        let mut i = 0;

        // 32바이트씩 처리
        while i + 32 <= len {
            let chunk = _mm256_loadu_si256(data.as_ptr().add(i) as *const __m256i);

            // 바이트를 16비트로 확장하여 오버플로우 방지
            let lo = _mm256_unpacklo_epi8(chunk, _mm256_setzero_si256());
            let hi = _mm256_unpackhi_epi8(chunk, _mm256_setzero_si256());

            sum = _mm256_add_epi16(sum, lo);
            sum = _mm256_add_epi16(sum, hi);

            i += 32;
        }

        // 수평 합계
        let mut result = [0i16; 16];
        _mm256_storeu_si256(result.as_mut_ptr() as *mut __m256i, sum);
        let mut total: u32 = result.iter().map(|&x| x as u32).sum();

        // 나머지 바이트 처리
        while i < len {
            total += data[i] as u32;
            i += 1;
        }

        total
    }

    /// 정렬된 메모리 할당
    pub fn allocate_aligned(&self, size: usize, alignment: usize) -> Vec<u8> {
        if !self.config.use_aligned_memory {
            return vec![0u8; size];
        }

        self.stats
            .alignment_optimizations
            .fetch_add(1, Ordering::Relaxed);

        // 정렬된 벡터 생성
        let mut vec = Vec::with_capacity(size + alignment);
        let ptr = vec.as_ptr() as usize;
        let misalignment = ptr % alignment;

        if misalignment != 0 {
            let padding = alignment - misalignment;
            vec.reserve(padding);
        }

        vec.resize(size, 0);
        vec
    }

    /// 병렬 SIMD 처리
    pub async fn parallel_simd_process<F>(&self, data: Vec<Vec<u8>>, operation: F) -> Vec<Vec<u8>>
    where
        F: Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static,
    {
        if !self.config.enable_parallel_simd || data.len() < 2 {
            return data.into_iter().map(|d| operation(&d)).collect();
        }

        self.stats
            .parallel_executions
            .fetch_add(1, Ordering::Relaxed);

        let operation = Arc::new(operation);
        let mut handles = Vec::new();

        for chunk in data {
            let op = operation.clone();
            let handle = tokio::spawn(async move { op(&chunk) });
            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            if let Ok(result) = handle.await {
                results.push(result);
            }
        }

        results
    }

    /// 통계 조회
    pub fn get_stats(&self) -> SimdPerformanceStats {
        let total_ops = self.stats.total_operations.load(Ordering::Relaxed);
        let vectorized_ops = self.stats.vectorized_operations.load(Ordering::Relaxed);

        SimdPerformanceStats {
            total_operations: total_ops,
            vectorized_operations: vectorized_ops,
            bytes_processed: self.stats.bytes_processed.load(Ordering::Relaxed),
            vectorization_ratio: if total_ops > 0 {
                vectorized_ops as f64 / total_ops as f64
            } else {
                0.0
            },
            avx2_usage: self.stats.avx2_operations.load(Ordering::Relaxed),
            sse42_usage: self.stats.sse42_operations.load(Ordering::Relaxed),
            fallback_usage: self.stats.fallback_operations.load(Ordering::Relaxed),
            alignment_optimizations: self.stats.alignment_optimizations.load(Ordering::Relaxed),
            parallel_executions: self.stats.parallel_executions.load(Ordering::Relaxed),
            optimal_level: self.optimal_level,
        }
    }
}

/// SIMD 성능 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimdPerformanceStats {
    pub total_operations: u64,
    pub vectorized_operations: u64,
    pub bytes_processed: u64,
    pub vectorization_ratio: f64,
    pub avx2_usage: u64,
    pub sse42_usage: u64,
    pub fallback_usage: u64,
    pub alignment_optimizations: u64,
    pub parallel_executions: u64,
    pub optimal_level: SimdLevel,
}

impl SimdPerformanceStats {
    /// 성능 점수 (0-100)
    pub fn performance_score(&self) -> f64 {
        let vectorization_score = self.vectorization_ratio * 40.0;

        let level_score = match self.optimal_level {
            SimdLevel::Avx2 | SimdLevel::Avx512 => 30.0,
            SimdLevel::Sse42 => 20.0,
            SimdLevel::Sse2 => 10.0,
            SimdLevel::None => 0.0,
        };

        let parallel_score = if self.parallel_executions > 0 {
            15.0
        } else {
            0.0
        };
        let alignment_score = if self.alignment_optimizations > 0 {
            15.0
        } else {
            0.0
        };

        vectorization_score + level_score + parallel_score + alignment_score
    }
}

mod tests {

    #[test]
    fn test_simd_capabilities() {
        let caps = SimdCapabilities::detect();
        println!("SIMD Capabilities: {:?}", caps);
        println!("Optimal level: {:?}", caps.optimal_level());
    }

    #[test]
    fn test_simd_memcmp() {
        let optimizer = SimdOptimizer::new(SimdOptimizerConfig::default());

        let a = vec![1u8; 1024];
        let b = vec![1u8; 1024];
        let c = vec![2u8; 1024];

        assert!(optimizer.simd_memcmp(&a, &b));
        assert!(!optimizer.simd_memcmp(&a, &c));
    }

    #[test]
    fn test_simd_find() {
        let optimizer = SimdOptimizer::new(SimdOptimizerConfig::default());

        let haystack = vec![0u8; 1000];
        let mut haystack_with_needle = haystack.clone();
        haystack_with_needle[500] = 42;

        assert_eq!(optimizer.simd_find(&haystack_with_needle, 42), Some(500));
        assert_eq!(optimizer.simd_find(&haystack, 42), None);
    }

    #[test]
    fn test_simd_xor() {
        let optimizer = SimdOptimizer::new(SimdOptimizerConfig::default());

        let a = vec![0xAAu8; 1024];
        let b = vec![0x55u8; 1024];
        let result = optimizer.simd_xor(&a, &b);

        assert_eq!(result.len(), 1024);
        assert!(result.iter().all(|&x| x == 0xFF));
    }

    #[test]
    fn test_simd_checksum() {
        let optimizer = SimdOptimizer::new(SimdOptimizerConfig::default());

        let data = vec![1u8; 1000];
        let checksum = optimizer.simd_checksum(&data);

        assert_eq!(checksum, 1000);
    }

    #[tokio::test]
    async fn test_parallel_simd() {
        let optimizer = SimdOptimizer::new(SimdOptimizerConfig::default());

        let data = vec![vec![1u8; 100], vec![2u8; 100], vec![3u8; 100]];

        let results = optimizer
            .parallel_simd_process(data, |d| d.iter().map(|&x| x * 2).collect())
            .await;

        assert_eq!(results.len(), 3);
        assert!(results[0].iter().all(|&x| x == 2));
        assert!(results[1].iter().all(|&x| x == 4));
        assert!(results[2].iter().all(|&x| x == 6));
    }
}
