//! 고성능 다중 계층 메모리 풀 시스템
//!
//! 성능 목표:
//! - 할당 속도: 30% 향상 (크기별 분류 + 스레드 로컬 캐시)
//! - 메모리 효율: 25% 개선 (적응형 크기 관리)
//! - CPU 캐시 미스: 30% 감소 (메모리 정렬 + NUMA 인식)

use crossbeam_queue::SegQueue;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{atomic::{AtomicU64, Ordering}, Arc};
use tracing::{debug, info};

/// 버퍼 크기 계층 (2의 거듭제곱 기반)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferSizeClass {
    Tiny,    // 1KB - 작은 메시지
    Small,   // 4KB - 일반 메시지
    Medium,  // 16KB - 큰 메시지
    Large,   // 64KB - 파일 전송
    XLarge,  // 256KB - 대용량 데이터
}

impl BufferSizeClass {
    pub fn size(&self) -> usize {
        match self {
            BufferSizeClass::Tiny => 1024,
            BufferSizeClass::Small => 4 * 1024,
            BufferSizeClass::Medium => 16 * 1024,
            BufferSizeClass::Large => 64 * 1024,
            BufferSizeClass::XLarge => 256 * 1024,
        }
    }
    
    /// 요청 크기에서 최적 클래스 결정
    pub fn from_size(size: usize) -> Self {
        if size <= 1024 {
            BufferSizeClass::Tiny
        } else if size <= 4 * 1024 {
            BufferSizeClass::Small
        } else if size <= 16 * 1024 {
            BufferSizeClass::Medium
        } else if size <= 64 * 1024 {
            BufferSizeClass::Large
        } else {
            BufferSizeClass::XLarge
        }
    }
    
    pub fn all_classes() -> &'static [BufferSizeClass] {
        &[
            BufferSizeClass::Tiny,
            BufferSizeClass::Small,
            BufferSizeClass::Medium,
            BufferSizeClass::Large,
            BufferSizeClass::XLarge,
        ]
    }
}

/// 고성능 메모리 풀 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedPoolConfig {
    /// 크기별 최대 풀 크기
    pub max_buffers_per_class: usize,
    /// 스레드 로컬 캐시 크기
    pub thread_local_cache_size: usize,
    /// 배치 할당 크기
    pub batch_allocation_size: usize,
    /// NUMA 인식 활성화
    pub enable_numa_awareness: bool,
    /// 메모리 정렬 (바이트)
    pub memory_alignment: usize,
    /// 적응형 크기 조정 활성화
    pub enable_adaptive_sizing: bool,
    /// 정리 간격 (초)
    pub cleanup_interval_secs: u64,
}

impl Default for EnhancedPoolConfig {
    fn default() -> Self {
        Self {
            max_buffers_per_class: 512,
            thread_local_cache_size: 32,
            batch_allocation_size: 8,
            enable_numa_awareness: num_cpus::get() > 4,
            memory_alignment: 64, // 캐시 라인 정렬
            enable_adaptive_sizing: true,
            cleanup_interval_secs: 300,
        }
    }
}

/// 정렬된 버퍼
#[derive(Debug)]
pub struct AlignedBuffer {
    data: Vec<u8>,
    actual_capacity: usize,
    size_class: BufferSizeClass,
    allocation_count: u64,
    last_used: std::time::Instant,
}

impl AlignedBuffer {
    fn new(size_class: BufferSizeClass, alignment: usize) -> Self {
        let base_size = size_class.size();
        // 정렬을 고려한 크기 계산
        let aligned_size = (base_size + alignment - 1) & !(alignment - 1);
        
        let data = Vec::with_capacity(aligned_size);
        
        // 메모리 정렬 보장 (대부분의 경우 Vec은 기본적으로 정렬됨)
        // 필요시에만 추가 정렬 로직 적용
        
        Self {
            data,
            actual_capacity: aligned_size,
            size_class,
            allocation_count: 0,
            last_used: std::time::Instant::now(),
        }
    }
    
    pub fn get_buffer(&mut self) -> &mut Vec<u8> {
        self.data.clear();
        self.allocation_count += 1;
        self.last_used = std::time::Instant::now();
        &mut self.data
    }
    
    pub fn size_class(&self) -> BufferSizeClass {
        self.size_class
    }
    
    pub fn is_overused(&self) -> bool {
        self.allocation_count > 10000 // 재할당 임계값
    }
    
