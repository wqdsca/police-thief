//! TCP 서버 방 강퇴 기능 예제
//! 
//! 이 예제는 TCP 서버를 사용하여 방 관리 및 사용자 강퇴 기능을 구현합니다.
//! - 방장 권한 확인
//! - 사용자 강퇴 처리
//! - 실시간 알림 시스템
//! - Redis 데이터 정리

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn, error};

/// 방 강퇴 메시지 (클라이언트 → 서버)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KickUserRequest {
    pub room_id: u32,
    pub kicker_id: u32,  // 강퇴를 요청하는 사용자 (방장)
    pub target_id: u32,  // 강퇴당할 사용자
    pub reason: String,   // 강퇴 사유
}

/// 방 강퇴 응답 (서버 → 클라이언트)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KickUserResponse {
    pub success: bool,
    pub room_id: u32,
    pub target_id: u32,
    pub reason: Option<String>,
    pub error: Option<String>,
}

/// 방 강퇴 알림 (서버 → 다른 사용자들)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserKickedNotification {
    pub room_id: u32,
    pub kicked_user_id: u32,
    pub kicker_id: u32,
    pub reason: String,
    pub remaining_users: u32,
    pub timestamp: i64,
}

/// 방 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub room_id: u32,
    pub room_name: String,
    pub owner_id: u32,      // 방장 ID
    pub users: Vec<u32>,    // 방 참가자 목록
    pub max_users: u32,
    pub created_at: i64,
}

/// 사용자 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub user_id: u32,
    pub nickname: String,
    pub current_room: Option<u32>,
    pub is_online: bool,
    pub last_activity: i64,
}

/// TCP 메시지 타입 (기존 프로토콜 확장)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TcpGameMessage {
    // 기존 메시지들...
    HeartBeat,
    Connect { room_id: u32, user_id: u32 },
    ConnectionAck { user_id: u32 },
    
    // 방 강퇴 관련 새로운 메시지들
    KickUser(KickUserRequest),
    KickUserResponse(KickUserResponse),
    UserKicked(UserKickedNotification),
    
    // 에러 메시지
    Error { code: u16, message: String },
}

/// 방 관리자
pub struct RoomManager {
    rooms: Arc<RwLock<HashMap<u32, RoomInfo>>>,
    users: Arc<RwLock<HashMap<u32, UserInfo>>>,
    redis_client: Option<redis::Client>,
}

