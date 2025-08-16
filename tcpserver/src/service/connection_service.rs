//! 연결 서비스
//!
//! 사용자 연결 관리, 메시지 라우팅, 상태 추적을 담당합니다.

use anyhow::{anyhow, Result};
use chrono;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::protocol::GameMessage;
use crate::tool::{
    error::{ErrorHandler, ErrorSeverity, TcpServerError},
    SimpleUtils,
};
use tokio::io::{BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

/// 개별 사용자 연결 정보
#[derive(Debug)]
pub struct UserConnection {
    pub user_id: u32,
    pub addr: String,
    pub last_heartbeat: Instant,
    pub writer: Arc<Mutex<BufWriter<OwnedWriteHalf>>>,
    pub connected_at: Instant,
}

impl UserConnection {
    /// 새로운 사용자 연결 생성
    ///
    /// TCP 스트림을 분리하여 쓰기 전용 연결을 생성합니다.
    /// 연결 시간과 마지막 하트비트 시간을 현재 시간으로 초기화합니다.
    ///
    /// # Arguments
    ///
    /// * `user_id` - 할당된 사용자 ID
    /// * `addr` - 클라이언트 주소
    /// * `stream` - TCP 연결 스트림
    ///
    /// # Returns
    ///
    /// 새로운 UserConnection 인스턴스
    pub fn new(user_id: u32, addr: String, stream: TcpStream) -> Self {
        let (_reader, writer) = stream.into_split();
        let writer = Arc::new(Mutex::new(BufWriter::new(writer)));

        Self {
            user_id,
            addr,
            last_heartbeat: Instant::now(),
            writer,
            connected_at: Instant::now(),
        }
    }

    /// 메시지 전송
    ///
    /// 이 사용자에게 게임 메시지를 전송합니다.
    /// 네트워크 오류 발생 시 TcpServerError로 래핑하여 반환합니다.
    ///
    /// # Arguments
    ///
    /// * `message` - 전송할 게임 메시지
    ///
    /// # Returns
    ///
    /// * `Result<()>` - 성공 시 Ok(()), 실패 시 에러
    ///
    /// # Errors
    ///
    /// * 네트워크 연결 문제
    /// * 메시지 직렬화 실패
    pub async fn send_message(&self, message: &GameMessage) -> Result<()> {
        let mut writer = self.writer.lock().await;
        message.write_to_stream(&mut writer).await.map_err(|e| {
            TcpServerError::network_error(Some(self.addr.clone()), "send_message", &e.to_string())
        })?;

        debug!("사용자 {}에게 메시지 전송: {:?}", self.user_id, message);
        Ok(())
    }

    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
        debug!("사용자 {} 하트비트 업데이트", self.user_id);
    }

    pub fn is_heartbeat_timeout(&self) -> bool {
        self.last_heartbeat.elapsed() > Duration::from_secs(1800) // 30분 타임아웃
    }
}

