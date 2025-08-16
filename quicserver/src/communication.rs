//! Communication Optimization Utilities
//!
//! 통신 최적화를 위한 유틸리티들을 제공합니다.
//! 압축, 시퀀싱, 체크섬, 델타 처리 등의 기능이 포함되어 있습니다.

use crate::optimization::optimizer::QuicOptimizer;
use anyhow::Result;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::trace;

/// 통신 최적화를 위한 메시지 프로세서
/// 게임 로직과 분리된 순수 통신 최적화 기능만 제공
pub struct MessageProcessor {
    optimizer: Arc<QuicOptimizer>,
    sequence_counter: AtomicU64,
}

impl MessageProcessor {
    pub fn new(optimizer: Arc<QuicOptimizer>) -> Self {
        Self {
            optimizer,
            sequence_counter: AtomicU64::new(0),
        }
    }

    /// 메시지를 압축하고 시퀀스를 추가
    pub async fn prepare_message(&self, data: &[u8]) -> Result<Vec<u8>> {
        let sequence = self.next_sequence();

        // 메시지에 시퀀스 헤더 추가
        let mut message = BytesMut::new();
        message.extend_from_slice(&sequence.to_le_bytes());
        message.extend_from_slice(data);

        // 압축이 유익한 경우에만 압축
        let compressed = self.optimizer.compress_if_beneficial(&message);

        trace!(
            "Message prepared: {} -> {} bytes (seq: {})",
            data.len(),
            compressed.len(),
            sequence
        );
        Ok(compressed)
    }

    /// 압축된 메시지를 해제하고 시퀀스 검증
    pub async fn process_message(&self, data: &[u8]) -> Result<(u64, Vec<u8>)> {
        // 압축 해제
        let decompressed = if self.is_compressed(data) {
            self.optimizer.decompress(data)?
        } else {
            data.to_vec()
        };

        // 시퀀스 헤더 추출
        if decompressed.len() < 8 {
            return Err(anyhow::anyhow!("Message too short for sequence header"));
        }

        let sequence_bytes: [u8; 8] = decompressed[0..8].try_into()?;
        let sequence = u64::from_le_bytes(sequence_bytes);
        let payload = decompressed[8..].to_vec();

        trace!(
            "Message processed: {} bytes (seq: {})",
            payload.len(),
            sequence
        );
        Ok((sequence, payload))
    }

    /// 게임 상태 델타 압축
    /// 이전 상태와 현재 상태의 차이만 전송하여 대역폭 절약
    pub async fn create_state_delta(
        &self,
        previous_state: &[u8],
        current_state: &[u8],
    ) -> Result<Vec<u8>> {
        let delta = self.calculate_delta(previous_state, current_state);
        let compressed_delta = self.optimizer.compress_if_beneficial(&delta);

        trace!(
            "State delta created: {} -> {} bytes",
            delta.len(),
            compressed_delta.len()
        );
        Ok(compressed_delta)
    }

    /// 게임 상태 델타 적용
    /// 이전 상태에 델타를 적용하여 새로운 상태 생성
    pub async fn apply_state_delta(&self, previous_state: &[u8], delta: &[u8]) -> Result<Vec<u8>> {
        let decompressed_delta = if self.is_compressed(delta) {
            self.optimizer.decompress(delta)?
        } else {
            delta.to_vec()
        };

        let new_state = self.apply_delta(previous_state, &decompressed_delta);
        trace!(
            "State delta applied: {} + {} -> {} bytes",
            previous_state.len(),
            decompressed_delta.len(),
            new_state.len()
        );

        Ok(new_state)
    }

    /// 메시지 무결성 검증을 위한 체크섬 생성
    pub fn create_checksum(&self, data: &[u8]) -> u32 {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(data);
        hasher.finalize()
    }

    /// 체크섬 검증
    pub fn verify_checksum(&self, data: &[u8], expected_checksum: u32) -> bool {
        let actual_checksum = self.create_checksum(data);
        actual_checksum == expected_checksum
    }

    /// 벌크 데이터 스트리밍 준비
    /// 큰 파일이나 맵 데이터 등을 청크 단위로 분할
    pub fn prepare_bulk_stream(&self, data: &[u8], chunk_size: usize) -> Vec<BulkChunk> {
        let total_chunks = data.len().div_ceil(chunk_size);
        let stream_id = uuid::Uuid::new_v4().to_string();

        data.chunks(chunk_size)
            .enumerate()
            .map(|(index, chunk)| BulkChunk {
                stream_id: stream_id.clone(),
                chunk_index: index as u32,
                total_chunks: total_chunks as u32,
                data: chunk.to_vec(),
                checksum: self.create_checksum(chunk),
            })
            .collect()
    }

