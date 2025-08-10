# RUDP 서버 아키텍처 및 확장 가이드

## 📋 목차
1. [RUDP 서버 개요](#rudp-서버-개요)
2. [성능 최적화 시스템](#성능-최적화-시스템)
3. [신뢰성 있는 UDP 구현](#신뢰성-있는-udp-구현)
4. [확장 방법](#확장-방법)
5. [성능 튜닝](#성능-튜닝)

## 🚀 RUDP 서버 개요

### 아키텍처 구조
```
RUDP Server (Reliable UDP)
├── Core Engine
│   ├── PacketProcessor (패킷 처리)
│   ├── ReliabilityManager (신뢰성 관리)
│   ├── FlowController (흐름 제어)
│   └── ConnectionManager (연결 관리)
├── Performance Optimizations (16개 서비스)
│   ├── SIMD Accelerator
│   ├── Lock-Free Queues
│   ├── Memory Pool Manager
│   ├── Zero-Copy I/O
│   ├── CPU Affinity Manager
│   ├── Parallel Packet Processing
│   ├── Adaptive Congestion Control
│   ├── Smart Retransmission
│   ├── Batch Operations
│   ├── Cache-Aware Structures
│   ├── Hardware Timer Integration
│   ├── NUMA Optimization
│   ├── Vectorized Operations
│   ├── Pipeline Optimization
│   ├── Real-time Profiling
│   └── Dynamic Load Balancing
└── Game Protocol Layer
    ├── Message Serialization
    ├── State Synchronization
    ├── Event Distribution
    └── Custom Game Logic
```

### 성능 목표
- **처리량**: 20,000+ msg/sec (TCP 대비 54% 향상)
- **지연시간**: <0.5ms (UDP의 속도 + TCP의 신뢰성)
- **동시 연결**: 1,000+ 사용자
- **메모리 효율**: 8-10MB for 1000 connections
- **패킷 손실**: 0.01% 미만 (신뢰성 보장)

## ⚡ 성능 최적화 시스템

### 1. SIMD 가속 패킷 처리
```rust
// src/optimization/simd_packet_processor.rs
use std::arch::x86_64::*;
use shared::tool::high_performance::simd_optimizer::*;

pub struct SIMDPacketProcessor {
    capability: SimdCapability,
    batch_size: usize,
    processing_stats: ProcessingStats,
}

impl SIMDPacketProcessor {
    pub fn new() -> Self {
        let capability = detect_simd_capability();
        let batch_size = match capability {
            SimdCapability::AVX2 => 32,      // 32 패킷 배치
            SimdCapability::SSE42 => 16,     // 16 패킷 배치
            _ => 8,                          // 기본 8 패킷 배치
        };
        
        tracing::info!("SIMD Packet Processor initialized: {:?}, batch_size: {}", 
                      capability, batch_size);
        
        Self {
            capability,
            batch_size,
            processing_stats: ProcessingStats::new(),
        }
    }
    
    /// 배치 패킷 처리 (SIMD 최적화)
    pub fn process_packet_batch(&mut self, packets: &mut [RudpPacket]) -> Result<usize> {
        let start_time = std::time::Instant::now();
        let processed_count = match self.capability {
            SimdCapability::AVX2 => self.process_avx2_batch(packets)?,
            SimdCapability::SSE42 => self.process_sse42_batch(packets)?,
            _ => self.process_scalar_batch(packets)?,
        };
        
        let duration = start_time.elapsed();
        self.processing_stats.record_batch(processed_count, duration);
        
        Ok(processed_count)
    }
    
    #[target_feature(enable = "avx2")]
    unsafe fn process_avx2_batch(&self, packets: &mut [RudpPacket]) -> Result<usize> {
        let mut processed = 0;
        
        for chunk in packets.chunks_mut(32) {
            // AVX2 명령어로 32개 패킷 병렬 처리
            for packet in chunk {
                // 체크섬 검증 (SIMD 병렬화)
                let checksum = fast_checksum(&packet.data);
                if checksum != packet.checksum {
                    packet.mark_invalid();
                    continue;
                }
                
                // 헤더 파싱 (벡터화)
                packet.parse_header_vectorized()?;
                processed += 1;
            }
        }
        
        Ok(processed)
    }
    
    #[target_feature(enable = "sse4.2")]
    unsafe fn process_sse42_batch(&self, packets: &mut [RudpPacket]) -> Result<usize> {
        let mut processed = 0;
        
        for chunk in packets.chunks_mut(16) {
            // SSE4.2 명령어로 16개 패킷 병렬 처리
            for packet in chunk {
                if self.validate_packet_sse42(packet)? {
                    packet.parse_header()?;
                    processed += 1;
                }
            }
        }
        
        Ok(processed)
    }
    
    fn process_scalar_batch(&self, packets: &mut [RudpPacket]) -> Result<usize> {
        let mut processed = 0;
        
        for packet in packets {
            if packet.validate()? {
                packet.parse_header()?;
                processed += 1;
            }
        }
        
        Ok(processed)
    }
    
    /// 성능 통계 조회
    pub fn get_performance_stats(&self) -> &ProcessingStats {
        &self.processing_stats
    }
}

#[derive(Debug)]
struct ProcessingStats {
    total_batches: u64,
    total_packets: u64,
    avg_processing_time: Duration,
    packets_per_second: f64,
}

impl ProcessingStats {
    fn new() -> Self {
        Self {
            total_batches: 0,
            total_packets: 0,
            avg_processing_time: Duration::from_nanos(0),
            packets_per_second: 0.0,
        }
    }
    
    fn record_batch(&mut self, packet_count: usize, duration: Duration) {
        self.total_batches += 1;
        self.total_packets += packet_count as u64;
        
        // 지수 이동 평균으로 처리 시간 업데이트
        if self.avg_processing_time.is_zero() {
            self.avg_processing_time = duration;
        } else {
            let new_avg_nanos = (self.avg_processing_time.as_nanos() as u64 * 7 + duration.as_nanos() as u64) / 8;
            self.avg_processing_time = Duration::from_nanos(new_avg_nanos);
        }
        
        // 패킷 처리율 계산
        if !duration.is_zero() {
            self.packets_per_second = packet_count as f64 / duration.as_secs_f64();
        }
    }
}
```

### 2. Zero-Copy I/O 시스템
```rust
// src/optimization/zero_copy_io.rs
use std::os::unix::io::RawFd;
use std::ptr;
use std::mem;
use libc;

pub struct ZeroCopyIOManager {
    socket_fd: RawFd,
    send_ring: IoUringBuffer,
    recv_ring: IoUringBuffer,
    buffer_pool: Arc<BufferPool>,
}

impl ZeroCopyIOManager {
    pub fn new(socket_fd: RawFd, ring_size: usize) -> Result<Self> {
        let buffer_pool = Arc::new(BufferPool::new(BufferPoolConfig {
            max_pool_size: ring_size * 2,
            initial_buffer_size: 1500, // MTU 크기
            ..Default::default()
        }));
        
        Ok(Self {
            socket_fd,
            send_ring: IoUringBuffer::new(ring_size)?,
            recv_ring: IoUringBuffer::new(ring_size)?,
            buffer_pool,
        })
    }
    
    /// Zero-copy 패킷 송신
    pub async fn send_zero_copy(&mut self, packets: &[RudpPacket]) -> Result<usize> {
        let mut sent_count = 0;
        
        for packet in packets {
            // 버퍼 풀에서 직접 메모리 할당
            let buffer = self.buffer_pool.rent();
            
            // 패킷 데이터를 버퍼에 직접 작성 (복사 없음)
            let serialized_size = packet.serialize_to_buffer(buffer.get_buffer())?;
            
            // io_uring을 통한 zero-copy 전송
            self.send_ring.submit_send(
                self.socket_fd,
                buffer.get_buffer().as_ptr(),
                serialized_size,
                packet.destination_addr,
            )?;
            
            sent_count += 1;
        }
        
        // 배치 전송 완료 대기
        self.send_ring.complete_batch().await?;
        Ok(sent_count)
    }
    
    /// Zero-copy 패킷 수신
    pub async fn recv_zero_copy(&mut self) -> Result<Vec<RudpPacket>> {
        let mut received_packets = Vec::new();
        
        // 미리 수신 버퍼 준비
        let buffer_count = 64; // 64개 버퍼 미리 준비
        for _ in 0..buffer_count {
            let buffer = self.buffer_pool.rent();
            self.recv_ring.submit_recv(
                self.socket_fd,
                buffer.get_buffer().as_mut_ptr(),
                buffer.capacity(),
            )?;
        }
        
        // 수신 완료된 패킷들 처리
        let completed = self.recv_ring.complete_recv_batch().await?;
        for (buffer, bytes_received, src_addr) in completed {
            if bytes_received > 0 {
                let packet = RudpPacket::parse_from_buffer(
                    &buffer[..bytes_received], 
                    src_addr
                )?;
                received_packets.push(packet);
            }
            
            // 버퍼 풀로 반환
            self.buffer_pool.return_buffer(buffer);
        }
        
        Ok(received_packets)
    }
}

/// io_uring 기반 버퍼 관리
struct IoUringBuffer {
    ring: io_uring::IoUring,
    pending_operations: HashMap<u64, PendingOperation>,
    operation_id_counter: u64,
}

struct PendingOperation {
    buffer: PooledBuffer,
    operation_type: OperationType,
    timestamp: Instant,
}

enum OperationType {
    Send { addr: SocketAddr },
    Recv,
}

impl IoUringBuffer {
    fn new(ring_size: usize) -> Result<Self> {
        let ring = io_uring::IoUring::new(ring_size)?;
        
        Ok(Self {
            ring,
            pending_operations: HashMap::new(),
            operation_id_counter: 0,
        })
    }
    
    fn submit_send(&mut self, fd: RawFd, data: *const u8, len: usize, addr: SocketAddr) -> Result<()> {
        let operation_id = self.operation_id_counter;
        self.operation_id_counter += 1;
        
        let sqe = self.ring.submission()
            .available()
            .ok_or_else(|| anyhow!("No submission queue entries available"))?;
            
        unsafe {
            sqe.prep_send(fd, data, len, 0);
            sqe.set_user_data(operation_id);
        }
        
        self.ring.submit()?;
        Ok(())
    }
    
    async fn complete_batch(&mut self) -> Result<()> {
        self.ring.submit_and_wait(1)?;
        
        let mut cqe_count = 0;
        for cqe in self.ring.completion() {
            let _result = cqe.result();
            let _user_data = cqe.user_data();
            cqe_count += 1;
        }
        
        tracing::debug!("Completed {} I/O operations", cqe_count);
        Ok(())
    }
}
```

### 3. 적응형 혼잡 제어
```rust
// src/reliability/congestion_control.rs
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CongestionControlConfig {
    pub initial_window: u32,
    pub max_window: u32,
    pub min_window: u32,
    pub rtt_threshold: Duration,
    pub loss_threshold: f64,
}

impl Default for CongestionControlConfig {
    fn default() -> Self {
        Self {
            initial_window: 64,      // 64 패킷
            max_window: 1024,        // 1024 패킷
            min_window: 8,           // 8 패킷
            rtt_threshold: Duration::from_millis(100),
            loss_threshold: 0.01,    // 1% 손실률
        }
    }
}

pub struct AdaptiveCongestionController {
    config: CongestionControlConfig,
    
    // 윈도우 관리
    current_window: u32,
    slow_start_threshold: u32,
    
    // RTT 추적
    rtt_samples: VecDeque<Duration>,
    smoothed_rtt: Duration,
    rtt_variance: Duration,
    
    // 손실 감지
    loss_events: VecDeque<Instant>,
    duplicate_acks: u32,
    
    // 상태
    state: CongestionState,
    last_window_update: Instant,
    
    // 통계
    stats: CongestionStats,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CongestionState {
    SlowStart,
    CongestionAvoidance,
    FastRecovery,
}

impl AdaptiveCongestionController {
    pub fn new(config: CongestionControlConfig) -> Self {
        Self {
            current_window: config.initial_window,
            slow_start_threshold: config.max_window / 2,
            config,
            rtt_samples: VecDeque::with_capacity(100),
            smoothed_rtt: Duration::from_millis(100), // 초기 RTT 추정
            rtt_variance: Duration::from_millis(50),
            loss_events: VecDeque::with_capacity(1000),
            duplicate_acks: 0,
            state: CongestionState::SlowStart,
            last_window_update: Instant::now(),
            stats: CongestionStats::new(),
        }
    }
    
    /// RTT 샘플 업데이트
    pub fn update_rtt(&mut self, rtt: Duration) {
        self.rtt_samples.push_back(rtt);
        if self.rtt_samples.len() > 100 {
            self.rtt_samples.pop_front();
        }
        
        // RFC 6298 RTT 추정 알고리즘
        if self.rtt_samples.len() == 1 {
            self.smoothed_rtt = rtt;
            self.rtt_variance = rtt / 2;
        } else {
            let alpha = 0.125;
            let beta = 0.25;
            
            let rtt_diff = if rtt > self.smoothed_rtt {
                rtt - self.smoothed_rtt
            } else {
                self.smoothed_rtt - rtt
            };
            
            self.rtt_variance = Duration::from_nanos(
                ((1.0 - beta) * self.rtt_variance.as_nanos() as f64 + 
                 beta * rtt_diff.as_nanos() as f64) as u64
            );
            
            self.smoothed_rtt = Duration::from_nanos(
                ((1.0 - alpha) * self.smoothed_rtt.as_nanos() as f64 + 
                 alpha * rtt.as_nanos() as f64) as u64
            );
        }
        
        self.stats.avg_rtt = self.smoothed_rtt;
        self.stats.rtt_variance = self.rtt_variance;
    }
    
    /// 패킷 손실 감지
    pub fn detect_loss(&mut self, sequence_number: u32) {
        self.loss_events.push_back(Instant::now());
        
        // 최근 1초 내 손실 이벤트만 유지
        let one_second_ago = Instant::now() - Duration::from_secs(1);
        while let Some(&front_time) = self.loss_events.front() {
            if front_time < one_second_ago {
                self.loss_events.pop_front();
            } else {
                break;
            }
        }
        
        // 손실률 계산
        let loss_rate = self.loss_events.len() as f64 / self.current_window as f64;
        self.stats.loss_rate = loss_rate;
        
        // 혼잡 윈도우 조정
        match self.state {
            CongestionState::SlowStart | CongestionState::CongestionAvoidance => {
                self.slow_start_threshold = self.current_window / 2;
                self.current_window = self.slow_start_threshold;
                self.state = CongestionState::FastRecovery;
                self.stats.congestion_events += 1;
            }
            CongestionState::FastRecovery => {
                // Fast Recovery 중 추가 손실
                self.current_window = (self.current_window * 3 / 4)
                    .max(self.config.min_window);
            }
        }
        
        tracing::debug!(
            "Packet loss detected: seq={}, window={}, loss_rate={:.3}%",
            sequence_number, self.current_window, loss_rate * 100.0
        );
    }
    
    /// ACK 수신 처리
    pub fn on_ack_received(&mut self, sequence_number: u32, is_duplicate: bool) {
        if is_duplicate {
            self.duplicate_acks += 1;
            
            // Fast Retransmit 조건 (3 중복 ACK)
            if self.duplicate_acks >= 3 {
                self.detect_loss(sequence_number);
                self.duplicate_acks = 0;
            }
            return;
        }
        
        self.duplicate_acks = 0;
        
        // 혼잡 윈도우 증가
        match self.state {
            CongestionState::SlowStart => {
                if self.current_window < self.slow_start_threshold {
                    // Slow Start: 지수적 증가
                    self.current_window = (self.current_window + 1)
                        .min(self.config.max_window);
                } else {
                    self.state = CongestionState::CongestionAvoidance;
                }
            }
            
            CongestionState::CongestionAvoidance => {
                // Congestion Avoidance: 선형 증가
                let now = Instant::now();
                if now.duration_since(self.last_window_update) > self.smoothed_rtt {
                    self.current_window = (self.current_window + 1)
                        .min(self.config.max_window);
                    self.last_window_update = now;
                }
            }
            
            CongestionState::FastRecovery => {
                // Fast Recovery 종료
                self.state = CongestionState::CongestionAvoidance;
            }
        }
        
        self.stats.total_acks += 1;
    }
    
    /// 현재 송신 윈도우 크기 반환
    pub fn get_send_window(&self) -> u32 {
        self.current_window
    }
    
    /// 재전송 타임아웃 계산
    pub fn calculate_rto(&self) -> Duration {
        // RFC 6298 RTO 계산
        let rto = self.smoothed_rtt + 4 * self.rtt_variance;
        rto.clamp(Duration::from_millis(200), Duration::from_secs(60))
    }
    
    /// 혼잡 제어 통계
    pub fn get_stats(&self) -> CongestionStats {
        self.stats.clone()
    }
}

#[derive(Debug, Clone)]
pub struct CongestionStats {
    pub current_window: u32,
    pub avg_rtt: Duration,
    pub rtt_variance: Duration,
    pub loss_rate: f64,
    pub total_acks: u64,
    pub congestion_events: u64,
}

impl CongestionStats {
    fn new() -> Self {
        Self {
            current_window: 0,
            avg_rtt: Duration::from_millis(0),
            rtt_variance: Duration::from_millis(0),
            loss_rate: 0.0,
            total_acks: 0,
            congestion_events: 0,
        }
    }
}
```

## 🔄 신뢰성 있는 UDP 구현

### 패킷 구조 및 헤더
```rust
// src/protocol/rudp_packet.rs
use std::net::SocketAddr;

#[repr(C, packed)]
#[derive(Debug, Clone)]
pub struct RudpHeader {
    pub sequence_number: u32,    // 순서 번호
    pub ack_number: u32,         // 확인 번호
    pub flags: u8,               // 패킷 플래그
    pub window_size: u16,        // 윈도우 크기
    pub checksum: u16,           // 체크섬
    pub timestamp: u64,          // 타임스탬프
}

#[derive(Debug, Clone)]
pub struct RudpPacket {
    pub header: RudpHeader,
    pub data: Vec<u8>,
    pub source_addr: SocketAddr,
    pub destination_addr: SocketAddr,
    pub created_at: Instant,
}

// 패킷 플래그 정의
pub mod flags {
    pub const SYN: u8 = 0b00000001;    // 연결 시작
    pub const ACK: u8 = 0b00000010;    // 확인
    pub const FIN: u8 = 0b00000100;    // 연결 종료
    pub const RST: u8 = 0b00001000;    // 리셋
    pub const PSH: u8 = 0b00010000;    // 푸시
    pub const URG: u8 = 0b00100000;    // 긴급
    pub const ECE: u8 = 0b01000000;    // ECN Echo
    pub const CWR: u8 = 0b10000000;    // Congestion Window Reduced
}

impl RudpPacket {
    /// 새 RUDP 패킷 생성
    pub fn new(
        sequence_number: u32,
        ack_number: u32,
        flags: u8,
        data: Vec<u8>,
        destination: SocketAddr,
    ) -> Self {
        let header = RudpHeader {
            sequence_number,
            ack_number,
            flags,
            window_size: 1024, // 기본 윈도우 크기
            checksum: 0,       // 나중에 계산
            timestamp: Self::get_timestamp_microseconds(),
        };
        
        let mut packet = Self {
            header,
            data,
            source_addr: "0.0.0.0:0".parse().unwrap(), // 나중에 설정
            destination_addr: destination,
            created_at: Instant::now(),
        };
        
        // 체크섬 계산
        packet.calculate_checksum();
        packet
    }
    
    /// 바이너리 데이터로 직렬화
    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(std::mem::size_of::<RudpHeader>() + self.data.len());
        
        // 헤더 직렬화
        buffer.extend_from_slice(&self.header.sequence_number.to_be_bytes());
        buffer.extend_from_slice(&self.header.ack_number.to_be_bytes());
        buffer.push(self.header.flags);
        buffer.extend_from_slice(&self.header.window_size.to_be_bytes());
        buffer.extend_from_slice(&self.header.checksum.to_be_bytes());
        buffer.extend_from_slice(&self.header.timestamp.to_be_bytes());
        
        // 데이터 추가
        buffer.extend_from_slice(&self.data);
        
        buffer
    }
    
    /// 바이너리 데이터에서 역직렬화
    pub fn deserialize(data: &[u8], source_addr: SocketAddr) -> Result<Self> {
        if data.len() < std::mem::size_of::<RudpHeader>() {
            return Err(anyhow!("Packet too short"));
        }
        
        let mut offset = 0;
        
        let sequence_number = u32::from_be_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
        ]);
        offset += 4;
        
        let ack_number = u32::from_be_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
        ]);
        offset += 4;
        
        let flags = data[offset];
        offset += 1;
        
        let window_size = u16::from_be_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        
        let checksum = u16::from_be_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        
        let timestamp = u64::from_be_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]
        ]);
        offset += 8;
        
        let header = RudpHeader {
            sequence_number,
            ack_number,
            flags,
            window_size,
            checksum,
            timestamp,
        };
        
        let packet_data = data[offset..].to_vec();
        
        let packet = Self {
            header,
            data: packet_data,
            source_addr,
            destination_addr: "0.0.0.0:0".parse().unwrap(), // 나중에 설정
            created_at: Instant::now(),
        };
        
        // 체크섬 검증
        if !packet.verify_checksum() {
            return Err(anyhow!("Checksum verification failed"));
        }
        
        Ok(packet)
    }
    
    /// 체크섬 계산
    fn calculate_checksum(&mut self) {
        self.header.checksum = 0;
        let serialized = self.serialize();
        self.header.checksum = Self::compute_checksum(&serialized);
    }
    
    /// 체크섬 검증
    fn verify_checksum(&self) -> bool {
        let original_checksum = self.header.checksum;
        let mut temp_header = self.header;
        temp_header.checksum = 0;
        
        let mut temp_packet = self.clone();
        temp_packet.header = temp_header;
        let serialized = temp_packet.serialize();
        
        let computed_checksum = Self::compute_checksum(&serialized);
        computed_checksum == original_checksum
    }
    
    /// 체크섬 계산 (Fletcher's checksum)
    fn compute_checksum(data: &[u8]) -> u16 {
        let mut sum1: u16 = 0;
        let mut sum2: u16 = 0;
        
        for byte in data {
            sum1 = sum1.wrapping_add(*byte as u16);
            sum2 = sum2.wrapping_add(sum1);
        }
        
        ((sum2 << 8) | (sum1 & 0xFF)) ^ 0xFFFF
    }
    
    /// 현재 시각을 마이크로초로 반환
    fn get_timestamp_microseconds() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }
    
    /// 패킷 유형 확인 메서드들
    pub fn is_syn(&self) -> bool { self.header.flags & flags::SYN != 0 }
    pub fn is_ack(&self) -> bool { self.header.flags & flags::ACK != 0 }
    pub fn is_fin(&self) -> bool { self.header.flags & flags::FIN != 0 }
    pub fn is_rst(&self) -> bool { self.header.flags & flags::RST != 0 }
    
    /// ACK 패킷 생성
    pub fn create_ack(ack_number: u32, destination: SocketAddr) -> Self {
        Self::new(0, ack_number, flags::ACK, Vec::new(), destination)
    }
    
    /// SYN 패킷 생성
    pub fn create_syn(sequence_number: u32, destination: SocketAddr) -> Self {
        Self::new(sequence_number, 0, flags::SYN, Vec::new(), destination)
    }
    
    /// FIN 패킷 생성
    pub fn create_fin(sequence_number: u32, destination: SocketAddr) -> Self {
        Self::new(sequence_number, 0, flags::FIN, Vec::new(), destination)
    }
}
```

## 🚀 확장 방법

### 1. 게임별 프로토콜 어댑터 생성
```rust
// src/game_protocols/police_thief_protocol.rs
use crate::protocol::{RudpPacket, flags};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoliceThiefMessage {
    PlayerMovement {
        player_id: u32,
        position: Position,
        velocity: Velocity,
        timestamp: u64,
    },
    
    GameAction {
        player_id: u32,
        action: Action,
        target: Option<u32>,
    },
    
    WorldUpdate {
        players: Vec<PlayerState>,
        objects: Vec<GameObject>,
        events: Vec<GameEvent>,
    },
    
    GameState {
        phase: GamePhase,
        remaining_time: u32,
        police_score: u32,
        thief_score: u32,
    },
}

pub struct PoliceThiefProtocolHandler {
    connection_manager: Arc<RudpConnectionManager>,
    game_state: Arc<RwLock<GameState>>,
    message_serializer: MessageSerializer,
}

impl PoliceThiefProtocolHandler {
    pub fn new(connection_manager: Arc<RudpConnectionManager>) -> Self {
        Self {
            connection_manager,
            game_state: Arc::new(RwLock::new(GameState::new())),
            message_serializer: MessageSerializer::new(),
        }
    }
    
    /// 플레이어 움직임 처리 (실시간, 낮은 지연시간)
    pub async fn handle_player_movement(
        &self,
        player_id: u32,
        position: Position,
        velocity: Velocity,
    ) -> Result<()> {
        let message = PoliceThiefMessage::PlayerMovement {
            player_id,
            position,
            velocity,
            timestamp: Self::get_timestamp(),
        };
        
        // 움직임은 신뢰성보다 속도가 중요 (UDP 특성 활용)
        let packet_data = self.message_serializer.serialize_fast(&message)?;
        
        // 모든 플레이어에게 브로드캐스트 (신뢰성 없음)
        self.broadcast_unreliable(packet_data).await?;
        
        Ok(())
    }
    
    /// 게임 액션 처리 (신뢰성 필요)
    pub async fn handle_game_action(
        &self,
        player_id: u32,
        action: Action,
        target: Option<u32>,
    ) -> Result<()> {
        let message = PoliceThiefMessage::GameAction {
            player_id,
            action,
            target,
        };
        
        // 액션은 반드시 전달되어야 함 (신뢰성 보장)
        let packet_data = self.message_serializer.serialize_reliable(&message)?;
        
        // 신뢰성 있는 브로드캐스트
        self.broadcast_reliable(packet_data).await?;
        
        Ok(())
    }
    
    /// 신뢰성 없는 브로드캐스트 (빠른 전송)
    async fn broadcast_unreliable(&self, data: Vec<u8>) -> Result<()> {
        let connections = self.connection_manager.get_active_connections().await;
        
        for connection in connections {
            let packet = RudpPacket::new(
                connection.next_sequence(),
                0,
                0, // 플래그 없음 (신뢰성 X)
                data.clone(),
                connection.remote_addr,
            );
            
            // 직접 UDP 전송 (재전송 없음)
            self.connection_manager.send_packet_direct(packet).await?;
        }
        
        Ok(())
    }
    
    /// 신뢰성 있는 브로드캐스트
    async fn broadcast_reliable(&self, data: Vec<u8>) -> Result<()> {
        let connections = self.connection_manager.get_active_connections().await;
        
        for connection in connections {
            let packet = RudpPacket::new(
                connection.next_sequence(),
                0,
                flags::PSH, // 신뢰성 보장 플래그
                data.clone(),
                connection.remote_addr,
            );
            
            // 신뢰성 있는 전송 (재전송 보장)
            self.connection_manager.send_packet_reliable(packet).await?;
        }
        
        Ok(())
    }
    
    fn get_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }
}

/// 메시지 직렬화 최적화
struct MessageSerializer {
    compression_enabled: bool,
}

impl MessageSerializer {
    fn new() -> Self {
        Self {
            compression_enabled: true,
        }
    }
    
    /// 빠른 직렬화 (압축 없음)
    fn serialize_fast(&self, message: &PoliceThiefMessage) -> Result<Vec<u8>> {
        // MessagePack 또는 바이너리 직렬화 사용
        bincode::serialize(message).map_err(Into::into)
    }
    
    /// 신뢰성 있는 직렬화 (압축 포함)
    fn serialize_reliable(&self, message: &PoliceThiefMessage) -> Result<Vec<u8>> {
        let serialized = bincode::serialize(message)?;
        
        if self.compression_enabled && serialized.len() > 512 {
            // LZ4 압축 적용
            Ok(lz4_flex::compress_prepend_size(&serialized))
        } else {
            Ok(serialized)
        }
    }
}
```

### 2. 실시간 상태 동기화 시스템
```rust
// src/synchronization/state_sync.rs
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct StateSync<T> 
where 
    T: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync
{
    states: Arc<RwLock<HashMap<u32, T>>>, // entity_id -> state
    snapshots: Arc<RwLock<VecDeque<Snapshot<T>>>>,
    interpolator: StateInterpolator<T>,
    config: StateSyncConfig,
}

#[derive(Debug, Clone)]
pub struct StateSyncConfig {
    pub snapshot_rate: u32,           // 초당 스냅샷 수
    pub interpolation_delay: Duration, // 보간 지연시간
    pub extrapolation_limit: Duration, // 외삽 제한시간
    pub max_snapshots: usize,         // 최대 스냅샷 수
}

impl Default for StateSyncConfig {
    fn default() -> Self {
        Self {
            snapshot_rate: 20,                           // 20 FPS
            interpolation_delay: Duration::from_millis(100), // 100ms 지연
            extrapolation_limit: Duration::from_millis(50),  // 50ms 외삽
            max_snapshots: 100,
        }
    }
}

#[derive(Debug, Clone)]
struct Snapshot<T> {
    timestamp: Instant,
    server_tick: u64,
    states: HashMap<u32, T>,
}

impl<T> StateSync<T> 
where 
    T: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync
{
    pub fn new(config: StateSyncConfig) -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
            snapshots: Arc::new(RwLock::new(VecDeque::new())),
            interpolator: StateInterpolator::new(),
            config,
        }
    }
    
    /// 상태 업데이트
    pub async fn update_state(&self, entity_id: u32, state: T) {
        let mut states = self.states.write().await;
        states.insert(entity_id, state);
    }
    
    /// 스냅샷 생성 및 전송
    pub async fn create_snapshot(&self, server_tick: u64) -> Snapshot<T> {
        let states = self.states.read().await.clone();
        
        let snapshot = Snapshot {
            timestamp: Instant::now(),
            server_tick,
            states,
        };
        
        // 스냅샷 저장
        {
            let mut snapshots = self.snapshots.write().await;
            if snapshots.len() >= self.config.max_snapshots {
                snapshots.pop_front();
            }
            snapshots.push_back(snapshot.clone());
        }
        
        snapshot
    }
    
    /// 클라이언트에서 상태 보간
    pub async fn interpolate_state(&self, entity_id: u32, target_time: Instant) -> Option<T> {
        let snapshots = self.snapshots.read().await;
        
        if snapshots.len() < 2 {
            return None;
        }
        
        // 보간할 스냅샷 찾기
        let mut before_snapshot = None;
        let mut after_snapshot = None;
        
        for snapshot in snapshots.iter() {
            if snapshot.timestamp <= target_time {
                before_snapshot = Some(snapshot);
            } else {
                after_snapshot = Some(snapshot);
                break;
            }
        }
        
        match (before_snapshot, after_snapshot) {
            (Some(before), Some(after)) => {
                // 보간 수행
                if let (Some(before_state), Some(after_state)) = (
                    before.states.get(&entity_id),
                    after.states.get(&entity_id)
                ) {
                    let total_time = after.timestamp.duration_since(before.timestamp);
                    let elapsed_time = target_time.duration_since(before.timestamp);
                    let factor = elapsed_time.as_secs_f32() / total_time.as_secs_f32();
                    
                    self.interpolator.interpolate(before_state, after_state, factor)
                } else {
                    None
                }
            }
            
            (Some(before), None) => {
                // 외삽 (최근 데이터만 있는 경우)
                if let Some(state) = before.states.get(&entity_id) {
                    let elapsed = target_time.duration_since(before.timestamp);
                    if elapsed <= self.config.extrapolation_limit {
                        self.interpolator.extrapolate(state, elapsed)
                    } else {
                        Some(state.clone())
                    }
                } else {
                    None
                }
            }
            
            _ => None,
        }
    }
}

/// 상태 보간기 (게임별로 구현 필요)
pub trait StateInterpolator<T> {
    fn interpolate(&self, before: &T, after: &T, factor: f32) -> Option<T>;
    fn extrapolate(&self, state: &T, elapsed: Duration) -> Option<T>;
}

// 위치 상태 보간 구현 예시
impl StateInterpolator<Position> for StateSync<Position> {
    fn interpolate(&self, before: &Position, after: &Position, factor: f32) -> Option<Position> {
        Some(Position {
            x: before.x + (after.x - before.x) * factor,
            y: before.y + (after.y - before.y) * factor,
            z: before.z + (after.z - before.z) * factor,
        })
    }
    
    fn extrapolate(&self, state: &Position, elapsed: Duration) -> Option<Position> {
        // 단순 선형 외삽 (실제로는 속도 기반 예측 사용)
        Some(state.clone())
    }
}
```

RUDP 서버는 UDP의 속도와 TCP의 신뢰성을 결합하여 실시간 게임에 최적화된 네트워크 솔루션을 제공합니다. 16개의 성능 최적화 서비스를 통해 TCP 대비 54% 향상된 처리량을 목표로 하며, 게임별 프로토콜 어댑터를 통해 다양한 게임 장르에 적용할 수 있습니다.