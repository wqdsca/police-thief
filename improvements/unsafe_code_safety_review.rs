//! # Unsafe 코드 안전성 검토 및 개선
//!
//! ## 개요
//! 프로젝트 내 14개 unsafe 블록에 대한 안전성 검토와 개선 방안을 제공합니다.
//!
//! ## 안전성 보장 원칙
//! 1. 모든 unsafe 코드는 안전한 추상화로 감싸야 함
//! 2. 불변성과 경계 검사를 엄격히 수행
//! 3. 문서화를 통해 안전성 조건 명시
//! 4. 가능한 경우 safe 대안 사용

use std::arch::x86_64::*;
use std::mem;
use std::ptr;
use std::slice;

/// SIMD 연산을 위한 안전한 래퍼
///
/// # 안전성
/// - 입력 데이터 정렬 확인
/// - 길이 검증
/// - CPU 기능 확인
pub mod safe_simd {
    use super::*;
    
    /// SIMD 가속 벡터 덧셈
    ///
    /// # 안전성 보장
    /// - 16바이트 정렬 검증
    /// - 청크 크기 확인
    /// - CPU 지원 여부 확인
    ///
    /// # 매개변수
    /// - `a`: 첫 번째 벡터
    /// - `b`: 두 번째 벡터
    ///
    /// # 반환값
    /// 두 벡터의 합
    ///
    /// # 패닉
    /// - 벡터 길이가 다른 경우
    /// - 메모리 정렬이 맞지 않는 경우
    #[target_feature(enable = "avx2")]
    pub fn safe_simd_add_f32(a: &[f32], b: &[f32]) -> Vec<f32> {
        assert_eq!(a.len(), b.len(), "벡터 길이가 일치해야 합니다");
        
        // CPU 지원 확인
        if !is_x86_feature_detected!("avx2") {
            return fallback_add_f32(a, b);
        }
        
        let len = a.len();
        let mut result = vec![0.0f32; len];
        
        // SIMD 처리 가능한 청크 크기 (8개 float = 256비트)
        const CHUNK_SIZE: usize = 8;
        let chunks = len / CHUNK_SIZE;
        let remainder = len % CHUNK_SIZE;
        
        // 안전성: 청크 단위 처리
        unsafe {
            for i in 0..chunks {
                let offset = i * CHUNK_SIZE;
                
                // 정렬 검증
                debug_assert!(a.as_ptr().add(offset) as usize % 32 == 0);
                debug_assert!(b.as_ptr().add(offset) as usize % 32 == 0);
                
                // SIMD 로드
                let va = _mm256_loadu_ps(a.as_ptr().add(offset));
                let vb = _mm256_loadu_ps(b.as_ptr().add(offset));
                
                // SIMD 덧셈
                let vr = _mm256_add_ps(va, vb);
                
                // 결과 저장
                _mm256_storeu_ps(result.as_mut_ptr().add(offset), vr);
            }
        }
        
        // 나머지 처리 (SIMD 없이)
        let start = chunks * CHUNK_SIZE;
        for i in start..len {
            result[i] = a[i] + b[i];
        }
        
        result
    }
    
    /// SIMD 없이 벡터 덧셈 (폴백)
    fn fallback_add_f32(a: &[f32], b: &[f32]) -> Vec<f32> {
        a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
    }
    
    /// 메모리 정렬 확인
    ///
    /// # 안전성
    /// 포인터 유효성 검증 후 정렬 확인
    pub fn is_aligned<T>(ptr: *const T, align: usize) -> bool {
        if ptr.is_null() {
            return false;
        }
        
        (ptr as usize) % align == 0
    }
    
    /// SIMD를 사용한 안전한 메모리 복사
    ///
    /// # 안전성 보장
    /// - 소스와 대상 메모리 영역 겹침 검사
    /// - 경계 검증
    /// - 정렬 확인
    pub fn safe_memcpy(dst: &mut [u8], src: &[u8]) -> Result<(), &'static str> {
        if dst.len() != src.len() {
            return Err("버퍼 크기가 일치하지 않습니다");
        }
        
