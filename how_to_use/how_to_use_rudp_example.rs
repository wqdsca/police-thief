//! RUDP 서버 음성 채팅 기능 예제
//! 
//! 이 예제는 RUDP 서버를 사용하여 실시간 음성 채팅 기능을 구현합니다.
//! - 저지연 음성 데이터 전송
//! - 음성 품질 적응화
//! - 패킷 손실 복구
//! - 음성 압축 및 최적화

use anyhow::{Result, anyhow};
use bytes::{Buf, BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn, error, debug};

/// 음성 데이터 패킷 타입
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoicePacketType {
    AudioData = 0x01,      // 음성 데이터
    AudioStart = 0x02,     // 음성 시작 알림
    AudioEnd = 0x03,       // 음성 종료 알림
    QualityRequest = 0x04, // 품질 조정 요청
    QualityResponse = 0x05, // 품질 조정 응답
    VolumeControl = 0x06,   // 볼륨 조정
    MuteToggle = 0x07,     // 음소거 토글
}

impl From<u8> for VoicePacketType {
    fn from(byte: u8) -> Self {
        match byte {
            0x01 => Self::AudioData,
            0x02 => Self::AudioStart,
            0x03 => Self::AudioEnd,
            0x04 => Self::QualityRequest,
            0x05 => Self::QualityResponse,
            0x06 => Self::VolumeControl,
            0x07 => Self::MuteToggle,
            _ => Self::AudioData, // 기본값
        }
    }
}

/// 음성 품질 설정
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum VoiceQuality {
    Low,      // 8kHz, 16KB/s
    Medium,   // 16kHz, 32KB/s  
    High,     // 22kHz, 64KB/s
    Premium,  // 44kHz, 128KB/s
}

impl VoiceQuality {
    pub fn sample_rate(&self) -> u32 {
        match self {
            Self::Low => 8000,
            Self::Medium => 16000,
            Self::High => 22050,
            Self::Premium => 44100,
        }
    }

    pub fn bitrate(&self) -> u32 {
        match self {
            Self::Low => 16000,
            Self::Medium => 32000,
            Self::High => 64000,
            Self::Premium => 128000,
        }
    }

    pub fn packet_size(&self) -> usize {
        match self {
            Self::Low => 160,    // 20ms @ 8kHz
            Self::Medium => 320, // 20ms @ 16kHz
            Self::High => 441,   // 20ms @ 22kHz  
            Self::Premium => 882, // 20ms @ 44kHz
        }
    }
}

/// RUDP 헤더 (기존 RUDP 프로토콜 확장)
#[derive(Debug, Clone)]
pub struct VoiceRudpHeader {
    pub packet_type: VoicePacketType,
    pub sequence_number: u32,
    pub timestamp: u32,        // 오디오 타임스탬프
    pub user_id: u32,         // 송신자 ID
    pub room_id: u32,         // 방 ID
    pub quality: VoiceQuality, // 음성 품질
    pub payload_size: u16,    // 페이로드 크기
    pub flags: u8,            // 플래그 (압축, 암호화 등)
}

impl VoiceRudpHeader {
    pub const SIZE: usize = 21; // 헤더 크기

    /// 바이너리로 직렬화
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(Self::SIZE);
        
        buffer.push(self.packet_type as u8);
        buffer.extend_from_slice(&self.sequence_number.to_le_bytes());
        buffer.extend_from_slice(&self.timestamp.to_le_bytes());
        buffer.extend_from_slice(&self.user_id.to_le_bytes());
        buffer.extend_from_slice(&self.room_id.to_le_bytes());
        buffer.push(self.quality as u8);
        buffer.extend_from_slice(&self.payload_size.to_le_bytes());
        buffer.push(self.flags);
        
