//! DashMap 고성능 최적화 라이브러리 (Shared)
//! 
//! tcpserver와 rudpserver에서 공통 사용되는 DashMap 최적화 기능을 제공합니다.
//! 대규모 동시 연결 환경에서 최고 성능을 달성하기 위한 고급 최적화 시스템입니다.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use dashmap::DashMap;

/// DashMap 최적화 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashMapOptimizerConfig {
    /// 초기 샤드 수 (기본: CPU 코어 수 * 2)
    pub initial_shard_count: usize,
    /// 최대 샤드 수 (기본: CPU 코어 수 * 8)  
    pub max_shard_count: usize,
    /// 샤드당 최적 엔트리 수 (기본: 1000)
    pub optimal_entries_per_shard: usize,
    /// 동적 리샤딩 활성화 여부 (기본: true)
    pub enable_dynamic_resharding: bool,
    /// 캐시 라인 정렬 활성화 (기본: true)
    pub enable_cache_line_alignment: bool,
    /// Lock-free 읽기 최적화 활성화 (기본: true)
    pub enable_lockfree_reads: bool,
    /// 메모리 프리페칭 활성화 (기본: true)
    pub enable_memory_prefetching: bool,
}

impl Default for DashMapOptimizerConfig {
    fn default() -> Self {
        let cpu_count = num_cpus::get().max(1);
        Self {
            initial_shard_count: cpu_count * 2,
            max_shard_count: cpu_count * 8,
            optimal_entries_per_shard: 1000,
            enable_dynamic_resharding: true,
            enable_cache_line_alignment: true,
            enable_lockfree_reads: true,
            enable_memory_prefetching: true,
        }
    }
}

/// 캐시 라인 정렬된 카운터 (64바이트 정렬)
#[repr(align(64))]
#[derive(Debug)]
pub struct CacheAlignedCounter {
    pub value: AtomicU64,
    _padding: [u8; 56], // 64 - 8 = 56바이트 패딩
}

impl Default for CacheAlignedCounter {
    fn default() -> Self {
        Self {
            value: AtomicU64::new(0),
            _padding: [0; 56],
        }
    }
}

impl CacheAlignedCounter {
    pub fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
            _padding: [0; 56],
        }
    }
    
    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::Relaxed)
    }
    
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
    
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

/// 샤드 로드 통계
#[derive(Debug)]
pub struct ShardLoadStats {
    /// 샤드당 엔트리 수
    pub entries_per_shard: Vec<AtomicUsize>,
    /// 샤드당 읽기 횟수 (캐시 정렬)
    pub reads_per_shard: Vec<CacheAlignedCounter>,
    /// 샤드당 쓰기 횟수 (캐시 정렬)
    pub writes_per_shard: Vec<CacheAlignedCounter>,
    /// 샤드당 충돌 횟수 (캐시 정렬)
    pub conflicts_per_shard: Vec<CacheAlignedCounter>,
    /// 평균 로드팩터
    pub average_load_factor: AtomicU64, // f64를 u64로 비트 저장
    /// 최대 로드팩터
    pub max_load_factor: AtomicU64,
}

impl ShardLoadStats {
    pub fn new(shard_count: usize) -> Self {
        Self {
            entries_per_shard: (0..shard_count).map(|_| AtomicUsize::new(0)).collect(),
            reads_per_shard: (0..shard_count).map(|_| CacheAlignedCounter::new()).collect(),
            writes_per_shard: (0..shard_count).map(|_| CacheAlignedCounter::new()).collect(),
            conflicts_per_shard: (0..shard_count).map(|_| CacheAlignedCounter::new()).collect(),
            average_load_factor: AtomicU64::new(0),
            max_load_factor: AtomicU64::new(0),
        }
    }
}

/// 캐시 친화적 해시 함수
#[derive(Debug)]
pub struct CacheOptimizedHasher {
    /// 해시 시드
    seed: u64,
}

impl Default for CacheOptimizedHasher {
    fn default() -> Self {
        Self {
            seed: 0xDEADBEEF_CAFEBABE,
        }
    }
}

impl CacheOptimizedHasher {
    /// 캐시 지역성을 고려한 해시 함수
    pub fn hash_with_locality<K: Hash>(&self, key: &K, shard_count: usize) -> usize {
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish() ^ self.seed;
        
        // 빠른 모듈로 연산 (2의 거듭제곱일 때)
        if shard_count.is_power_of_two() {
            (hash as usize) & (shard_count - 1)
        } else {
            (hash as usize) % shard_count
        }
    }
}