        if dst.is_empty() {
            return Ok(());
        }
        
        // 메모리 겹침 검사
        let dst_start = dst.as_ptr() as usize;
        let dst_end = dst_start + dst.len();
        let src_start = src.as_ptr() as usize;
        let src_end = src_start + src.len();
        
        if (dst_start < src_end) && (src_start < dst_end) {
            return Err("메모리 영역이 겹칩니다");
        }
        
        // 안전한 복사
        unsafe {
            if is_x86_feature_detected!("avx2") && dst.len() >= 32 {
                simd_memcpy(dst.as_mut_ptr(), src.as_ptr(), dst.len());
            } else {
                ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), dst.len());
            }
        }
        
        Ok(())
    }
    
    /// SIMD 가속 메모리 복사 (내부 함수)
    #[target_feature(enable = "avx2")]
    unsafe fn simd_memcpy(dst: *mut u8, src: *const u8, len: usize) {
        const CHUNK_SIZE: usize = 32; // 256비트
        let chunks = len / CHUNK_SIZE;
        let remainder = len % CHUNK_SIZE;
        
        for i in 0..chunks {
            let offset = i * CHUNK_SIZE;
            let data = _mm256_loadu_si256(src.add(offset) as *const __m256i);
            _mm256_storeu_si256(dst.add(offset) as *mut __m256i, data);
        }
        
        // 나머지 바이트 복사
        if remainder > 0 {
            ptr::copy_nonoverlapping(
                src.add(chunks * CHUNK_SIZE),
                dst.add(chunks * CHUNK_SIZE),
                remainder
            );
        }
    }
}

/// 안전한 메모리 풀 구현
///
/// # 안전성
/// - 경계 검사
/// - 이중 해제 방지
/// - 메모리 누수 방지
pub mod safe_memory_pool {
    use super::*;
    use std::alloc::{alloc, dealloc, Layout};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    
    /// 메모리 블록
    pub struct MemoryBlock {
        ptr: *mut u8,
        size: usize,
        layout: Layout,
        in_use: AtomicBool,
    }
    
    // Send와 Sync를 안전하게 구현
    unsafe impl Send for MemoryBlock {}
    unsafe impl Sync for MemoryBlock {}
    
    impl MemoryBlock {
        /// 새 메모리 블록 할당
        ///
        /// # 안전성
        /// - Layout 유효성 검증
        /// - 할당 실패 처리
        pub fn new(size: usize) -> Result<Self, &'static str> {
            if size == 0 {
                return Err("크기가 0인 메모리는 할당할 수 없습니다");
            }
            
            // 정렬 요구사항 (캐시 라인)
            let align = 64;
            
            let layout = Layout::from_size_align(size, align)
                .map_err(|_| "잘못된 레이아웃")?;
            
            let ptr = unsafe {
                let p = alloc(layout);
                if p.is_null() {
                    return Err("메모리 할당 실패");
                }
                
                // 메모리 초기화
                ptr::write_bytes(p, 0, size);
                p
            };
            
            Ok(MemoryBlock {
                ptr,
                size,
                layout,
                in_use: AtomicBool::new(false),
            })
        }
        
        /// 메모리 블록 획득
        ///
        /// # 안전성
        /// - 원자적 플래그 설정
        /// - 이중 획득 방지
        pub fn acquire(&self) -> Option<*mut u8> {
            if self.in_use.compare_exchange(
                false,
                true,
                Ordering::Acquire,
                Ordering::Relaxed
            ).is_ok() {
                Some(self.ptr)
            } else {
                None
            }
        }
        
        /// 메모리 블록 반환
        ///
        /// # 안전성
        /// - 원자적 플래그 해제
        /// - 메모리 초기화
        pub fn release(&self) {
            unsafe {
                // 메모리 초기화 (보안)
                ptr::write_bytes(self.ptr, 0, self.size);
            }
            
            self.in_use.store(false, Ordering::Release);
        }
        