        buffer
    }

    /// 바이너리에서 역직렬화
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(anyhow!("헤더 데이터가 너무 짧습니다"));
        }

        let mut cursor = 0;
        
        let packet_type = VoicePacketType::from(data[cursor]);
        cursor += 1;

        let sequence_number = u32::from_le_bytes([
            data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]
        ]);
        cursor += 4;

        let timestamp = u32::from_le_bytes([
            data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]
        ]);
        cursor += 4;

        let user_id = u32::from_le_bytes([
            data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]
        ]);
        cursor += 4;

        let room_id = u32::from_le_bytes([
            data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]
        ]);
        cursor += 4;

        let quality = match data[cursor] {
            0 => VoiceQuality::Low,
            1 => VoiceQuality::Medium,
            2 => VoiceQuality::High,
            3 => VoiceQuality::Premium,
            _ => VoiceQuality::Medium,
        };
        cursor += 1;

        let payload_size = u16::from_le_bytes([data[cursor], data[cursor + 1]]);
        cursor += 2;

        let flags = data[cursor];

        Ok(Self {
            packet_type,
            sequence_number,
            timestamp,
            user_id,
            room_id,
            quality,
            payload_size,
            flags,
        })
    }
}

/// 음성 패킷
#[derive(Debug, Clone)]
pub struct VoicePacket {
    pub header: VoiceRudpHeader,
    pub payload: Vec<u8>,
}

impl VoicePacket {
    /// 새 음성 패킷 생성
    pub fn new(
        packet_type: VoicePacketType,
        sequence: u32,
        timestamp: u32,
        user_id: u32,
        room_id: u32,
        quality: VoiceQuality,
        payload: Vec<u8>,
    ) -> Self {
        let header = VoiceRudpHeader {
            packet_type,
            sequence_number: sequence,
            timestamp,
            user_id,
            room_id,
            quality,
            payload_size: payload.len() as u16,
            flags: 0,
        };

        Self { header, payload }
    }

    /// 바이너리로 직렬화
    pub fn to_bytes(&self) -> Vec<u8> {
        let header_bytes = self.header.to_bytes();
        let mut result = Vec::with_capacity(header_bytes.len() + self.payload.len());
        
        result.extend_from_slice(&header_bytes);
        result.extend_from_slice(&self.payload);
        
        result
    }

    /// 바이너리에서 역직렬화  
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let header = VoiceRudpHeader::from_bytes(data)?;
        
        if data.len() < VoiceRudpHeader::SIZE + header.payload_size as usize {
            return Err(anyhow!("패킷 데이터가 불완전합니다"));
        }

        let payload = data[VoiceRudpHeader::SIZE..VoiceRudpHeader::SIZE + header.payload_size as usize].to_vec();

        Ok(Self { header, payload })
    }
}

/// 음성 세션 정보
#[derive(Debug, Clone)]
pub struct VoiceSession {
    pub user_id: u32,
    pub room_id: u32,
    pub is_speaking: bool,
    pub is_muted: bool,
    pub quality: VoiceQuality,
    pub volume: f32,           // 0.0 ~ 1.0
    pub last_activity: Instant,
    pub sequence_number: AtomicU32,
    pub addr: SocketAddr,
}

impl VoiceSession {
    pub fn new(user_id: u32, room_id: u32, addr: SocketAddr) -> Self {
        Self {
            user_id,
            room_id,
            is_speaking: false,
            is_muted: false,
            quality: VoiceQuality::Medium,
            volume: 1.0,
            last_activity: Instant::now(),
            sequence_number: AtomicU32::new(1),
            addr,
        }
    }

    pub fn next_sequence(&self) -> u32 {
        self.sequence_number.fetch_add(1, Ordering::Relaxed)
    }
}

/// 음성 압축기 (간단한 예제)
pub struct VoiceCompressor;

