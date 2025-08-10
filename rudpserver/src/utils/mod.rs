//! RUDP 전용 유틸리티 모듈
//!
//! RUDP 서버를 위한 공통 함수 및 헬퍼 기능들
//! - 패킷 처리 유틸리티
//! - 네트워크 최적화 헬퍼
//! - 성능 모니터링 도구
//! - 메모리 관리 유틸리티
//! - 시간 기반 연산 헬퍼

pub mod performance;

// 공통 imports
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// RUDP 패킷 유형 식별자
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacketType {
    /// 일반 데이터 패킷
    Data = 0x01,
    /// ACK 패킷 (확인응답)
    Ack = 0x02,
    /// NAK 패킷 (부정응답)
    Nak = 0x03,
    /// 연결 요청 (Handshake)
    Connect = 0x04,
    /// 연결 응답
    ConnectAck = 0x05,
    /// 연결 해제 요청
    Disconnect = 0x06,
    /// 연결 해제 응답
    DisconnectAck = 0x07,
    /// Keep-alive (심장박동)
    Heartbeat = 0x08,
    /// 혼잡 제어 패킷
    CongestionControl = 0x09,
    /// Ping (Keep-alive)
    Ping = 0x0A,
    /// Pong (Keep-alive 응답)  
    Pong = 0x0B,
}

impl From<u8> for PacketType {
    fn from(byte: u8) -> Self {
        match byte {
            0x01 => PacketType::Data,
            0x02 => PacketType::Ack,
            0x03 => PacketType::Nak,
            0x04 => PacketType::Connect,
            0x05 => PacketType::ConnectAck,
            0x06 => PacketType::Disconnect,
            0x07 => PacketType::DisconnectAck,
            0x08 => PacketType::Heartbeat,
            0x09 => PacketType::CongestionControl,
            0x0A => PacketType::Ping,
            0x0B => PacketType::Pong,
            _ => PacketType::Data, // 기본값
        }
    }
}

/// RUDP 패킷 헤더 구조체
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RudpPacketHeader {
    /// 패킷 유형
    pub packet_type: PacketType,
    /// 순서 번호 (16비트)
    pub sequence_number: u16,
    /// 확인응답 번호 (16비트)
    pub ack_number: u16,
    /// 체크섬 (16비트)
    pub checksum: u16,
    /// 페이로드 길이 (16비트)
    pub payload_length: u16,
    /// 플래그들 (8비트)
    pub flags: u8,
    /// 예약 필드 (8비트)
    pub reserved: u8,
}

impl RudpPacketHeader {
    /// 헤더 크기 (바이트)
    pub const SIZE: usize = 12;

    /// 새로운 헤더 생성
    pub fn new(packet_type: PacketType, sequence_number: u16, payload_length: u16) -> Self {
        Self {
            packet_type,
            sequence_number,
            ack_number: 0,
            checksum: 0,
            payload_length,
            flags: 0,
            reserved: 0,
        }
    }

    /// 바이트 배열로 직렬화
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0] = self.packet_type as u8;
        bytes[1] = self.flags;
        bytes[2..4].copy_from_slice(&self.sequence_number.to_be_bytes());
        bytes[4..6].copy_from_slice(&self.ack_number.to_be_bytes());
        bytes[6..8].copy_from_slice(&self.checksum.to_be_bytes());
        bytes[8..10].copy_from_slice(&self.payload_length.to_be_bytes());
        bytes[10] = self.reserved;
        bytes[11] = 0; // 패딩
        bytes
    }

    /// 바이트 배열에서 역직렬화
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < Self::SIZE {
            return Err(anyhow::anyhow!("Invalid header size: {}", bytes.len()));
        }

        Ok(Self {
            packet_type: PacketType::from(bytes[0]),
            flags: bytes[1],
            sequence_number: u16::from_be_bytes([bytes[2], bytes[3]]),
            ack_number: u16::from_be_bytes([bytes[4], bytes[5]]),
            checksum: u16::from_be_bytes([bytes[6], bytes[7]]),
            payload_length: u16::from_be_bytes([bytes[8], bytes[9]]),
            reserved: bytes[10],
        })
    }

    /// 체크섬 계산 및 설정
    pub fn calculate_checksum(&mut self, payload: &[u8]) {
        self.checksum = 0; // 체크섬 필드 초기화
        let header_bytes = self.to_bytes();
        self.checksum = crc16_checksum(&header_bytes, payload);
    }

    /// 체크섬 검증
    pub fn verify_checksum(&self, payload: &[u8]) -> bool {
        let mut temp_header = self.clone();
        temp_header.checksum = 0;
        let header_bytes = temp_header.to_bytes();
        let calculated_checksum = crc16_checksum(&header_bytes, payload);
        self.checksum == calculated_checksum
    }
}

