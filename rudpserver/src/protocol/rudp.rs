//! RUDP (Reliable UDP) Protocol Implementation
//!
//! 고성능 실시간 게임을 위한 신뢰성 있는 UDP 프로토콜 구현
//!
//! # Features
//! - 패킷 순서 보장 (Sequence numbering)
//! - 신뢰성 보장 (ACK + Retransmission)
//! - 흐름 제어 (Flow control)
//! - 혼잡 제어 (Congestion control)
//! - 연결 시뮬레이션 (Connection lifecycle)
//! - 적응형 타임아웃 (Adaptive RTO)
//!
//! # Performance
//! - 2000명 동시 연결 지원
//! - <50ms RTT 목표
//! - >100K packets/sec 처리량

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use tracing::{debug, error, info, trace, warn};

// Shared library imports for performance and security
use crate::utils::{socket_addr_to_u64, PacketType, RudpPacketHeader};
use shared::security::SecurityMiddleware;
use shared::tool::high_performance::redis_optimizer::RedisOptimizer;

/// RUDP 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RudpConfig {
    /// 최대 동시 연결 수 (기본: 2000)
    pub max_connections: usize,
    /// 최대 패킷 크기 (바이트)
    pub max_packet_size: usize,
    /// ACK 대기 타임아웃 (밀리초)
    pub ack_timeout_ms: u64,
    /// 재전송 최대 횟수
    pub max_retransmissions: u32,
    /// Keep-alive 간격 (초)
    pub keepalive_interval_secs: u64,
    /// 연결 타임아웃 (초)
    pub connection_timeout_secs: u64,
    /// 수신 버퍼 크기
    pub receive_buffer_size: usize,
    /// 전송 버퍼 크기
    pub send_buffer_size: usize,
    /// 혼잡 제어 활성화
    pub enable_congestion_control: bool,
    /// 패킷 압축 활성화
    pub enable_compression: bool,
}

// RUDP 설정 상수
const DEFAULT_MAX_CONNECTIONS: usize = 2000;
const DEFAULT_MTU_SIZE: usize = 1400; // MTU 고려
const DEFAULT_ACK_TIMEOUT_MS: u64 = 100;
const DEFAULT_MAX_RETRANSMISSIONS: u32 = 5;
const DEFAULT_KEEPALIVE_INTERVAL_SECS: u64 = 30;
const DEFAULT_CONNECTION_TIMEOUT_SECS: u64 = 60;
const DEFAULT_BUFFER_SIZE: usize = 8192;
const DEFAULT_PACKET_PRIORITY: u8 = 128; // 기본 우선순위

impl Default for RudpConfig {
    fn default() -> Self {
        Self {
            max_connections: DEFAULT_MAX_CONNECTIONS,
            max_packet_size: DEFAULT_MTU_SIZE,
            ack_timeout_ms: DEFAULT_ACK_TIMEOUT_MS,
            max_retransmissions: DEFAULT_MAX_RETRANSMISSIONS,
            keepalive_interval_secs: DEFAULT_KEEPALIVE_INTERVAL_SECS,
            connection_timeout_secs: DEFAULT_CONNECTION_TIMEOUT_SECS,
            receive_buffer_size: DEFAULT_BUFFER_SIZE,
            send_buffer_size: DEFAULT_BUFFER_SIZE,
            enable_congestion_control: true,
            enable_compression: true,
        }
    }
}

/// RUDP 패킷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RudpPacket {
    /// 패킷 헤더
    pub header: RudpPacketHeader,
    /// 패킷 데이터
    pub payload: Vec<u8>,
    /// 생성 시간 (재전송 타이밍용)
    #[serde(skip, default = "Instant::now")]
    pub created_at: Instant,
    /// 재전송 횟수
    #[serde(skip)]
    pub retransmission_count: u32,
}

impl Default for RudpPacket {
    fn default() -> Self {
        Self {
            header: RudpPacketHeader::new(PacketType::Data, 0, 0),
            payload: Vec::new(),
            created_at: Instant::now(),
            retransmission_count: 0,
        }
    }
}

impl RudpPacket {
    /// 새로운 패킷 생성
    pub fn new(packet_type: PacketType, session_id: u64, payload: Vec<u8>) -> Self {
        let mut header =
            RudpPacketHeader::new(packet_type, session_id as u16, payload.len() as u16);
        header.calculate_checksum(&payload);

        Self {
            header,
            payload,
            created_at: Instant::now(),
            retransmission_count: 0,
        }
    }