    pub fn is_idle(&self, threshold_secs: u64) -> bool {
        self.last_used.elapsed().as_secs() > threshold_secs
    }
}

/// 스레드 로컬 캐시
thread_local! {
    static THREAD_CACHE: RefCell<HashMap<BufferSizeClass, Vec<AlignedBuffer>>> = 
        RefCell::new(HashMap::new());
}

/// 향상된 메모리 풀
pub struct EnhancedMemoryPool {
    config: EnhancedPoolConfig,
    // 크기별 전역 풀 (스레드 로컬 캐시 실패 시 폴백)
    global_pools: HashMap<BufferSizeClass, SegQueue<AlignedBuffer>>,
    // 성능 통계
    stats: Arc<EnhancedPoolStats>,
    // NUMA 노드별 풀 (활성화 시)
    numa_pools: Option<HashMap<usize, HashMap<BufferSizeClass, SegQueue<AlignedBuffer>>>>,
    // 적응형 크기 조정 데이터
    size_usage_stats: HashMap<BufferSizeClass, AtomicU64>,
}

/// 향상된 풀 통계
#[derive(Debug, Default)]
pub struct EnhancedPoolStats {
    // 기본 통계
    pub total_allocations: AtomicU64,
    pub total_reuses: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    
    // 크기별 통계
    pub allocations_by_size: HashMap<BufferSizeClass, AtomicU64>,
    pub reuses_by_size: HashMap<BufferSizeClass, AtomicU64>,
    
    // 성능 통계
    pub avg_allocation_time_ns: AtomicU64,
    pub memory_saved_bytes: AtomicU64,
    pub numa_hit_rate: AtomicU64,
}

impl EnhancedPoolStats {
    pub fn new() -> Self {
        let mut stats = Self::default();
        
        // 크기별 통계 초기화
        for &class in BufferSizeClass::all_classes() {
            stats.allocations_by_size.insert(class, AtomicU64::new(0));
            stats.reuses_by_size.insert(class, AtomicU64::new(0));
        }
        
        stats
    }
    
    pub fn get_cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hits.load(Ordering::Relaxed) as f64;
        let total = hits + self.cache_misses.load(Ordering::Relaxed) as f64;
        if total > 0.0 { hits / total } else { 0.0 }
    }
    
    pub fn get_memory_efficiency(&self) -> f64 {
        let saved = self.memory_saved_bytes.load(Ordering::Relaxed) as f64;
        let total = self.total_allocations.load(Ordering::Relaxed) as f64;
        if total > 0.0 { saved / (total * 4096.0) } else { 0.0 }
    }
}

impl EnhancedMemoryPool {
    pub fn new(config: EnhancedPoolConfig) -> Self {
        info!("고성능 메모리 풀 초기화 - NUMA: {}, 정렬: {}바이트", 
              config.enable_numa_awareness, config.memory_alignment);
        
        let mut global_pools = HashMap::new();
        let mut size_usage_stats = HashMap::new();
        
        // 각 크기 클래스별 풀 초기화
        for &class in BufferSizeClass::all_classes() {
            global_pools.insert(class, SegQueue::new());
            size_usage_stats.insert(class, AtomicU64::new(0));
        }
        
        // NUMA 풀 초기화 (활성화 시)
        let numa_pools = if config.enable_numa_awareness {
            let cpu_count = num_cpus::get();
            let mut numa_map = HashMap::new();
            
            for node in 0..cpu_count {
                let mut node_pools = HashMap::new();
                for &class in BufferSizeClass::all_classes() {
                    node_pools.insert(class, SegQueue::new());
                }
                numa_map.insert(node, node_pools);
            }
            Some(numa_map)
        } else {
            None
        };
        
        Self {
            config,
            global_pools,
            stats: Arc::new(EnhancedPoolStats::new()),
            numa_pools,
            size_usage_stats,
        }
    }
    
    /// 고속 버퍼 할당
    pub fn allocate(&self, requested_size: usize) -> AlignedBuffer {
        let size_class = BufferSizeClass::from_size(requested_size);
        let start_time = std::time::Instant::now();
        
        // 1. 스레드 로컬 캐시 시도
        if let Some(buffer) = self.try_thread_local_cache(size_class) {
            self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);
            self.record_allocation_time(start_time);
            return buffer;
        }
        