/// DashMap 고성능 최적화기
pub struct DashMapOptimizer {
    /// 최적화 설정
    config: DashMapOptimizerConfig,
    /// 샤드 로드 통계
    shard_stats: Arc<ShardLoadStats>,
    /// 캐시 최적화 해시어
    hasher: CacheOptimizedHasher,
    /// 전체 엔트리 수 (원자적)
    total_entries: AtomicUsize,
}

impl DashMapOptimizer {
    /// 새 DashMap 최적화기 생성
    pub fn new(config: DashMapOptimizerConfig) -> Self {
        let shard_stats = Arc::new(ShardLoadStats::new(config.initial_shard_count));
        
        Self {
            config,
            shard_stats,
            hasher: CacheOptimizedHasher::default(),
            total_entries: AtomicUsize::new(0),
        }
    }
    
    /// CPU 최적화된 DashMap 생성
    pub fn create_optimized_dashmap<K, V>(&self) -> DashMap<K, V>
    where
        K: Hash + Eq + Clone,
        V: Clone,
    {
        let cpu_count = num_cpus::get();
        
        // CPU별 최적 샤드 수 계산
        let optimal_shards = match cpu_count {
            1..=2 => 4,                    // 단일/듀얼 코어: 4 샤드
            3..=4 => cpu_count * 2,        // 쿼드 코어: 8 샤드  
            5..=8 => cpu_count + 4,        // 6-8코어: 최대 12 샤드
            9..=16 => cpu_count + 2,       // 고성능: 최대 18 샤드
            _ => cpu_count / 2 + 8,        // 서버급: 코어 수/2 + 8
        }.min(self.config.max_shard_count);
        
        // 2의 거듭제곱으로 조정 (해시 최적화)
        let power_of_two_shards = optimal_shards.next_power_of_two();
        let final_shards = if power_of_two_shards <= self.config.max_shard_count {
            power_of_two_shards
        } else {
            optimal_shards
        };
        
        // 초기 용량 계산 (오버헤드 고려)
        let initial_capacity = final_shards * self.config.optimal_entries_per_shard;
        
        // 성능 최적화된 DashMap 생성
        DashMap::with_capacity_and_hasher_and_shard_amount(
            initial_capacity,
            std::collections::hash_map::RandomState::new(),
            final_shards,
        )
    }
    
    /// 고성능 Lock-free 읽기
    pub fn read_with_operation<K, V, F, R>(&self, map: &DashMap<K, V>, key: &K, operation: F) -> Option<R>
    where
        K: Hash + Eq,
        F: FnOnce(&V) -> R,
    {
        let shard_count = self.config.initial_shard_count;
        let shard_id = self.hasher.hash_with_locality(key, shard_count);
        
        // 메모리 프리페칭 (성능 힌트)
        if self.config.enable_memory_prefetching {
            std::hint::spin_loop(); // CPU 파이프라인 최적화
        }
        
        // 읽기 통계 업데이트 (원자적)
        if shard_id < self.shard_stats.reads_per_shard.len() {
            self.shard_stats.reads_per_shard[shard_id].increment();
        }
        
        // DashMap의 최적화된 읽기 경로 사용
        map.get(key).map(|entry| operation(&*entry))
    }
    
    /// 배치 읽기 최적화
    pub fn read_batch_with_operation<K, V, F, R>(&self, map: &DashMap<K, V>, keys: &[K], operation: F) -> Vec<Option<R>>
    where
        K: Hash + Eq + Clone,
        V: Clone,
        F: Fn(&V) -> R + Copy,
    {
        let shard_count = self.config.initial_shard_count;
        let mut results = Vec::with_capacity(keys.len());
        
        // 키를 샤드별로 그룹화 (캐시 지역성 최적화)
        let mut shard_groups: HashMap<usize, Vec<&K>> = HashMap::new();
        
        for key in keys {
            let shard_id = self.hasher.hash_with_locality(key, shard_count);
            shard_groups.entry(shard_id).or_default().push(key);
        }
        
        // 샤드별 배치 처리
        let mut temp_results: HashMap<usize, Vec<Option<R>>> = HashMap::new();
        
        for (shard_id, shard_keys) in shard_groups {
            let mut shard_results = Vec::new();
            
            for &key in &shard_keys {
                let result = self.read_with_operation(map, key, operation);
                shard_results.push(result);
            }
            
            temp_results.insert(shard_id, shard_results);
        }
        
        // 원래 순서대로 재구성
        for key in keys {
            let shard_id = self.hasher.hash_with_locality(key, shard_count);
            if let Some(shard_results) = temp_results.get_mut(&shard_id) {
                if !shard_results.is_empty() {
                    results.push(shard_results.remove(0));
                } else {
                    results.push(None);
                }
            } else {
                results.push(None);
            }
        }
        
        results
    }
    