/// 플래그 비트 정의
pub mod flags {
    pub const RELIABLE: u8 = 0x01; // 신뢰성 보장 필요
    pub const ORDERED: u8 = 0x02; // 순서 보장 필요
    pub const FRAGMENTED: u8 = 0x04; // 분할된 패킷
    pub const LAST_FRAGMENT: u8 = 0x08; // 마지막 분할 패킷
    pub const COMPRESSED: u8 = 0x10; // 압축된 데이터
    pub const ENCRYPTED: u8 = 0x20; // 암호화된 데이터
}

/// CRC16 체크섬 계산
pub fn crc16_checksum(header: &[u8], payload: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;

    // 헤더 체크섬 계산
    for &byte in header {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }

    // 페이로드 체크섬 계산
    for &byte in payload {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }

    !crc
}

/// 네트워크 주소 유틸리티
pub fn socket_addr_to_u64(addr: SocketAddr) -> u64 {
    use std::net::IpAddr;

    match addr.ip() {
        IpAddr::V4(ipv4) => {
            let ip_bytes = ipv4.octets();
            let port_bytes = addr.port().to_be_bytes();
            u64::from_be_bytes([
                0,
                0,
                ip_bytes[0],
                ip_bytes[1],
                ip_bytes[2],
                ip_bytes[3],
                port_bytes[0],
                port_bytes[1],
            ])
        }
        IpAddr::V6(_) => {
            // IPv6의 경우 해시 함수 사용
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            addr.hash(&mut hasher);
            hasher.finish()
        }
    }
}

/// 현재 타임스탬프 (밀리초)
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64
}

/// 현재 타임스탬프 (마이크로초)
pub fn current_timestamp_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_micros() as u64
}

/// 경과 시간 측정 헬퍼
pub struct ElapsedTimer {
    start_time: Instant,
}

impl ElapsedTimer {
    /// 새로운 타이머 시작
    pub fn start() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    /// 경과 시간 (밀리초)
    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    /// 경과 시간 (마이크로초)
    pub fn elapsed_us(&self) -> u64 {
        self.start_time.elapsed().as_micros() as u64
    }

    /// 타이머 리셋
    pub fn reset(&mut self) {
        self.start_time = Instant::now();
    }
}

/// 링 버퍼 구현 (순환 버퍼)
pub struct RingBuffer<T> {
    buffer: Vec<Option<T>>,
    head: usize,
    tail: usize,
    size: usize,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    /// 새로운 링 버퍼 생성
    pub fn new(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        buffer.resize_with(capacity, || None);

        Self {
            buffer,
            head: 0,
            tail: 0,
            size: 0,
            capacity,
        }
    }

    /// 요소 추가
    pub fn push(&mut self, item: T) -> Option<T> {
        let old_item = self.buffer[self.tail].take();
        self.buffer[self.tail] = Some(item);

        self.tail = (self.tail + 1) % self.capacity;

        if self.size < self.capacity {
            self.size += 1;
        } else {
            self.head = (self.head + 1) % self.capacity;
        }

        old_item
    }

    /// 요소 제거
    pub fn pop(&mut self) -> Option<T> {
        if self.size == 0 {
            return None;
        }

        let item = self.buffer[self.head].take();
        self.head = (self.head + 1) % self.capacity;
        self.size -= 1;

        item
    }

    /// 버퍼가 비어있는지 확인
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// 버퍼가 가득 찬지 확인
    pub fn is_full(&self) -> bool {
        self.size == self.capacity
    }

    /// 현재 크기
    pub fn len(&self) -> usize {
        self.size
    }

    /// 용량
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// 특정 인덱스의 요소에 접근 (순서: head부터)
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.size {
            return None;
        }

        let actual_index = (self.head + index) % self.capacity;
        self.buffer[actual_index].as_ref()
    }
}

/// 비트 마스크 유틸리티
pub struct BitMask {
    bits: u64,
}

impl BitMask {
    /// 새로운 비트 마스크 생성
    pub fn new() -> Self {
        Self { bits: 0 }
    }

    /// 특정 비트 설정
    pub fn set_bit(&mut self, bit: u8) {
        if bit < 64 {
            self.bits |= 1 << bit;
        }
    }

    /// 특정 비트 해제
    pub fn clear_bit(&mut self, bit: u8) {
        if bit < 64 {
            self.bits &= !(1 << bit);
        }
    }

    /// 특정 비트 토글
    pub fn toggle_bit(&mut self, bit: u8) {
        if bit < 64 {
            self.bits ^= 1 << bit;
        }
    }

    /// 특정 비트가 설정되어 있는지 확인
    pub fn is_set(&self, bit: u8) -> bool {
        if bit < 64 {
            (self.bits & (1 << bit)) != 0
        } else {
            false
        }
    }

    /// 설정된 비트 수 계산
    pub fn count_set_bits(&self) -> u32 {
        self.bits.count_ones()
    }

    /// 모든 비트 해제
    pub fn clear_all(&mut self) {
        self.bits = 0;
    }