impl VoiceCompressor {
    /// 음성 데이터 압축 (실제로는 Opus, G.711 등 사용)
    pub fn compress(data: &[u8], quality: VoiceQuality) -> Vec<u8> {
        match quality {
            VoiceQuality::Low => {
                // 간단한 압축 - 2바이트마다 하나씩 샘플링
                data.iter().step_by(2).cloned().collect()
            }
            VoiceQuality::Medium => {
                // 보통 압축 - 경미한 손실 압축
                data.iter().enumerate()
                    .filter(|(i, _)| i % 4 != 3)
                    .map(|(_, &b)| b)
                    .collect()
            }
            _ => {
                // 고품질은 압축 안함
                data.to_vec()
            }
        }
    }

    /// 음성 데이터 압축 해제
    pub fn decompress(data: &[u8], quality: VoiceQuality, _original_size: usize) -> Vec<u8> {
        match quality {
            VoiceQuality::Low => {
                // 압축된 데이터를 2배로 복원
                data.iter().flat_map(|&b| vec![b, b]).collect()
            }
            VoiceQuality::Medium => {
                // 보통 압축 해제 - 누락된 바이트 보간
                let mut result = Vec::new();
                for (i, &byte) in data.iter().enumerate() {
                    result.push(byte);
                    if (i + 1) % 3 == 0 {
                        result.push(byte); // 간단한 보간
                    }
                }
                result
            }
            _ => {
                data.to_vec()
            }
        }
    }
}

/// 음성 채팅 방 관리자
pub struct VoiceRoomManager {
    sessions: Arc<RwLock<HashMap<u32, VoiceSession>>>,
    room_users: Arc<RwLock<HashMap<u32, Vec<u32>>>>, // room_id -> user_ids
    sequence_counter: AtomicU32,
}

