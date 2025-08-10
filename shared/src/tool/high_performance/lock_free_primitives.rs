use parking_lot::Mutex;
use std::hint::spin_loop;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Lock-free 상태 플래그
pub struct LockFreeFlag {
    flag: AtomicBool,
}

impl LockFreeFlag {
    pub fn new(initial: bool) -> Self {
        Self {
            flag: AtomicBool::new(initial),
        }
    }
    
    /// 플래그 설정
    #[inline]
    pub fn set(&self, value: bool) {
        self.flag.store(value, Ordering::Release);
    }
    
    /// 플래그 읽기
    #[inline]
    pub fn get(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
    
    /// Compare-and-swap
    #[inline]
    pub fn compare_exchange(&self, current: bool, new: bool) -> Result<bool, bool> {
        self.flag.compare_exchange(
            current,
            new,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
    }
    
    /// 플래그가 true가 될 때까지 대기 (spin-wait)
    #[inline]
    pub fn wait_for_true(&self) {
        while !self.get() {
            spin_loop();
        }
    }
    
    /// 플래그가 false가 될 때까지 대기 (spin-wait)
    #[inline]
    pub fn wait_for_false(&self) {
        while self.get() {
            spin_loop();
        }
    }
}

/// Lock-free 카운터
pub struct LockFreeCounter {
    count: AtomicU64,
}

impl LockFreeCounter {
    pub fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
        }
    }
    
    /// 증가
    #[inline]
    pub fn increment(&self) -> u64 {
        self.count.fetch_add(1, Ordering::AcqRel)
    }
    
    /// 감소
    #[inline]
    pub fn decrement(&self) -> u64 {
        self.count.fetch_sub(1, Ordering::AcqRel)
    }
    
    /// 현재 값
    #[inline]
    pub fn get(&self) -> u64 {
        self.count.load(Ordering::Acquire)
    }
    
    /// 값 설정
    #[inline]
    pub fn set(&self, value: u64) {
        self.count.store(value, Ordering::Release);
    }
    
    /// 값 추가
    #[inline]
    pub fn add(&self, value: u64) -> u64 {
        self.count.fetch_add(value, Ordering::AcqRel)
    }
}

/// Lock-free 스핀락 (AtomicBool 기반)
pub struct SpinLock {
    locked: AtomicBool,
}

impl SpinLock {
    pub fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }
    
    /// 락 획득
    #[inline]
    pub fn lock(&self) -> SpinLockGuard<'_> {
        // Spin until we acquire the lock
        while self.locked.compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_err() {
            // CPU 힌트를 주어 효율적인 스핀
            spin_loop();
        }
        
        SpinLockGuard { lock: self }
    }
    
    /// 락 시도 (non-blocking)
    #[inline]
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_>> {
        if self.locked.compare_exchange(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_ok() {
            Some(SpinLockGuard { lock: self })
        } else {
            None
        }
    }
    
    /// 락 해제 (내부용)
    #[inline]
    fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

/// SpinLock 가드
pub struct SpinLockGuard<'a> {
    lock: &'a SpinLock,
}

impl<'a> Drop for SpinLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

/// Adaptive 스핀락 (스핀 후 블로킹)
pub struct AdaptiveSpinLock {
    locked: AtomicBool,
    spin_count: usize,
}

impl AdaptiveSpinLock {
    pub fn new(spin_count: usize) -> Self {
        Self {
            locked: AtomicBool::new(false),
            spin_count,
        }
    }
    
    /// 락 획득 (adaptive spinning)
    pub fn lock(&self) -> AdaptiveSpinLockGuard<'_> {
        let mut spins = 0;
        
        // 먼저 스핀 시도
        while spins < self.spin_count {
            if self.locked.compare_exchange_weak(
                false,
                true,
                Ordering::Acquire,
                Ordering::Relaxed,
            ).is_ok() {
                return AdaptiveSpinLockGuard { lock: self };
            }
            
            spin_loop();
            spins += 1;
        }
        
        // 스핀 실패 시 yield
        while self.locked.compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_err() {
            std::thread::yield_now();
        }
        
        AdaptiveSpinLockGuard { lock: self }
    }
    
    fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

/// AdaptiveSpinLock 가드
pub struct AdaptiveSpinLockGuard<'a> {
    lock: &'a AdaptiveSpinLock,
}

impl<'a> Drop for AdaptiveSpinLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

/// SeqLock (읽기 최적화)
pub struct SeqLock<T: Clone> {
    sequence: AtomicUsize,
    data: parking_lot::RwLock<T>,
}

impl<T: Clone> SeqLock<T> {
    pub fn new(data: T) -> Self {
        Self {
            sequence: AtomicUsize::new(0),
            data: parking_lot::RwLock::new(data),
        }
    }
    
    /// 데이터 읽기 (lock-free)
    pub fn read(&self) -> T {
        loop {
            // 시퀀스 번호 읽기
            let seq1 = self.sequence.load(Ordering::Acquire);
            
            // 홀수면 쓰기 중이므로 대기
            if seq1 & 1 != 0 {
                spin_loop();
                continue;
            }
            
            // 데이터 읽기
            let data = {
                let guard = self.data.read();
                guard.clone()
            };
            
            // 시퀀스 번호 재확인
            let seq2 = self.sequence.load(Ordering::Acquire);
            
            // 시퀀스가 같으면 유효한 읽기
            if seq1 == seq2 {
                return data;
            }
            
            // 다르면 재시도
            spin_loop();
        }
    }
    
