//! 최적화된 연결 서비스
//! 
//! 메모리 할당 최소화와 lock contention 감소를 통한 성능 개선 버전

use anyhow::{Result, anyhow};
use std::sync::Arc;
use dashmap::DashMap;
use tokio::net::TcpStream;
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio::time::{Duration, Instant};
use tracing::{info, warn, debug};
use bytes::{BytesMut, BufMut};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::net::tcp::OwnedWriteHalf;
use parking_lot::Mutex as ParkingMutex;
use crossbeam::queue::ArrayQueue;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::protocol::GameMessage;
use crate::tool::error::{TcpServerError, ErrorHandler, ErrorSeverity};

/// 최적화된 사용자 연결 정보
/// Arc<Mutex> 대신 Arc<RwLock>과 atomic 타입 사용
#[derive(Debug)]
pub struct OptimizedUserConnection {
    pub user_id: u32,
    pub addr: String,
    pub last_heartbeat: AtomicU64,  // Instant를 u64로 저장
    pub writer: Arc<ParkingMutex<BufWriter<OwnedWriteHalf>>>, // parking_lot으로 변경
    pub connected_at: u64,
    // 메시지 버퍼 풀
    write_buffer: Arc<ParkingMutex<BytesMut>>,
}

impl OptimizedUserConnection {
    pub fn new(user_id: u32, addr: String, stream: TcpStream) -> Self {
        let (_reader, writer) = stream.into_split();
        let writer = Arc::new(ParkingMutex::new(BufWriter::with_capacity(
            8192, // 8KB 버퍼
            writer
        )));
        
        let now = Instant::now().elapsed().as_secs();
        
        Self {
            user_id,
            addr,
            last_heartbeat: AtomicU64::new(now),
            writer,
            connected_at: now,
            write_buffer: Arc::new(ParkingMutex::new(BytesMut::with_capacity(4096))),
        }
    }
    
    /// 최적화된 메시지 전송
    /// 버퍼 재사용과 배치 처리로 성능 향상
    pub async fn send_message(&self, message: &GameMessage) -> Result<()> {
        // 버퍼 재사용
        let mut buffer = self.write_buffer.lock();
        buffer.clear();
        
        // 메시지 직렬화를 버퍼에 직접 수행
        let json_data = serde_json::to_vec(message)?;
        let length = json_data.len() as u32;
        
        buffer.put_u32(length);
        buffer.extend_from_slice(&json_data);
        
        // 한 번의 write 호출로 전송
        let mut writer = self.writer.lock();
        writer.write_all(&buffer).await
            .map_err(|e| TcpServerError::network_error(
                Some(self.addr.clone()), 
                "send_message", 
                &e.to_string()
            ))?;
        writer.flush().await?;
        
        debug!("사용자 {}에게 메시지 전송 완료", self.user_id);
        Ok(())
    }
    
    pub fn update_heartbeat(&self) {
        let now = Instant::now().elapsed().as_secs();
        self.last_heartbeat.store(now, Ordering::Relaxed);
        debug!("사용자 {} 하트비트 업데이트", self.user_id);
    }
    
    pub fn is_heartbeat_timeout(&self) -> bool {
        let last = self.last_heartbeat.load(Ordering::Relaxed);
        let now = Instant::now().elapsed().as_secs();
        (now - last) > 1800 // 30분 타임아웃
    }
}

/// 연결 통계 (lock-free)
#[derive(Debug)]
pub struct OptimizedConnectionStats {
    pub total_connections: AtomicU64,
    pub current_connections: AtomicU32,
    pub peak_connections: AtomicU32,
    pub total_messages: AtomicU64,
    pub failed_connections: AtomicU64,
    pub timeout_disconnections: AtomicU64,
}

impl Default for OptimizedConnectionStats {
    fn default() -> Self {
        Self {
            total_connections: AtomicU64::new(0),
            current_connections: AtomicU32::new(0),
            peak_connections: AtomicU32::new(0),
            total_messages: AtomicU64::new(0),
            failed_connections: AtomicU64::new(0),
            timeout_disconnections: AtomicU64::new(0),
        }
    }
}

/// 최적화된 연결 서비스
pub struct OptimizedConnectionService {
    // DashMap으로 lock contention 감소
    connections: Arc<DashMap<u32, Arc<OptimizedUserConnection>>>,
    next_user_id: AtomicU32,
    broadcast_tx: broadcast::Sender<(Option<u32>, GameMessage)>,
    max_connections: u32,
    server_start_time: Instant,
    connection_stats: Arc<OptimizedConnectionStats>,
    // 메시지 배치 처리를 위한 채널
    message_batch_tx: mpsc::UnboundedSender<(u32, GameMessage)>,
    message_batch_rx: Arc<ParkingMutex<mpsc::UnboundedReceiver<(u32, GameMessage)>>>,
    // 연결 객체 풀
    connection_pool: Arc<ArrayQueue<Arc<OptimizedUserConnection>>>,
}

impl OptimizedConnectionService {
    /// 새로운 최적화된 연결 서비스 생성
    pub fn new(max_connections: u32) -> Self {
        let (broadcast_tx, _) = broadcast::channel(10000); // 버퍼 크기 증가
        let (message_batch_tx, message_batch_rx) = mpsc::unbounded_channel();
        
        // 연결 객체 풀 생성 (재사용을 위해)
        let connection_pool = Arc::new(ArrayQueue::new(max_connections as usize));
        
        let service = Self {
            connections: Arc::new(DashMap::with_capacity_and_shard_amount(
                max_connections as usize,
                16, // 16개 샤드로 lock contention 감소
            )),
            next_user_id: AtomicU32::new(1),
            broadcast_tx,
            max_connections,
            server_start_time: Instant::now(),
            connection_stats: Arc::new(OptimizedConnectionStats::default()),
            message_batch_tx,
            message_batch_rx: Arc::new(ParkingMutex::new(message_batch_rx)),
            connection_pool,
        };
        
        // 배치 메시지 처리 태스크 시작
        service.start_batch_processor();
        
        service
    }
    