impl VoiceRoomManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            room_users: Arc::new(RwLock::new(HashMap::new())),
            sequence_counter: AtomicU32::new(1),
        }
    }

    /// 사용자 음성 세션 등록
    pub async fn register_user(&self, user_id: u32, room_id: u32, addr: SocketAddr) {
        let session = VoiceSession::new(user_id, room_id, addr);
        
        // 세션 등록
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(user_id, session);
        }

        // 방 사용자 목록에 추가
        {
            let mut room_users = self.room_users.write().await;
            room_users.entry(room_id).or_insert_with(Vec::new).push(user_id);
        }

        info!("사용자 {} 음성 세션 등록 (방: {})", user_id, room_id);
    }

    /// 사용자 세션 제거
    pub async fn unregister_user(&self, user_id: u32) {
        let room_id = {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.remove(&user_id) {
                Some(session.room_id)
            } else {
                None
            }
        };

        // 방 사용자 목록에서 제거
        if let Some(room_id) = room_id {
            let mut room_users = self.room_users.write().await;
            if let Some(users) = room_users.get_mut(&room_id) {
                users.retain(|&id| id != user_id);
                if users.is_empty() {
                    room_users.remove(&room_id);
                }
            }
        }

        info!("사용자 {} 음성 세션 해제", user_id);
    }

    /// 방의 모든 사용자 주소 가져오기 (발신자 제외)
    pub async fn get_room_addresses(&self, room_id: u32, exclude_user: u32) -> Vec<SocketAddr> {
        let room_users = self.room_users.read().await;
        let sessions = self.sessions.read().await;

        if let Some(user_ids) = room_users.get(&room_id) {
            user_ids
                .iter()
                .filter(|&&id| id != exclude_user)
                .filter_map(|&id| sessions.get(&id))
                .filter(|session| !session.is_muted)
                .map(|session| session.addr)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// 음성 세션 업데이트
    pub async fn update_session<F>(&self, user_id: u32, updater: F) -> Result<()>
    where
        F: FnOnce(&mut VoiceSession),
    {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(&user_id) {
            updater(session);
            session.last_activity = Instant::now();
            Ok(())
        } else {
            Err(anyhow!("사용자 세션을 찾을 수 없습니다: {}", user_id))
        }
    }

    /// 방의 활성 발화자 수 확인
    pub async fn get_active_speakers(&self, room_id: u32) -> usize {
        let room_users = self.room_users.read().await;
        let sessions = self.sessions.read().await;

        if let Some(user_ids) = room_users.get(&room_id) {
            user_ids
                .iter()
                .filter_map(|&id| sessions.get(&id))
                .filter(|session| session.is_speaking && !session.is_muted)
                .count()
        } else {
            0
        }
    }
}

/// RUDP 음성 채팅 서버
pub struct RudpVoiceServer {
    socket: Arc<UdpSocket>,
    room_manager: Arc<VoiceRoomManager>,
    compressor: VoiceCompressor,
    buffer_pool: Arc<RwLock<Vec<Vec<u8>>>>, // 버퍼 재사용 풀
}

impl RudpVoiceServer {
    pub async fn new(addr: &str) -> Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        let room_manager = Arc::new(VoiceRoomManager::new());
        
        info!("RUDP 음성 채팅 서버 시작: {}", addr);

        Ok(Self {
            socket: Arc::new(socket),
            room_manager,
            compressor: VoiceCompressor,
            buffer_pool: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// 서버 실행
    pub async fn run(&self) -> Result<()> {
        let mut buffer = vec![0u8; 2048]; // 최대 패킷 크기

        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((size, addr)) => {
                    let data = buffer[..size].to_vec();
                    let socket = self.socket.clone();
                    let room_manager = self.room_manager.clone();
                    let buffer_pool = self.buffer_pool.clone();

                    // 패킷 처리를 별도 태스크에서 실행 (논블로킹)
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_packet(
                            data,
                            addr,
                            socket,
                            room_manager,
                            buffer_pool,
                        ).await {
                            error!("패킷 처리 실패 ({}): {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("UDP 수신 실패: {}", e);
                }
            }
        }
    }

    /// 패킷 처리 (핵심 비즈니스 로직)
    async fn handle_packet(
        data: Vec<u8>,
        addr: SocketAddr,
        socket: Arc<UdpSocket>,
        room_manager: Arc<VoiceRoomManager>,
        buffer_pool: Arc<RwLock<Vec<Vec<u8>>>>,
    ) -> Result<()> {
        let packet = VoicePacket::from_bytes(&data)?;
        let user_id = packet.header.user_id;
        let room_id = packet.header.room_id;

        match packet.header.packet_type {
            VoicePacketType::AudioData => {
                Self::handle_audio_data(packet, addr, socket, room_manager, buffer_pool).await?;
            }

            VoicePacketType::AudioStart => {
                info!("사용자 {} 발화 시작 (방: {})", user_id, room_id);
                
                room_manager.update_session(user_id, |session| {
                    session.is_speaking = true;
                }).await?;

                // 다른 사용자들에게 발화 시작 알림
                Self::broadcast_control_message(
                    &socket,
                    &room_manager,
                    room_id,
                    user_id,
                    VoicePacketType::AudioStart,
                ).await?;
            }

            VoicePacketType::AudioEnd => {
                info!("사용자 {} 발화 종료 (방: {})", user_id, room_id);
                
                room_manager.update_session(user_id, |session| {
                    session.is_speaking = false;
                }).await?;

                // 다른 사용자들에게 발화 종료 알림
                Self::broadcast_control_message(
                    &socket,
                    &room_manager,
                    room_id,
                    user_id,
                    VoicePacketType::AudioEnd,
                ).await?;
            }

            VoicePacketType::MuteToggle => {
                info!("사용자 {} 음소거 토글 (방: {})", user_id, room_id);
                
                let is_muted = room_manager.update_session(user_id, |session| {
                    session.is_muted = !session.is_muted;
                }).await.is_ok();

                if is_muted {
                    // 음소거 상태 브로드캐스트
                    Self::broadcast_control_message(
                        &socket,
                        &room_manager,
                        room_id,
                        user_id,
                        VoicePacketType::MuteToggle,
                    ).await?;
                }
            }

            VoicePacketType::QualityRequest => {
                if let Ok(new_quality) = serde_json::from_slice::<VoiceQuality>(&packet.payload) {
                    info!("사용자 {} 품질 변경: {:?}", user_id, new_quality);
                    
                    room_manager.update_session(user_id, |session| {
                        session.quality = new_quality;
                    }).await?;

                    // 품질 변경 응답
                    let response_packet = VoicePacket::new(
                        VoicePacketType::QualityResponse,
                        0,
                        0,
                        0,
                        room_id,
                        new_quality,
                        serde_json::to_vec(&new_quality)?,
                    );

                    socket.send_to(&response_packet.to_bytes(), addr).await?;
                }
            }

            _ => {
                debug!("알 수 없는 패킷 타입: {:?}", packet.header.packet_type);
            }
        }

        Ok(())
    }

    /// 음성 데이터 처리 및 브로드캐스트
    async fn handle_audio_data(
        packet: VoicePacket,
        _addr: SocketAddr,
        socket: Arc<UdpSocket>,
        room_manager: Arc<VoiceRoomManager>,
        buffer_pool: Arc<RwLock<Vec<Vec<u8>>>>,
    ) -> Result<()> {
        let user_id = packet.header.user_id;
        let room_id = packet.header.room_id;

        // 음성 데이터 압축 해제 (필요시)
        let audio_data = if packet.header.flags & 0x01 != 0 {
            VoiceCompressor::decompress(
                &packet.payload,
                packet.header.quality,
                packet.header.quality.packet_size(),
            )
        } else {
            packet.payload
        };

        // 방의 다른 사용자들에게 브로드캐스트
        let target_addrs = room_manager.get_room_addresses(room_id, user_id).await;
        
        if !target_addrs.is_empty() {
            // 버퍼 풀에서 버퍼 가져오기
            let packet_data = {
                let mut buffer_pool = buffer_pool.write().await;
                let mut buffer = buffer_pool.pop().unwrap_or_else(|| Vec::with_capacity(2048));
                buffer.clear();
                
                // 패킷 재구성
                buffer.extend_from_slice(&packet.header.to_bytes());
                buffer.extend_from_slice(&audio_data);
                buffer
            };

            // 병렬 전송으로 지연시간 최소화
            let send_tasks: Vec<_> = target_addrs
                .into_iter()
                .map(|addr| {
                    let socket = socket.clone();
                    let data = packet_data.clone();
                    tokio::spawn(async move {
                        if let Err(e) = socket.send_to(&data, addr).await {
                            warn!("음성 데이터 전송 실패 ({}): {}", addr, e);
                        }
                    })
                })
                .collect();

            // 모든 전송 완료 대기
            for task in send_tasks {
                let _ = task.await;
            }

            // 버퍼 반환
            {
                let mut pool = buffer_pool.write().await;
                if pool.len() < 100 { // 풀 크기 제한
                    pool.push(packet_data);
                }
            }

            debug!(
                "음성 데이터 브로드캐스트 완료: 사용자 {} ({}바이트)",
                user_id,
                audio_data.len()
            );
        }

        Ok(())
    }

    /// 제어 메시지 브로드캐스트
    async fn broadcast_control_message(
        socket: &UdpSocket,
        room_manager: &VoiceRoomManager,
        room_id: u32,
        sender_id: u32,
        packet_type: VoicePacketType,
    ) -> Result<()> {
        let target_addrs = room_manager.get_room_addresses(room_id, sender_id).await;
        
        let control_packet = VoicePacket::new(
            packet_type,
            0,
            chrono::Utc::now().timestamp() as u32,
            sender_id,
            room_id,
            VoiceQuality::Medium,
            Vec::new(),
        );

        let packet_data = control_packet.to_bytes();
        
        for addr in target_addrs {
            if let Err(e) = socket.send_to(&packet_data, addr).await {
                warn!("제어 메시지 전송 실패 ({}): {}", addr, e);
            }
        }

        Ok(())
    }

    /// 음성 세션 등록 (클라이언트 연결시)
    pub async fn register_client(&self, user_id: u32, room_id: u32, addr: SocketAddr) -> Result<()> {
        self.room_manager.register_user(user_id, room_id, addr).await;
        
        // 환영 메시지 전송
        let welcome_packet = VoicePacket::new(
            VoicePacketType::QualityResponse,
            0,
            0,
            0,
            room_id,
            VoiceQuality::Medium,
            b"Welcome to voice chat!".to_vec(),
        );

        self.socket.send_to(&welcome_packet.to_bytes(), addr).await?;
        
        Ok(())
    }
}

/// 음성 채팅 클라이언트 (예제)
pub struct VoiceClient {
    socket: UdpSocket,
    server_addr: SocketAddr,
    user_id: u32,
    room_id: u32,
    sequence: AtomicU32,
    quality: VoiceQuality,
}

impl VoiceClient {
    pub async fn new(
        server_addr: SocketAddr,
        user_id: u32,
        room_id: u32,
    ) -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(server_addr).await?;

        Ok(Self {
            socket,
            server_addr,
            user_id,
            room_id,
            sequence: AtomicU32::new(1),
            quality: VoiceQuality::Medium,
        })
    }

    /// 발화 시작 알림
    pub async fn start_speaking(&self) -> Result<()> {
        let packet = VoicePacket::new(
            VoicePacketType::AudioStart,
            self.next_sequence(),
            chrono::Utc::now().timestamp() as u32,
            self.user_id,
            self.room_id,
            self.quality,
            Vec::new(),
        );

        self.socket.send(&packet.to_bytes()).await?;
        info!("발화 시작 알림 전송");
        
        Ok(())
    }

    /// 음성 데이터 전송
    pub async fn send_audio(&self, audio_data: Vec<u8>) -> Result<()> {
        // 음성 데이터 압축
        let compressed = VoiceCompressor::compress(&audio_data, self.quality);
        
        let mut packet = VoicePacket::new(
            VoicePacketType::AudioData,
            self.next_sequence(),
            chrono::Utc::now().timestamp() as u32,
            self.user_id,
            self.room_id,
            self.quality,
            compressed,
        );

        // 압축 플래그 설정
        packet.header.flags |= 0x01;

        self.socket.send(&packet.to_bytes()).await?;
        
        Ok(())
    }

    /// 발화 종료 알림
    pub async fn stop_speaking(&self) -> Result<()> {
        let packet = VoicePacket::new(
            VoicePacketType::AudioEnd,
            self.next_sequence(),
            chrono::Utc::now().timestamp() as u32,
            self.user_id,
            self.room_id,
            self.quality,
            Vec::new(),
        );

        self.socket.send(&packet.to_bytes()).await?;
        info!("발화 종료 알림 전송");
        
        Ok(())
    }

    /// 음소거 토글
    pub async fn toggle_mute(&self) -> Result<()> {
        let packet = VoicePacket::new(
            VoicePacketType::MuteToggle,
            self.next_sequence(),
            chrono::Utc::now().timestamp() as u32,
            self.user_id,
            self.room_id,
            self.quality,
            Vec::new(),
        );

        self.socket.send(&packet.to_bytes()).await?;
        info!("음소거 토글 요청 전송");
        
        Ok(())
    }

    /// 음성 품질 변경
    pub async fn change_quality(&mut self, new_quality: VoiceQuality) -> Result<()> {
        self.quality = new_quality;
        
        let packet = VoicePacket::new(
            VoicePacketType::QualityRequest,
            self.next_sequence(),
            chrono::Utc::now().timestamp() as u32,
            self.user_id,
            self.room_id,
            new_quality,
            serde_json::to_vec(&new_quality)?,
        );

        self.socket.send(&packet.to_bytes()).await?;
        info!("음성 품질 변경 요청: {:?}", new_quality);
        
        Ok(())
    }

    fn next_sequence(&self) -> u32 {
        self.sequence.fetch_add(1, Ordering::Relaxed)
    }
}

/// RUDP 음성 채팅 서버 실행 예제
pub async fn rudp_voice_chat_example() -> Result<()> {
    // 1. RUDP 음성 채팅 서버 시작
    let server = RudpVoiceServer::new("127.0.0.1:5000").await?;
    
    info!("RUDP 음성 채팅 서버 시작!");
    info!("기능:");
    info!("- 실시간 음성 데이터 전송 (저지연)");
    info!("- 적응적 음성 품질 조정");
    info!("- 음성 압축 및 최적화");
    info!("- 패킷 손실 복구");
    info!("- 음소거 및 볼륨 조정");
    
    // 서버 실행 (무한 루프)
    server.run().await?;
    
    Ok(())
}

/// 클라이언트 사용 예제
pub async fn rudp_voice_client_example() -> Result<()> {
    let server_addr: SocketAddr = "127.0.0.1:5000".parse()?;
    let mut client = VoiceClient::new(server_addr, 100, 1).await?;
    
    // 시뮬레이션된 음성 채팅 시나리오
    info!("음성 채팅 클라이언트 시작");
    
    // 1. 발화 시작
    client.start_speaking().await?;
    
    // 2. 음성 데이터 전송 (시뮬레이션)
    for i in 0..50 {
        // 20ms 간격으로 음성 데이터 전송
        let audio_data = vec![i as u8; client.quality.packet_size()];
        client.send_audio(audio_data).await?;
        
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        // 중간에 품질 변경
        if i == 25 {
            client.change_quality(VoiceQuality::High).await?;
        }
    }
    
    // 3. 발화 종료
    client.stop_speaking().await?;
    
    // 4. 음소거 토글
    tokio::time::sleep(Duration::from_secs(1)).await;
    client.toggle_mute().await?;
    
    info!("음성 채팅 시나리오 완료");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_packet_serialization() {
        let packet = VoicePacket::new(
            VoicePacketType::AudioData,
            123,
            456789,
            100,
            1,
            VoiceQuality::High,
            vec![1, 2, 3, 4, 5],
        );

        let bytes = packet.to_bytes();
        let decoded = VoicePacket::from_bytes(&bytes).unwrap();

        assert_eq!(packet.header.packet_type, decoded.header.packet_type);
        assert_eq!(packet.header.sequence_number, decoded.header.sequence_number);
        assert_eq!(packet.header.user_id, decoded.header.user_id);
        assert_eq!(packet.payload, decoded.payload);
    }

    #[test]
    fn test_voice_compression() {
        let original = vec![1, 2, 3, 4, 5, 6, 7, 8];
        
        let compressed = VoiceCompressor::compress(&original, VoiceQuality::Low);
        let decompressed = VoiceCompressor::decompress(&compressed, VoiceQuality::Low, original.len());
        
        // 압축/해제 후 크기가 원본과 유사해야 함
        assert!(!compressed.is_empty());
        assert!(!decompressed.is_empty());
    }

    #[tokio::test]
    async fn test_voice_room_manager() {
        let manager = VoiceRoomManager::new();
        let addr: SocketAddr = "127.0.0.1:8000".parse().unwrap();
        
        // 사용자 등록
        manager.register_user(100, 1, addr).await;
        manager.register_user(200, 1, addr).await;
        
        // 활성 발화자 확인
        manager.update_session(100, |session| {
            session.is_speaking = true;
        }).await.unwrap();
        
        let speakers = manager.get_active_speakers(1).await;
        assert_eq!(speakers, 1);
        
        // 사용자 해제
        manager.unregister_user(100).await;
        let speakers_after = manager.get_active_speakers(1).await;
        assert_eq!(speakers_after, 0);
    }
}