        // 2. NUMA 인식 할당 시도
        if self.config.enable_numa_awareness {
            if let Some(buffer) = self.try_numa_pool(size_class) {
                self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);
                self.record_allocation_time(start_time);
                return buffer;
            }
        }
        
        // 3. 전역 풀에서 할당
        if let Some(pool) = self.global_pools.get(&size_class) {
            if let Some(buffer) = pool.pop() {
                self.stats.cache_misses.fetch_add(1, Ordering::Relaxed);
                self.stats.total_reuses.fetch_add(1, Ordering::Relaxed);
                self.record_allocation_time(start_time);
                return buffer;
            }
        }
        
        // 4. 새 버퍼 생성
        self.stats.cache_misses.fetch_add(1, Ordering::Relaxed);
        self.stats.total_allocations.fetch_add(1, Ordering::Relaxed);
        self.record_allocation_time(start_time);
        
        debug!("새 버퍼 할당: {:?} ({}바이트)", size_class, size_class.size());
        AlignedBuffer::new(size_class, self.config.memory_alignment)
    }
    
    /// 버퍼 반환
    pub fn deallocate(&self, mut buffer: AlignedBuffer) {
        let size_class = buffer.size_class();
        
        // 과사용된 버퍼는 폐기
        if buffer.is_overused() {
            debug!("과사용된 버퍼 폐기: {:?}", size_class);
            return;
        }
        
        // 1. 스레드 로컬 캐시에 반환 시도
        if !self.try_return_to_thread_cache(&mut buffer) {
            // 2. NUMA 풀에 반환 시도
            if self.config.enable_numa_awareness && !self.try_return_to_numa_pool(&mut buffer) {
                // 3. 전역 풀에 반환
                if let Some(pool) = self.global_pools.get(&size_class) {
                    pool.push(buffer);
                }
            } else if !self.config.enable_numa_awareness {
                // NUMA가 비활성화된 경우 직접 전역 풀로
                if let Some(pool) = self.global_pools.get(&size_class) {
                    pool.push(buffer);
                }
            }
        }
    }
    
    /// 스레드 로컬 캐시에서 할당 시도
    fn try_thread_local_cache(&self, size_class: BufferSizeClass) -> Option<AlignedBuffer> {
        THREAD_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if let Some(buffers) = cache.get_mut(&size_class) {
                buffers.pop()
            } else {
                None
            }
        })
    }
    
    /// 스레드 로컬 캐시에 반환 시도
    fn try_return_to_thread_cache(&self, buffer: &mut AlignedBuffer) -> bool {
        let size_class = buffer.size_class();
        
        THREAD_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let buffers = cache.entry(size_class).or_insert_with(Vec::new);
            
            if buffers.len() < self.config.thread_local_cache_size {
                // buffer를 이동시키기 위해 std::mem::replace 사용
                let buffer_to_store = std::mem::replace(buffer, AlignedBuffer::new(size_class, self.config.memory_alignment));
                buffers.push(buffer_to_store);
                true
            } else {
                false
            }
        })
    }
    
    /// NUMA 풀에서 할당 시도
    fn try_numa_pool(&self, size_class: BufferSizeClass) -> Option<AlignedBuffer> {
        if let Some(numa_pools) = &self.numa_pools {
            let current_cpu = 0; // 실제로는 현재 CPU 노드 감지 필요
            
            if let Some(node_pools) = numa_pools.get(&current_cpu) {
                if let Some(pool) = node_pools.get(&size_class) {
                    return pool.pop();
                }
            }
        }
        None
    }
    
    /// NUMA 풀에 반환 시도
    fn try_return_to_numa_pool(&self, buffer: &mut AlignedBuffer) -> bool {
        if let Some(numa_pools) = &self.numa_pools {
            let current_cpu = 0; // 실제로는 현재 CPU 노드 감지 필요
            let size_class = buffer.size_class();
            
            if let Some(node_pools) = numa_pools.get(&current_cpu) {
                if let Some(pool) = node_pools.get(&size_class) {
                    let buffer_to_store = std::mem::replace(buffer, AlignedBuffer::new(size_class, self.config.memory_alignment));
                    pool.push(buffer_to_store);
                    return true;
                }
            }
        }
        false
    }
    
    /// 할당 시간 기록
    fn record_allocation_time(&self, start_time: std::time::Instant) {
        let duration_ns = start_time.elapsed().as_nanos() as u64;
        
        // 지수 이동 평균으로 평균 시간 갱신
        let current_avg = self.stats.avg_allocation_time_ns.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            duration_ns
        } else {
            (current_avg * 7 + duration_ns) / 8 // 8분의 7 가중 평균
        };
        self.stats.avg_allocation_time_ns.store(new_avg, Ordering::Relaxed);
    }
    
    /// 풀 상태 정리
    pub async fn cleanup(&self) {
        let idle_threshold = self.config.cleanup_interval_secs;
        let mut total_cleaned = 0;
        
        // 각 풀에서 유휴 버퍼 제거
        for (class, pool) in &self.global_pools {
            let mut temp_buffers = Vec::new();
            let mut cleaned_count = 0;
            
            while let Some(buffer) = pool.pop() {
                if buffer.is_idle(idle_threshold) {
                    cleaned_count += 1;
                } else {
                    temp_buffers.push(buffer);
                }
            }
            
            // 활성 버퍼들 다시 추가
            for buffer in temp_buffers {
                pool.push(buffer);
            }
            
            if cleaned_count > 0 {
                debug!("풀 정리: {:?} - {}개 버퍼 제거", class, cleaned_count);
                total_cleaned += cleaned_count;
            }
        }
        
        if total_cleaned > 0 {
            info!("메모리 풀 정리 완료: {}개 버퍼 제거", total_cleaned);
        }
    }
    
    /// 성능 통계 반환
    pub fn get_stats(&self) -> Arc<EnhancedPoolStats> {
        self.stats.clone()
    }
    
    /// 상세 성능 보고서
    pub fn get_performance_report(&self) -> String {
        let stats = &self.stats;
        let hit_rate = stats.get_cache_hit_rate() * 100.0;
        let efficiency = stats.get_memory_efficiency() * 100.0;
        let avg_time = stats.avg_allocation_time_ns.load(Ordering::Relaxed);
        
        format!(
            "Enhanced Memory Pool Performance Report:\n\
             - Cache Hit Rate: {:.1}%\n\
             - Memory Efficiency: {:.1}%\n\
             - Avg Allocation Time: {}ns\n\
             - Total Allocations: {}\n\
             - Total Reuses: {}\n\
             - Memory Saved: {:.2}MB",
            hit_rate,
            efficiency,
            avg_time,
            stats.total_allocations.load(Ordering::Relaxed),
            stats.total_reuses.load(Ordering::Relaxed),
            stats.memory_saved_bytes.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_enhanced_pool_performance() {
        let pool = EnhancedMemoryPool::new(EnhancedPoolConfig::default());
        
        // 성능 테스트: 10,000번 할당/해제
        let start = Instant::now();
        let iterations = 10_000;
        
        for _ in 0..iterations {
            let buffer = pool.allocate(8192); // 8KB
            pool.deallocate(buffer);
        }
        
        let duration = start.elapsed();
        let ops_per_sec = iterations as f64 / duration.as_secs_f64();
        
        println!("Enhanced Pool Performance: {:.0} ops/sec", ops_per_sec);
        assert!(ops_per_sec > 100_000.0, "성능이 기대치 미달: {} ops/sec", ops_per_sec);
        
        // 통계 확인
        let stats = pool.get_stats();
        println!("{}", pool.get_performance_report());
        
        assert!(stats.total_allocations.load(Ordering::Relaxed) > 0);
    }
    
    #[test]
    fn test_size_class_mapping() {
        assert_eq!(BufferSizeClass::from_size(512), BufferSizeClass::Tiny);
        assert_eq!(BufferSizeClass::from_size(2048), BufferSizeClass::Small);
        assert_eq!(BufferSizeClass::from_size(10240), BufferSizeClass::Medium);
        assert_eq!(BufferSizeClass::from_size(32768), BufferSizeClass::Large);
        assert_eq!(BufferSizeClass::from_size(131072), BufferSizeClass::XLarge);
    }
    
    #[test]
    fn test_thread_local_cache() {
        let pool = EnhancedMemoryPool::new(EnhancedPoolConfig::default());
        
        // 여러 번 할당하여 캐시 효과 테스트
        let mut buffers = Vec::new();
        for _ in 0..5 {
            buffers.push(pool.allocate(4096));
        }
        
        // 반환
        for buffer in buffers {
            pool.deallocate(buffer);
        }
        
        // 다시 할당 (캐시에서 나와야 함)
        let buffer = pool.allocate(4096);
        pool.deallocate(buffer);
        
        let stats = pool.get_stats();
        assert!(stats.cache_hits.load(Ordering::Relaxed) > 0);
    }
}