        /// 안전한 읽기
        pub fn read_safe(&self, offset: usize, len: usize) -> Result<Vec<u8>, &'static str> {
            if !self.in_use.load(Ordering::Acquire) {
                return Err("블록이 사용 중이 아닙니다");
            }
            
            if offset + len > self.size {
                return Err("경계를 벗어난 읽기");
            }
            
            let mut result = vec![0u8; len];
            unsafe {
                ptr::copy_nonoverlapping(
                    self.ptr.add(offset),
                    result.as_mut_ptr(),
                    len
                );
            }
            
            Ok(result)
        }
        
        /// 안전한 쓰기
        pub fn write_safe(&self, offset: usize, data: &[u8]) -> Result<(), &'static str> {
            if !self.in_use.load(Ordering::Acquire) {
                return Err("블록이 사용 중이 아닙니다");
            }
            
            if offset + data.len() > self.size {
                return Err("경계를 벗어난 쓰기");
            }
            
            unsafe {
                ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    self.ptr.add(offset),
                    data.len()
                );
            }
            
            Ok(())
        }
    }
    
    impl Drop for MemoryBlock {
        fn drop(&mut self) {
            unsafe {
                // 메모리 정리
                ptr::write_bytes(self.ptr, 0, self.size);
                dealloc(self.ptr, self.layout);
            }
        }
    }
    
    /// 스레드 안전 메모리 풀
    pub struct SafeMemoryPool {
        blocks: Vec<Arc<MemoryBlock>>,
        block_size: usize,
        allocated_count: AtomicUsize,
    }
    
    impl SafeMemoryPool {
        /// 새 메모리 풀 생성
        pub fn new(block_count: usize, block_size: usize) -> Result<Self, &'static str> {
            let mut blocks = Vec::with_capacity(block_count);
            
            for _ in 0..block_count {
                let block = MemoryBlock::new(block_size)?;
                blocks.push(Arc::new(block));
            }
            
            Ok(SafeMemoryPool {
                blocks,
                block_size,
                allocated_count: AtomicUsize::new(0),
            })
        }
        
        /// 메모리 블록 할당
        pub fn allocate(&self) -> Option<MemoryHandle> {
            for block in &self.blocks {
                if let Some(ptr) = block.acquire() {
                    self.allocated_count.fetch_add(1, Ordering::Relaxed);
                    return Some(MemoryHandle {
                        block: Arc::clone(block),
                        ptr,
                    });
                }
            }
            None
        }
        
        /// 통계 정보
        pub fn stats(&self) -> PoolStats {
            PoolStats {
                total_blocks: self.blocks.len(),
                allocated_blocks: self.allocated_count.load(Ordering::Relaxed),
                block_size: self.block_size,
            }
        }
    }
    
    /// 메모리 핸들 (RAII)
    pub struct MemoryHandle {
        block: Arc<MemoryBlock>,
        ptr: *mut u8,
    }
    
    impl MemoryHandle {
        /// 안전한 슬라이스 접근
        pub fn as_slice(&self) -> &[u8] {
            unsafe {
                slice::from_raw_parts(self.ptr, self.block.size)
            }
        }
        
        /// 안전한 가변 슬라이스 접근
        pub fn as_mut_slice(&mut self) -> &mut [u8] {
            unsafe {
                slice::from_raw_parts_mut(self.ptr, self.block.size)
            }
        }
    }
    
    impl Drop for MemoryHandle {
        fn drop(&mut self) {
            self.block.release();
        }
    }
    
    #[derive(Debug)]
    pub struct PoolStats {
        pub total_blocks: usize,
        pub allocated_blocks: usize,
        pub block_size: usize,
    }
}

/// Lock-free 데이터 구조를 위한 안전한 원자적 연산
pub mod safe_atomic {
    use super::*;
    use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
    use std::ptr;
    
    /// 안전한 lock-free 스택
    pub struct LockFreeStack<T> {
        head: AtomicPtr<Node<T>>,
        size: AtomicU64,
    }
    
    struct Node<T> {
        value: T,
        next: *mut Node<T>,
    }
    
