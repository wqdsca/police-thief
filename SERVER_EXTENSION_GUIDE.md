# 🚀 Police Thief 게임 서버 기능 확장 가이드

각 서버별 아키텍처와 기능 확장 방법을 실제 예시와 함께 설명합니다.

---

## 📡 1. gRPC 서버 - CRUD 게시판 추가

### 현재 아키텍처
```
proto 정의 → tonic 코드 생성 → Service 구현 → Controller 연결
```

### 🔧 Step 1: Proto 파일 정의
```protobuf
// grpcserver/proto/board.proto
syntax = "proto3";

package board;

// 게시판 서비스 정의
service BoardService {
  // 게시글 CRUD
  rpc CreatePost (CreatePostRequest) returns (CreatePostResponse);
  rpc GetPost (GetPostRequest) returns (GetPostResponse);
  rpc UpdatePost (UpdatePostRequest) returns (UpdatePostResponse);
  rpc DeletePost (DeletePostRequest) returns (DeletePostResponse);
  rpc ListPosts (ListPostsRequest) returns (ListPostsResponse);
  
  // 댓글 기능
  rpc AddComment (AddCommentRequest) returns (AddCommentResponse);
  rpc GetComments (GetCommentsRequest) returns (GetCommentsResponse);
}

// 게시글 모델
message Post {
  int64 id = 1;
  int32 author_id = 2;
  string title = 3;
  string content = 4;
  string category = 5;
  int64 created_at = 6;
  int64 updated_at = 7;
  int32 view_count = 8;
  int32 like_count = 9;
}

// 댓글 모델
message Comment {
  int64 id = 1;
  int64 post_id = 2;
  int32 author_id = 3;
  string content = 4;
  int64 created_at = 5;
}

// 요청/응답 메시지들
message CreatePostRequest {
  string title = 1;
  string content = 2;
  string category = 3;
  int32 author_id = 4;
}

message CreatePostResponse {
  bool success = 1;
  int64 post_id = 2;
  string message = 3;
}

message GetPostRequest {
  int64 post_id = 1;
}

message GetPostResponse {
  Post post = 1;
}

message ListPostsRequest {
  int32 page = 1;
  int32 page_size = 2;
  string category = 3;
  string search_keyword = 4;
}

message ListPostsResponse {
  repeated Post posts = 1;
  int32 total_count = 2;
  int32 current_page = 3;
}
```

### 🔧 Step 2: Service 구현
```rust
// grpcserver/src/service/board_service.rs
use crate::proto::board::*;
use shared::service::db::BoardDatabaseService;
use shared::service::redis::BoardRedisService;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct BoardServiceImpl {
    db: Arc<BoardDatabaseService>,
    redis: Arc<BoardRedisService>,
}

impl BoardServiceImpl {
    pub fn new(pool: sqlx::MySqlPool, redis: redis::aio::ConnectionManager) -> Self {
        Self {
            db: Arc::new(BoardDatabaseService::new(pool)),
            redis: Arc::new(BoardRedisService::new(redis)),
        }
    }

    /// 게시글 생성
    async fn create_post_internal(
        &self,
        req: CreatePostRequest,
    ) -> Result<CreatePostResponse, Status> {
        // 입력 검증
        if req.title.is_empty() || req.content.is_empty() {
            return Err(Status::invalid_argument("Title and content required"));
        }

        // DB 저장
        let post_id = self.db
            .create_post(&req.title, &req.content, &req.category, req.author_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Redis 캐시 무효화
        self.redis.invalidate_post_list_cache().await;

        Ok(CreatePostResponse {
            success: true,
            post_id,
            message: "Post created successfully".to_string(),
        })
    }

    /// 게시글 조회 (캐시 활용)
    async fn get_post_internal(&self, post_id: i64) -> Result<Post, Status> {
        // Redis 캐시 확인
        if let Some(cached) = self.redis.get_post(post_id).await {
            return Ok(cached);
        }

        // DB에서 조회
        let post = self.db
            .get_post(post_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Post not found"))?;

        // 캐시 저장 (TTL: 1시간)
        self.redis.cache_post(&post, 3600).await;

        // 조회수 증가 (비동기)
        tokio::spawn({
            let db = self.db.clone();
            async move {
                let _ = db.increment_view_count(post_id).await;
            }
        });

        Ok(post)
    }

    /// 게시글 목록 (페이징)
    async fn list_posts_internal(
        &self,
        req: ListPostsRequest,
    ) -> Result<ListPostsResponse, Status> {
        let page = req.page.max(1) as usize;
        let page_size = req.page_size.clamp(1, 100) as usize;
        let offset = (page - 1) * page_size;

        // DB 조회
        let (posts, total) = self.db
            .list_posts(offset, page_size, req.category, req.search_keyword)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(ListPostsResponse {
            posts,
            total_count: total as i32,
            current_page: page as i32,
        })
    }
}

// gRPC 서비스 구현
#[tonic::async_trait]
impl board_service_server::BoardService for BoardServiceImpl {
    async fn create_post(
        &self,
        request: Request<CreatePostRequest>,
    ) -> Result<Response<CreatePostResponse>, Status> {
        let response = self.create_post_internal(request.into_inner()).await?;
        Ok(Response::new(response))
    }

    async fn get_post(
        &self,
        request: Request<GetPostRequest>,
    ) -> Result<Response<GetPostResponse>, Status> {
        let post = self.get_post_internal(request.into_inner().post_id).await?;
        Ok(Response::new(GetPostResponse { post: Some(post) }))
    }

    // ... 나머지 메서드들
}
```