/// 연결 서비스
pub struct ConnectionService {
    connections: Arc<Mutex<HashMap<u32, Arc<Mutex<UserConnection>>>>>,
    next_user_id: Arc<Mutex<u32>>,
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
            next_user_id: Arc::new(Mutex::new(1)),
            broadcast_tx,
            max_connections,
            server_start_time: Instant::now(),
            connection_stats: Arc::new(Mutex::new(ConnectionStats::default())),
        }
    }

    /// 새로운 연결 처리
    ///
    /// 새로운 클라이언트 연결을 받아들이고 고유한 사용자 ID를 할당합니다.
    /// 최대 연결 수를 확인하고 연결을 등록합니다.
    ///
    /// # Arguments
    ///
    /// * `stream` - 클라이언트 TCP 스트림
    /// * `addr` - 클라이언트 주소 문자열
    ///
    /// # Returns
    ///
    /// * `Result<u32>` - 성공 시 할당된 사용자 ID, 실패 시 에러
    ///
    /// # Errors
    ///
    /// * 최대 연결 수 초과
    /// * 사용자 ID 할당 실패
    /// * 연결 등록 실패
    ///
    /// # Examples
    ///
    /// ```rust
    /// let user_id = service.handle_new_connection(stream, "127.0.0.1:1234".to_string()).await?;
    /// println!("새 사용자 {} 연결됨", user_id);
    /// ```
    pub async fn handle_new_connection(&self, stream: TcpStream, addr: String) -> Result<u32> {
        // 최대 연결 수 확인
        let current_count = self.get_connection_count().await;
        if current_count >= self.max_connections as usize {
            warn!(
                "최대 연결 수 초과: {}/{}",
                current_count, self.max_connections
            );
            return Err(anyhow!("서버가 가득 참"));
        }

        // 사용자 ID 할당
        let mut next_id = self.next_user_id.lock().await;
        let user_id = *next_id;
        *next_id += 1;
        drop(next_id);

        debug!("사용자 연결 요청: {}", addr);

        // 연결 생성 및 저장
        let (reader, writer) = stream.into_split();
        let connection = Arc::new(Mutex::new(UserConnection {
            user_id,
            addr: addr.clone(),
            last_heartbeat: Instant::now(),
            writer: Arc::new(Mutex::new(BufWriter::new(writer))),
            connected_at: Instant::now(),
        }));

        {
            let mut connections = self.connections.lock().await;
            connections.insert(user_id, connection.clone());
        }

        // 통계 업데이트
        self.update_connection_stats(|stats| {
            stats.total_connections += 1;
            stats.current_connections += 1;
            stats.peak_connections = stats.peak_connections.max(stats.current_connections);
        })
        .await;

        // 연결 확인 메시지 전송
        let ack_message = GameMessage::ConnectionAck { user_id };
        if let Err(e) = connection.lock().await.send_message(&ack_message).await {
            let tcp_error = TcpServerError::connection_error(
                Some(user_id),
                Some(addr.clone()),
                &format!("연결 확인 메시지 전송 실패: {}", e),
            );
            ErrorHandler::handle_error(
                tcp_error.clone(),
                ErrorSeverity::Error,
                "ConnectionService",
                "send_ack_message",
            );
            self.remove_connection(user_id).await;
            return Err(anyhow::anyhow!(tcp_error));
        }

        // 메시지 수신 처리 시작
        self.start_message_handling(user_id, connection.clone(), reader)
            .await;

        info!("✅ 사용자 {} 연결 완료 ({})", user_id, addr);
        Ok(user_id)
    }

    /// 특정 사용자 ID로 새로운 연결 처리
    ///
    /// 클라이언트가 제공한 user_id를 사용하여 연결을 등록합니다.
    ///
    /// # Arguments
    ///
    /// * `stream` - 클라이언트 TCP 스트림
    /// * `addr` - 클라이언트 주소 문자열
    /// * `user_id` - 클라이언트가 제공한 사용자 ID
    ///
    /// # Returns
    ///
    /// * `Result<u32>` - 성공 시 사용자 ID, 실패 시 에러
    pub async fn handle_new_connection_with_id(
        &self,
        stream: TcpStream,
        addr: String,
        user_id: u32,
    ) -> Result<u32> {
        // 최대 연결 수 확인
        let current_count = self.get_connection_count().await;
        if current_count >= self.max_connections as usize {
            warn!(
                "최대 연결 수 초과: {}/{}",
                current_count, self.max_connections
            );
            return Err(anyhow!("서버가 가득 참"));
        }

        debug!("사용자 {} 연결 요청: {}", user_id, addr);

        // 기존 연결이 있으면 제거
        if self.connections.lock().await.contains_key(&user_id) {
            warn!("사용자 {}의 기존 연결을 제거합니다", user_id);
            self.remove_connection(user_id).await;
        }

        // 연결 생성 및 저장
        let (reader, writer) = stream.into_split();
        let connection = Arc::new(Mutex::new(UserConnection {
            user_id,
            addr: addr.clone(),
            last_heartbeat: Instant::now(),
            writer: Arc::new(Mutex::new(BufWriter::new(writer))),
            connected_at: Instant::now(),
        }));

        {
            let mut connections = self.connections.lock().await;
            connections.insert(user_id, connection.clone());
        }

        // 통계 업데이트
        self.update_connection_stats(|stats| {
            stats.total_connections += 1;
            stats.current_connections += 1;
            stats.peak_connections = stats.peak_connections.max(stats.current_connections);
        })
        .await;

        // 메시지 수신 처리 시작
        self.start_message_handling(user_id, connection.clone(), reader)
            .await;

        info!("✅ 사용자 {} 연결 완료 ({})", user_id, addr);
        Ok(user_id)
    }

    /// 메시지 수신 처리 시작
    async fn start_message_handling(
        &self,
        user_id: u32,
        _connection: Arc<Mutex<UserConnection>>,
        reader: OwnedReadHalf,
    ) {
        let connections_ref = self.connections.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        let stats_ref = self.connection_stats.clone();

        tokio::spawn(async move {
            let mut reader = BufReader::new(reader);

            loop {
                match GameMessage::read_from_stream(&mut reader).await {
                    Ok(message) => {
                        debug!("사용자 {}에서 메시지 수신: {:?}", user_id, message);

                        // 하트비트 처리
                        if matches!(message, GameMessage::HeartBeat) {
                            if let Some(conn) = connections_ref.lock().await.get(&user_id) {
                                conn.lock().await.update_heartbeat();

                                let response = GameMessage::HeartBeatResponse {
                                    timestamp: chrono::Utc::now().timestamp(),
                                };

                                if let Err(e) = conn.lock().await.send_message(&response).await {
                                    let tcp_error = TcpServerError::heartbeat_error(
                                        Some(user_id),
                                        "send_response",
                                        &e.to_string(),
                                    );
                                    ErrorHandler::handle_error(
                                        tcp_error,
                                        ErrorSeverity::Warning,
                                        "ConnectionService",
                                        "heartbeat_response",
                                    );
                                    break;
                                }
                            }
                        }

                        // 메시지 통계 업데이트
                        if let Ok(mut stats) = stats_ref.try_lock() {
                            stats.total_messages += 1;
                        }

                        // 다른 메시지들은 브로드캐스트 채널로 전송
                        if let Err(e) = broadcast_tx.send((Some(user_id), message)) {
                            warn!("브로드캐스트 전송 실패: {}", e);
                        }
                    }
                    Err(e) => {
                        info!("사용자 {} 연결 종료: {}", user_id, e);
                        break;
                    }
                }
            }

            // 연결 정리
            connections_ref.lock().await.remove(&user_id);

            // 통계 업데이트
            if let Ok(mut stats) = stats_ref.try_lock() {
                stats.current_connections = stats.current_connections.saturating_sub(1);
            }

            info!("사용자 {} 연결 해제 완료", user_id);
        });
    }

    /// 연결 제거
    pub async fn remove_connection(&self, user_id: u32) -> bool {
        let mut connections = self.connections.lock().await;
        let removed = connections.remove(&user_id).is_some();

        if removed {
            self.update_connection_stats(|stats| {
                stats.current_connections = stats.current_connections.saturating_sub(1);
            })
            .await;

            debug!("사용자 {} 연결 제거됨", user_id);
        }

        removed
    }

    /// 특정 사용자에게 메시지 전송
    pub async fn send_to_user(&self, user_id: u32, message: &GameMessage) -> Result<()> {
        let connections = self.connections.lock().await;

        if let Some(connection) = connections.get(&user_id) {
            connection.lock().await.send_message(message).await?;

            self.update_connection_stats(|stats| {
                stats.total_messages += 1;
            })
            .await;

            Ok(())
        } else {
            Err(anyhow!("사용자 {}를 찾을 수 없습니다", user_id))
        }
    }

    /// 모든 사용자에게 브로드캐스트
    pub async fn broadcast_message(&self, message: &GameMessage) -> Result<usize> {
        let connections = self.connections.lock().await;
        let mut success_count = 0;

        for (user_id, connection) in connections.iter() {
            if let Ok(()) = connection.lock().await.send_message(message).await {
                success_count += 1;
            } else {
                warn!("사용자 {}에게 브로드캐스트 실패", user_id);
            }
        }

        self.update_connection_stats(|stats| {
            stats.total_messages += success_count as u64;
        })
        .await;

        debug!(
            "브로드캐스트 완료: {}/{} 성공",
            success_count,
            connections.len()
        );
        Ok(success_count)
    }

    /// 타임아웃된 연결 정리
    pub async fn cleanup_timeout_connections(&self) -> usize {
        let mut connections = self.connections.lock().await;
        let mut timeout_users = Vec::new();

        for (user_id, connection) in connections.iter() {
            if connection.lock().await.is_heartbeat_timeout() {
                timeout_users.push(*user_id);
            }
        }

        for user_id in &timeout_users {
            connections.remove(user_id);
            warn!("사용자 {} 하트비트 타임아웃으로 연결 해제", user_id);
        }

        if !timeout_users.is_empty() {
            self.update_connection_stats(|stats| {
                stats.timeout_disconnections += timeout_users.len() as u64;
                stats.current_connections = stats
                    .current_connections
                    .saturating_sub(timeout_users.len() as u32);
            })
            .await;
        }

        timeout_users.len()
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
        })
        .await;

        info!("모든 사용자 연결 해제: {}개", count);
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
        F: FnOnce(&mut ConnectionStats),
    {
        if let Ok(mut stats) = self.connection_stats.try_lock() {
            update_fn(&mut stats);
        }
    }

    /// 연결 통계 조회
    pub async fn get_connection_stats(&self) -> ConnectionStats {
        self.connection_stats.lock().await.clone()
    }

    /// 사용자 연결 정보 조회
    pub async fn get_user_info(&self, user_id: u32) -> Option<UserInfo> {
        let connections = self.connections.lock().await;

        if let Some(connection) = connections.get(&user_id) {
            let conn = connection.lock().await;
            Some(UserInfo {
                user_id: conn.user_id,
                addr: conn.addr.clone(),
                connected_at: conn.connected_at,
                last_heartbeat: conn.last_heartbeat,
                uptime_seconds: conn.connected_at.elapsed().as_secs(),
                connected_timestamp: SimpleUtils::instant_to_timestamp(conn.connected_at),
                last_heartbeat_timestamp: SimpleUtils::instant_to_timestamp(conn.last_heartbeat),
            })
        } else {
            None
        }
    }

    /// 모든 사용자 목록 조회
    pub async fn get_all_users(&self) -> Vec<UserInfo> {
        let connections = self.connections.lock().await;
        let mut users = Vec::new();

        for connection in connections.values() {
            let conn = connection.lock().await;
            users.push(UserInfo {
                user_id: conn.user_id,
                addr: conn.addr.clone(),
                connected_at: conn.connected_at,
                last_heartbeat: conn.last_heartbeat,
                uptime_seconds: conn.connected_at.elapsed().as_secs(),
                connected_timestamp: SimpleUtils::instant_to_timestamp(conn.connected_at),
                last_heartbeat_timestamp: SimpleUtils::instant_to_timestamp(conn.last_heartbeat),
            });
        }

        users.sort_by_key(|u| u.user_id);
        users
    }
}

/// 사용자 정보
#[derive(Debug, Clone, serde::Serialize)]
pub struct UserInfo {
    pub user_id: u32,
    pub addr: String,
    #[serde(skip)]
    pub connected_at: Instant,
    #[serde(skip)]
    pub last_heartbeat: Instant,
    pub uptime_seconds: u64,
    /// 연결 시간 (Unix timestamp)
    pub connected_timestamp: i64,
    /// 마지막 하트비트 시간 (Unix timestamp)
    pub last_heartbeat_timestamp: i64,
}

mod tests {

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
