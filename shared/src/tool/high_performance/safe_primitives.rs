//! Safe alternatives to unsafe lock-free primitives
//!
//! This module provides safe abstractions without using unsafe code.

use crossbeam_queue::{ArrayQueue, SegQueue};
use parking_lot::{Mutex, RwLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Safe lock-free queue using crossbeam
pub struct SafeLockFreeQueue<T> {
    queue: Arc<SegQueue<T>>,
}

impl<T> SafeLockFreeQueue<T> {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(SegQueue::new()),
        }
    }

    pub fn push(&self, item: T) {
        self.queue.push(item);
    }

    pub fn pop(&self) -> Option<T> {
        self.queue.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

impl<T> Default for SafeLockFreeQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for SafeLockFreeQueue<T> {
    fn clone(&self) -> Self {
        Self {
            queue: Arc::clone(&self.queue),
        }
    }
}

/// Safe bounded lock-free queue
pub struct SafeBoundedQueue<T> {
    queue: Arc<ArrayQueue<T>>,
}

impl<T> SafeBoundedQueue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Arc::new(ArrayQueue::new(capacity)),
        }
    }

    pub fn push(&self, item: T) -> Result<(), T> {
        self.queue.push(item)
    }

    pub fn pop(&self) -> Option<T> {
        self.queue.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.queue.is_full()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn capacity(&self) -> usize {
        self.queue.capacity()
    }
}

impl<T> Clone for SafeBoundedQueue<T> {
    fn clone(&self) -> Self {
        Self {
            queue: Arc::clone(&self.queue),
        }
    }
}

/// Safe memory pool using parking_lot
pub struct SafeMemoryPool<T: Default + Send> {
    pool: Arc<Mutex<Vec<T>>>,
    max_size: usize,
}

impl<T: Default + Send> SafeMemoryPool<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(Vec::with_capacity(max_size))),
            max_size,
        }
    }

    pub fn acquire(&self) -> T {
        let mut pool = self.pool.lock();
        pool.pop().unwrap_or_default()
    }

    pub fn release(&self, item: T) {
        let mut pool = self.pool.lock();
        if pool.len() < self.max_size {
            pool.push(item);
        }
    }

    pub fn len(&self) -> usize {
        self.pool.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.pool.lock().is_empty()
    }

    pub fn clear(&self) {
        self.pool.lock().clear();
    }
}

impl<T: Default + Send> Clone for SafeMemoryPool<T> {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
            max_size: self.max_size,
        }
    }
}

/// Safe atomic statistics collector
pub struct SafeStatsCollector {
    count: AtomicU64,
    sum: AtomicU64,
    max: AtomicU64,
    min: AtomicU64,
}

impl SafeStatsCollector {
    pub fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            sum: AtomicU64::new(0),
            max: AtomicU64::new(0),
            min: AtomicU64::new(u64::MAX),
        }
    }

    pub fn record(&self, value: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(value, Ordering::Relaxed);

        // Update max
        let mut current_max = self.max.load(Ordering::Relaxed);
        while value > current_max {
            match self.max.compare_exchange_weak(
                current_max,
                value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }

        // Update min
        let mut current_min = self.min.load(Ordering::Relaxed);
        while value < current_min {
            match self.min.compare_exchange_weak(
                current_min,
                value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_min = x,
            }
        }
    }

    pub fn get_stats(&self) -> (u64, u64, u64, u64, f64) {
        let count = self.count.load(Ordering::Relaxed);
        let sum = self.sum.load(Ordering::Relaxed);
        let max = self.max.load(Ordering::Relaxed);
        let min = if count == 0 {
            0
        } else {
            self.min.load(Ordering::Relaxed)
        };
        let avg = if count == 0 {
            0.0
        } else {
            sum as f64 / count as f64
        };

        (count, sum, max, min, avg)
    }

    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        self.sum.store(0, Ordering::Relaxed);
        self.max.store(0, Ordering::Relaxed);
        self.min.store(u64::MAX, Ordering::Relaxed);
    }
}

impl Default for SafeStatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Safe reader-writer lock with priority
pub struct SafeRwLock<T> {
    lock: Arc<RwLock<T>>,
}

impl<T> SafeRwLock<T> {
    pub fn new(value: T) -> Self {
        Self {
            lock: Arc::new(RwLock::new(value)),
        }
    }

    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, T> {
        self.lock.read()
    }

    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, T> {
        self.lock.write()
    }

    pub fn try_read(&self) -> Option<parking_lot::RwLockReadGuard<'_, T>> {
        self.lock.try_read()
    }

    pub fn try_write(&self) -> Option<parking_lot::RwLockWriteGuard<'_, T>> {
        self.lock.try_write()
    }
}

impl<T> Clone for SafeRwLock<T> {
    fn clone(&self) -> Self {
        Self {
            lock: Arc::clone(&self.lock),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_queue() {
        let queue = SafeLockFreeQueue::new();
        queue.push(1);
        queue.push(2);
        queue.push(3);

        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn test_bounded_queue() {
        let queue = SafeBoundedQueue::new(2);
        assert!(queue.push(1).is_ok());
        assert!(queue.push(2).is_ok());
        assert!(queue.push(3).is_err()); // Full

        assert_eq!(queue.pop(), Some(1));
        assert!(queue.push(3).is_ok());
    }

    #[test]
    fn test_memory_pool() {
        let pool: SafeMemoryPool<Vec<u8>> = SafeMemoryPool::new(10);

        let mut item = pool.acquire();
        item.push(1);
        item.push(2);

        pool.release(item);
        assert_eq!(pool.len(), 1);

        let item2 = pool.acquire();
        assert_eq!(pool.len(), 0);
        pool.release(item2);
    }

    #[test]
    fn test_stats_collector() {
        let stats = SafeStatsCollector::new();

        stats.record(10);
        stats.record(20);
        stats.record(5);
        stats.record(15);

        let (count, sum, max, min, avg) = stats.get_stats();
        assert_eq!(count, 4);
        assert_eq!(sum, 50);
        assert_eq!(max, 20);
        assert_eq!(min, 5);
        assert_eq!(avg, 12.5);
    }
}