### 🔧 Step 3: 서버에 등록
```rust
// grpcserver/src/main.rs
use proto::board::board_service_server::BoardServiceServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ... 기존 코드

    // 게시판 서비스 추가
    let board_service = BoardServiceImpl::new(pool.clone(), redis.clone());
    
    Server::builder()
        .add_service(UserServiceServer::new(user_service))
        .add_service(RoomServiceServer::new(room_service))
        .add_service(BoardServiceServer::new(board_service))  // 새 서비스 추가
        .serve(addr)
        .await?;

    Ok(())
}
```

---

## 🌐 2. TCP 서버 - 강퇴 기능 추가

### 현재 아키텍처
```
메시지 타입 정의 → 핸들러 구현 → 서비스 로직 → 브로드캐스트
```

### 🔧 Step 1: 프로토콜 메시지 추가
```rust
// tcpserver/src/protocol.rs

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum GameMessage {
    // ... 기존 메시지들
    
    // 강퇴 관련 메시지
    KickUser {
        room_id: u32,
        target_user_id: u32,
        reason: String,
    },
    KickUserResponse {
        success: bool,
        message: String,
    },
    UserKicked {
        room_id: u32,
        kicked_user_id: u32,
        kicked_by: u32,
        reason: String,
    },
    
    // 차단 관련
    BanUser {
        room_id: u32,
        target_user_id: u32,
        duration_minutes: u32,
        reason: String,
    },
    BanUserResponse {
        success: bool,
        message: String,
    },
    
    // 투표 강퇴
    VoteKick {
        room_id: u32,
        target_user_id: u32,
    },
    VoteKickStatus {
        room_id: u32,
        target_user_id: u32,
        votes_needed: u32,
        current_votes: u32,
    },
}
```