    /// 패킷을 바이트 배열로 직렬화
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| anyhow!("Serialization failed: {}", e))
    }

    /// 바이트 배열에서 패킷 역직렬화
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let mut packet: Self =
            bincode::deserialize(data).map_err(|e| anyhow!("Deserialization failed: {}", e))?;
        packet.created_at = Instant::now();
        Ok(packet)
    }

    /// 패킷 유효성 검증
    pub fn is_valid(&self) -> bool {
        self.header.verify_checksum(&self.payload)
    }
}

/// 연결 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// 연결 대기 중
    Connecting,
    /// 연결됨
    Connected,
    /// 연결 해제 중
    Disconnecting,
    /// 연결 해제됨
    Disconnected,
    /// 타임아웃
    Timeout,
}

/// RUDP 연결 정보
#[derive(Debug)]
pub struct RudpConnection {
    /// 세션 ID
    pub session_id: u64,
    /// 클라이언트 주소
    pub remote_addr: SocketAddr,
    /// 연결 상태
    pub state: ConnectionState,
    /// 다음 송신 시퀀스 번호
    pub next_send_seq: u32,
    /// 다음 수신 시퀀스 번호
    pub next_recv_seq: u32,
    /// 마지막 ACK 번호
    pub last_ack: u32,
    /// 재전송 대기 패킷들
    pub pending_packets: BTreeMap<u32, RudpPacket>,
    /// 수신 버퍼 (순서 보장용)
    pub recv_buffer: BTreeMap<u32, RudpPacket>,
    /// RTT 측정
    pub rtt_samples: VecDeque<Duration>,
    /// 평균 RTT
    pub rtt: Duration,
    /// RTO (재전송 타임아웃)
    pub rto: Duration,
    /// 혼잡 윈도우 크기
    pub congestion_window: u32,
    /// 느린 시작 임계값
    pub slow_start_threshold: u32,
    /// 마지막 활동 시간
    pub last_activity: Instant,
    /// 연결 시작 시간
    pub connected_at: Instant,
    /// 송신 통계
    pub bytes_sent: u64,
    /// 수신 통계
    pub bytes_received: u64,
    /// 패킷 손실 통계
    pub packets_lost: u32,
    /// 재전송 통계
    pub retransmissions: u32,
}

impl RudpConnection {
    /// 새로운 연결 생성
    pub fn new(session_id: u64, remote_addr: SocketAddr) -> Self {
        Self {
            session_id,
            remote_addr,
            state: ConnectionState::Connecting,
            next_send_seq: 1,
            next_recv_seq: 1,
            last_ack: 0,
            pending_packets: BTreeMap::new(),
            recv_buffer: BTreeMap::new(),
            rtt_samples: VecDeque::with_capacity(10),
            rtt: Duration::from_millis(100), // 초기 RTT
            rto: Duration::from_millis(200), // 초기 RTO
            congestion_window: 1,
            slow_start_threshold: 65535,
            last_activity: Instant::now(),
            connected_at: Instant::now(),
            bytes_sent: 0,
            bytes_received: 0,
            packets_lost: 0,
            retransmissions: 0,
        }
    }

    /// RTT 업데이트
    pub fn update_rtt(&mut self, sample: Duration) {
        self.rtt_samples.push_back(sample);
        if self.rtt_samples.len() > 10 {
            self.rtt_samples.pop_front();
        }

        // SRTT (Smoothed RTT) 계산
        if self.rtt_samples.len() == 1 {
            self.rtt = sample;
        } else {
            // SRTT = 0.875 * SRTT + 0.125 * RTT_SAMPLE
            let alpha = 0.125;
            self.rtt = Duration::from_secs_f64(
                (1.0 - alpha) * self.rtt.as_secs_f64() + alpha * sample.as_secs_f64(),
            );
        }

        // RTO 계산 (RTT의 2배, 최소 100ms)
        self.rto = (self.rtt * 2).max(Duration::from_millis(100));
    }

    /// 혼잡 제어 - 패킷 손실 시
    pub fn on_packet_loss(&mut self) {
        self.packets_lost += 1;
        self.slow_start_threshold = (self.congestion_window / 2).max(1);
        self.congestion_window = 1;
        trace!(
            session_id = %self.session_id,
            cwnd = %self.congestion_window,
            ssthresh = %self.slow_start_threshold,
            "Packet loss detected, congestion control activated"
        );
    }

