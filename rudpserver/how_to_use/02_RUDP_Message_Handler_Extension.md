# RUDP 서버 메시지 핸들러 확장 가이드

## 📋 목차
1. [RUDP 메시지 핸들러 구조](#rudp-메시지-핸들러-구조)
2. [실시간 게임 메시지 처리](#실시간-게임-메시지-처리)
3. [SIMD 최적화 메시지 핸들러](#simd-최적화-메시지-핸들러)
4. [RUDP 프로토콜 확장](#rudp-프로토콜-확장)
5. [성능 최적화 패턴](#성능-최적화-패턴)

## 🔧 RUDP 메시지 핸들러 구조

### 현재 핸들러 계층구조
```
RUDP Message Handlers
├── RudpMessageProcessor (SIMD 최적화)
├── RealTimeGameHandler (실시간 게임 로직)
├── PacketFragmentationHandler (패킷 분할/재조합)
├── ReliabilityHandler (신뢰성 보장)
├── CongestionControlHandler (혼잡 제어)
└── ZeroCopyIOHandler (제로카피 I/O)
```

### RUDP 프로토콜 구조
```
RUDP Packet Format:
[2-byte seq][1-byte flags][1-byte priority][4-byte timestamp][payload]

Flags:
- ACK (0x01): 확인응답
- SYN (0x02): 연결 시작
- FIN (0x04): 연결 종료
- RST (0x08): 연결 리셋
- FRAG (0x10): 분할된 패킷
- URGENT (0x20): 긴급 패킷
```

## 🚀 실시간 게임 메시지 처리

### 1. 실시간 위치 동기화 핸들러

```rust
// handlers/realtime_position_handler.rs
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use glam::{Vec3, Quat}; // 3D 수학 라이브러리
use crate::protocol::rudp::{RudpMessage, RudpPriority};
use crate::service::zero_copy_io::ZeroCopyBuffer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPosition {
    pub user_id: u32,
    pub position: Vec3,
    pub rotation: Quat,
    pub velocity: Vec3,
    pub timestamp: u64,
    pub sequence: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub room_id: u32,
    pub players: HashMap<u32, PlayerPosition>,
    pub last_update: u64,
    pub tick_rate: f32, // 초당 업데이트 수
}

pub struct RealTimePositionHandler {
    game_states: Arc<RwLock<HashMap<u32, GameState>>>, // room_id -> GameState
    interpolation_buffer: Arc<RwLock<HashMap<u32, Vec<PlayerPosition>>>>, // 보간용 버퍼
    config: PositionConfig,
}

#[derive(Debug, Clone)]
pub struct PositionConfig {
    pub max_position_delta: f32, // 최대 위치 변화량
    pub interpolation_time: f32,  // 보간 시간 (초)
    pub extrapolation_limit: f32, // 외삽 제한
    pub tick_rate: f32,           // 서버 틱 레이트
}

impl Default for PositionConfig {
    fn default() -> Self {
        Self {
            max_position_delta: 100.0, // 100 유닛
            interpolation_time: 0.1,   // 100ms
            extrapolation_limit: 0.05, // 50ms
            tick_rate: 60.0,           // 60 FPS
        }
    }
}

impl RealTimePositionHandler {
    pub fn new() -> Self {
        Self {
            game_states: Arc::new(RwLock::new(HashMap::new())),
            interpolation_buffer: Arc::new(RwLock::new(HashMap::new())),
            config: PositionConfig::default(),
        }
    }
    
    /// 플레이어 위치 업데이트 처리
    pub async fn handle_position_update(
        &self, 
        room_id: u32, 
        position: PlayerPosition
    ) -> Result<Vec<RudpMessage>, Box<dyn std::error::Error + Send + Sync>> {
        let current_time = self.get_current_timestamp();
        
        // 위치 검증
        if !self.validate_position_update(&position, current_time).await? {
            return Ok(vec![]); // 잘못된 위치 업데이트 무시
        }
        
        // 게임 상태 업데이트
        let mut game_states = self.game_states.write().await;
        let game_state = game_states.entry(room_id).or_insert_with(|| GameState {
            room_id,
            players: HashMap::new(),
            last_update: current_time,
            tick_rate: self.config.tick_rate,
        });
        
        // 이전 위치와 비교하여 보간 처리
        let interpolated_position = if let Some(prev_pos) = game_state.players.get(&position.user_id) {
            self.interpolate_position(prev_pos, &position, current_time).await?
        } else {
            position.clone()
        };
        
        // 위치 업데이트
        game_state.players.insert(position.user_id, interpolated_position.clone());
        game_state.last_update = current_time;
        
        // 다른 플레이어들에게 브로드캐스트
        let mut broadcast_messages = Vec::new();
        
        // 관심 영역(AOI) 기반 최적화
        let interested_players = self.get_players_in_area_of_interest(
            room_id, 
            &interpolated_position
        ).await?;
        
        for interested_player_id in interested_players {
            if interested_player_id != position.user_id {
                broadcast_messages.push(RudpMessage::PositionUpdate {
                    room_id,
                    position: interpolated_position.clone(),
                    priority: RudpPriority::High, // 실시간 데이터는 높은 우선순위
                });
            }
        }
        
        Ok(broadcast_messages)
    }
    
    /// 위치 업데이트 검증 (치팅 방지)
    async fn validate_position_update(
        &self, 
        position: &PlayerPosition, 
        current_time: u64
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let game_states = self.game_states.read().await;
        
        // 타임스탬프 검증
        if position.timestamp > current_time + 1000 { // 1초 미래까지만 허용
            return Ok(false);
        }
        
        // 속도 검증 (이전 위치와 비교)
        if let Some(game_state) = game_states.values().find(|gs| gs.players.contains_key(&position.user_id)) {
            if let Some(prev_pos) = game_state.players.get(&position.user_id) {
                let time_delta = (current_time - prev_pos.timestamp) as f32 / 1000.0;
                if time_delta > 0.0 {
                    let distance = position.position.distance(prev_pos.position);
                    let max_distance = self.config.max_position_delta * time_delta;
                    
                    if distance > max_distance {
                        tracing::warn!(
                            "플레이어 {} 비정상적 위치 변화: {}유닛/{}초", 
                            position.user_id, distance, time_delta
                        );
                        return Ok(false);
                    }
                }
            }
        }
        
        Ok(true)
    }
    
    /// 위치 보간 처리
    async fn interpolate_position(
        &self, 
        prev_pos: &PlayerPosition, 
        new_pos: &PlayerPosition,
        current_time: u64
    ) -> Result<PlayerPosition, Box<dyn std::error::Error + Send + Sync>> {
        let time_delta = (new_pos.timestamp - prev_pos.timestamp) as f32 / 1000.0;
        
        // 보간 계수 계산
        let interpolation_factor = if time_delta > 0.0 {
            (self.config.interpolation_time / time_delta).min(1.0)
        } else {
            1.0
        };
        
        // 선형 보간
        let interpolated_position = prev_pos.position.lerp(new_pos.position, interpolation_factor);
        let interpolated_rotation = prev_pos.rotation.slerp(new_pos.rotation, interpolation_factor);
        
        Ok(PlayerPosition {
            user_id: new_pos.user_id,
            position: interpolated_position,
            rotation: interpolated_rotation,
            velocity: new_pos.velocity,
            timestamp: current_time,
            sequence: new_pos.sequence,
        })
    }
    
    /// 관심 영역(AOI) 내 플레이어 조회
    async fn get_players_in_area_of_interest(
        &self, 
        room_id: u32, 
        position: &PlayerPosition
    ) -> Result<Vec<u32>, Box<dyn std::error::Error + Send + Sync>> {
        let game_states = self.game_states.read().await;
        let mut interested_players = Vec::new();
        
        if let Some(game_state) = game_states.get(&room_id) {
            const AOI_RADIUS: f32 = 50.0; // 50 유닛 반경
            
            for (player_id, player_pos) in &game_state.players {
                let distance = position.position.distance(player_pos.position);
                if distance <= AOI_RADIUS {
                    interested_players.push(*player_id);
                }
            }
        }
        
        Ok(interested_players)
    }
    
    fn get_current_timestamp(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}
```

### 2. SIMD 최적화 메시지 핸들러

```rust
// handlers/simd_message_handler.rs
use std::arch::x86_64::*;
use crate::service::simd_optimizer::*;
use crate::protocol::rudp::RudpPacket;

pub struct SimdMessageHandler {
    packet_buffer: Vec<RudpPacket>,
    batch_size: usize,
}

impl SimdMessageHandler {
    pub fn new(batch_size: usize) -> Self {
        Self {
            packet_buffer: Vec::with_capacity(batch_size * 2),
            batch_size,
        }
    }
    
    /// 배치 패킷 처리 (SIMD 최적화)
    pub async fn process_packet_batch(
        &mut self, 
        packets: &[RudpPacket]
    ) -> Result<Vec<ProcessedPacket>, Box<dyn std::error::Error + Send + Sync>> {
        let mut processed_packets = Vec::with_capacity(packets.len());
        
        // SIMD로 처리할 수 있는 청크 단위로 분할
        for chunk in packets.chunks(8) { // AVX2는 8개 동시 처리
            let processed_chunk = self.process_chunk_simd(chunk).await?;
            processed_packets.extend(processed_chunk);
        }
        
        Ok(processed_packets)
    }
    
    /// SIMD 청크 처리
    async fn process_chunk_simd(
        &self, 
        chunk: &[RudpPacket]
    ) -> Result<Vec<ProcessedPacket>, Box<dyn std::error::Error + Send + Sync>> {
        unsafe {
            let mut processed = Vec::with_capacity(chunk.len());
            
            // 패킷 크기 검증 (SIMD)
            let sizes: [u32; 8] = [
                chunk.get(0).map(|p| p.payload.len() as u32).unwrap_or(0),
                chunk.get(1).map(|p| p.payload.len() as u32).unwrap_or(0),
                chunk.get(2).map(|p| p.payload.len() as u32).unwrap_or(0),
                chunk.get(3).map(|p| p.payload.len() as u32).unwrap_or(0),
                chunk.get(4).map(|p| p.payload.len() as u32).unwrap_or(0),
                chunk.get(5).map(|p| p.payload.len() as u32).unwrap_or(0),
                chunk.get(6).map(|p| p.payload.len() as u32).unwrap_or(0),
                chunk.get(7).map(|p| p.payload.len() as u32).unwrap_or(0),
            ];
            
            let size_vec = _mm256_loadu_si256(sizes.as_ptr() as *const __m256i);
            let max_size = _mm256_set1_epi32(1024); // 최대 1KB
            let valid_mask = _mm256_cmpgt_epi32(max_size, size_vec);
            
            // 체크섬 계산 (SIMD)
            for (i, packet) in chunk.iter().enumerate() {
                let is_valid = ((_mm256_extract_epi32(valid_mask, i as i32)) & 0xFF) != 0;
                
                if is_valid {
                    let checksum = fast_checksum(&packet.payload);
                    processed.push(ProcessedPacket {
                        sequence: packet.header.sequence,
                        checksum,
                        payload: packet.payload.clone(),
                        validated: true,
                    });
                } else {
                    // 잘못된 패킷
                    processed.push(ProcessedPacket {
                        sequence: packet.header.sequence,
                        checksum: 0,
                        payload: Vec::new(),
                        validated: false,
                    });
                }
            }
            
            Ok(processed)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessedPacket {
    pub sequence: u16,
    pub checksum: u32,
    pub payload: Vec<u8>,
    pub validated: bool,
}
```

### 3. 프로토콜 확장 예시

```rust
// protocol/rudp_extended.rs
use serde::{Serialize, Deserialize};

/// 확장된 RUDP 메시지 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RudpGameMessage {
    // 기본 메시지들
    Connect { user_id: u32, room_id: u32 },
    Disconnect { user_id: u32 },
    
    // 실시간 게임 메시지들
    PositionUpdate {
        user_id: u32,
        position: PlayerPosition,
        priority: RudpPriority,
    },
    
    PlayerAction {
        user_id: u32,
        action: GameAction,
        sequence: u16,
        timestamp: u64,
    },
    
    GameStateSync {
        room_id: u32,
        full_state: GameState,
        tick: u64,
    },
    
    // 신뢰성 보장이 필요한 메시지
    ReliableCommand {
        command_id: u64,
        user_id: u32,
        command: String,
        parameters: Vec<u8>,
    },
    
    // 대용량 데이터 전송 (분할 전송)
    LargeDataTransfer {
        transfer_id: u64,
        chunk_index: u32,
        total_chunks: u32,
        chunk_data: Vec<u8>,
    },
    
    // 음성/비디오 스트리밍
    MediaStream {
        stream_id: u32,
        frame_data: Vec<u8>,
        frame_type: MediaFrameType,
        timestamp: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RudpPriority {
    Critical = 0,  // 연결 관련
    High = 1,      // 실시간 위치, 액션
    Normal = 2,    // 채팅, UI 업데이트  
    Low = 3,       // 백그라운드 동기화
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameAction {
    Move { direction: Vec3, speed: f32 },
    Attack { target_id: u32, damage: i32 },
    UseItem { item_id: String },
    Interact { object_id: u32 },
    Cast { skill_id: u32, target_pos: Vec3 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaFrameType {
    KeyFrame,      // I-frame
    PredictFrame,  // P-frame
    Audio,
}
```

### 4. 분할 전송 핸들러

```rust
// handlers/fragmentation_handler.rs
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct FragmentationHandler {
    reassembly_buffers: Arc<RwLock<HashMap<u64, ReassemblyBuffer>>>,
    config: FragmentationConfig,
}

#[derive(Debug, Clone)]
pub struct FragmentationConfig {
    pub max_fragment_size: usize,
    pub reassembly_timeout_ms: u64,
    pub max_concurrent_transfers: usize,
}

#[derive(Debug)]
struct ReassemblyBuffer {
    transfer_id: u64,
    total_chunks: u32,
    received_chunks: HashMap<u32, Vec<u8>>,
    first_chunk_time: std::time::Instant,
}

impl FragmentationHandler {
    pub fn new() -> Self {
        Self {
            reassembly_buffers: Arc::new(RwLock::new(HashMap::new())),
            config: FragmentationConfig {
                max_fragment_size: 1024,
                reassembly_timeout_ms: 10000, // 10초
                max_concurrent_transfers: 100,
            },
        }
    }
    
    /// 대용량 데이터 분할
    pub async fn fragment_large_data(
        &self, 
        data: &[u8], 
        transfer_id: u64
    ) -> Result<Vec<RudpGameMessage>, Box<dyn std::error::Error + Send + Sync>> {
        let chunk_size = self.config.max_fragment_size;
        let total_chunks = (data.len() + chunk_size - 1) / chunk_size;
        let mut fragments = Vec::with_capacity(total_chunks);
        
        for (chunk_index, chunk) in data.chunks(chunk_size).enumerate() {
            fragments.push(RudpGameMessage::LargeDataTransfer {
                transfer_id,
                chunk_index: chunk_index as u32,
                total_chunks: total_chunks as u32,
                chunk_data: chunk.to_vec(),
            });
        }
        
        Ok(fragments)
    }
    
    /// 분할된 데이터 재조립
    pub async fn reassemble_fragment(
        &self,
        transfer_id: u64,
        chunk_index: u32,
        total_chunks: u32,
        chunk_data: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        let mut buffers = self.reassembly_buffers.write().await;
        
        let buffer = buffers.entry(transfer_id).or_insert_with(|| ReassemblyBuffer {
            transfer_id,
            total_chunks,
            received_chunks: HashMap::new(),
            first_chunk_time: std::time::Instant::now(),
        });
        
        // 청크 저장
        buffer.received_chunks.insert(chunk_index, chunk_data);
        
        // 모든 청크가 도착했는지 확인
        if buffer.received_chunks.len() == total_chunks as usize {
            let mut reassembled_data = Vec::new();
            
            // 순서대로 데이터 결합
            for i in 0..total_chunks {
                if let Some(chunk) = buffer.received_chunks.get(&i) {
                    reassembled_data.extend_from_slice(chunk);
                } else {
                    return Err("Missing chunk during reassembly".into());
                }
            }
            
            // 버퍼 정리
            buffers.remove(&transfer_id);
            
            Ok(Some(reassembled_data))
        } else {
            Ok(None)
        }
    }
    
    /// 타임아웃된 재조립 버퍼 정리
    pub async fn cleanup_expired_buffers(&self) {
        let mut buffers = self.reassembly_buffers.write().await;
        let now = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(self.config.reassembly_timeout_ms);
        
        buffers.retain(|_, buffer| {
            now.duration_since(buffer.first_chunk_time) < timeout
        });
    }
}
```

## ⚡ 성능 최적화 패턴

### 1. 제로카피 메시지 처리

```rust
// handlers/zero_copy_handler.rs
use crate::service::zero_copy_io::ZeroCopyBuffer;

pub struct ZeroCopyMessageHandler {
    buffer_pool: Arc<ZeroCopyBufferPool>,
}

impl ZeroCopyMessageHandler {
    /// 제로카피로 메시지 처리
    pub async fn process_message_zero_copy(
        &self, 
        buffer: ZeroCopyBuffer
    ) -> Result<ZeroCopyBuffer, Box<dyn std::error::Error + Send + Sync>> {
        // 직접 메모리 조작으로 메시지 처리
        let message_type = buffer.read_u8_at(0)?;
        let sequence = buffer.read_u16_at(1)?;
        
        match message_type {
            0x01 => { // Position Update
                // 위치 데이터 직접 처리 (복사 없이)
                let x = buffer.read_f32_at(8)?;
                let y = buffer.read_f32_at(12)?;
                let z = buffer.read_f32_at(16)?;
                
                // 응답 버퍼 준비 (재사용)
                let mut response_buffer = self.buffer_pool.get_buffer(32).await?;
                response_buffer.write_u8_at(0, 0x81)?; // ACK
                response_buffer.write_u16_at(1, sequence)?;
                
                Ok(response_buffer)
            }
            _ => Err("Unknown message type".into())
        }
    }
}
```

### 2. 배치 처리 최적화

```rust
// handlers/batch_processor.rs
pub struct BatchMessageProcessor {
    pending_messages: Vec<RudpGameMessage>,
    batch_timer: tokio::time::Interval,
    batch_size_limit: usize,
}

impl BatchMessageProcessor {
    /// 배치 메시지 처리
    pub async fn process_message_batch(
        &mut self,
        messages: Vec<RudpGameMessage>,
    ) -> Result<Vec<RudpGameMessage>, Box<dyn std::error::Error + Send + Sync>> {
        // 우선순위별로 메시지 분류
        let mut critical_messages = Vec::new();
        let mut high_priority_messages = Vec::new();
        let mut normal_messages = Vec::new();
        
        for message in messages {
            match self.get_message_priority(&message) {
                RudpPriority::Critical => critical_messages.push(message),
                RudpPriority::High => high_priority_messages.push(message),
                _ => normal_messages.push(message),
            }
        }
        
        // 우선순위 순서대로 처리
        let mut responses = Vec::new();
        
        // Critical 메시지는 즉시 처리
        for message in critical_messages {
            responses.extend(self.process_single_message(message).await?);
        }
        
        // High 우선순위는 배치 처리
        if !high_priority_messages.is_empty() {
            responses.extend(self.process_high_priority_batch(high_priority_messages).await?);
        }
        
        // Normal 메시지는 대기열에 추가
        self.pending_messages.extend(normal_messages);
        
        // 배치 크기 또는 타이머에 따라 pending 메시지 처리
        if self.pending_messages.len() >= self.batch_size_limit {
            let batch = std::mem::take(&mut self.pending_messages);
            responses.extend(self.process_normal_batch(batch).await?);
        }
        
        Ok(responses)
    }
}
```

### 3. 메모리 풀 통합

```rust
// handlers/pooled_handler.rs
use shared::tool::high_performance::enhanced_memory_pool::*;

pub struct PooledMessageHandler {
    memory_pool: Arc<EnhancedMemoryPool>,
    message_cache: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl PooledMessageHandler {
    /// 메모리 풀을 사용한 메시지 처리
    pub async fn handle_with_pool(
        &self,
        message_data: &[u8],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        // 적절한 크기의 버퍼 할당
        let buffer_class = if message_data.len() <= 1024 {
            BufferSizeClass::Tiny
        } else if message_data.len() <= 4096 {
            BufferSizeClass::Small
        } else {
            BufferSizeClass::Medium
        };
        
        if let Some(pooled_buffer) = self.memory_pool.get_buffer(buffer_class).await {
            let mut buffer = pooled_buffer.get_buffer();
            
            // 메시지 처리
            buffer.extend_from_slice(message_data);
            self.process_message_in_place(&mut buffer).await?;
            
            let result = buffer.clone();
            
            // 버퍼 반환
            self.memory_pool.return_buffer(pooled_buffer).await;
            
            Ok(result)
        } else {
            // 풀에서 할당 실패시 일반 할당
            let mut buffer = Vec::with_capacity(message_data.len() * 2);
            buffer.extend_from_slice(message_data);
            self.process_message_in_place(&mut buffer).await?;
            Ok(buffer)
        }
    }
}
```

## 🔧 확장 모범 사례

### 1. 실시간 성능 요구사항
- **지연시간**: <10ms 목표
- **처리량**: 20,000+ 메시지/초
- **메모리 효율**: 패킷당 <1KB 오버헤드
- **CPU 최적화**: SIMD 활용으로 30% 성능 향상

### 2. 신뢰성 보장
- 중요한 메시지는 ACK 기반 재전송
- 순서 보장이 필요한 메시지는 시퀀스 번호 활용
- 타임아웃과 재전송 로직 구현

### 3. 확장성 고려사항
- 상태는 가능한 stateless하게 설계
- 캐시와 메모리 풀 적극 활용
- 배치 처리로 시스템 콜 최소화
- AOI(Area of Interest) 기반 최적화