### 🔧 Step 2: 강퇴 서비스 구현
```rust
// tcpserver/src/service/kick_service.rs
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 강퇴 관리 서비스
pub struct KickService {
    /// 방별 권한 정보 (방장 ID)
    room_owners: Arc<DashMap<u32, u32>>,
    
    /// 차단된 사용자 (user_id -> 차단 종료 시간)
    banned_users: Arc<DashMap<u32, Instant>>,
    
    /// 투표 강퇴 상태
    vote_kicks: Arc<DashMap<(u32, u32), VoteKickState>>,
    
    /// 연결 서비스
    connection_service: Arc<ConnectionService>,
}

#[derive(Clone)]
struct VoteKickState {
    voters: Vec<u32>,
    target_user: u32,
    room_id: u32,
    started_at: Instant,
}

impl KickService {
    pub fn new(connection_service: Arc<ConnectionService>) -> Self {
        Self {
            room_owners: Arc::new(DashMap::new()),
            banned_users: Arc::new(DashMap::new()),
            vote_kicks: Arc::new(DashMap::new()),
            connection_service,
        }
    }

    /// 사용자 강퇴 (방장 권한)
    pub async fn kick_user(
        &self,
        requester_id: u32,
        room_id: u32,
        target_user_id: u32,
        reason: String,
    ) -> Result<()> {
        // 권한 확인
        let owner_id = self.room_owners
            .get(&room_id)
            .ok_or_else(|| anyhow!("Room not found"))?
            .value()
            .clone();

        if owner_id != requester_id {
            return Err(anyhow!("Only room owner can kick users"));
        }

        // 대상 사용자가 방에 있는지 확인
        if !self.is_user_in_room(room_id, target_user_id).await? {
            return Err(anyhow!("User not in room"));
        }

        // 강퇴 실행
        self.execute_kick(room_id, target_user_id, requester_id, reason).await?;

        Ok(())
    }

    /// 투표 강퇴 시작
    pub async fn start_vote_kick(
        &self,
        requester_id: u32,
        room_id: u32,
        target_user_id: u32,
    ) -> Result<()> {
        let key = (room_id, target_user_id);
        
        // 이미 진행 중인 투표가 있는지 확인
        if self.vote_kicks.contains_key(&key) {
            return Err(anyhow!("Vote kick already in progress"));
        }

        // 투표 상태 초기화
        let vote_state = VoteKickState {
            voters: vec![requester_id],
            target_user: target_user_id,
            room_id,
            started_at: Instant::now(),
        };

        self.vote_kicks.insert(key, vote_state);

        // 방 멤버에게 투표 상태 브로드캐스트
        self.broadcast_vote_status(room_id, target_user_id).await?;

        // 타임아웃 설정 (2분)
        tokio::spawn({
            let service = self.clone();
            async move {
                tokio::time::sleep(Duration::from_secs(120)).await;
                service.vote_kicks.remove(&key);
            }
        });

        Ok(())
    }

    /// 투표하기
    pub async fn vote_for_kick(
        &self,
        voter_id: u32,
        room_id: u32,
        target_user_id: u32,
    ) -> Result<()> {
        let key = (room_id, target_user_id);
        
        let mut vote_state = self.vote_kicks
            .get_mut(&key)
            .ok_or_else(|| anyhow!("No active vote kick"))?;

        // 중복 투표 방지
        if vote_state.voters.contains(&voter_id) {
            return Err(anyhow!("Already voted"));
        }

        vote_state.voters.push(voter_id);
        let vote_count = vote_state.voters.len();

        // 방 인원수 확인
        let room_members = self.get_room_member_count(room_id).await?;
        let required_votes = (room_members / 2) + 1; // 과반수

        if vote_count >= required_votes {
            // 강퇴 실행
            self.execute_kick(
                room_id,
                target_user_id,
                0, // 시스템 강퇴
                "Kicked by vote".to_string(),
            ).await?;

            // 투표 상태 제거
            self.vote_kicks.remove(&key);
        } else {
            // 투표 상태 업데이트 브로드캐스트
            self.broadcast_vote_status(room_id, target_user_id).await?;
        }

        Ok(())
    }

    /// 사용자 차단 (일정 시간)
    pub async fn ban_user(
        &self,
        requester_id: u32,
        room_id: u32,
        target_user_id: u32,
        duration_minutes: u32,
        reason: String,
    ) -> Result<()> {
        // 먼저 강퇴
        self.kick_user(requester_id, room_id, target_user_id, reason.clone()).await?;

        // 차단 목록에 추가
        let ban_until = Instant::now() + Duration::from_secs(duration_minutes as u64 * 60);
        self.banned_users.insert(target_user_id, ban_until);

        // Redis에도 저장 (영구 보관)
        self.save_ban_to_redis(target_user_id, ban_until).await?;

        Ok(())
    }

    /// 차단 확인
    pub async fn is_banned(&self, user_id: u32) -> bool {
        if let Some(ban_until) = self.banned_users.get(&user_id) {
            if Instant::now() < *ban_until {
                return true;
            } else {
                // 차단 시간 만료
                self.banned_users.remove(&user_id);
            }
        }
        false
    }

    /// 강퇴 실행 (내부)
    async fn execute_kick(
        &self,
        room_id: u32,
        target_user_id: u32,
        kicked_by: u32,
        reason: String,
    ) -> Result<()> {
        // 1. 연결 종료
        self.connection_service
            .disconnect_user(target_user_id)
            .await?;

        // 2. 방에서 제거
        self.remove_from_room(room_id, target_user_id).await?;

        // 3. 모든 멤버에게 알림
        let kick_message = GameMessage::UserKicked {
            room_id,
            kicked_user_id: target_user_id,
            kicked_by,
            reason,
        };

        self.broadcast_to_room(room_id, kick_message).await?;

        // 4. 로그 기록
        tracing::info!(
            "User {} kicked from room {} by {} - reason: {}",
            target_user_id,
            room_id,
            kicked_by,
            reason
        );

        Ok(())
    }

    // ... 헬퍼 메서드들
}
```