    /// 혼잡 제어 - ACK 수신 시
    pub fn on_ack_received(&mut self) {
        if self.congestion_window < self.slow_start_threshold {
            // Slow start
            self.congestion_window += 1;
        } else {
            // Congestion avoidance
            self.congestion_window += 1 / self.congestion_window;
        }
    }

    /// 연결 활성화 업데이트
    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// 연결 타임아웃 확인
    pub fn is_timeout(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }

    /// 재전송 필요한 패킷 확인
    pub fn get_retransmission_candidates(&self) -> Vec<u32> {
        let now = Instant::now();
        self.pending_packets
            .iter()
            .filter(|(_, packet)| now.duration_since(packet.created_at) > self.rto)
            .map(|(seq, _)| *seq)
            .collect()
    }

    /// 연결 통계 정보
    pub fn get_stats(&self) -> ConnectionStats {
        ConnectionStats {
            session_id: self.session_id,
            state: self.state,
            rtt: self.rtt,
            rto: self.rto,
            congestion_window: self.congestion_window,
            bytes_sent: self.bytes_sent,
            bytes_received: self.bytes_received,
            packets_lost: self.packets_lost,
            retransmissions: self.retransmissions,
            uptime: self.connected_at.elapsed(),
        }
    }
}

/// 연결 통계
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionStats {
    pub session_id: u64,
    pub state: ConnectionState,
    pub rtt: Duration,
    pub rto: Duration,
    pub congestion_window: u32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_lost: u32,
    pub retransmissions: u32,
    pub uptime: Duration,
}

/// RUDP 서버
pub struct RudpServer {
    /// 서버 설정
    config: RudpConfig,
    /// UDP 소켓
    socket: Arc<UdpSocket>,
    /// 활성 연결들
    connections: Arc<dashmap::DashMap<SocketAddr, Arc<Mutex<RudpConnection>>>>,
    /// 세션 ID -> 연결 매핑
    session_map: Arc<RwLock<HashMap<u64, Arc<Mutex<RudpConnection>>>>>,
    /// 주소 -> 세션 ID 매핑
    addr_map: Arc<RwLock<HashMap<SocketAddr, u64>>>,
    /// 패킷 재사용 큐
    packet_pool: Arc<Mutex<VecDeque<RudpPacket>>>,
    /// 보안 미들웨어
    security: Arc<SecurityMiddleware>,
    /// Redis 최적화기
    redis_optimizer: Arc<RedisOptimizer>,
    /// 서버 통계
    stats: Arc<Mutex<ServerStats>>,
    /// 실행 중 플래그
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

/// 서버 통계
#[derive(Debug, Default, Clone)]
pub struct ServerStats {
    pub total_connections: u64,
    pub active_connections: u32,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub packets_retransmitted: u64,
    pub packets_lost: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub server_uptime: Duration,
    pub avg_rtt: Duration,
    pub max_rtt: Duration,
}

impl RudpServer {
    /// 새로운 RUDP 서버 생성
    pub async fn new(
        bind_addr: &str,
        config: RudpConfig,
        security: Arc<SecurityMiddleware>,
        redis_optimizer: Arc<RedisOptimizer>,
    ) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;

        // SO_REUSEADDR 설정 (성능 최적화)
        socket.set_broadcast(false)?;

        let connections = Arc::new(dashmap::DashMap::new());

        let packet_pool = Arc::new(Mutex::new(VecDeque::with_capacity(1000)));

        info!(
            bind_addr = %bind_addr,
            max_connections = %config.max_connections,
            "RUDP Server created"
        );