impl RoomManager {
    pub fn new(redis_url: Option<&str>) -> Result<Self> {
        let redis_client = if let Some(url) = redis_url {
            Some(redis::Client::open(url)?)
        } else {
            None
        };

        Ok(Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            users: Arc::new(RwLock::new(HashMap::new())),
            redis_client,
        })
    }

    /// 방장 권한 확인
    pub async fn is_room_owner(&self, room_id: u32, user_id: u32) -> Result<bool> {
        let rooms = self.rooms.read().await;
        
        if let Some(room) = rooms.get(&room_id) {
            Ok(room.owner_id == user_id)
        } else {
            Err(anyhow!("방이 존재하지 않습니다: {}", room_id))
        }
    }

    /// 사용자가 방에 있는지 확인
    pub async fn is_user_in_room(&self, room_id: u32, user_id: u32) -> Result<bool> {
        let rooms = self.rooms.read().await;
        
        if let Some(room) = rooms.get(&room_id) {
            Ok(room.users.contains(&user_id))
        } else {
            Err(anyhow!("방이 존재하지 않습니다: {}", room_id))
        }
    }

    /// 사용자를 방에서 제거
    pub async fn remove_user_from_room(&self, room_id: u32, user_id: u32) -> Result<()> {
        // 1. 메모리에서 제거
        {
            let mut rooms = self.rooms.write().await;
            if let Some(room) = rooms.get_mut(&room_id) {
                room.users.retain(|&id| id != user_id);
                
                // 방이 비어있으면 삭제
                if room.users.is_empty() {
                    rooms.remove(&room_id);
                    info!("방 {} 삭제됨 (사용자 없음)", room_id);
                }
            }
        }

        // 2. 사용자의 현재 방 정보 업데이트
        {
            let mut users = self.users.write().await;
            if let Some(user) = users.get_mut(&user_id) {
                user.current_room = None;
            }
        }

        // 3. Redis에서 제거
        if let Some(ref redis_client) = self.redis_client {
            self.remove_from_redis(room_id, user_id, redis_client).await?;
        }

        Ok(())
    }

    /// Redis에서 사용자 제거
    async fn remove_from_redis(
        &self,
        room_id: u32,
        user_id: u32,
        redis_client: &redis::Client,
    ) -> Result<()> {
        use redis::AsyncCommands;
        
        let mut conn = redis_client.get_async_connection().await?;
        
        // Redis 파이프라인으로 원자적 처리
        let mut pipe = redis::pipe();
        
        // 방 사용자 목록에서 제거
        pipe.srem(format!("room:{}:users", room_id), user_id);
        
        // 사용자의 현재 방 정보 제거
        pipe.hdel(format!("user:{}", user_id), "current_room");
        
        // 방 사용자 수 업데이트
        pipe.scard(format!("room:{}:users", room_id));
        
        let results: Vec<redis::Value> = pipe.query_async(&mut conn).await?;
        
        // 방이 비어있으면 방 정보도 삭제
        if let Some(redis::Value::Int(user_count)) = results.get(2) {
            if *user_count == 0 {
                pipe.del(format!("room:{}", room_id));
                pipe.del(format!("room:{}:users", room_id));
                pipe.zrem("room:list:time:index", room_id.to_string());
                
                let _: Vec<redis::Value> = pipe.query_async(&mut conn).await?;
                info!("Redis에서 빈 방 {} 정보 삭제", room_id);
            }
        }

        Ok(())
    }

    /// 강퇴 처리 (핵심 비즈니스 로직)
    pub async fn kick_user(&self, request: KickUserRequest) -> Result<KickUserResponse> {
        let KickUserRequest { room_id, kicker_id, target_id, reason } = request;

        // 1. 방장 권한 확인
        if !self.is_room_owner(room_id, kicker_id).await? {
            return Ok(KickUserResponse {
                success: false,
                room_id,
                target_id,
                reason: None,
                error: Some("방장만 사용자를 강퇴할 수 있습니다".to_string()),
            });
        }

        // 2. 자기 자신 강퇴 방지
        if kicker_id == target_id {
            return Ok(KickUserResponse {
                success: false,
                room_id,
                target_id,
                reason: None,
                error: Some("자기 자신을 강퇴할 수 없습니다".to_string()),
            });
        }

        // 3. 대상 사용자가 방에 있는지 확인
        if !self.is_user_in_room(room_id, target_id).await? {
            return Ok(KickUserResponse {
                success: false,
                room_id,
                target_id,
                reason: None,
                error: Some("해당 사용자가 방에 없습니다".to_string()),
            });
        }

        // 4. 강퇴 실행
        match self.remove_user_from_room(room_id, target_id).await {
            Ok(_) => {
                info!(
                    "사용자 {} 강퇴 성공: 방 {}, 요청자 {}, 사유: {}",
                    target_id, room_id, kicker_id, reason
                );

                Ok(KickUserResponse {
                    success: true,
                    room_id,
                    target_id,
                    reason: Some(reason),
                    error: None,
                })
            }
            Err(e) => {
                error!("강퇴 처리 실패: {}", e);
                Ok(KickUserResponse {
                    success: false,
                    room_id,
                    target_id,
                    reason: None,
                    error: Some(format!("강퇴 처리 실패: {}", e)),
                })
            }
        }
    }

    /// 방의 현재 사용자 수 반환
    pub async fn get_room_user_count(&self, room_id: u32) -> Result<u32> {
        let rooms = self.rooms.read().await;
        
        if let Some(room) = rooms.get(&room_id) {
            Ok(room.users.len() as u32)
        } else {
            Ok(0)
        }
    }
}

/// 메시지 브로드캐스터
pub struct MessageBroadcaster {
    connections: Arc<RwLock<HashMap<u32, mpsc::UnboundedSender<TcpGameMessage>>>>,
}

impl MessageBroadcaster {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 연결 등록
    pub async fn register_connection(&self, user_id: u32, sender: mpsc::UnboundedSender<TcpGameMessage>) {
        let mut connections = self.connections.write().await;
        connections.insert(user_id, sender);
        info!("사용자 {} 연결 등록", user_id);
    }

    /// 연결 해제
    pub async fn unregister_connection(&self, user_id: u32) {
        let mut connections = self.connections.write().await;
        connections.remove(&user_id);
        info!("사용자 {} 연결 해제", user_id);
    }

    /// 특정 사용자에게 메시지 전송
    pub async fn send_to_user(&self, user_id: u32, message: TcpGameMessage) -> Result<()> {
        let connections = self.connections.read().await;
        
        if let Some(sender) = connections.get(&user_id) {
            sender.send(message)
                .map_err(|_| anyhow!("메시지 전송 실패: 사용자 {}", user_id))?;
        }

        Ok(())
    }