### 🔧 Step 3: 메시지 핸들러 추가
```rust
// tcpserver/src/handler/kick_handler.rs
use crate::protocol::GameMessage;
use crate::service::KickService;
use std::sync::Arc;

pub struct KickHandler {
    kick_service: Arc<KickService>,
}

impl KickHandler {
    pub fn new(kick_service: Arc<KickService>) -> Self {
        Self { kick_service }
    }

    pub async fn handle_message(
        &self,
        user_id: u32,
        message: GameMessage,
    ) -> Result<GameMessage> {
        match message {
            GameMessage::KickUser { room_id, target_user_id, reason } => {
                match self.kick_service
                    .kick_user(user_id, room_id, target_user_id, reason)
                    .await
                {
                    Ok(_) => Ok(GameMessage::KickUserResponse {
                        success: true,
                        message: "User kicked successfully".to_string(),
                    }),
                    Err(e) => Ok(GameMessage::KickUserResponse {
                        success: false,
                        message: e.to_string(),
                    }),
                }
            }
            
            GameMessage::BanUser { room_id, target_user_id, duration_minutes, reason } => {
                match self.kick_service
                    .ban_user(user_id, room_id, target_user_id, duration_minutes, reason)
                    .await
                {
                    Ok(_) => Ok(GameMessage::BanUserResponse {
                        success: true,
                        message: "User banned successfully".to_string(),
                    }),
                    Err(e) => Ok(GameMessage::BanUserResponse {
                        success: false,
                        message: e.to_string(),
                    }),
                }
            }
            
            GameMessage::VoteKick { room_id, target_user_id } => {
                self.kick_service
                    .vote_for_kick(user_id, room_id, target_user_id)
                    .await?;
                Ok(GameMessage::Empty)
            }
            
            _ => Ok(GameMessage::Empty),
        }
    }
}
```

---

## 🎤 3. RUDP 서버 - 음성 채팅 추가

### 현재 아키텍처
```
UDP 소켓 → 신뢰성 레이어 → 게임 로직 → 실시간 브로드캐스트
```

### 🔧 Step 1: 음성 프로토콜 정의
```rust
// rudpserver/src/voice/protocol.rs
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};

/// 음성 패킷 타입
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum VoicePacketType {
    AudioData = 0x01,      // 실제 음성 데이터
    StartTalking = 0x02,   // 말하기 시작
    StopTalking = 0x03,    // 말하기 종료
    JoinChannel = 0x04,    // 채널 참가
    LeaveChannel = 0x05,   // 채널 나가기
    Mute = 0x06,          // 음소거
    Unmute = 0x07,        // 음소거 해제
}

/// 음성 패킷 구조
/// [1바이트 타입][2바이트 시퀀스][4바이트 타임스탬프][가변 길이 데이터]
#[derive(Debug, Clone)]
pub struct VoicePacket {
    pub packet_type: VoicePacketType,
    pub sequence: u16,
    pub timestamp: u32,
    pub channel_id: u32,
    pub sender_id: u32,
    pub data: Bytes,
}

impl VoicePacket {
    /// 오디오 데이터 패킷 생성
    pub fn audio(
        channel_id: u32,
        sender_id: u32,
        sequence: u16,
        audio_data: Bytes,
    ) -> Self {
        Self {
            packet_type: VoicePacketType::AudioData,
            sequence,
            timestamp: Self::current_timestamp(),
            channel_id,
            sender_id,
            data: audio_data,
        }
    }

    /// 바이너리로 직렬화 (최소 오버헤드)
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(15 + self.data.len());
        
        buf.put_u8(self.packet_type as u8);
        buf.put_u16(self.sequence);
        buf.put_u32(self.timestamp);
        buf.put_u32(self.channel_id);
        buf.put_u32(self.sender_id);
        buf.put_u16(self.data.len() as u16);
        buf.extend_from_slice(&self.data);
        
        buf.freeze()
    }

    /// 바이너리에서 역직렬화
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 15 {
            return Err(anyhow!("Packet too small"));
        }

        let packet_type = match data[0] {
            0x01 => VoicePacketType::AudioData,
            0x02 => VoicePacketType::StartTalking,
            0x03 => VoicePacketType::StopTalking,
            0x04 => VoicePacketType::JoinChannel,
            0x05 => VoicePacketType::LeaveChannel,
            0x06 => VoicePacketType::Mute,
            0x07 => VoicePacketType::Unmute,
            _ => return Err(anyhow!("Invalid packet type")),
        };

        let sequence = u16::from_be_bytes([data[1], data[2]]);
        let timestamp = u32::from_be_bytes([data[3], data[4], data[5], data[6]]);
        let channel_id = u32::from_be_bytes([data[7], data[8], data[9], data[10]]);
        let sender_id = u32::from_be_bytes([data[11], data[12], data[13], data[14]]);
        
        let audio_data = if data.len() > 15 {
            Bytes::copy_from_slice(&data[15..])
        } else {
            Bytes::new()
        };

        Ok(Self {
            packet_type,
            sequence,
            timestamp,
            channel_id,
            sender_id,
            data: audio_data,
        })
    }

    fn current_timestamp() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u32
    }
}
```

