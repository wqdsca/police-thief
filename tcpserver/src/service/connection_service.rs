//! 연결 서비스
//! 
//! 클라이언트 연결 관리, 메시지 라우팅, 상태 추적을 담당합니다.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, broadcast};
use tokio::time::{Duration, Instant};
use tracing::{info, error, warn, debug};
use chrono;

use crate::protocol::GameMessage;
use crate::tool::{SimpleUtils, error::{TcpServerError, ErrorHandler, ErrorSeverity}};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::io::{BufReader, BufWriter};

/// 개별 클라이언트 연결 정보
#[derive(Debug)]
pub struct ClientConnection {
    pub client_id: u32,
    pub addr: String,
    pub last_heartbeat: Instant,
    pub writer: Arc<Mutex<BufWriter<OwnedWriteHalf>>>,
    pub connected_at: Instant,
}

impl ClientConnection {
    pub fn new(client_id: u32, addr: String, stream: TcpStream) -> Self {
        let (reader, writer) = stream.into_split();
        let writer = Arc::new(Mutex::new(BufWriter::new(writer)));
        
        Self {
            client_id,
            addr,
            last_heartbeat: Instant::now(),
            writer,
            connected_at: Instant::now(),
        }
    }
    
    pub async fn send_message(&self, message: &GameMessage) -> Result<()> {
        let mut writer = self.writer.lock().await;
        message.write_to_stream(&mut *writer).await
            .map_err(|e| TcpServerError::network_error(Some(self.addr.clone()), "send_message", &e.to_string()))?;
        
        debug!("클라이언트 {}에게 메시지 전송: {:?}", self.client_id, message);
        Ok(())
    }
    
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
        debug!("클라이언트 {} 하트비트 업데이트", self.client_id);
    }
    
    pub fn is_heartbeat_timeout(&self) -> bool {
        self.last_heartbeat.elapsed() > Duration::from_secs(30)
    }
}

/// 연결 서비스
pub struct ConnectionService {
    connections: Arc<Mutex<HashMap<u32, Arc<Mutex<ClientConnection>>>>>,
    next_client_id: Arc<Mutex<u32>>,
    broadcast_tx: broadcast::Sender<(Option<u32>, GameMessage)>,
    max_connections: u32,
    server_start_time: Instant,
    connection_stats: Arc<Mutex<ConnectionStats>>,
}

/// 연결 통계
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    pub total_connections: u64,
    pub current_connections: u32,
    pub peak_connections: u32,
    pub total_messages: u64,
    pub failed_connections: u64,
    pub timeout_disconnections: u64,
}