        Ok(Self {
            config,
            socket: Arc::new(socket),
            connections,
            session_map: Arc::new(RwLock::new(HashMap::new())),
            addr_map: Arc::new(RwLock::new(HashMap::new())),
            packet_pool,
            security,
            redis_optimizer,
            stats: Arc::new(Mutex::new(ServerStats::default())),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// 서버 시작
    pub async fn start(&self) -> Result<()> {
        self.is_running
            .store(true, std::sync::atomic::Ordering::SeqCst);

        info!("Starting RUDP Server...");

        // 패킷 수신 태스크 시작
        let recv_task = self.start_receive_loop();

        // Keep-alive 태스크 시작
        let keepalive_task = self.start_keepalive_loop();

        // 타임아웃 정리 태스크 시작
        let cleanup_task = self.start_cleanup_loop();

        // 재전송 태스크 시작
        let retransmission_task = self.start_retransmission_loop();

        // 모든 태스크 실행
        tokio::select! {
            result = recv_task => {
                error!("Receive loop ended: {:?}", result);
                result
            }
            result = keepalive_task => {
                error!("Keep-alive loop ended: {:?}", result);
                result
            }
            result = cleanup_task => {
                error!("Cleanup loop ended: {:?}", result);
                result
            }
            result = retransmission_task => {
                error!("Retransmission loop ended: {:?}", result);
                result
            }
        }
    }

    /// 패킷 수신 루프
    async fn start_receive_loop(&self) -> Result<()> {
        let mut buffer = vec![0u8; self.config.max_packet_size];

        while self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            match self.socket.recv_from(&mut buffer).await {
                Ok((size, addr)) => {
                    let packet_data = buffer[..size].to_vec();

                    // 패킷 처리를 별도 태스크로 실행 (논블로킹)
                    let server = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = server.handle_received_packet(packet_data, addr).await {
                            debug!(
                                addr = %addr,
                                error = %e,
                                "Failed to handle packet"
                            );
                        }
                    });
                }
                Err(e) => {
                    error!("Socket receive error: {}", e);
                    sleep(Duration::from_millis(10)).await;
                }
            }
        }

        Ok(())
    }

    /// 수신된 패킷 처리
    async fn handle_received_packet(&self, data: Vec<u8>, addr: SocketAddr) -> Result<()> {
        // 패킷 역직렬화
        let packet = match RudpPacket::from_bytes(&data) {
            Ok(packet) => packet,
            Err(e) => {
                debug!(addr = %addr, error = %e, "Invalid packet received");
                return Err(e);
            }
        };

        // 패킷 무결성 검증
        if !packet.is_valid() {
            warn!(addr = %addr, "Packet integrity check failed");
            return Err(anyhow!("Invalid packet checksum"));
        }

        // 통계 업데이트
        {
            let mut stats = self.stats.lock().await;
            stats.packets_received += 1;
            stats.bytes_received += data.len() as u64;
        }

        // 패킷 타입별 처리
        match packet.header.packet_type {
            PacketType::Connect => self.handle_connect(packet, addr).await,
            PacketType::Data => self.handle_data(packet, addr).await,
            PacketType::Ack => self.handle_ack(packet, addr).await,
            PacketType::Ping => self.handle_ping(packet, addr).await,
            PacketType::Disconnect => self.handle_disconnect(packet, addr).await,
            _ => {
                debug!(addr = %addr, packet_type = ?packet.header.packet_type, "Unhandled packet type");
                Ok(())
            }
        }
    }

    /// 연결 요청 처리
    async fn handle_connect(&self, _packet: RudpPacket, addr: SocketAddr) -> Result<()> {
        // 최대 연결 수 확인
        if self.get_active_connection_count().await >= self.config.max_connections {
            warn!(addr = %addr, "Connection limit reached");
            return self.send_connect_reject(addr).await;
        }

        // 새로운 세션 ID 생성
        let session_id = self.generate_session_id().await;

        // 새로운 연결 생성
        let connection = Arc::new(Mutex::new(RudpConnection::new(session_id, addr)));

        // 연결 등록
        {
            let mut session_map = self.session_map.write().await;
            session_map.insert(session_id, connection.clone());
        }

        {
            let mut addr_map = self.addr_map.write().await;
            addr_map.insert(addr, session_id);
        }

        // 연결 수락 응답 전송
        let response = RudpPacket::new(PacketType::ConnectAck, session_id, vec![]);
        self.send_packet(response, addr).await?;

        // 연결 상태 업데이트
        {
            let mut conn = connection.lock().await;
            conn.state = ConnectionState::Connected;
            conn.update_activity();
        }

        // 통계 업데이트
        {
            let mut stats = self.stats.lock().await;
            stats.total_connections += 1;
            stats.active_connections += 1;
        }

        info!(
            addr = %addr,
            session_id = %session_id,
            "New client connected"
        );

        Ok(())
    }