    /// 방의 모든 사용자에게 브로드캐스트 (특정 사용자 제외)
    pub async fn broadcast_to_room(
        &self,
        room_manager: &RoomManager,
        room_id: u32,
        message: TcpGameMessage,
        exclude_user: Option<u32>,
    ) -> Result<()> {
        let rooms = room_manager.rooms.read().await;
        
        if let Some(room) = rooms.get(&room_id) {
            let connections = self.connections.read().await;
            let mut sent_count = 0;
            
            for &user_id in &room.users {
                if let Some(exclude) = exclude_user {
                    if user_id == exclude {
                        continue;
                    }
                }
                
                if let Some(sender) = connections.get(&user_id) {
                    if let Err(e) = sender.send(message.clone()) {
                        warn!("사용자 {}에게 메시지 전송 실패: {}", user_id, e);
                    } else {
                        sent_count += 1;
                    }
                }
            }
            
            info!("방 {}에 메시지 브로드캐스트: {} 명", room_id, sent_count);
        }

        Ok(())
    }
}

/// TCP 서버 (강퇴 기능 포함)
pub struct TcpRoomServer {
    room_manager: Arc<RoomManager>,
    broadcaster: Arc<MessageBroadcaster>,
    listener: TcpListener,
}

impl TcpRoomServer {
    pub async fn new(addr: &str, redis_url: Option<&str>) -> Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let room_manager = Arc::new(RoomManager::new(redis_url)?);
        let broadcaster = Arc::new(MessageBroadcaster::new());

        info!("TCP 방 서버 시작: {}", addr);

        Ok(Self {
            room_manager,
            broadcaster,
            listener,
        })
    }

    /// 서버 실행
    pub async fn run(&self) -> Result<()> {
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    info!("새 연결: {}", addr);
                    
                    let room_manager = self.room_manager.clone();
                    let broadcaster = self.broadcaster.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, room_manager, broadcaster).await {
                            error!("연결 처리 실패 ({}): {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("연결 수락 실패: {}", e);
                }
            }
        }
    }

    /// 개별 연결 처리
    async fn handle_connection(
        stream: TcpStream,
        room_manager: Arc<RoomManager>,
        broadcaster: Arc<MessageBroadcaster>,
    ) -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
        
        let (reader, writer) = stream.into_split();
        let mut buf_reader = BufReader::new(reader);
        let mut buf_writer = BufWriter::new(writer);
        
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut user_id: Option<u32> = None;

        loop {
            tokio::select! {
                // 클라이언트로부터 메시지 수신
                result = Self::read_message(&mut buf_reader) => {
                    match result {
                        Ok(message) => {
                            if let Err(e) = Self::process_message(
                                message,
                                &room_manager,
                                &broadcaster,
                                &tx,
                                &mut user_id,
                            ).await {
                                error!("메시지 처리 실패: {}", e);
                            }
                        }
                        Err(e) => {
                            warn!("메시지 읽기 실패: {}", e);
                            break;
                        }
                    }
                }
                
                // 클라이언트로 메시지 전송
                Some(message) = rx.recv() => {
                    if let Err(e) = Self::send_message(&mut buf_writer, &message).await {
                        error!("메시지 전송 실패: {}", e);
                        break;
                    }
                }
            }
        }

        // 연결 정리
        if let Some(id) = user_id {
            broadcaster.unregister_connection(id).await;
            info!("사용자 {} 연결 종료", id);
        }

        Ok(())
    }

    /// 메시지 처리 (강퇴 기능 포함)
    async fn process_message(
        message: TcpGameMessage,
        room_manager: &Arc<RoomManager>,
        broadcaster: &Arc<MessageBroadcaster>,
        tx: &mpsc::UnboundedSender<TcpGameMessage>,
        user_id: &mut Option<u32>,
    ) -> Result<()> {
        match message {
            TcpGameMessage::Connect { room_id: _, user_id: id } => {
                *user_id = Some(id);
                broadcaster.register_connection(id, tx.clone()).await;
                
                let response = TcpGameMessage::ConnectionAck { user_id: id };
                tx.send(response)?;
            }

            TcpGameMessage::KickUser(request) => {
                info!(
                    "강퇴 요청: 방 {}, 요청자 {}, 대상 {}, 사유: {}",
                    request.room_id, request.kicker_id, request.target_id, request.reason
                );

                // 강퇴 처리
                let response = room_manager.kick_user(request.clone()).await?;
                
                // 요청자에게 응답 전송
                tx.send(TcpGameMessage::KickUserResponse(response.clone()))?;

                // 강퇴 성공시 알림 브로드캐스트
                if response.success {
                    let remaining_users = room_manager
                        .get_room_user_count(request.room_id)
                        .await
                        .unwrap_or(0);

                    let notification = UserKickedNotification {
                        room_id: request.room_id,
                        kicked_user_id: request.target_id,
                        kicker_id: request.kicker_id,
                        reason: request.reason,
                        remaining_users,
                        timestamp: chrono::Utc::now().timestamp(),
                    };

                    // 강퇴당한 사용자에게 직접 알림
                    broadcaster
                        .send_to_user(
                            request.target_id,
                            TcpGameMessage::UserKicked(notification.clone()),
                        )
                        .await?;

                    // 방의 다른 사용자들에게 브로드캐스트 (강퇴당한 사용자 제외)
                    broadcaster
                        .broadcast_to_room(
                            room_manager,
                            request.room_id,
                            TcpGameMessage::UserKicked(notification),
                            Some(request.target_id),
                        )
                        .await?;
                }
            }

            TcpGameMessage::HeartBeat => {
                // 하트비트 응답은 기존 로직과 동일
                info!("하트비트 수신");
            }

            _ => {
                info!("기타 메시지 처리: {:?}", message);
            }
        }

        Ok(())
    }

    /// 메시지 읽기 (4바이트 헤더 + JSON)
    async fn read_message(reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) -> Result<TcpGameMessage> {
        use tokio::io::AsyncReadExt;
        
        // 4바이트 길이 헤더 읽기
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes).await?;
        let len = u32::from_be_bytes(len_bytes) as usize;

        // 메시지 데이터 읽기
        let mut buffer = vec![0u8; len];
        reader.read_exact(&mut buffer).await?;

        // JSON 역직렬화
        let json_str = std::str::from_utf8(&buffer)?;
        let message: TcpGameMessage = serde_json::from_str(json_str)?;

        Ok(message)
    }

    /// 메시지 전송 (4바이트 헤더 + JSON)
    async fn send_message(
        writer: &mut BufWriter<tokio::net::tcp::OwnedWriteHalf>,
        message: &TcpGameMessage,
    ) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        
        // JSON 직렬화
        let json = serde_json::to_string(message)?;
        let data = json.as_bytes();
        let len = data.len() as u32;

        // 4바이트 길이 헤더 전송
        writer.write_all(&len.to_be_bytes()).await?;
        
        // 데이터 전송
        writer.write_all(data).await?;
        writer.flush().await?;

        Ok(())
    }
}