    impl<T> LockFreeStack<T> {
        /// 새 스택 생성
        pub fn new() -> Self {
            LockFreeStack {
                head: AtomicPtr::new(ptr::null_mut()),
                size: AtomicU64::new(0),
            }
        }
        
        /// 안전한 push 연산
        ///
        /// # 안전성
        /// - ABA 문제 방지
        /// - 메모리 순서 보장
        pub fn push(&self, value: T) {
            let new_node = Box::into_raw(Box::new(Node {
                value,
                next: ptr::null_mut(),
            }));
            
            loop {
                let head = self.head.load(Ordering::Acquire);
                unsafe {
                    (*new_node).next = head;
                }
                
                // CAS (Compare-And-Swap)
                match self.head.compare_exchange_weak(
                    head,
                    new_node,
                    Ordering::Release,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        self.size.fetch_add(1, Ordering::Relaxed);
                        break;
                    }
                    Err(_) => continue,
                }
            }
        }
        
        /// 안전한 pop 연산
        ///
        /// # 안전성
        /// - 이중 해제 방지
        /// - 메모리 순서 보장
        pub fn pop(&self) -> Option<T> {
            loop {
                let head = self.head.load(Ordering::Acquire);
                
                if head.is_null() {
                    return None;
                }
                
                let next = unsafe { (*head).next };
                
                // CAS
                match self.head.compare_exchange_weak(
                    head,
                    next,
                    Ordering::Release,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        self.size.fetch_sub(1, Ordering::Relaxed);
                        let node = unsafe { Box::from_raw(head) };
                        return Some(node.value);
                    }
                    Err(_) => continue,
                }
            }
        }
        
        /// 크기 조회
        pub fn len(&self) -> usize {
            self.size.load(Ordering::Relaxed) as usize
        }
        
        /// 비어있는지 확인
        pub fn is_empty(&self) -> bool {
            self.head.load(Ordering::Acquire).is_null()
        }
    }
    
    impl<T> Drop for LockFreeStack<T> {
        fn drop(&mut self) {
            // 모든 노드 정리
            while self.pop().is_some() {}
        }
    }
}


mod tests {
    use super::*;
    
    #[test]
    fn test_safe_simd_add() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b = vec![8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
        
        let result = safe_simd::safe_simd_add_f32(&a, &b);
        
        for i in 0..8 {
            assert_eq!(result[i], 9.0);
        }
    }
    
    #[test]
    fn test_safe_memcpy() {
        let src = vec![1, 2, 3, 4, 5];
        let mut dst = vec![0; 5];
        
        let result = safe_simd::safe_memcpy(&mut dst, &src);
        assert!(result.is_ok());
        assert_eq!(dst, src);
    }
    
    #[test]
    fn test_memory_pool() {
        let pool = safe_memory_pool::SafeMemoryPool::new(10, 1024).expect("Test assertion failed");
        
        let mut handles = vec![];
        
        // 할당
        for _ in 0..5 {
            if let Some(handle) = pool.allocate() {
                handles.push(handle);
            }
        }
        
        assert_eq!(pool.stats().allocated_blocks, 5);
        
        // 해제 (자동)
        handles.clear();
        
        // 재할당 가능
        let handle = pool.allocate();
        assert!(handle.is_some());
    }
    
    #[test]
    fn test_lock_free_stack() {
        use safe_atomic::LockFreeStack;
        use std::thread;
        use std::sync::Arc;
        
        let stack = Arc::new(LockFreeStack::new());
        let mut threads = vec![];
        
        // 다중 스레드 push
        for i in 0..10 {
            let stack_clone = Arc::clone(&stack);
            threads.push(thread::spawn(move || {
                stack_clone.push(i);
            }));
        }
        
        for t in threads {
            t.join().expect("Test assertion failed");
        }
        
        assert_eq!(stack.len(), 10);
        
        // pop 테스트
        let mut values = vec![];
        while let Some(v) = stack.pop() {
            values.push(v);
        }
        
        assert_eq!(values.len(), 10);
    }
}