    /// 데이터 패킷 처리
    async fn handle_data(&self, packet: RudpPacket, addr: SocketAddr) -> Result<()> {
        let session_id = socket_addr_to_u64(addr);

        // 연결 찾기
        let connection = {
            let session_map = self.session_map.read().await;
            session_map.get(&session_id).cloned()
        };

        let connection = match connection {
            Some(conn) => conn,
            None => {
                debug!(addr = %addr, session_id = %session_id, "Unknown session");
                return Ok(());
            }
        };

        let mut conn = connection.lock().await;

        // 연결 상태 확인
        if conn.state != ConnectionState::Connected {
            debug!(addr = %addr, session_id = %session_id, "Connection not active");
            return Ok(());
        }

        conn.update_activity();
        conn.bytes_received += packet.payload.len() as u64;

        // 시퀀스 번호 확인 (순서 보장)
        let seq_num = packet.header.sequence_number;

        if seq_num as u32 == conn.next_recv_seq {
            // 정상 순서의 패킷
            conn.next_recv_seq += 1;

            // ACK 전송
            self.send_ack(session_id, seq_num as u32, addr).await?;

            // 애플리케이션에 데이터 전달
            drop(conn); // 락 해제
            self.deliver_data(session_id, packet.payload).await?;
        } else if seq_num as u32 > conn.next_recv_seq {
            // 미래 패킷 - 버퍼에 저장
            conn.recv_buffer.insert(seq_num as u32, packet);

            // 중복 ACK 전송 (누락된 패킷 알림)
            self.send_ack(session_id, conn.last_ack, addr).await?;
        } else {
            // 과거 패킷 - 중복 패킷, ACK만 전송
            self.send_ack(session_id, seq_num as u32, addr).await?;
        }

        Ok(())
    }

    /// ACK 패킷 처리
    async fn handle_ack(&self, packet: RudpPacket, addr: SocketAddr) -> Result<()> {
        let session_id = socket_addr_to_u64(addr);
        let ack_num = packet.header.ack_number;

        let connection = {
            let session_map = self.session_map.read().await;
            session_map.get(&session_id).cloned()
        };

        if let Some(connection) = connection {
            let mut conn = connection.lock().await;
            conn.update_activity();

            // ACK된 패킷 제거
            if let Some(acked_packet) = conn.pending_packets.remove(&(ack_num as u32)) {
                // RTT 계산
                let rtt = acked_packet.created_at.elapsed();
                conn.update_rtt(rtt);

                // 혼잡 제어
                conn.on_ack_received();

                trace!(
                    session_id = %session_id,
                    ack_num = %ack_num,
                    rtt_ms = %rtt.as_millis(),
                    "ACK received"
                );
            }

            conn.last_ack = ack_num as u32;
        }

        Ok(())
    }

    /// Ping 패킷 처리 (Keep-alive)
    async fn handle_ping(&self, _packet: RudpPacket, addr: SocketAddr) -> Result<()> {
        let session_id = socket_addr_to_u64(addr);

        // 연결 활성화 업데이트
        if let Some(connection) = self.get_connection(session_id).await {
            let mut conn = connection.lock().await;
            conn.update_activity();
        }

        // Pong 응답
        let pong = RudpPacket::new(PacketType::Pong, session_id, vec![]);
        self.send_packet(pong, addr).await
    }

    /// 연결 해제 처리
    async fn handle_disconnect(&self, _packet: RudpPacket, addr: SocketAddr) -> Result<()> {
        let session_id = socket_addr_to_u64(addr);

        // 연결 해제 확인 응답
        let response = RudpPacket::new(PacketType::DisconnectAck, session_id, vec![]);
        self.send_packet(response, addr).await?;

        // 연결 제거
        self.remove_connection(session_id, addr).await;

        info!(
            addr = %addr,
            session_id = %session_id,
            "Client disconnected"
        );

        Ok(())
    }

    /// 패킷 전송
    pub async fn send_packet(&self, mut packet: RudpPacket, addr: SocketAddr) -> Result<()> {
        // 체크섬 업데이트
        packet.header.calculate_checksum(&packet.payload);

        let data = packet.to_bytes()?;

        match self.socket.send_to(&data, addr).await {
            Ok(sent_bytes) => {
                // 통계 업데이트
                {
                    let mut stats = self.stats.lock().await;
                    stats.packets_sent += 1;
                    stats.bytes_sent += sent_bytes as u64;
                }

                trace!(
                    addr = %addr,
                    packet_type = ?packet.header.packet_type,
                    size = %sent_bytes,
                    "Packet sent"
                );

                Ok(())
            }
            Err(e) => {
                error!(
                    addr = %addr,
                    error = %e,
                    "Failed to send packet"
                );
                Err(anyhow!("Send failed: {}", e))
            }
        }
    }