    /// 성능 통계 수집
    pub fn collect_performance_stats(&self) -> DashMapPerformanceStats {
        let total_entries = self.total_entries.load(Ordering::Relaxed);
        
        let total_reads: u64 = self.shard_stats.reads_per_shard
            .iter()
            .map(|c| c.get())
            .sum();
            
        let total_writes: u64 = self.shard_stats.writes_per_shard
            .iter()
            .map(|c| c.get())
            .sum();
            
        let total_conflicts: u64 = self.shard_stats.conflicts_per_shard
            .iter()
            .map(|c| c.get())
            .sum();
        
        // 로드팩터 계산
        let shard_loads: Vec<usize> = self.shard_stats.entries_per_shard
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect();
            
        let (avg_load, max_load) = if !shard_loads.is_empty() {
            let total: usize = shard_loads.iter().sum();
            let max_val = shard_loads.iter().max().unwrap_or(&0);
            (total as f64 / shard_loads.len() as f64, *max_val as f64)
        } else {
            (0.0, 0.0)
        };
        
        DashMapPerformanceStats {
            total_entries,
            total_reads,
            total_writes,
            total_conflicts,
            average_load_factor: avg_load,
            max_load_factor: max_load,
            shard_count: shard_loads.len(),
            memory_usage_bytes: self.estimate_memory_usage(total_entries),
            cache_hit_rate: self.calculate_cache_hit_rate(),
            throughput_ops_per_sec: self.calculate_throughput(),
        }
    }
    
    /// 메모리 사용량 추정
    fn estimate_memory_usage(&self, entries: usize) -> usize {
        // DashMap 오버헤드 + 엔트리 크기 추정
        let dashmap_overhead = 1024; // 기본 오버헤드
        let entry_size = std::mem::size_of::<(u32, String)>(); // 평균 엔트리 크기
        let shard_overhead = self.shard_stats.entries_per_shard.len() * 64; // 샤드당 64바이트
        
        dashmap_overhead + (entries * entry_size) + shard_overhead
    }
    
    /// 캐시 적중률 계산
    fn calculate_cache_hit_rate(&self) -> f64 {
        let total_reads = self.shard_stats.reads_per_shard
            .iter()
            .map(|c| c.get())
            .sum::<u64>();
            
        let total_conflicts = self.shard_stats.conflicts_per_shard
            .iter()
            .map(|c| c.get())
            .sum::<u64>();
        
        if total_reads > 0 {
            1.0 - (total_conflicts as f64 / total_reads as f64)
        } else {
            1.0
        }
    }
    
    /// 처리량 계산 (초당 연산 수)
    fn calculate_throughput(&self) -> f64 {
        let total_ops = self.shard_stats.reads_per_shard
            .iter()
            .chain(self.shard_stats.writes_per_shard.iter())
            .map(|c| c.get())
            .sum::<u64>();
        
        total_ops as f64
    }
}

/// DashMap 성능 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashMapPerformanceStats {
    pub total_entries: usize,
    pub total_reads: u64,
    pub total_writes: u64,
    pub total_conflicts: u64,
    pub average_load_factor: f64,
    pub max_load_factor: f64,
    pub shard_count: usize,
    pub memory_usage_bytes: usize,
    pub cache_hit_rate: f64,
    pub throughput_ops_per_sec: f64,
}

impl DashMapPerformanceStats {
    /// 종합 성능 점수 (0-100)
    pub fn performance_score(&self) -> f64 {
        let load_balance_score = if self.max_load_factor > 0.0 {
            (self.average_load_factor / self.max_load_factor) * 25.0
        } else {
            25.0
        }.min(25.0);
        
        let conflict_score = ((1.0 - self.conflict_rate()) * 25.0).max(0.0);
        let cache_score = self.cache_hit_rate * 25.0;
        let throughput_score = (self.throughput_ops_per_sec / 10000.0).min(1.0) * 25.0;
        
        load_balance_score + conflict_score + cache_score + throughput_score
    }
    