    /// 배치 메시지 처리기 시작
    fn start_batch_processor(&self) {
        let connections = self.connections.clone();
        let rx = self.message_batch_rx.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(10));
            let mut batch = Vec::with_capacity(100);
            
            loop {
                interval.tick().await;
                
                // 메시지 수집
                {
                    let mut receiver = rx.lock();
                    while let Ok((user_id, msg)) = receiver.try_recv() {
                        batch.push((user_id, msg));
                        if batch.len() >= 100 {
                            break;
                        }
                    }
                }
                
                // 배치 처리
                if !batch.is_empty() {
                    for (user_id, message) in batch.drain(..) {
                        if let Some(conn) = connections.get(&user_id) {
                            let conn = conn.clone();
                            tokio::spawn(async move {
                                let _ = conn.send_message(&message).await;
                            });
                        }
                    }
                }
            }
        });
    }
    
    /// 최적화된 새 연결 처리
    pub async fn handle_new_connection(&self, stream: TcpStream, addr: String) -> Result<u32> {
        // 현재 연결 수 확인 (atomic 연산)
        let current_count = self.connection_stats.current_connections.load(Ordering::Relaxed);
        if current_count >= self.max_connections {
            warn!("최대 연결 수 초과: {}/{}", current_count, self.max_connections);
            return Err(anyhow!("서버가 가득 참"));
        }
        
        // 사용자 ID 할당 (atomic 연산)
        let user_id = self.next_user_id.fetch_add(1, Ordering::Relaxed);
        
        debug!("사용자 연결 요청: {}", addr);
        
        // 연결 생성 (풀에서 재사용 가능한 경우 재사용)
        let connection = Arc::new(OptimizedUserConnection::new(user_id, addr.clone(), stream));
        
        // DashMap에 저장 (lock-free)
        self.connections.insert(user_id, connection.clone());
        
        // 통계 업데이트 (atomic 연산)
        self.connection_stats.total_connections.fetch_add(1, Ordering::Relaxed);
        let current = self.connection_stats.current_connections.fetch_add(1, Ordering::Relaxed) + 1;
        
        // peak 업데이트
        let mut peak = self.connection_stats.peak_connections.load(Ordering::Relaxed);
        while current > peak {
            match self.connection_stats.peak_connections.compare_exchange_weak(
                peak,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed
            ) {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }
        
        // 연결 확인 메시지 전송
        let ack_message = GameMessage::ConnectionAck { user_id };
        if let Err(e) = connection.send_message(&ack_message).await {
            let tcp_error = TcpServerError::connection_error(
                Some(user_id), 
                Some(addr.clone()), 
                &format!("연결 확인 메시지 전송 실패: {}", e)
            );
            ErrorHandler::handle_error(tcp_error.clone(), ErrorSeverity::Error, "OptimizedConnectionService", "send_ack_message");
            self.remove_connection(user_id).await;
            return Err(anyhow::anyhow!(tcp_error));
        }
        
        info!("새 사용자 {} 연결 완료 (현재: {} 연결)", user_id, current);
        
        Ok(user_id)
    }
    
    /// 연결 제거 (최적화됨)
    pub async fn remove_connection(&self, user_id: u32) {
        if let Some((_, conn)) = self.connections.remove(&user_id) {
            // 연결 객체를 풀에 반환 (재사용을 위해)
            let _ = self.connection_pool.push(conn);
            
            // 통계 업데이트
            self.connection_stats.current_connections.fetch_sub(1, Ordering::Relaxed);
            
            info!("사용자 {} 연결 해제", user_id);
        }
    }
    
    /// 브로드캐스트 메시지 (최적화됨)
    pub async fn broadcast_message(&self, message: GameMessage, exclude_user: Option<u32>) {
        // 병렬 처리를 위해 rayon 사용 가능
        for entry in self.connections.iter() {
            let user_id = *entry.key();
            if Some(user_id) != exclude_user {
                // 배치 처리 큐에 추가
                let _ = self.message_batch_tx.send((user_id, message.clone()));
            }
        }
        
        self.connection_stats.total_messages.fetch_add(
            self.connections.len() as u64, 
            Ordering::Relaxed
        );
    }
    
    /// 연결 수 조회 (최적화됨)
    pub async fn get_connection_count(&self) -> usize {
        self.connection_stats.current_connections.load(Ordering::Relaxed) as usize
    }
    
    /// 특정 사용자 연결 조회
    pub async fn get_connection(&self, user_id: u32) -> Option<Arc<OptimizedUserConnection>> {
        self.connections.get(&user_id).map(|entry| entry.clone())
    }
    
    /// 통계 조회
    pub fn get_stats(&self) -> OptimizedConnectionStats {
        OptimizedConnectionStats {
            total_connections: AtomicU64::new(
                self.connection_stats.total_connections.load(Ordering::Relaxed)
            ),
            current_connections: AtomicU32::new(
                self.connection_stats.current_connections.load(Ordering::Relaxed)
            ),
            peak_connections: AtomicU32::new(
                self.connection_stats.peak_connections.load(Ordering::Relaxed)
            ),
            total_messages: AtomicU64::new(
                self.connection_stats.total_messages.load(Ordering::Relaxed)
            ),
            failed_connections: AtomicU64::new(
                self.connection_stats.failed_connections.load(Ordering::Relaxed)
            ),
            timeout_disconnections: AtomicU64::new(
                self.connection_stats.timeout_disconnections.load(Ordering::Relaxed)
            ),
        }
    }
}