impl ConnectionService {
    /// 새로운 연결 서비스 생성
    pub fn new(max_connections: u32) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            next_client_id: Arc::new(Mutex::new(1)),
            broadcast_tx,
            max_connections,
            server_start_time: Instant::now(),
            connection_stats: Arc::new(Mutex::new(ConnectionStats::default())),
        }
    }
    
    /// 새로운 클라이언트 연결 처리
    pub async fn handle_new_connection(&self, stream: TcpStream, addr: String) -> Result<u32> {
        // 최대 연결 수 확인
        let current_count = self.get_connection_count().await;
        if current_count >= self.max_connections as usize {
            warn!("최대 연결 수 초과: {}/{}", current_count, self.max_connections);
            return Err(anyhow!("서버가 가득 참"));
        }
        
        // 클라이언트 ID 할당
        let mut next_id = self.next_client_id.lock().await;
        let client_id = *next_id;
        *next_id += 1;
        drop(next_id);
        
        debug!("클라이언트 연결 요청: {}", addr);
        
        // 연결 생성 및 저장
        let (reader, writer) = stream.into_split();
        let connection = Arc::new(Mutex::new(ClientConnection {
            client_id,
            addr: addr.clone(),
            last_heartbeat: Instant::now(),
            writer: Arc::new(Mutex::new(BufWriter::new(writer))),
            connected_at: Instant::now(),
        }));
        
        {
            let mut connections = self.connections.lock().await;
            connections.insert(client_id, connection.clone());
        }
        
        // 통계 업데이트
        self.update_connection_stats(|stats| {
            stats.total_connections += 1;
            stats.current_connections += 1;
            stats.peak_connections = stats.peak_connections.max(stats.current_connections);
        }).await;
        
        // 연결 확인 메시지 전송
        let ack_message = GameMessage::ConnectionAck { client_id };
        if let Err(e) = connection.lock().await.send_message(&ack_message).await {
            let tcp_error = TcpServerError::connection_error(Some(client_id), Some(addr.clone()), &format!("연결 확인 메시지 전송 실패: {}", e));
            ErrorHandler::handle_error(tcp_error.clone(), ErrorSeverity::Error, "ConnectionService", "send_ack_message");
            self.remove_connection(client_id).await;
            return Err(anyhow::anyhow!(tcp_error));
        }
        
        // 메시지 수신 처리 시작
        self.start_message_handling(client_id, connection.clone(), reader).await;
        
        info!("✅ 클라이언트 {} 연결 완료 ({})", client_id, addr);
        Ok(client_id)
    }
    
    /// 메시지 수신 처리 시작
    async fn start_message_handling(&self, client_id: u32, connection: Arc<Mutex<ClientConnection>>, reader: OwnedReadHalf) {
        let connections_ref = self.connections.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        let stats_ref = self.connection_stats.clone();
        
        tokio::spawn(async move {
            let mut reader = BufReader::new(reader);
            
            loop {
                match GameMessage::read_from_stream(&mut reader).await {
                    Ok(message) => {
                        debug!("클라이언트 {}에서 메시지 수신: {:?}", client_id, message);
                        
                        // 하트비트 처리
                        if matches!(message, GameMessage::HeartBeat) {
                            if let Some(conn) = connections_ref.lock().await.get(&client_id) {
                                conn.lock().await.update_heartbeat();
                                
                                let response = GameMessage::HeartBeatResponse { 
                                    timestamp: chrono::Utc::now().timestamp() 
                                };
                                
                                if let Err(e) = conn.lock().await.send_message(&response).await {
                                    let tcp_error = TcpServerError::heartbeat_error(Some(client_id), "send_response", &e.to_string());
                                    ErrorHandler::handle_error(tcp_error, ErrorSeverity::Warning, "ConnectionService", "heartbeat_response");
                                    break;
                                }
                            }
                        }
                        
                        // 메시지 통계 업데이트
                        if let Ok(mut stats) = stats_ref.try_lock() {
                            stats.total_messages += 1;
                        }
                        
                        // 다른 메시지들은 브로드캐스트 채널로 전송
                        if let Err(e) = broadcast_tx.send((Some(client_id), message)) {
                            warn!("브로드캐스트 전송 실패: {}", e);
                        }
                    }
                    Err(e) => {
                        info!("클라이언트 {} 연결 종료: {}", client_id, e);
                        break;
                    }
                }
            }
            
            // 연결 정리
            connections_ref.lock().await.remove(&client_id);
            
            // 통계 업데이트
            if let Ok(mut stats) = stats_ref.try_lock() {
                stats.current_connections = stats.current_connections.saturating_sub(1);
            }
            
            info!("클라이언트 {} 연결 해제 완료", client_id);
        });
    }
    
    /// 연결 제거
    pub async fn remove_connection(&self, client_id: u32) -> bool {
        let mut connections = self.connections.lock().await;
        let removed = connections.remove(&client_id).is_some();
        
        if removed {
            self.update_connection_stats(|stats| {
                stats.current_connections = stats.current_connections.saturating_sub(1);
            }).await;
            
            debug!("클라이언트 {} 연결 제거됨", client_id);
        }
        
        removed
    }
    
    /// 특정 클라이언트에게 메시지 전송
    pub async fn send_to_client(&self, client_id: u32, message: &GameMessage) -> Result<()> {
        let connections = self.connections.lock().await;
        
        if let Some(connection) = connections.get(&client_id) {
            connection.lock().await.send_message(message).await?;
            
            self.update_connection_stats(|stats| {
                stats.total_messages += 1;
            }).await;
            
            Ok(())
        } else {
            Err(anyhow!("클라이언트 {}를 찾을 수 없습니다", client_id))
        }
    }
    
    /// 모든 클라이언트에게 브로드캐스트
    pub async fn broadcast_message(&self, message: &GameMessage) -> Result<usize> {
        let connections = self.connections.lock().await;
        let mut success_count = 0;
        
        for (client_id, connection) in connections.iter() {
            if let Ok(()) = connection.lock().await.send_message(message).await {
                success_count += 1;
            } else {
                warn!("클라이언트 {}에게 브로드캐스트 실패", client_id);
            }
        }
        
        self.update_connection_stats(|stats| {
            stats.total_messages += success_count as u64;
        }).await;
        
        debug!("브로드캐스트 완료: {}/{} 성공", success_count, connections.len());
        Ok(success_count)
    }
    
    /// 타임아웃된 연결 정리
    pub async fn cleanup_timeout_connections(&self) -> usize {
        let mut connections = self.connections.lock().await;
        let mut timeout_clients = Vec::new();
        
        for (client_id, connection) in connections.iter() {
            if connection.lock().await.is_heartbeat_timeout() {
                timeout_clients.push(*client_id);
            }
        }
        
        for client_id in &timeout_clients {
            connections.remove(client_id);
            warn!("클라이언트 {} 하트비트 타임아웃으로 연결 해제", client_id);
        }
        
        if !timeout_clients.is_empty() {
            self.update_connection_stats(|stats| {
                stats.timeout_disconnections += timeout_clients.len() as u64;
                stats.current_connections = stats.current_connections.saturating_sub(timeout_clients.len() as u32);
            }).await;
        }
        
        timeout_clients.len()
    }
    
    /// 연결 수 조회
    pub async fn get_connection_count(&self) -> usize {
        self.connections.lock().await.len()
    }
    
    /// 모든 연결 종료
    pub async fn close_all_connections(&self) {
        let mut connections = self.connections.lock().await;
        let count = connections.len();
        connections.clear();
        
        self.update_connection_stats(|stats| {
            stats.current_connections = 0;
        }).await;
        
        info!("모든 클라이언트 연결 해제: {}개", count);
    }
    
    /// 브로드캐스트 수신자 생성
    pub fn subscribe_broadcast(&self) -> broadcast::Receiver<(Option<u32>, GameMessage)> {
        self.broadcast_tx.subscribe()
    }
    
    /// 서버 업타임 (초)
    pub async fn get_uptime_seconds(&self) -> u64 {
        self.server_start_time.elapsed().as_secs()
    }
    
    /// 연결 통계 업데이트
    async fn update_connection_stats<F>(&self, update_fn: F) 
    where 
        F: FnOnce(&mut ConnectionStats)
    {
        if let Ok(mut stats) = self.connection_stats.try_lock() {
            update_fn(&mut *stats);
        }
    }
    
    /// 연결 통계 조회
    pub async fn get_connection_stats(&self) -> ConnectionStats {
        self.connection_stats.lock().await.clone()
    }
    
    /// 클라이언트 연결 정보 조회
    pub async fn get_client_info(&self, client_id: u32) -> Option<ClientInfo> {
        let connections = self.connections.lock().await;
        
        if let Some(connection) = connections.get(&client_id) {
            let conn = connection.lock().await;
            Some(ClientInfo {
                client_id: conn.client_id,
                addr: conn.addr.clone(),
                connected_at: conn.connected_at,
                last_heartbeat: conn.last_heartbeat,
                uptime_seconds: conn.connected_at.elapsed().as_secs(),
            })
        } else {
            None
        }
    }
    
    /// 모든 클라이언트 목록 조회
    pub async fn get_all_clients(&self) -> Vec<ClientInfo> {
        let connections = self.connections.lock().await;
        let mut clients = Vec::new();
        
        for connection in connections.values() {
            let conn = connection.lock().await;
            clients.push(ClientInfo {
                client_id: conn.client_id,
                addr: conn.addr.clone(),
                connected_at: conn.connected_at,
                last_heartbeat: conn.last_heartbeat,
                uptime_seconds: conn.connected_at.elapsed().as_secs(),
            });
        }
        
        clients.sort_by_key(|c| c.client_id);
        clients
    }
}

/// 클라이언트 정보
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClientInfo {
    pub client_id: u32,
    pub addr: String,
    #[serde(skip)]
    pub connected_at: Instant,
    #[serde(skip)]
    pub last_heartbeat: Instant,
    pub uptime_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_connection_service() {
        let service = ConnectionService::new(100);
        
        assert_eq!(service.get_connection_count().await, 0);
        assert!(service.get_uptime_seconds().await >= 0);
        
        let stats = service.get_connection_stats().await;
        assert_eq!(stats.current_connections, 0);
        assert_eq!(stats.total_connections, 0);
    }
    
    #[tokio::test]
    async fn test_broadcast_subscription() {
        let service = ConnectionService::new(100);
        let mut receiver = service.subscribe_broadcast();
        
        // 브로드캐스트 테스트는 실제 연결이 있어야 의미있음
        assert!(receiver.try_recv().is_err()); // 아직 메시지 없음
    }
}