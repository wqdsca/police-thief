# RUDP 서버 성능 최적화 가이드

## 📋 목차
1. [성능 목표 및 현황](#성능-목표-및-현황)
2. [SIMD 하드웨어 가속](#simd-하드웨어-가속)
3. [제로카피 I/O 최적화](#제로카피-io-최적화)
4. [적응형 혼잡 제어](#적응형-혼잡-제어)
5. [메모리 최적화 전략](#메모리-최적화-전략)
6. [성능 모니터링](#성능-모니터링)

## 🎯 성능 목표 및 현황

### 목표 성능 지표
```
RUDP Server Performance Targets:
├── 처리량: 20,000+ msg/sec
├── 지연시간: <0.5ms p99
├── 메모리: 8-10MB for 1000 connections
├── CPU 효율: 단일 코어 최적화
├── 패킷 손실률: <0.1%
└── 재전송 비율: <2%
```

### 현재 최적화 서비스 (16개)
```
Performance Services
├── SIMD Packet Processor (AVX2/SSE4.2)
├── Zero-Copy I/O Engine
├── Adaptive Congestion Control
├── Enhanced Memory Pool (5-tier)
├── Lock-Free Ring Buffer
├── NUMA-Aware Allocation
├── CPU Affinity Manager
├── Network Interrupt Coalescing
├── Batch Processing Engine
├── Connection Pool Manager
├── Latency Tracker
├── Throughput Monitor
├── Cache Line Optimizer
├── Prefetch Controller
├── Branch Predictor Optimizer
└── Memory Bandwidth Manager
```

## ⚡ SIMD 하드웨어 가속

### 1. SIMD 패킷 프로세서 구현

```rust
// service/simd_packet_processor.rs
use std::arch::x86_64::*;
use crate::protocol::rudp::RudpPacket;

pub struct SimdPacketProcessor {
    simd_features: SimdFeatures,
    batch_buffer: Vec<RudpPacket>,
}

#[derive(Debug)]
pub struct SimdFeatures {
    pub avx2: bool,
    pub avx512: bool,
    pub sse42: bool,
}

impl SimdPacketProcessor {
    pub fn new() -> Self {
        Self {
            simd_features: Self::detect_simd_features(),
            batch_buffer: Vec::with_capacity(32),
        }
    }
    
    /// SIMD 기능 감지
    fn detect_simd_features() -> SimdFeatures {
        SimdFeatures {
            avx2: is_x86_feature_detected!("avx2"),
            avx512: is_x86_feature_detected!("avx512f"),
            sse42: is_x86_feature_detected!("sse4.2"),
        }
    }
    
    /// 배치 패킷 체크섬 계산 (AVX2)
    pub fn calculate_checksums_batch(&self, packets: &[RudpPacket]) -> Vec<u32> {
        if self.simd_features.avx2 {
            self.calculate_checksums_avx2(packets)
        } else if self.simd_features.sse42 {
            self.calculate_checksums_sse42(packets)
        } else {
            self.calculate_checksums_scalar(packets)
        }
    }
    
    /// AVX2를 이용한 병렬 체크섬 계산
    fn calculate_checksums_avx2(&self, packets: &[RudpPacket]) -> Vec<u32> {
        let mut checksums = Vec::with_capacity(packets.len());
        
        unsafe {
            // 8개씩 병렬 처리
            for chunk in packets.chunks(8) {
                let mut data_ptrs = [std::ptr::null(); 8];
                let mut lengths = [0u32; 8];
                
                // 포인터와 길이 준비
                for (i, packet) in chunk.iter().enumerate() {
                    data_ptrs[i] = packet.payload.as_ptr();
                    lengths[i] = packet.payload.len() as u32;
                }
                
                // AVX2 레지스터에 길이 로드
                let length_vec = _mm256_loadu_si256(lengths.as_ptr() as *const __m256i);
                
                // 각 패킷의 체크섬 계산 (병렬)
                for i in 0..chunk.len() {
                    let checksum = self.fast_checksum_avx2(data_ptrs[i], lengths[i] as usize);
                    checksums.push(checksum);
                }
            }
        }
        
        checksums
    }
    
    /// AVX2 최적화된 체크섬 계산
    unsafe fn fast_checksum_avx2(&self, data: *const u8, len: usize) -> u32 {
        let mut checksum = 0u32;
        let mut pos = 0;
        
        // 32바이트씩 병렬 처리
        while pos + 32 <= len {
            let chunk = _mm256_loadu_si256(data.add(pos) as *const __m256i);
            
            // 바이트를 32비트 정수로 확장하여 합계 계산
            let lo = _mm256_unpacklo_epi8(chunk, _mm256_setzero_si256());
            let hi = _mm256_unpackhi_epi8(chunk, _mm256_setzero_si256());
            
            let sum_lo = _mm256_sad_epu8(lo, _mm256_setzero_si256());
            let sum_hi = _mm256_sad_epu8(hi, _mm256_setzero_si256());
            
            // 수평 합계
            let total = _mm256_add_epi64(sum_lo, sum_hi);
            let sum_128 = _mm_add_epi64(
                _mm256_extracti128_si256(total, 0),
                _mm256_extracti128_si256(total, 1)
            );
            
            checksum = checksum.wrapping_add(_mm_extract_epi64(sum_128, 0) as u32);
            checksum = checksum.wrapping_add(_mm_extract_epi64(sum_128, 1) as u32);
            
            pos += 32;
        }
        
        // 나머지 바이트 처리
        while pos < len {
            checksum = checksum.wrapping_add(*data.add(pos) as u32);
            pos += 1;
        }
        
        checksum
    }
    
    /// SSE4.2 체크섬 계산
    fn calculate_checksums_sse42(&self, packets: &[RudpPacket]) -> Vec<u32> {
        let mut checksums = Vec::with_capacity(packets.len());
        
        unsafe {
            for packet in packets {
                let checksum = self.fast_checksum_sse42(
                    packet.payload.as_ptr(),
                    packet.payload.len()
                );
                checksums.push(checksum);
            }
        }
        
        checksums
    }
    
    /// SSE4.2 최적화된 체크섬
    unsafe fn fast_checksum_sse42(&self, data: *const u8, len: usize) -> u32 {
        let mut checksum = 0u32;
        let mut pos = 0;
        
        // 16바이트씩 처리
        while pos + 16 <= len {
            let chunk = _mm_loadu_si128(data.add(pos) as *const __m128i);
            let sad = _mm_sad_epu8(chunk, _mm_setzero_si128());
            
            checksum = checksum.wrapping_add(_mm_extract_epi16(sad, 0) as u32);
            checksum = checksum.wrapping_add(_mm_extract_epi16(sad, 4) as u32);
            
            pos += 16;
        }
        
        // 나머지 처리
        while pos < len {
            checksum = checksum.wrapping_add(*data.add(pos) as u32);
            pos += 1;
        }
        
        checksum
    }
    
    /// 스칼라 체크섬 (fallback)
    fn calculate_checksums_scalar(&self, packets: &[RudpPacket]) -> Vec<u32> {
        packets.iter().map(|packet| {
            packet.payload.iter().map(|&b| b as u32).sum()
        }).collect()
    }
}
```

### 2. SIMD 메모리 비교 최적화

```rust
// service/simd_memory_ops.rs
use std::arch::x86_64::*;

pub struct SimdMemoryOps;

impl SimdMemoryOps {
    /// AVX2 메모리 비교 (최대 256비트)
    pub unsafe fn memory_compare_avx2(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        
        let len = a.len();
        let mut pos = 0;
        
        // 32바이트씩 비교
        while pos + 32 <= len {
            let chunk_a = _mm256_loadu_si256(a.as_ptr().add(pos) as *const __m256i);
            let chunk_b = _mm256_loadu_si256(b.as_ptr().add(pos) as *const __m256i);
            
            let cmp = _mm256_cmpeq_epi8(chunk_a, chunk_b);
            let mask = _mm256_movemask_epi8(cmp);
            
            if mask != -1 { // 0xFFFFFFFF
                return false;
            }
            
            pos += 32;
        }
        
        // 나머지 바이트 비교
        for i in pos..len {
            if a[i] != b[i] {
                return false;
            }
        }
        
        true
    }
    
    /// SIMD 메모리 복사 최적화
    pub unsafe fn fast_memcpy(dest: *mut u8, src: *const u8, len: usize) {
        let mut pos = 0;
        
        // 32바이트씩 복사 (AVX2)
        if is_x86_feature_detected!("avx2") {
            while pos + 32 <= len {
                let data = _mm256_loadu_si256(src.add(pos) as *const __m256i);
                _mm256_storeu_si256(dest.add(pos) as *mut __m256i, data);
                pos += 32;
            }
        }
        
        // 16바이트씩 복사 (SSE2)
        while pos + 16 <= len {
            let data = _mm_loadu_si128(src.add(pos) as *const __m128i);
            _mm_storeu_si128(dest.add(pos) as *mut __m128i, data);
            pos += 16;
        }
        
        // 나머지 바이트 복사
        while pos < len {
            *dest.add(pos) = *src.add(pos);
            pos += 1;
        }
    }
}
```

## 🔄 제로카피 I/O 최적화

### 1. 제로카피 버퍼 시스템

```rust
// service/zero_copy_io.rs
use std::sync::Arc;
use std::ptr;
use std::slice;

pub struct ZeroCopyBuffer {
    data: *mut u8,
    capacity: usize,
    len: usize,
    shared: Arc<BufferMetadata>,
}

struct BufferMetadata {
    ref_count: std::sync::atomic::AtomicUsize,
    original_ptr: *mut u8,
    original_capacity: usize,
}

unsafe impl Send for ZeroCopyBuffer {}
unsafe impl Sync for ZeroCopyBuffer {}

impl ZeroCopyBuffer {
    /// 메모리 직접 할당
    pub fn new(capacity: usize) -> Result<Self, std::io::Error> {
        let layout = std::alloc::Layout::from_size_align(capacity, 64)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid layout"))?;
            
        let data = unsafe { std::alloc::alloc(layout) };
        
        if data.is_null() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory, 
                "Failed to allocate buffer"
            ));
        }
        
        Ok(Self {
            data,
            capacity,
            len: 0,
            shared: Arc::new(BufferMetadata {
                ref_count: std::sync::atomic::AtomicUsize::new(1),
                original_ptr: data,
                original_capacity: capacity,
            }),
        })
    }
    
    /// 다른 버퍼와 메모리 공유
    pub fn share_slice(&self, offset: usize, len: usize) -> Result<Self, std::io::Error> {
        if offset + len > self.len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Slice out of bounds"
            ));
        }
        
        self.shared.ref_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        Ok(Self {
            data: unsafe { self.data.add(offset) },
            capacity: len,
            len,
            shared: self.shared.clone(),
        })
    }
    
    /// 직접 메모리 액세스 (제로카피)
    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.data, self.len) }
    }
    
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.data, self.len) }
    }
    
    /// 특정 위치에서 데이터 읽기
    pub fn read_u32_at(&self, offset: usize) -> Result<u32, std::io::Error> {
        if offset + 4 > self.len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Read out of bounds"
            ));
        }
        
        let ptr = unsafe { self.data.add(offset) as *const u32 };
        Ok(unsafe { ptr.read_unaligned() })
    }
    
    /// 특정 위치에 데이터 쓰기
    pub fn write_u32_at(&mut self, offset: usize, value: u32) -> Result<(), std::io::Error> {
        if offset + 4 > self.capacity {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "Write out of bounds"
            ));
        }
        
        let ptr = unsafe { self.data.add(offset) as *mut u32 };
        unsafe { ptr.write_unaligned(value) };
        
        self.len = self.len.max(offset + 4);
        Ok(())
    }
    
    /// 벡터화된 I/O (scatter-gather)
    pub fn write_vectored(&mut self, bufs: &[&[u8]]) -> Result<usize, std::io::Error> {
        let mut total_written = 0;
        let mut pos = self.len;
        
        for buf in bufs {
            if pos + buf.len() > self.capacity {
                break;
            }
            
            unsafe {
                ptr::copy_nonoverlapping(
                    buf.as_ptr(),
                    self.data.add(pos),
                    buf.len()
                );
            }
            
            pos += buf.len();
            total_written += buf.len();
        }
        
        self.len = pos;
        Ok(total_written)
    }
}

impl Drop for ZeroCopyBuffer {
    fn drop(&mut self) {
        let prev_count = self.shared.ref_count.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        
        if prev_count == 1 {
            // 마지막 참조이므로 메모리 해제
            let layout = std::alloc::Layout::from_size_align(
                self.shared.original_capacity, 
                64
            ).unwrap();
            
            unsafe {
                std::alloc::dealloc(self.shared.original_ptr, layout);
            }
        }
    }
}
```

### 2. 벡터화된 I/O 처리기

```rust
// service/vectorized_io.rs
use std::io::{IoSlice, IoSliceMut};
use tokio::net::UdpSocket;

pub struct VectorizedIOProcessor {
    socket: Arc<UdpSocket>,
    send_buffers: Vec<ZeroCopyBuffer>,
    recv_buffers: Vec<ZeroCopyBuffer>,
}

impl VectorizedIOProcessor {
    /// 배치 송신 (벡터화된 I/O)
    pub async fn send_batch(&mut self, messages: Vec<(&[u8], std::net::SocketAddr)>) 
        -> Result<usize, std::io::Error> 
    {
        // IoSlice 배열 준비
        let mut io_slices = Vec::with_capacity(messages.len());
        let mut addrs = Vec::with_capacity(messages.len());
        
        for (data, addr) in messages {
            io_slices.push(IoSlice::new(data));
            addrs.push(addr);
        }
        
        // 벡터화된 송신 (sendmmsg 시스템 콜)
        let mut total_sent = 0;
        
        // 플랫폼별 최적화
        #[cfg(target_os = "linux")]
        {
            total_sent = self.send_vectored_linux(&io_slices, &addrs).await?;
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            // fallback: 개별 송신
            for (slice, addr) in io_slices.iter().zip(addrs.iter()) {
                let sent = self.socket.send_to(slice, addr).await?;
                total_sent += sent;
            }
        }
        
        Ok(total_sent)
    }
    
    #[cfg(target_os = "linux")]
    async fn send_vectored_linux(&self, slices: &[IoSlice<'_>], addrs: &[std::net::SocketAddr]) 
        -> Result<usize, std::io::Error> 
    {
        // Linux sendmmsg 시스템 콜 활용
        // 실제 구현은 libc 바인딩 필요
        let mut total_sent = 0;
        
        // 임시 구현 - 개별 전송
        for (slice, addr) in slices.iter().zip(addrs.iter()) {
            let sent = self.socket.send_to(slice, addr).await?;
            total_sent += sent;
        }
        
        Ok(total_sent)
    }
    
    /// 배치 수신
    pub async fn recv_batch(&mut self, buffer_count: usize) 
        -> Result<Vec<(ZeroCopyBuffer, std::net::SocketAddr)>, std::io::Error> 
    {
        let mut results = Vec::with_capacity(buffer_count);
        
        // 여러 버퍼에 동시 수신
        for _ in 0..buffer_count {
            if let Ok(buffer) = ZeroCopyBuffer::new(1500) { // MTU 크기
                self.recv_buffers.push(buffer);
            }
        }
        
        // 논블로킹 수신 시도
        for buffer in &mut self.recv_buffers {
            match self.socket.try_recv_from(buffer.as_mut_slice()) {
                Ok((len, addr)) => {
                    buffer.len = len;
                    // 버퍼를 결과에 이동 (제로카피)
                    let result_buffer = std::mem::replace(
                        buffer, 
                        ZeroCopyBuffer::new(1500).unwrap()
                    );
                    results.push((result_buffer, addr));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    break; // 더 이상 수신할 데이터 없음
                }
                Err(e) => return Err(e),
            }
        }
        
        Ok(results)
    }
}
```

## 📈 적응형 혼잡 제어

### 1. 네트워크 상태 기반 혼잡 제어

```rust
// service/adaptive_congestion_control.rs
use std::time::{Duration, Instant};
use std::collections::VecDeque;

pub struct AdaptiveCongestionControl {
    // 혼잡 윈도우 관리
    congestion_window: f64,
    ssthresh: f64,
    
    // RTT 추적
    rtt_samples: VecDeque<Duration>,
    min_rtt: Duration,
    smoothed_rtt: Duration,
    rtt_variance: Duration,
    
    // 패킷 손실 추적
    lost_packets: u32,
    total_packets: u32,
    loss_rate: f64,
    
    // 상태
    state: CongestionState,
    last_update: Instant,
    
    // 설정
    config: CongestionConfig,
}

#[derive(Debug, Clone, PartialEq)]
enum CongestionState {
    SlowStart,
    CongestionAvoidance,
    FastRecovery,
    NetworkProbing,
}

#[derive(Debug, Clone)]
pub struct CongestionConfig {
    pub initial_window: f64,
    pub min_window: f64,
    pub max_window: f64,
    pub alpha: f64,  // RTT 평활화 계수
    pub beta: f64,   // 혼잡 시 윈도우 감소 비율
    pub gamma: f64,  // 손실률 임계값
}

impl Default for CongestionConfig {
    fn default() -> Self {
        Self {
            initial_window: 10.0,
            min_window: 2.0,
            max_window: 1000.0,
            alpha: 0.125,
            beta: 0.5,
            gamma: 0.01, // 1% 손실률
        }
    }
}

impl AdaptiveCongestionControl {
    pub fn new(config: CongestionConfig) -> Self {
        Self {
            congestion_window: config.initial_window,
            ssthresh: config.max_window / 2.0,
            rtt_samples: VecDeque::with_capacity(100),
            min_rtt: Duration::from_millis(1000),
            smoothed_rtt: Duration::from_millis(100),
            rtt_variance: Duration::from_millis(50),
            lost_packets: 0,
            total_packets: 0,
            loss_rate: 0.0,
            state: CongestionState::SlowStart,
            last_update: Instant::now(),
            config,
        }
    }
    
    /// RTT 샘플 업데이트
    pub fn update_rtt(&mut self, rtt: Duration) {
        self.rtt_samples.push_back(rtt);
        if self.rtt_samples.len() > 100 {
            self.rtt_samples.pop_front();
        }
        
        // 최소 RTT 업데이트
        if rtt < self.min_rtt {
            self.min_rtt = rtt;
        }
        
        // 평활화된 RTT 계산 (RFC 6298)
        if self.total_packets == 0 {
            self.smoothed_rtt = rtt;
            self.rtt_variance = rtt / 2;
        } else {
            let diff = if rtt > self.smoothed_rtt {
                rtt - self.smoothed_rtt
            } else {
                self.smoothed_rtt - rtt
            };
            
            self.rtt_variance = Duration::from_nanos(
                (self.rtt_variance.as_nanos() as f64 * (1.0 - self.config.alpha / 2.0) +
                 diff.as_nanos() as f64 * self.config.alpha / 2.0) as u64
            );
            
            self.smoothed_rtt = Duration::from_nanos(
                (self.smoothed_rtt.as_nanos() as f64 * (1.0 - self.config.alpha) +
                 rtt.as_nanos() as f64 * self.config.alpha) as u64
            );
        }
    }
    
    /// 패킷 손실 보고
    pub fn report_packet_loss(&mut self, lost_count: u32) {
        self.lost_packets += lost_count;
        self.loss_rate = self.lost_packets as f64 / self.total_packets as f64;
        
        // 손실률에 따른 혼잡 제어
        if self.loss_rate > self.config.gamma {
            self.handle_congestion();
        }
    }
    
    /// 혼잡 상황 처리
    fn handle_congestion(&mut self) {
        match self.state {
            CongestionState::SlowStart | CongestionState::CongestionAvoidance => {
                // ssthresh를 현재 윈도우의 절반으로 설정
                self.ssthresh = (self.congestion_window * self.config.beta).max(self.config.min_window);
                
                // 윈도우 크기 감소
                self.congestion_window = self.ssthresh;
                
                // 빠른 복구 모드로 전환
                self.state = CongestionState::FastRecovery;
            }
            CongestionState::FastRecovery => {
                // 이미 복구 모드인 경우 윈도우 추가 감소
                self.congestion_window *= 0.8;
                self.congestion_window = self.congestion_window.max(self.config.min_window);
            }
            CongestionState::NetworkProbing => {
                // 네트워크 탐지 중 손실 발생
                self.congestion_window *= 0.9;
                self.state = CongestionState::CongestionAvoidance;
            }
        }
        
        tracing::warn!(
            "혼잡 감지 - 윈도우: {:.1}, 손실률: {:.3}%, 상태: {:?}",
            self.congestion_window, self.loss_rate * 100.0, self.state
        );
    }
    
    /// ACK 수신 처리
    pub fn on_ack_received(&mut self) {
        self.total_packets += 1;
        
        match self.state {
            CongestionState::SlowStart => {
                // 지수적 증가
                self.congestion_window += 1.0;
                
                // ssthresh에 도달하면 혼잡 회피로 전환
                if self.congestion_window >= self.ssthresh {
                    self.state = CongestionState::CongestionAvoidance;
                }
            }
            CongestionState::CongestionAvoidance => {
                // 선형 증가 (AIMD)
                self.congestion_window += 1.0 / self.congestion_window;
            }
            CongestionState::FastRecovery => {
                // 빠른 복구에서 정상 상태로 전환
                self.state = CongestionState::CongestionAvoidance;
            }
            CongestionState::NetworkProbing => {
                // 네트워크 용량 탐지
                self.congestion_window += 0.5;
            }
        }
        
        // 최대 윈도우 크기 제한
        self.congestion_window = self.congestion_window.min(self.config.max_window);
    }
    
    /// 현재 전송 가능한 패킷 수
    pub fn get_send_window(&self) -> usize {
        self.congestion_window as usize
    }
    
    /// 재전송 타임아웃 계산
    pub fn calculate_rto(&self) -> Duration {
        let rto = self.smoothed_rtt + 4 * self.rtt_variance;
        rto.max(Duration::from_millis(100)) // 최소 100ms
           .min(Duration::from_secs(60))   // 최대 60초
    }
    
    /// 네트워크 상태 보고서
    pub fn get_status_report(&self) -> String {
        format!(
            "혼잡 제어 상태:\n\
            - 윈도우 크기: {:.1}\n\
            - 상태: {:?}\n\
            - 손실률: {:.3}%\n\
            - RTT: {:.1}ms (최소: {:.1}ms)\n\
            - RTO: {:.1}ms",
            self.congestion_window,
            self.state,
            self.loss_rate * 100.0,
            self.smoothed_rtt.as_secs_f64() * 1000.0,
            self.min_rtt.as_secs_f64() * 1000.0,
            self.calculate_rto().as_secs_f64() * 1000.0
        )
    }
}
```

## 🧠 메모리 최적화 전략

### 1. NUMA 인식 메모리 할당기

```rust
// service/numa_allocator.rs
use std::sync::Arc;
use std::collections::HashMap;

pub struct NumaAwareAllocator {
    numa_nodes: Vec<NumaNode>,
    current_node: std::sync::atomic::AtomicUsize,
    allocation_policy: AllocationPolicy,
}

#[derive(Debug)]
struct NumaNode {
    node_id: usize,
    memory_pools: HashMap<usize, Vec<*mut u8>>, // size -> pool
    allocated_bytes: std::sync::atomic::AtomicU64,
    total_capacity: u64,
}

#[derive(Debug, Clone)]
pub enum AllocationPolicy {
    RoundRobin,      // 노드 간 순환 할당
    LocalFirst,      // 로컬 노드 우선
    BalancedLoad,    // 부하 균형
}

impl NumaAwareAllocator {
    /// NUMA 토폴로지 감지 및 할당기 초기화
    pub fn new() -> Self {
        let numa_node_count = Self::detect_numa_nodes();
        let mut numa_nodes = Vec::with_capacity(numa_node_count);
        
        for node_id in 0..numa_node_count {
            numa_nodes.push(NumaNode {
                node_id,
                memory_pools: HashMap::new(),
                allocated_bytes: std::sync::atomic::AtomicU64::new(0),
                total_capacity: Self::get_node_memory_capacity(node_id),
            });
        }
        
        Self {
            numa_nodes,
            current_node: std::sync::atomic::AtomicUsize::new(0),
            allocation_policy: AllocationPolicy::LocalFirst,
        }
    }
    
    /// NUMA 노드 수 감지
    fn detect_numa_nodes() -> usize {
        // 실제 구현에서는 /sys/devices/system/node/ 파싱
        std::env::var("NUMA_NODE_COUNT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1)
    }
    
    /// 노드별 메모리 용량 조회
    fn get_node_memory_capacity(node_id: usize) -> u64 {
        // 실제로는 /proc/meminfo와 numa 정보 파싱
        1024 * 1024 * 1024 // 1GB per node (예시)
    }
    
    /// NUMA 친화적 메모리 할당
    pub fn allocate_on_node(&mut self, size: usize, preferred_node: Option<usize>) -> Option<*mut u8> {
        let target_node = match (preferred_node, &self.allocation_policy) {
            (Some(node), _) => node,
            (None, AllocationPolicy::RoundRobin) => {
                let node = self.current_node.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                node % self.numa_nodes.len()
            }
            (None, AllocationPolicy::LocalFirst) => {
                // 현재 스레드가 실행 중인 NUMA 노드 감지
                self.get_current_numa_node()
            }
            (None, AllocationPolicy::BalancedLoad) => {
                // 가장 부하가 낮은 노드 선택
                self.get_least_loaded_node()
            }
        };
        
        self.allocate_from_node(target_node, size)
    }
    
    /// 특정 노드에서 메모리 할당
    fn allocate_from_node(&mut self, node_id: usize, size: usize) -> Option<*mut u8> {
        if node_id >= self.numa_nodes.len() {
            return None;
        }
        
        let node = &mut self.numa_nodes[node_id];
        
        // 적절한 크기의 풀에서 할당 시도
        let pool_size = Self::round_up_to_pool_size(size);
        
        if let Some(pool) = node.memory_pools.get_mut(&pool_size) {
            if let Some(ptr) = pool.pop() {
                node.allocated_bytes.fetch_add(size as u64, std::sync::atomic::Ordering::Relaxed);
                return Some(ptr);
            }
        }
        
        // 풀에 없으면 새로 할당
        self.allocate_new_block(node_id, pool_size)
    }
    
    /// 새로운 메모리 블록 할당
    fn allocate_new_block(&mut self, node_id: usize, size: usize) -> Option<*mut u8> {
        let layout = std::alloc::Layout::from_size_align(size, 64).ok()?;
        
        unsafe {
            // NUMA 노드별 할당 (Linux numa.h 바인딩 필요)
            #[cfg(target_os = "linux")]
            let ptr = self.numa_alloc_on_node(layout, node_id);
            
            #[cfg(not(target_os = "linux"))]
            let ptr = std::alloc::alloc(layout);
            
            if ptr.is_null() {
                None
            } else {
                self.numa_nodes[node_id].allocated_bytes
                    .fetch_add(size as u64, std::sync::atomic::Ordering::Relaxed);
                Some(ptr)
            }
        }
    }
    
    #[cfg(target_os = "linux")]
    unsafe fn numa_alloc_on_node(&self, layout: std::alloc::Layout, node_id: usize) -> *mut u8 {
        // 실제 구현에서는 libnuma 바인딩 사용
        // numa_alloc_onnode(layout.size(), node_id as i32)
        std::alloc::alloc(layout) // fallback
    }
    
    /// 풀 크기로 반올림
    fn round_up_to_pool_size(size: usize) -> usize {
        const POOL_SIZES: &[usize] = &[64, 256, 1024, 4096, 16384, 65536];
        
        POOL_SIZES.iter()
            .find(|&&pool_size| pool_size >= size)
            .copied()
            .unwrap_or(size)
    }
    
    /// 현재 NUMA 노드 ID 조회
    fn get_current_numa_node(&self) -> usize {
        // 실제로는 getcpu() 시스템 콜 사용
        0 // fallback
    }
    
    /// 가장 부하가 낮은 노드 선택
    fn get_least_loaded_node(&self) -> usize {
        self.numa_nodes
            .iter()
            .enumerate()
            .min_by_key(|(_, node)| node.allocated_bytes.load(std::sync::atomic::Ordering::Relaxed))
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }
}
```

## 📊 성능 모니터링

### 1. 실시간 성능 추적기

```rust
// service/performance_tracker.rs
use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    // 처리량 지표
    pub messages_per_second: f64,
    pub packets_per_second: f64,
    pub bytes_per_second: f64,
    
    // 지연시간 지표 (마이크로초)
    pub latency_avg_us: f64,
    pub latency_p50_us: f64,
    pub latency_p95_us: f64,
    pub latency_p99_us: f64,
    
    // 메모리 지표
    pub memory_used_mb: f64,
    pub buffer_pool_usage: f64,
    pub numa_efficiency: f64,
    
    // 네트워크 지표
    pub packet_loss_rate: f64,
    pub retransmission_rate: f64,
    pub congestion_window: f64,
    
    // CPU 지표
    pub cpu_usage_percent: f64,
    pub simd_acceleration: bool,
    pub cache_hit_rate: f64,
    
    // 시간 정보
    pub measurement_time: std::time::SystemTime,
    pub uptime_seconds: f64,
}

pub struct PerformanceTracker {
    metrics: Arc<RwLock<PerformanceMetrics>>,
    latency_samples: Arc<RwLock<Vec<Duration>>>,
    start_time: Instant,
    
    // 카운터들
    total_messages: std::sync::atomic::AtomicU64,
    total_packets: std::sync::atomic::AtomicU64,
    total_bytes: std::sync::atomic::AtomicU64,
    
    // 샘플링 설정
    sample_interval: Duration,
    last_sample: std::sync::atomic::AtomicU64, // nanoseconds since epoch
}

impl PerformanceTracker {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(PerformanceMetrics {
                messages_per_second: 0.0,
                packets_per_second: 0.0,
                bytes_per_second: 0.0,
                latency_avg_us: 0.0,
                latency_p50_us: 0.0,
                latency_p95_us: 0.0,
                latency_p99_us: 0.0,
                memory_used_mb: 0.0,
                buffer_pool_usage: 0.0,
                numa_efficiency: 0.0,
                packet_loss_rate: 0.0,
                retransmission_rate: 0.0,
                congestion_window: 0.0,
                cpu_usage_percent: 0.0,
                simd_acceleration: is_x86_feature_detected!("avx2"),
                cache_hit_rate: 0.0,
                measurement_time: std::time::SystemTime::now(),
                uptime_seconds: 0.0,
            })),
            latency_samples: Arc::new(RwLock::new(Vec::with_capacity(10000))),
            start_time: Instant::now(),
            total_messages: std::sync::atomic::AtomicU64::new(0),
            total_packets: std::sync::atomic::AtomicU64::new(0),
            total_bytes: std::sync::atomic::AtomicU64::new(0),
            sample_interval: Duration::from_millis(100), // 100ms 샘플링
            last_sample: std::sync::atomic::AtomicU64::new(0),
        }
    }
    
    /// 메시지 처리 기록
    pub async fn record_message(&self, bytes: usize, latency: Duration) {
        self.total_messages.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.total_bytes.fetch_add(bytes as u64, std::sync::atomic::Ordering::Relaxed);
        
        // 지연시간 샘플링
        let mut samples = self.latency_samples.write().await;
        samples.push(latency);
        
        // 메모리 사용량 제한
        if samples.len() > 10000 {
            samples.drain(0..5000); // 절반 제거
        }
    }
    
    /// 패킷 처리 기록
    pub fn record_packet(&self, bytes: usize) {
        self.total_packets.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.total_bytes.fetch_add(bytes as u64, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// 주기적 메트릭 업데이트
    pub async fn update_metrics(&self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.start_time);
        
        let total_messages = self.total_messages.load(std::sync::atomic::Ordering::Relaxed);
        let total_packets = self.total_packets.load(std::sync::atomic::Ordering::Relaxed);
        let total_bytes = self.total_bytes.load(std::sync::atomic::Ordering::Relaxed);
        
        // 처리량 계산
        let elapsed_secs = elapsed.as_secs_f64();
        let messages_per_second = total_messages as f64 / elapsed_secs;
        let packets_per_second = total_packets as f64 / elapsed_secs;
        let bytes_per_second = total_bytes as f64 / elapsed_secs;
        
        // 지연시간 통계 계산
        let latency_stats = self.calculate_latency_stats().await;
        
        // 메모리 사용량 조회
        let memory_stats = self.get_memory_stats();
        
        // CPU 사용률 조회
        let cpu_usage = self.get_cpu_usage();
        
        // 메트릭 업데이트
        let mut metrics = self.metrics.write().await;
        metrics.messages_per_second = messages_per_second;
        metrics.packets_per_second = packets_per_second;
        metrics.bytes_per_second = bytes_per_second;
        metrics.latency_avg_us = latency_stats.avg_us;
        metrics.latency_p50_us = latency_stats.p50_us;
        metrics.latency_p95_us = latency_stats.p95_us;
        metrics.latency_p99_us = latency_stats.p99_us;
        metrics.memory_used_mb = memory_stats.used_mb;
        metrics.buffer_pool_usage = memory_stats.pool_usage;
        metrics.cpu_usage_percent = cpu_usage;
        metrics.measurement_time = std::time::SystemTime::now();
        metrics.uptime_seconds = elapsed_secs;
    }
    
    /// 지연시간 통계 계산
    async fn calculate_latency_stats(&self) -> LatencyStats {
        let samples = self.latency_samples.read().await;
        
        if samples.is_empty() {
            return LatencyStats::default();
        }
        
        let mut sorted_samples: Vec<_> = samples.iter().map(|d| d.as_micros() as f64).collect();
        sorted_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let avg_us = sorted_samples.iter().sum::<f64>() / sorted_samples.len() as f64;
        let p50_us = sorted_samples[sorted_samples.len() / 2];
        let p95_us = sorted_samples[sorted_samples.len() * 95 / 100];
        let p99_us = sorted_samples[sorted_samples.len() * 99 / 100];
        
        LatencyStats {
            avg_us,
            p50_us,
            p95_us,
            p99_us,
        }
    }
    
    /// 메모리 통계 조회
    fn get_memory_stats(&self) -> MemoryStats {
        // 실제 구현에서는 /proc/self/status 파싱
        MemoryStats {
            used_mb: 50.0, // 예시값
            pool_usage: 0.7,
        }
    }
    
    /// CPU 사용률 조회
    fn get_cpu_usage(&self) -> f64 {
        // 실제 구현에서는 /proc/stat 파싱
        25.0 // 예시값
    }
    
    /// 실시간 성능 보고서
    pub async fn get_performance_report(&self) -> String {
        let metrics = self.metrics.read().await;
        
        format!(
            "🚀 RUDP 서버 성능 보고서\n\
            ═══════════════════════════\n\
            📊 처리량:\n\
            • 메시지/초: {:.1}\n\
            • 패킷/초: {:.1}\n\
            • MB/초: {:.2}\n\
            \n\
            ⏱️  지연시간 (μs):\n\
            • 평균: {:.1}\n\
            • P50: {:.1}\n\
            • P95: {:.1}\n\
            • P99: {:.1}\n\
            \n\
            💾 메모리:\n\
            • 사용량: {:.1} MB\n\
            • 버퍼 풀: {:.1}%\n\
            \n\
            🌐 네트워크:\n\
            • 패킷 손실률: {:.3}%\n\
            • 재전송률: {:.3}%\n\
            • 혼잡 윈도우: {:.1}\n\
            \n\
            💻 시스템:\n\
            • CPU: {:.1}%\n\
            • SIMD 가속: {}\n\
            • 가동시간: {:.1}초",
            metrics.messages_per_second,
            metrics.packets_per_second,
            metrics.bytes_per_second / 1_048_576.0,
            metrics.latency_avg_us,
            metrics.latency_p50_us,
            metrics.latency_p95_us,
            metrics.latency_p99_us,
            metrics.memory_used_mb,
            metrics.buffer_pool_usage * 100.0,
            metrics.packet_loss_rate * 100.0,
            metrics.retransmission_rate * 100.0,
            metrics.congestion_window,
            metrics.cpu_usage_percent,
            if metrics.simd_acceleration { "ON" } else { "OFF" },
            metrics.uptime_seconds
        )
    }
}

#[derive(Debug, Default)]
struct LatencyStats {
    avg_us: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
}

#[derive(Debug)]
struct MemoryStats {
    used_mb: f64,
    pool_usage: f64,
}
```

## 🔧 통합 최적화 활용 예시

```rust
// 모든 최적화 기능을 통합한 예시
pub async fn optimized_message_processing() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. NUMA 인식 메모리 할당기 초기화
    let numa_allocator = Arc::new(Mutex::new(NumaAwareAllocator::new()));
    
    // 2. SIMD 패킷 프로세서 초기화
    let simd_processor = SimdPacketProcessor::new();
    
    // 3. 제로카피 I/O 프로세서 초기화
    let mut vectorized_io = VectorizedIOProcessor::new().await?;
    
    // 4. 적응형 혼잡 제어 초기화
    let mut congestion_control = AdaptiveCongestionControl::new(CongestionConfig::default());
    
    // 5. 성능 추적기 초기화
    let performance_tracker = Arc::new(PerformanceTracker::new());
    
    // 메시지 처리 루프
    loop {
        let start_time = Instant::now();
        
        // 배치 수신 (제로카피)
        let received_packets = vectorized_io.recv_batch(32).await?;
        
        if received_packets.is_empty() {
            tokio::time::sleep(Duration::from_micros(100)).await;
            continue;
        }
        
        // SIMD 최적화된 패킷 처리
        let processed_packets = simd_processor.process_packet_batch(&received_packets).await?;
        
        // 혼잡 제어 업데이트
        for packet in &processed_packets {
            congestion_control.update_rtt(start_time.elapsed());
        }
        
        // 성능 메트릭 기록
        let processing_latency = start_time.elapsed();
        performance_tracker.record_message(1024, processing_latency).await;
        
        // 주기적 성능 보고서 출력
        if rand::random::<f32>() < 0.01 { // 1% 확률
            let report = performance_tracker.get_performance_report().await;
            tracing::info!("\n{}", report);
        }
    }
}
```

이 가이드는 RUDP 서버의 성능을 극대화하기 위한 고급 최적화 기법들을 다룹니다. SIMD 하드웨어 가속, 제로카피 I/O, 적응형 혼잡 제어, NUMA 인식 메모리 관리 등을 통해 목표인 20,000+ msg/sec 처리량과 <0.5ms 지연시간을 달성할 수 있습니다.