### 🔧 Step 2: 음성 채널 서비스
```rust
// rudpserver/src/voice/channel_service.rs
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use bytes::Bytes;

/// 음성 채널 관리 서비스
pub struct VoiceChannelService {
    /// 채널별 참가자 목록
    channels: Arc<DashMap<u32, ChannelState>>,
    
    /// 사용자별 채널 정보
    user_channels: Arc<DashMap<u32, u32>>,
    
    /// 사용자별 음성 상태
    user_states: Arc<DashMap<u32, UserVoiceState>>,
    
    /// 오디오 버퍼 (지터 버퍼)
    audio_buffers: Arc<DashMap<u32, AudioBuffer>>,
    
    /// 네트워크 전송기
    network: Arc<NetworkHandler>,
}

#[derive(Clone)]
struct ChannelState {
    channel_id: u32,
    members: Vec<u32>,
    max_members: usize,
    quality_mode: AudioQuality,
}

#[derive(Clone)]
struct UserVoiceState {
    user_id: u32,
    is_talking: bool,
    is_muted: bool,
    volume: f32,
    last_packet_time: Instant,
}

#[derive(Clone, Copy)]
enum AudioQuality {
    Low = 8000,      // 8kHz, 낮은 대역폭
    Normal = 16000,  // 16kHz, 일반 품질
    High = 48000,    // 48kHz, 고품질
}

/// 오디오 버퍼 (지터 처리)
struct AudioBuffer {
    buffer: VecDeque<(u16, Bytes)>,  // (시퀀스, 데이터)
    expected_seq: u16,
    max_size: usize,
}

impl VoiceChannelService {
    pub fn new(network: Arc<NetworkHandler>) -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
            user_channels: Arc::new(DashMap::new()),
            user_states: Arc::new(DashMap::new()),
            audio_buffers: Arc::new(DashMap::new()),
            network,
        }
    }

    /// 채널 생성
    pub async fn create_channel(
        &self,
        channel_id: u32,
        max_members: usize,
        quality: AudioQuality,
    ) -> Result<()> {
        let channel = ChannelState {
            channel_id,
            members: Vec::new(),
            max_members,
            quality_mode: quality,
        };

        self.channels.insert(channel_id, channel);
        
        tracing::info!("Voice channel {} created", channel_id);
        Ok(())
    }

    /// 채널 참가
    pub async fn join_channel(
        &self,
        user_id: u32,
        channel_id: u32,
    ) -> Result<()> {
        // 이미 다른 채널에 있으면 나가기
        if let Some(old_channel) = self.user_channels.get(&user_id) {
            self.leave_channel(user_id).await?;
        }

        // 채널 확인
        let mut channel = self.channels
            .get_mut(&channel_id)
            .ok_or_else(|| anyhow!("Channel not found"))?;

        // 인원 제한 확인
        if channel.members.len() >= channel.max_members {
            return Err(anyhow!("Channel is full"));
        }

        // 멤버 추가
        channel.members.push(user_id);

        // 사용자 상태 초기화
        self.user_channels.insert(user_id, channel_id);
        self.user_states.insert(user_id, UserVoiceState {
            user_id,
            is_talking: false,
            is_muted: false,
            volume: 1.0,
            last_packet_time: Instant::now(),
        });

        // 오디오 버퍼 초기화
        self.audio_buffers.insert(user_id, AudioBuffer {
            buffer: VecDeque::with_capacity(10),
            expected_seq: 0,
            max_size: 10,
        });

        // 다른 멤버들에게 알림
        self.broadcast_to_channel(
            channel_id,
            VoicePacket {
                packet_type: VoicePacketType::JoinChannel,
                sequence: 0,
                timestamp: VoicePacket::current_timestamp(),
                channel_id,
                sender_id: user_id,
                data: Bytes::new(),
            },
        ).await?;

        tracing::info!("User {} joined voice channel {}", user_id, channel_id);
        Ok(())
    }

    /// 음성 데이터 처리
    pub async fn process_audio(
        &self,
        user_id: u32,
        packet: VoicePacket,
    ) -> Result<()> {
        // 사용자가 채널에 있는지 확인
        let channel_id = self.user_channels
            .get(&user_id)
            .ok_or_else(|| anyhow!("User not in any channel"))?
            .clone();

        // 음소거 상태 확인
        if let Some(state) = self.user_states.get(&user_id) {
            if state.is_muted {
                return Ok(()); // 음소거 상태면 무시
            }
        }

        // 오디오 처리 (옵션)
        let processed_audio = self.process_audio_effects(packet.data.clone()).await?;

        // 같은 채널의 다른 멤버들에게 브로드캐스트
        let channel = self.channels
            .get(&channel_id)
            .ok_or_else(|| anyhow!("Channel not found"))?;

        for member_id in &channel.members {
            if *member_id != user_id {
                // 지터 버퍼에 추가
                if let Some(mut buffer) = self.audio_buffers.get_mut(member_id) {
                    buffer.buffer.push_back((packet.sequence, processed_audio.clone()));
                    
                    // 버퍼 크기 제한
                    while buffer.buffer.len() > buffer.max_size {
                        buffer.buffer.pop_front();
                    }
                }

                // 네트워크로 전송
                self.network.send_to_user(
                    *member_id,
                    packet.to_bytes(),
                ).await?;
            }
        }

        // 상태 업데이트
        if let Some(mut state) = self.user_states.get_mut(&user_id) {
            state.is_talking = true;
            state.last_packet_time = Instant::now();
        }

        Ok(())
    }

    /// 오디오 효과 처리 (노이즈 제거, 에코 캔슬링 등)
    async fn process_audio_effects(&self, audio: Bytes) -> Result<Bytes> {
        // TODO: 실제 오디오 처리 구현
        // - 노이즈 게이트
        // - 에코 캔슬링
        // - 자동 게인 제어 (AGC)
        // - 음성 활동 감지 (VAD)
        
        Ok(audio) // 현재는 그대로 반환
    }

    /// 음소거 토글
    pub async fn toggle_mute(&self, user_id: u32) -> Result<bool> {
        let mut state = self.user_states
            .get_mut(&user_id)
            .ok_or_else(|| anyhow!("User not in voice"))?;

        state.is_muted = !state.is_muted;
        let is_muted = state.is_muted;

        // 채널 멤버들에게 알림
        if let Some(channel_id) = self.user_channels.get(&user_id) {
            let packet = VoicePacket {
                packet_type: if is_muted { 
                    VoicePacketType::Mute 
                } else { 
                    VoicePacketType::Unmute 
                },
                sequence: 0,
                timestamp: VoicePacket::current_timestamp(),
                channel_id: *channel_id,
                sender_id: user_id,
                data: Bytes::new(),
            };

            self.broadcast_to_channel(*channel_id, packet).await?;
        }

        Ok(is_muted)
    }

    /// 지터 버퍼에서 오디오 읽기
    pub async fn read_audio_buffer(&self, user_id: u32) -> Option<Bytes> {
        let mut buffer = self.audio_buffers.get_mut(&user_id)?;
        
        // 시퀀스 순서대로 꺼내기
        while let Some((seq, data)) = buffer.buffer.front() {
            if *seq == buffer.expected_seq {
                buffer.expected_seq = buffer.expected_seq.wrapping_add(1);
                return buffer.buffer.pop_front().map(|(_, d)| d);
            } else if seq < &buffer.expected_seq {
                // 오래된 패킷 제거
                buffer.buffer.pop_front();
            } else {
                // 아직 도착하지 않은 패킷 대기
                break;
            }
        }
        
        None
    }

    // ... 추가 헬퍼 메서드들
}
```