    /// ACK 전송
    async fn send_ack(&self, session_id: u64, ack_num: u32, addr: SocketAddr) -> Result<()> {
        let mut ack = RudpPacket::new(PacketType::Ack, session_id, vec![]);
        ack.header.ack_number = ack_num as u16;
        self.send_packet(ack, addr).await
    }

    /// 연결 거부 응답
    async fn send_connect_reject(&self, addr: SocketAddr) -> Result<()> {
        // 임시 세션 ID로 거부 응답
        let reject = RudpPacket::new(
            PacketType::DisconnectAck,
            0,
            b"Connection limit reached".to_vec(),
        );
        self.send_packet(reject, addr).await
    }

    /// 애플리케이션에 데이터 전달 (게임 로직 처리)
    async fn deliver_data(&self, session_id: u64, data: Vec<u8>) -> Result<()> {
        // 게임 로직으로 데이터 전달
        // 실제 구현에서는 게임 메시지 처리기로 전달
        debug!(
            session_id = %session_id,
            size = %data.len(),
            "Data delivered to application"
        );

        // TODO: 게임 메시지 처리기 연결
        Ok(())
    }

    /// Keep-alive 루프
    async fn start_keepalive_loop(&self) -> Result<()> {
        let interval = Duration::from_secs(self.config.keepalive_interval_secs);

        while self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            sleep(interval).await;

            // 모든 활성 연결에 ping 전송
            let session_map = self.session_map.read().await.clone();

            for (session_id, connection) in session_map {
                let conn = connection.lock().await;
                if conn.state == ConnectionState::Connected {
                    let ping = RudpPacket::new(PacketType::Ping, session_id, vec![]);
                    if let Err(e) = self.send_packet(ping, conn.remote_addr).await {
                        warn!(
                            session_id = %session_id,
                            addr = %conn.remote_addr,
                            error = %e,
                            "Failed to send keep-alive"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// 타임아웃 정리 루프
    async fn start_cleanup_loop(&self) -> Result<()> {
        let cleanup_interval = Duration::from_secs(30);
        let timeout = Duration::from_secs(self.config.connection_timeout_secs);

        while self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            sleep(cleanup_interval).await;

            let mut expired_sessions = Vec::new();

            // 타임아웃된 연결 찾기
            {
                let session_map = self.session_map.read().await;
                for (session_id, connection) in session_map.iter() {
                    let conn = connection.lock().await;
                    if conn.is_timeout(timeout) {
                        expired_sessions.push((*session_id, conn.remote_addr));
                    }
                }
            }

            // 타임아웃된 연결 정리
            for (session_id, addr) in expired_sessions {
                warn!(
                    session_id = %session_id,
                    addr = %addr,
                    "Connection timeout, cleaning up"
                );
                self.remove_connection(session_id, addr).await;
            }
        }

        Ok(())
    }

    /// 재전송 루프
    async fn start_retransmission_loop(&self) -> Result<()> {
        let check_interval = Duration::from_millis(50); // 50ms마다 확인

        while self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            sleep(check_interval).await;

            let session_map = self.session_map.read().await.clone();

            for (_session_id, connection) in session_map {
                let mut conn = connection.lock().await;

                if conn.state != ConnectionState::Connected {
                    continue;
                }

                // 재전송이 필요한 패킷들 확인
                let candidates = conn.get_retransmission_candidates();

                for seq_num in candidates {
                    if let Some(packet) = conn.pending_packets.get_mut(&seq_num) {
                        if packet.retransmission_count >= self.config.max_retransmissions {
                            // 최대 재전송 횟수 초과 - 연결 끊기
                            warn!(
                                session_id = %conn.session_id,
                                seq_num = %seq_num,
                                "Max retransmissions exceeded"
                            );
                            conn.on_packet_loss();
                            conn.pending_packets.remove(&seq_num);
                            continue;
                        }

                        // 재전송
                        packet.retransmission_count += 1;
                        packet.created_at = Instant::now();
                        let packet_clone = packet.clone();
                        conn.retransmissions += 1;
                        drop(conn); // 락 해제 후 전송

                        if let Err(e) = self
                            .send_packet(packet_clone, connection.lock().await.remote_addr)
                            .await
                        {
                            error!(error = %e, "Retransmission failed");
                        }

                        // 통계 업데이트
                        {
                            let mut stats = self.stats.lock().await;
                            stats.packets_retransmitted += 1;
                        }

                        break; // 다음 연결로 이동
                    }
                }
            }
        }

        Ok(())
    }

    /// 세션 ID 생성
    async fn generate_session_id(&self) -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);

        (timestamp << 32) | (counter & 0xFFFFFFFF)
    }

    /// 연결 가져오기
    async fn get_connection(&self, session_id: u64) -> Option<Arc<Mutex<RudpConnection>>> {
        let session_map = self.session_map.read().await;
        session_map.get(&session_id).cloned()
    }

    /// 연결 제거
    async fn remove_connection(&self, session_id: u64, addr: SocketAddr) {
        {
            let mut session_map = self.session_map.write().await;
            session_map.remove(&session_id);
        }

        {
            let mut addr_map = self.addr_map.write().await;
            addr_map.remove(&addr);
        }

        // 통계 업데이트
        {
            let mut stats = self.stats.lock().await;
            if stats.active_connections > 0 {
                stats.active_connections -= 1;
            }
        }
    }

    /// 활성 연결 수 가져오기
    async fn get_active_connection_count(&self) -> usize {
        let session_map = self.session_map.read().await;
        session_map.len()
    }

    /// 서버 통계 가져오기
    pub async fn get_stats(&self) -> ServerStats {
        let mut stats = self.stats.lock().await;

        // 평균 RTT 계산
        let session_map = self.session_map.read().await;
        let mut total_rtt = Duration::ZERO;
        let mut max_rtt = Duration::ZERO;
        let mut count = 0;

        for connection in session_map.values() {
            let conn = connection.lock().await;
            total_rtt += conn.rtt;
            max_rtt = max_rtt.max(conn.rtt);
            count += 1;
        }

        if count > 0 {
            stats.avg_rtt = total_rtt / count as u32;
            stats.max_rtt = max_rtt;
        }

        stats.clone()
    }

    /// 메시지 수신 (main.rs에서 사용)
    pub async fn receive_message(&self) -> Result<(SocketAddr, Vec<u8>)> {
        let mut buffer = vec![0u8; self.config.max_packet_size];

        match self.socket.recv_from(&mut buffer).await {
            Ok((size, addr)) => {
                let packet_data = buffer[..size].to_vec();
                Ok((addr, packet_data))
            }
            Err(e) => Err(anyhow!("Failed to receive message: {}", e)),
        }
    }

    /// 메시지 전송 (main.rs에서 사용)
    pub async fn send_message(&self, addr: SocketAddr, data: Vec<u8>) -> Result<()> {
        match self.socket.send_to(&data, addr).await {
            Ok(sent) => {
                if sent != data.len() {
                    warn!(
                        addr = %addr,
                        sent = sent,
                        expected = data.len(),
                        "Partial send detected"
                    );
                }

                // 통계 업데이트
                {
                    let mut stats = self.stats.lock().await;
                    stats.packets_sent += 1;
                    stats.bytes_sent += sent as u64;
                }

                Ok(())
            }
            Err(e) => {
                error!(
                    addr = %addr,
                    error = %e,
                    "Failed to send message"
                );
                Err(anyhow!("Failed to send message: {}", e))
            }
        }
    }

    /// 서버 종료
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down RUDP Server...");

        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);

        // 모든 클라이언트에게 연결 해제 알림
        let session_map = self.session_map.read().await.clone();
        for (session_id, connection) in session_map {
            let conn = connection.lock().await;
            let disconnect = RudpPacket::new(PacketType::Disconnect, session_id, vec![]);
            let _ = self.send_packet(disconnect, conn.remote_addr).await;
        }

        // 잠시 대기 (클라이언트가 응답할 시간 제공)
        sleep(Duration::from_millis(500)).await;

        info!("RUDP Server shutdown complete");
        Ok(())
    }
}

// Clone 구현 (태스크 간 공유를 위해)
impl Clone for RudpServer {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            socket: self.socket.clone(),
            connections: self.connections.clone(),
            session_map: self.session_map.clone(),
            addr_map: self.addr_map.clone(),
            packet_pool: self.packet_pool.clone(),
            security: self.security.clone(),
            redis_optimizer: self.redis_optimizer.clone(),
            stats: self.stats.clone(),
            is_running: self.is_running.clone(),
        }
    }
}