    /// 음성 패킷 최적화 처리
    /// 실시간 음성 통신을 위한 특별한 처리
    pub async fn process_voice_packet(&self, packet: &[u8]) -> Result<Vec<u8>> {
        // 음성 패킷은 지연시간이 중요하므로 압축하지 않음
        trace!("Voice packet processed: {} bytes", packet.len());
        Ok(packet.to_vec())
    }

    /// 단방향 데이터 처리 (텔레메트리, 로그 등)
    pub async fn process_unidirectional_data(&self, data: &[u8]) -> Result<()> {
        // 단방향 데이터는 응답이 필요 없는 데이터
        // 분석, 모니터링 등에 사용
        trace!("Unidirectional data processed: {} bytes", data.len());
        Ok(())
    }

    /// 다음 시퀀스 번호 생성
    pub fn next_sequence(&self) -> u64 {
        self.sequence_counter.fetch_add(1, Ordering::SeqCst)
    }

    /// 현재 시퀀스 번호 조회
    pub fn current_sequence(&self) -> u64 {
        self.sequence_counter.load(Ordering::SeqCst)
    }

    // 내부 유틸리티 메소드들

    fn is_compressed(&self, data: &[u8]) -> bool {
        // 간단한 압축 감지 로직 (실제로는 더 정교한 방법 사용)
        data.len() > 4 && &data[0..4] == b"COMP"
    }

    fn calculate_delta(&self, previous: &[u8], current: &[u8]) -> Vec<u8> {
        // 간단한 XOR 기반 델타 계산 (실제로는 더 정교한 알고리즘 사용)
        let max_len = previous.len().max(current.len());
        let mut delta = Vec::with_capacity(max_len);

        for i in 0..max_len {
            let prev_byte = previous.get(i).copied().unwrap_or(0);
            let curr_byte = current.get(i).copied().unwrap_or(0);
            delta.push(prev_byte ^ curr_byte);
        }

        delta
    }

    fn apply_delta(&self, previous: &[u8], delta: &[u8]) -> Vec<u8> {
        // 델타를 적용하여 새로운 상태 생성
        let max_len = previous.len().max(delta.len());
        let mut result = Vec::with_capacity(max_len);

        for i in 0..max_len {
            let prev_byte = previous.get(i).copied().unwrap_or(0);
            let delta_byte = delta.get(i).copied().unwrap_or(0);
            result.push(prev_byte ^ delta_byte);
        }

        result
    }
}

/// 벌크 데이터 전송을 위한 청크 구조체
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkChunk {
    pub stream_id: String,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub data: Vec<u8>,
    pub checksum: u32,
}

/// 스트림 타입별 최적화 설정
#[derive(Debug, Clone)]
pub struct StreamOptimization {
    pub enable_compression: bool,
    pub compression_threshold: usize,
    pub enable_delta_compression: bool,
    pub priority_level: u8,
}

impl StreamOptimization {
    /// 제어 메시지용 최적화 설정
    pub fn control_stream() -> Self {
        Self {
            enable_compression: true,
            compression_threshold: 256,
            enable_delta_compression: false,
            priority_level: 1,
        }
    }

    /// 게임 상태용 최적화 설정  
    pub fn game_state_stream() -> Self {
        Self {
            enable_compression: true,
            compression_threshold: 128,
            enable_delta_compression: true,
            priority_level: 0, // 최고 우선순위
        }
    }

    /// 채팅용 최적화 설정
    pub fn chat_stream() -> Self {
        Self {
            enable_compression: false,
            compression_threshold: 1024,
            enable_delta_compression: false,
            priority_level: 3,
        }
    }

    /// 음성용 최적화 설정
    pub fn voice_stream() -> Self {
        Self {
            enable_compression: false,
            compression_threshold: usize::MAX,
            enable_delta_compression: false,
            priority_level: 0, // 최고 우선순위
        }
    }

    /// 벌크 데이터용 최적화 설정
    pub fn bulk_stream() -> Self {
        Self {
            enable_compression: true,
            compression_threshold: 1024,
            enable_delta_compression: false,
            priority_level: 5, // 낮은 우선순위
        }
    }
}

/// 통신 성능 메트릭
#[derive(Debug, Clone, Default)]
pub struct CommunicationMetrics {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub compression_ratio: f64,
    pub average_latency_ms: f64,
    pub packet_loss_rate: f64,
}

impl CommunicationMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_sent_message(&mut self, bytes: u64) {
        self.messages_sent += 1;
        self.bytes_sent += bytes;
    }

    pub fn record_received_message(&mut self, bytes: u64) {
        self.messages_received += 1;
        self.bytes_received += bytes;
    }

    pub fn update_compression_ratio(&mut self, original_size: u64, compressed_size: u64) {
        self.compression_ratio = compressed_size as f64 / original_size as f64;
    }

    pub fn update_latency(&mut self, latency_ms: f64) {
        // 간단한 이동 평균
        self.average_latency_ms = (self.average_latency_ms * 0.9) + (latency_ms * 0.1);
    }
}