### 🔧 Step 3: WebRTC 통합 (선택적)
```rust
// rudpserver/src/voice/webrtc_bridge.rs
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;

/// WebRTC 브리지 (브라우저 클라이언트 지원)
pub struct WebRTCBridge {
    peer_connections: Arc<DashMap<u32, RTCPeerConnection>>,
    audio_tracks: Arc<DashMap<u32, Arc<TrackLocalStaticRTP>>>,
}

impl WebRTCBridge {
    pub async fn create_peer_connection(
        &self,
        user_id: u32,
    ) -> Result<String> {
        // STUN/TURN 서버 설정
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_string()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // Peer Connection 생성
        let peer_connection = Arc::new(
            RTCPeerConnection::new(config).await?
        );

        // 오디오 트랙 추가
        let audio_track = Arc::new(TrackLocalStaticRTP::new(
            RTCRtpCodecCapability {
                mime_type: "audio/opus".to_string(),
                ..Default::default()
            },
            "audio".to_string(),
            "webrtc-audio".to_string(),
        ));

        peer_connection
            .add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await?;

        // SDP Offer 생성
        let offer = peer_connection.create_offer(None).await?;
        peer_connection.set_local_description(offer.clone()).await?;

        // 저장
        self.peer_connections.insert(user_id, peer_connection);
        self.audio_tracks.insert(user_id, audio_track);

        // SDP 반환 (클라이언트로 전송)
        Ok(offer.sdp)
    }

    // ... WebRTC 시그널링 처리
}
```