    /// 첫 번째 설정된 비트 찾기
    pub fn find_first_set(&self) -> Option<u8> {
        if self.bits == 0 {
            None
        } else {
            Some(self.bits.trailing_zeros() as u8)
        }
    }
}

/// 슬라이딩 윈도우 평균 계산
pub struct SlidingWindowAverage {
    window: RingBuffer<f64>,
    sum: f64,
}

impl SlidingWindowAverage {
    /// 새로운 슬라이딩 윈도우 평균 생성
    pub fn new(window_size: usize) -> Self {
        Self {
            window: RingBuffer::new(window_size),
            sum: 0.0,
        }
    }

    /// 새로운 값 추가 및 평균 계산
    pub fn add_value(&mut self, value: f64) -> f64 {
        if let Some(old_value) = self.window.push(value) {
            self.sum = self.sum - old_value + value;
        } else {
            self.sum += value;
        }

        if self.window.len() > 0 {
            self.sum / self.window.len() as f64
        } else {
            0.0
        }
    }

    /// 현재 평균 값
    pub fn average(&self) -> f64 {
        if self.window.len() > 0 {
            self.sum / self.window.len() as f64
        } else {
            0.0
        }
    }

    /// 윈도우 초기화
    pub fn reset(&mut self) {
        self.window = RingBuffer::new(self.window.capacity());
        self.sum = 0.0;
    }
}

/// 지수 가중 이동 평균 (EWMA)
pub struct ExponentialMovingAverage {
    alpha: f64,
    value: f64,
    initialized: bool,
}

impl ExponentialMovingAverage {
    /// 새로운 EWMA 생성
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha: alpha.clamp(0.0, 1.0),
            value: 0.0,
            initialized: false,
        }
    }

    /// 새로운 값으로 업데이트
    pub fn update(&mut self, new_value: f64) {
        if !self.initialized {
            self.value = new_value;
            self.initialized = true;
        } else {
            self.value = self.alpha * new_value + (1.0 - self.alpha) * self.value;
        }
    }

    /// 현재 평균 값
    pub fn value(&self) -> f64 {
        self.value
    }

    /// 초기화 여부
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// 리셋
    pub fn reset(&mut self) {
        self.value = 0.0;
        self.initialized = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_header_serialization() {
        let mut header = RudpPacketHeader::new(PacketType::Data, 12345, 1000);
        header.ack_number = 54321;
        header.flags = flags::RELIABLE | flags::ORDERED;

        let payload = b"test payload data";
        header.calculate_checksum(payload);

        let bytes = header.to_bytes();
        let deserialized = RudpPacketHeader::from_bytes(&bytes).unwrap();

        assert_eq!(header.packet_type, deserialized.packet_type);
        assert_eq!(header.sequence_number, deserialized.sequence_number);
        assert_eq!(header.ack_number, deserialized.ack_number);
        assert_eq!(header.payload_length, deserialized.payload_length);
        assert_eq!(header.flags, deserialized.flags);

        assert!(deserialized.verify_checksum(payload));
    }

    #[test]
    fn test_ring_buffer() {
        let mut buffer = RingBuffer::new(3);

        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);

        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        assert!(buffer.is_full());
        assert_eq!(buffer.len(), 3);

        // 오버플로우 테스트
        let old_value = buffer.push(4);
        assert_eq!(old_value, Some(1));
        assert_eq!(buffer.get(0), Some(&2));
        assert_eq!(buffer.get(1), Some(&3));
        assert_eq!(buffer.get(2), Some(&4));
    }

    #[test]
    fn test_sliding_window_average() {
        let mut avg = SlidingWindowAverage::new(3);

        assert_eq!(avg.add_value(10.0), 10.0);
        assert_eq!(avg.add_value(20.0), 15.0);
        assert_eq!(avg.add_value(30.0), 20.0);

        // 윈도우 크기 초과
        avg.add_value(40.0);
        assert!((avg.average() - 30.0).abs() < 0.001); // (20+30+40)/3 = 30
    }

    #[test]
    fn test_bit_mask() {
        let mut mask = BitMask::new();

        mask.set_bit(5);
        mask.set_bit(10);
        mask.set_bit(15);

        assert!(mask.is_set(5));
        assert!(mask.is_set(10));
        assert!(mask.is_set(15));
        assert!(!mask.is_set(7));

        assert_eq!(mask.count_set_bits(), 3);

        mask.clear_bit(10);
        assert!(!mask.is_set(10));
        assert_eq!(mask.count_set_bits(), 2);

        assert_eq!(mask.find_first_set(), Some(5));
    }

    #[test]
    fn test_ewma() {
        let mut ewma = ExponentialMovingAverage::new(0.5);

        ewma.update(100.0);
        assert_eq!(ewma.value(), 100.0);

        ewma.update(200.0);
        assert_eq!(ewma.value(), 150.0); // 0.5 * 200 + 0.5 * 100 = 150

        ewma.update(100.0);
        assert_eq!(ewma.value(), 125.0); // 0.5 * 100 + 0.5 * 150 = 125
    }
}