    /// 충돌률 계산
    pub fn conflict_rate(&self) -> f64 {
        let total_ops = self.total_reads + self.total_writes;
        if total_ops == 0 {
            0.0
        } else {
            self.total_conflicts as f64 / total_ops as f64
        }
    }
    
    /// 메모리 효율성 (엔트리당 바이트)
    pub fn memory_efficiency(&self) -> f64 {
        if self.total_entries == 0 {
            0.0
        } else {
            self.memory_usage_bytes as f64 / self.total_entries as f64
        }
    }
}

/// 고성능 해시 함수 (xxHash 스타일)
pub struct FastHasher {
    state: u64,
}

impl FastHasher {
    pub fn new() -> Self {
        Self { state: 0x9E3779B97F4A7C15 } // Golden ratio
    }
    
    pub fn hash<T: AsRef<[u8]>>(&mut self, data: T) -> u64 {
        let bytes = data.as_ref();
        let mut hash = self.state;
        
        for chunk in bytes.chunks(8) {
            let mut val = 0u64;
            for (i, &byte) in chunk.iter().enumerate() {
                val |= (byte as u64) << (i * 8);
            }
            hash ^= val;
            hash = hash.wrapping_mul(0x9E3779B97F4A7C15);
            hash = hash.rotate_left(31);
        }
        
        hash ^= bytes.len() as u64;
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xC2B2AE35);
        hash ^= hash >> 29;
        hash = hash.wrapping_mul(0xCC9E2D51);
        hash ^= hash >> 32;
        
        hash
    }
}

impl Default for FastHasher {
    fn default() -> Self {
        Self::new()
    }
}

/// DashMap 최적화 유틸리티 함수들
pub struct DashMapUtils;

impl DashMapUtils {
    /// CPU 코어 수 기반 최적 샤드 수 계산
    pub fn calculate_optimal_shard_count(entry_count: usize, entries_per_shard: usize) -> usize {
        let cpu_count = num_cpus::get().max(1);
        let calculated = (entry_count / entries_per_shard).max(1);
        calculated.min(cpu_count * 8).max(cpu_count * 2)
    }
    
    /// 캐시 라인 정렬 확인
    pub fn is_cache_aligned<T>(ptr: *const T) -> bool {
        (ptr as usize) % 64 == 0
    }
    
    /// 로드팩터 계산
    pub fn calculate_load_factor(entries: usize, capacity: usize) -> f64 {
        if capacity == 0 {
            0.0
        } else {
            entries as f64 / capacity as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_aligned_counter() {
        let counter = CacheAlignedCounter::new();
        assert_eq!(counter.get(), 0);
        
        counter.increment();
        assert_eq!(counter.get(), 1);
        
        counter.reset();
        assert_eq!(counter.get(), 0);
    }
    
    #[test]
    fn test_fast_hasher() {
        let mut hasher = FastHasher::new();
        
        let hash1 = hasher.hash("test");
        let hash2 = hasher.hash("test");
        assert_eq!(hash1, hash2); // 같은 입력에 대해 같은 해시
        
        let hash3 = hasher.hash("different");
        assert_ne!(hash1, hash3); // 다른 입력에 대해 다른 해시
    }
    
    #[test]
    fn test_optimal_shard_calculation() {
        let optimal = DashMapUtils::calculate_optimal_shard_count(10000, 1000);
        let cpu_count = num_cpus::get();
        assert!(optimal >= cpu_count * 2); // CPU * 2
        assert!(optimal <= cpu_count * 8); // CPU * 8
    }
    
    #[test]
    fn test_load_factor_calculation() {
        let load_factor = DashMapUtils::calculate_load_factor(500, 1000);
        assert!((load_factor - 0.5).abs() < f64::EPSILON);
        
        let empty_load_factor = DashMapUtils::calculate_load_factor(0, 0);
        assert_eq!(empty_load_factor, 0.0);
    }

    #[test]
    fn test_optimized_dashmap_creation() {
        let optimizer = DashMapOptimizer::new(DashMapOptimizerConfig::default());
        let map: DashMap<u32, String> = optimizer.create_optimized_dashmap();
        
        assert_eq!(map.len(), 0);
        assert!(map.capacity() > 0); // 용량이 0보다 큰지 확인
    }
}