---

## 🧪 테스트 방법

### gRPC 게시판 테스트
```bash
# gRPC 서버 실행
cargo run --bin grpcserver

# grpcurl로 테스트
grpcurl -plaintext -d '{
  "title": "Test Post",
  "content": "Hello World",
  "category": "general",
  "author_id": 1
}' localhost:50051 board.BoardService/CreatePost

# 게시글 조회
grpcurl -plaintext -d '{"post_id": 1}' \
  localhost:50051 board.BoardService/GetPost
```

### TCP 강퇴 기능 테스트
```rust
// tcpserver/src/tests/kick_test.rs
#[tokio::test]
async fn test_kick_user() {
    let service = create_test_service().await;
    
    // 방장이 사용자 강퇴
    let message = GameMessage::KickUser {
        room_id: 1,
        target_user_id: 2,
        reason: "Inappropriate behavior".to_string(),
    };
    
    let response = service.handle_message(1, message).await.unwrap();
    
    match response {
        GameMessage::KickUserResponse { success, .. } => {
            assert!(success);
        }
        _ => panic!("Unexpected response"),
    }
}
```

### RUDP 음성 채팅 테스트
```rust
// rudpserver/tests/voice_test.rs
#[tokio::test]
async fn test_voice_channel() {
    let service = VoiceChannelService::new(mock_network());
    
    // 채널 생성
    service.create_channel(1, 10, AudioQuality::Normal).await.unwrap();
    
    // 사용자 참가
    service.join_channel(1, 1).await.unwrap();
    service.join_channel(2, 1).await.unwrap();
    
    // 음성 패킷 전송
    let audio_data = Bytes::from(vec![0u8; 960]); // 20ms @ 48kHz
    let packet = VoicePacket::audio(1, 1, 0, audio_data);
    
    service.process_audio(1, packet).await.unwrap();
}
```

---

## 📚 추가 리소스

### 필요한 의존성
```toml
# Cargo.toml

# gRPC 게시판
[dependencies]
tonic = "0.9"
prost = "0.11"
sqlx = { version = "0.7", features = ["mysql", "runtime-tokio"] }

# TCP 강퇴
dashmap = "5.5"
tokio = { version = "1", features = ["full"] }

# RUDP 음성
bytes = "1.4"
webrtc = "0.9"  # 선택적
opus = "0.5"     # 오디오 코덱
```

### 보안 고려사항
1. **권한 검증**: 모든 작업 전 권한 확인
2. **Rate Limiting**: 스팸 방지
3. **입력 검증**: SQL Injection, XSS 방지
4. **암호화**: 음성 데이터 E2E 암호화 고려

### 성능 최적화
1. **캐싱**: Redis로 빈번한 조회 캐싱
2. **배치 처리**: 대량 작업 배치 처리
3. **비동기 처리**: 긴 작업은 백그라운드 처리
4. **연결 풀링**: DB/Redis 연결 재사용

---

이 가이드를 따라 각 서버에 새로운 기능을 쉽게 추가할 수 있습니다!