    /// 데이터 쓰기
    pub fn write(&self, data: T) {
        // 시퀀스 증가 (홀수로 만들어 쓰기 시작 표시)
        self.sequence.fetch_add(1, Ordering::AcqRel);
        
        // 데이터 쓰기
        {
            let mut guard = self.data.write();
            *guard = data;
        }
        
        // 시퀀스 증가 (짝수로 만들어 쓰기 완료 표시)
        self.sequence.fetch_add(1, Ordering::AcqRel);
    }
}

/// Lock-free 큐 노드
struct QueueNode<T> {
    data: Option<T>,
    #[allow(dead_code)]
    next: AtomicUsize,
}

/// 간단한 Lock-free SPSC 큐
pub struct LockFreeSPSCQueue<T> {
    head: AtomicUsize,
    tail: AtomicUsize,
    buffer: Vec<QueueNode<T>>,
    capacity: usize,
}

impl<T> LockFreeSPSCQueue<T> {
    pub fn new(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(QueueNode {
                data: None,
                next: AtomicUsize::new(0),
            });
        }
        
        Self {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            buffer,
            capacity,
        }
    }
    
    /// 엔큐 (생산자)
    pub fn enqueue(&self, item: T) -> bool {
        let tail = self.tail.load(Ordering::Acquire);
        let next = (tail + 1) % self.capacity;
        
        if next == self.head.load(Ordering::Acquire) {
            // 큐가 가득 참
            return false;
        }
        
        unsafe {
            let node = &mut *(self.buffer.as_ptr().add(tail) as *const QueueNode<T> as *mut QueueNode<T>);
            node.data = Some(item);
        }
        
        self.tail.store(next, Ordering::Release);
        true
    }
    
    /// 디큐 (소비자)
    pub fn dequeue(&self) -> Option<T> {
        let head = self.head.load(Ordering::Acquire);
        
        if head == self.tail.load(Ordering::Acquire) {
            // 큐가 비어있음
            return None;
        }
        
        let item = unsafe {
            let node = &mut *(self.buffer.as_ptr().add(head) as *const QueueNode<T> as *mut QueueNode<T>);
            node.data.take()
        };
        
        self.head.store((head + 1) % self.capacity, Ordering::Release);
        item
    }
    
    /// 큐가 비어있는지 확인
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }
}

/// 성능 벤치마크용 헬퍼
pub struct LockFreeBenchmark;

impl LockFreeBenchmark {
    /// Mutex vs AtomicBool 성능 비교
    pub fn compare_mutex_vs_atomic(iterations: usize) {
        println!("=== Lock Performance Comparison ===");
        
        // Mutex 테스트
        let mutex = Arc::new(Mutex::new(false));
        let start = Instant::now();
        
        for _ in 0..iterations {
            let mut guard = mutex.lock();
            *guard = !*guard;
        }
        
        let mutex_time = start.elapsed();
        
        // AtomicBool 테스트
        let atomic = Arc::new(AtomicBool::new(false));
        let start = Instant::now();
        
        for _ in 0..iterations {
            atomic.fetch_xor(true, Ordering::AcqRel);
        }
        
        let atomic_time = start.elapsed();
        
        // SpinLock 테스트
        let spinlock = Arc::new(SpinLock::new());
        let start = Instant::now();
        
        for _ in 0..iterations {
            let _guard = spinlock.lock();
        }
        
        let spinlock_time = start.elapsed();
        
        println!("Iterations: {}", iterations);
        println!("Mutex: {:?}", mutex_time);
        println!("AtomicBool: {:?}", atomic_time);
        println!("SpinLock: {:?}", spinlock_time);
        println!(
            "Performance gain: {:.2}x faster",
            mutex_time.as_nanos() as f64 / atomic_time.as_nanos() as f64
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_free_flag() {
        let flag = LockFreeFlag::new(false);
        assert!(!flag.get());
        
        flag.set(true);
        assert!(flag.get());
        
        assert!(flag.compare_exchange(true, false).is_ok());
        assert!(!flag.get());
    }
    
    #[test]
    fn test_lock_free_counter() {
        let counter = LockFreeCounter::new();
        assert_eq!(counter.get(), 0);
        
        assert_eq!(counter.increment(), 0);
        assert_eq!(counter.get(), 1);
        
        assert_eq!(counter.add(5), 1);
        assert_eq!(counter.get(), 6);
        
        assert_eq!(counter.decrement(), 6);
        assert_eq!(counter.get(), 5);
    }
    
    #[test]
    fn test_spinlock() {
        let lock = SpinLock::new();
        
        {
            let _guard = lock.lock();
            // 락이 잡혔을 때 다른 시도는 실패해야 함
            assert!(lock.try_lock().is_none());
        }
        
        // 락이 해제되면 다시 획득 가능
        assert!(lock.try_lock().is_some());
    }
    
    #[test]
    fn test_seqlock() {
        let seqlock = SeqLock::new(42);
        
        assert_eq!(seqlock.read(), 42);
        
        seqlock.write(100);
        assert_eq!(seqlock.read(), 100);
    }
    
    #[test]
    fn test_lockfree_queue() {
        let queue = LockFreeSPSCQueue::new(10);
        
        assert!(queue.is_empty());
        assert!(queue.enqueue(1));
        assert!(queue.enqueue(2));
        assert!(queue.enqueue(3));
        
        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.dequeue(), None);
        assert!(queue.is_empty());
    }
    
    #[test]
    fn test_performance_comparison() {
        LockFreeBenchmark::compare_mutex_vs_atomic(10000);
    }
}