/// 사용 예시 및 테스트 함수
pub async fn tcp_room_kick_example() -> Result<()> {
    // 1. TCP 서버 시작
    let server = TcpRoomServer::new("127.0.0.1:4000", Some("redis://127.0.0.1:6379")).await?;
    
    info!("TCP 방 강퇴 서버 시작!");
    info!("사용 방법:");
    info!("1. 클라이언트에서 Connect 메시지로 방에 입장");
    info!("2. 방장이 KickUser 메시지로 사용자 강퇴");
    info!("3. 실시간으로 강퇴 알림 브로드캐스트");
    
    // 서버 실행 (무한 루프)
    server.run().await?;
    
    Ok(())
}

/// 클라이언트 예제 (테스트용)
pub async fn tcp_kick_client_example() -> Result<()> {
    use tokio::net::TcpStream;
    use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};

    // 서버에 연결
    let stream = TcpStream::connect("127.0.0.1:4000").await?;
    let (reader, writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut buf_writer = BufWriter::new(writer);

    // 1. 방에 연결 (방장으로)
    let connect_msg = TcpGameMessage::Connect {
        room_id: 1,
        user_id: 100, // 방장 ID
    };
    
    TcpRoomServer::send_message(&mut buf_writer, &connect_msg).await?;
    let response = TcpRoomServer::read_message(&mut buf_reader).await?;
    info!("연결 응답: {:?}", response);

    // 2. 사용자 강퇴 요청
    let kick_msg = TcpGameMessage::KickUser(KickUserRequest {
        room_id: 1,
        kicker_id: 100,  // 방장
        target_id: 200,  // 강퇴할 사용자
        reason: "규칙 위반".to_string(),
    });
    
    TcpRoomServer::send_message(&mut buf_writer, &kick_msg).await?;
    let kick_response = TcpRoomServer::read_message(&mut buf_reader).await?;
    info!("강퇴 응답: {:?}", kick_response);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_room_manager_kick_functionality() {
        let manager = RoomManager::new(None).unwrap();

        // 테스트 방과 사용자 설정
        {
            let mut rooms = manager.rooms.write().await;
            rooms.insert(1, RoomInfo {
                room_id: 1,
                room_name: "테스트방".to_string(),
                owner_id: 100,
                users: vec![100, 200, 300],
                max_users: 10,
                created_at: chrono::Utc::now().timestamp(),
            });
        }

        // 방장 권한 확인
        assert!(manager.is_room_owner(1, 100).await.unwrap());
        assert!(!manager.is_room_owner(1, 200).await.unwrap());

        // 강퇴 요청 처리
        let kick_request = KickUserRequest {
            room_id: 1,
            kicker_id: 100,
            target_id: 200,
            reason: "테스트".to_string(),
        };

        let response = manager.kick_user(kick_request).await.unwrap();
        assert!(response.success);
        assert_eq!(response.target_id, 200);

        // 사용자가 방에서 제거되었는지 확인
        assert!(!manager.is_user_in_room(1, 200).await.unwrap());
    }
}