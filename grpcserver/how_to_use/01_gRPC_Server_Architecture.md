# gRPC 서버 아키텍처 및 확장 가이드

## 📋 목차
1. [gRPC 서버 개요](#grpc-서버-개요)
2. [핵심 컴포넌트](#핵심-컴포넌트)
3. [인증 및 보안 시스템](#인증-및-보안-시스템)
4. [확장 방법](#확장-방법)
5. [성능 최적화](#성능-최적화)

## 🚀 gRPC 서버 개요

### 아키텍처 구조
```
gRPC Server Architecture
├── API Gateway Layer
│   ├── Authentication Interceptor
│   ├── Authorization Middleware
│   ├── Rate Limiting
│   └── Request Validation
├── Service Layer
│   ├── UserService (사용자 관리)
│   ├── RoomService (방 관리)
│   ├── GameService (게임 로직)
│   └── AdminService (관리자 기능)
├── Business Logic Layer
│   ├── Service Implementations
│   ├── Validation Logic
│   ├── Business Rules
│   └── Event Processing
├── Data Access Layer
│   ├── Redis Cache Manager
│   ├── MariaDB Connection Pool
│   ├── Transaction Manager
│   └── Migration System
└── Infrastructure Layer
    ├── JWT Token Service
    ├── Error Handling
    ├── Logging & Monitoring
    └── Configuration Management
```

### 현재 성능 지표
- **처리량**: HTTP/2 기반 고성능 RPC
- **동시 요청**: 1000+ concurrent requests
- **인증**: JWT 기반 3단계 인증 (Required/Optional/Conditional)
- **데이터베이스**: Redis Cache + MariaDB 하이브리드
- **프로토콜**: Protocol Buffers v3

## 🔧 핵심 컴포넌트

### 1. gRPC 서비스 정의 (.proto)

```protobuf
// proto/user.proto
syntax = "proto3";

package user;

// 사용자 관리 서비스
service UserService {
  // 사용자 등록
  rpc RegisterUser(RegisterUserRequest) returns (RegisterUserResponse);
  
  // 사용자 로그인
  rpc LoginUser(LoginUserRequest) returns (LoginUserResponse);
  
  // 사용자 정보 조회
  rpc GetUserInfo(GetUserInfoRequest) returns (GetUserInfoResponse);
  
  // 사용자 정보 수정
  rpc UpdateUserInfo(UpdateUserInfoRequest) returns (UpdateUserInfoResponse);
  
  // 사용자 상태 변경
  rpc UpdateUserStatus(UpdateUserStatusRequest) returns (UpdateUserStatusResponse);
}

message RegisterUserRequest {
  string username = 1;
  string password = 2;
  string email = 3;
  string nickname = 4;
}

message RegisterUserResponse {
  bool success = 1;
  string message = 2;
  int32 user_id = 3;
  string access_token = 4;
  string refresh_token = 5;
}

message LoginUserRequest {
  string username = 1;
  string password = 2;
}

message LoginUserResponse {
  bool success = 1;
  string message = 2;
  UserInfo user_info = 3;
  string access_token = 4;
  string refresh_token = 5;
}

message UserInfo {
  int32 user_id = 1;
  string username = 2;
  string nickname = 3;
  string email = 4;
  UserStatus status = 5;
  int64 created_at = 6;
  int64 last_login = 7;
}

enum UserStatus {
  OFFLINE = 0;
  ONLINE = 1;
  IN_GAME = 2;
  AWAY = 3;
}
```

```protobuf
// proto/room.proto
syntax = "proto3";

package room;

// 방 관리 서비스
service RoomService {
  // 방 생성
  rpc CreateRoom(CreateRoomRequest) returns (CreateRoomResponse);
  
  // 방 목록 조회
  rpc ListRooms(ListRoomsRequest) returns (ListRoomsResponse);
  
  // 방 참가
  rpc JoinRoom(JoinRoomRequest) returns (JoinRoomResponse);
  
  // 방 나가기
  rpc LeaveRoom(LeaveRoomRequest) returns (LeaveRoomResponse);
  
  // 방 상태 조회
  rpc GetRoomInfo(GetRoomInfoRequest) returns (GetRoomInfoResponse);
  
  // 방 설정 변경
  rpc UpdateRoomSettings(UpdateRoomSettingsRequest) returns (UpdateRoomSettingsResponse);
}

message CreateRoomRequest {
  string room_name = 1;
  int32 max_players = 2;
  bool is_private = 3;
  string password = 4;
  RoomSettings settings = 5;
}

message CreateRoomResponse {
  bool success = 1;
  string message = 2;
  int32 room_id = 3;
  RoomInfo room_info = 4;
}

message RoomInfo {
  int32 room_id = 1;
  string room_name = 2;
  int32 current_players = 3;
  int32 max_players = 4;
  bool is_private = 5;
  RoomStatus status = 6;
  int32 host_id = 7;
  repeated UserInfo players = 8;
  RoomSettings settings = 9;
  int64 created_at = 10;
}

message RoomSettings {
  int32 game_mode = 1;
  int32 round_time = 2;
  int32 max_rounds = 3;
  bool allow_spectators = 4;
  map<string, string> custom_settings = 5;
}

enum RoomStatus {
  WAITING = 0;
  IN_PROGRESS = 1;
  FINISHED = 2;
}
```

### 2. 서비스 구현

```rust
// src/service/user_service.rs
use tonic::{Request, Response, Status};
use crate::proto::user::user_service_server::UserService;
use crate::proto::user::*;
use crate::auth::{AuthContext, AuthLevel};
use shared::service::token::JwtService;
use shared::service::redis::UserRedisService;

pub struct UserServiceImpl {
    jwt_service: Arc<JwtService>,
    user_redis: Arc<UserRedisService>,
    db_pool: Arc<SqlxPool>,
}

impl UserServiceImpl {
    pub fn new(
        jwt_service: Arc<JwtService>,
        user_redis: Arc<UserRedisService>,
        db_pool: Arc<SqlxPool>,
    ) -> Self {
        Self {
            jwt_service,
            user_redis,
            db_pool,
        }
    }
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    /// 사용자 등록
    async fn register_user(
        &self,
        request: Request<RegisterUserRequest>,
    ) -> Result<Response<RegisterUserResponse>, Status> {
        let req = request.into_inner();
        
        // 입력 검증
        self.validate_register_request(&req)?;
        
        // 중복 사용자 확인
        if self.check_user_exists(&req.username, &req.email).await? {
            return Ok(Response::new(RegisterUserResponse {
                success: false,
                message: "이미 존재하는 사용자명 또는 이메일입니다.".to_string(),
                user_id: 0,
                access_token: String::new(),
                refresh_token: String::new(),
            }));
        }
        
        // 비밀번호 해시화
        let password_hash = self.hash_password(&req.password)?;
        
        // 데이터베이스에 사용자 생성
        let user_id = self.create_user_in_db(&req, &password_hash).await?;
        
        // JWT 토큰 생성
        let (access_token, refresh_token) = self.jwt_service.generate_token_pair(
            user_id,
            &req.username,
            None
        ).map_err(|e| Status::internal(format!("Token generation failed: {}", e)))?;
        
        // Redis에 사용자 세션 저장
        self.user_redis.create_user_session(user_id, &req.username, &req.nickname).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
        
        tracing::info!("사용자 등록 완료: user_id={}, username={}", user_id, req.username);
        
        Ok(Response::new(RegisterUserResponse {
            success: true,
            message: "사용자 등록이 완료되었습니다.".to_string(),
            user_id,
            access_token,
            refresh_token,
        }))
    }
    
    /// 사용자 로그인
    async fn login_user(
        &self,
        request: Request<LoginUserRequest>,
    ) -> Result<Response<LoginUserResponse>, Status> {
        let req = request.into_inner();
        
        // 사용자 인증
        let user = self.authenticate_user(&req.username, &req.password).await?;
        
        // JWT 토큰 생성
        let (access_token, refresh_token) = self.jwt_service.generate_token_pair(
            user.user_id,
            &user.username,
            None
        ).map_err(|e| Status::internal(format!("Token generation failed: {}", e)))?;
        
        // Redis에 로그인 상태 업데이트
        self.user_redis.update_user_login_status(user.user_id, true).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
        
        // 마지막 로그인 시간 업데이트
        self.update_last_login(user.user_id).await?;
        
        tracing::info!("사용자 로그인: user_id={}, username={}", user.user_id, user.username);
        
        Ok(Response::new(LoginUserResponse {
            success: true,
            message: "로그인이 완료되었습니다.".to_string(),
            user_info: Some(UserInfo {
                user_id: user.user_id,
                username: user.username.clone(),
                nickname: user.nickname,
                email: user.email,
                status: UserStatus::Online as i32,
                created_at: user.created_at.timestamp(),
                last_login: chrono::Utc::now().timestamp(),
            }),
            access_token,
            refresh_token,
        }))
    }
    
    /// 사용자 정보 조회 (인증 필요)
    async fn get_user_info(
        &self,
        request: Request<GetUserInfoRequest>,
    ) -> Result<Response<GetUserInfoResponse>, Status> {
        let auth_context = AuthContext::from_request(&request, AuthLevel::Required)?;
        let req = request.into_inner();
        
        let target_user_id = if req.user_id == 0 {
            auth_context.user_id // 자신의 정보 조회
        } else {
            req.user_id // 다른 사용자 정보 조회
        };
        
        // 권한 검증 (다른 사용자 정보 조회 시)
        if target_user_id != auth_context.user_id && !auth_context.is_admin() {
            return Err(Status::permission_denied("다른 사용자의 정보에 접근할 권한이 없습니다."));
        }
        
        // 사용자 정보 조회
        let user = self.get_user_by_id(target_user_id).await?;
        let online_status = self.user_redis.is_user_online(target_user_id).await
            .unwrap_or(false);
        
        Ok(Response::new(GetUserInfoResponse {
            success: true,
            message: "사용자 정보를 조회했습니다.".to_string(),
            user_info: Some(UserInfo {
                user_id: user.user_id,
                username: user.username,
                nickname: user.nickname,
                email: user.email,
                status: if online_status { UserStatus::Online } else { UserStatus::Offline } as i32,
                created_at: user.created_at.timestamp(),
                last_login: user.last_login.unwrap_or(user.created_at).timestamp(),
            }),
        }))
    }
}

impl UserServiceImpl {
    /// 등록 요청 검증
    fn validate_register_request(&self, req: &RegisterUserRequest) -> Result<(), Status> {
        if req.username.is_empty() || req.username.len() < 3 {
            return Err(Status::invalid_argument("사용자명은 3자 이상이어야 합니다."));
        }
        
        if req.password.len() < 8 {
            return Err(Status::invalid_argument("비밀번호는 8자 이상이어야 합니다."));
        }
        
        if !req.email.contains('@') {
            return Err(Status::invalid_argument("올바른 이메일 주소를 입력해주세요."));
        }
        
        if req.nickname.is_empty() || req.nickname.len() < 2 {
            return Err(Status::invalid_argument("닉네임은 2자 이상이어야 합니다."));
        }
        
        Ok(())
    }
    
    /// 사용자 중복 확인
    async fn check_user_exists(&self, username: &str, email: &str) -> Result<bool, Status> {
        let query = r#"
            SELECT COUNT(*) as count 
            FROM users 
            WHERE username = ? OR email = ?
        "#;
        
        let row: (i64,) = sqlx::query_as(query)
            .bind(username)
            .bind(email)
            .fetch_one(&*self.db_pool)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;
        
        Ok(row.0 > 0)
    }
    
    /// 비밀번호 해시화
    fn hash_password(&self, password: &str) -> Result<String, Status> {
        use bcrypt::{hash, DEFAULT_COST};
        hash(password, DEFAULT_COST)
            .map_err(|e| Status::internal(format!("Password hashing failed: {}", e)))
    }
    
    /// 데이터베이스에 사용자 생성
    async fn create_user_in_db(
        &self, 
        req: &RegisterUserRequest, 
        password_hash: &str
    ) -> Result<i32, Status> {
        let query = r#"
            INSERT INTO users (username, password_hash, email, nickname, created_at)
            VALUES (?, ?, ?, ?, NOW())
        "#;
        
        let result = sqlx::query(query)
            .bind(&req.username)
            .bind(password_hash)
            .bind(&req.email)
            .bind(&req.nickname)
            .execute(&*self.db_pool)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;
        
        Ok(result.last_insert_id() as i32)
    }
    
    /// 사용자 인증
    async fn authenticate_user(&self, username: &str, password: &str) -> Result<User, Status> {
        let query = r#"
            SELECT user_id, username, password_hash, email, nickname, created_at, last_login
            FROM users 
            WHERE username = ?
        "#;
        
        let user: User = sqlx::query_as(query)
            .bind(username)
            .fetch_optional(&*self.db_pool)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::unauthenticated("잘못된 사용자명 또는 비밀번호입니다."))?;
        
        // 비밀번호 검증
        use bcrypt::verify;
        if !verify(password, &user.password_hash)
            .map_err(|e| Status::internal(format!("Password verification failed: {}", e)))? {
            return Err(Status::unauthenticated("잘못된 사용자명 또는 비밀번호입니다."));
        }
        
        Ok(user)
    }
    
    /// 마지막 로그인 시간 업데이트
    async fn update_last_login(&self, user_id: i32) -> Result<(), Status> {
        let query = "UPDATE users SET last_login = NOW() WHERE user_id = ?";
        
        sqlx::query(query)
            .bind(user_id)
            .execute(&*self.db_pool)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;
        
        Ok(())
    }
    
    /// ID로 사용자 조회
    async fn get_user_by_id(&self, user_id: i32) -> Result<User, Status> {
        let query = r#"
            SELECT user_id, username, password_hash, email, nickname, created_at, last_login
            FROM users 
            WHERE user_id = ?
        "#;
        
        sqlx::query_as(query)
            .bind(user_id)
            .fetch_optional(&*self.db_pool)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("사용자를 찾을 수 없습니다."))
    }
}

#[derive(sqlx::FromRow)]
struct User {
    user_id: i32,
    username: String,
    password_hash: String,
    email: String,
    nickname: String,
    created_at: chrono::DateTime<chrono::Utc>,
    last_login: Option<chrono::DateTime<chrono::Utc>>,
}
```

### 3. 방 관리 서비스

```rust
// src/service/room_service.rs
use tonic::{Request, Response, Status};
use crate::proto::room::room_service_server::RoomService;
use crate::proto::room::*;
use crate::auth::{AuthContext, AuthLevel};
use shared::service::redis::RoomRedisService;

pub struct RoomServiceImpl {
    room_redis: Arc<RoomRedisService>,
    db_pool: Arc<SqlxPool>,
}

impl RoomServiceImpl {
    pub fn new(room_redis: Arc<RoomRedisService>, db_pool: Arc<SqlxPool>) -> Self {
        Self {
            room_redis,
            db_pool,
        }
    }
}

#[tonic::async_trait]
impl RoomService for RoomServiceImpl {
    /// 방 생성
    async fn create_room(
        &self,
        request: Request<CreateRoomRequest>,
    ) -> Result<Response<CreateRoomResponse>, Status> {
        let auth_context = AuthContext::from_request(&request, AuthLevel::Required)?;
        let req = request.into_inner();
        
        // 입력 검증
        self.validate_create_room_request(&req)?;
        
        // 사용자의 기존 방 확인
        if let Some(existing_room_id) = self.room_redis.get_user_current_room(auth_context.user_id).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))? {
            return Ok(Response::new(CreateRoomResponse {
                success: false,
                message: format!("이미 방 {}에 참가중입니다.", existing_room_id),
                room_id: 0,
                room_info: None,
            }));
        }
        
        // Redis에서 새 방 ID 생성
        let room_id = self.room_redis.generate_room_id().await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
        
        // 방 설정 생성
        let room_settings = req.settings.unwrap_or_else(|| RoomSettings {
            game_mode: 0,
            round_time: 300, // 5분
            max_rounds: 10,
            allow_spectators: true,
            custom_settings: std::collections::HashMap::new(),
        });
        
        // Redis에 방 정보 저장
        let room_info = RoomInfo {
            room_id,
            room_name: req.room_name.clone(),
            current_players: 1,
            max_players: req.max_players,
            is_private: req.is_private,
            status: RoomStatus::Waiting as i32,
            host_id: auth_context.user_id,
            players: vec![], // 나중에 채움
            settings: Some(room_settings.clone()),
            created_at: chrono::Utc::now().timestamp(),
        };
        
        self.room_redis.create_room(room_id, &room_info, &req.password).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
        
        // 방장을 방에 추가
        self.room_redis.add_user_to_room(room_id, auth_context.user_id).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
        
        // 데이터베이스에 방 기록 저장 (통계용)
        self.save_room_to_db(room_id, &req, auth_context.user_id).await?;
        
        tracing::info!(
            "방 생성 완료: room_id={}, host_id={}, room_name='{}'", 
            room_id, auth_context.user_id, req.room_name
        );
        
        Ok(Response::new(CreateRoomResponse {
            success: true,
            message: "방이 성공적으로 생성되었습니다.".to_string(),
            room_id,
            room_info: Some(room_info),
        }))
    }
    
    /// 방 목록 조회
    async fn list_rooms(
        &self,
        request: Request<ListRoomsRequest>,
    ) -> Result<Response<ListRoomsResponse>, Status> {
        let _auth_context = AuthContext::from_request(&request, AuthLevel::Optional)?;
        let req = request.into_inner();
        
        // 페이징 파라미터 처리
        let page = req.page.max(1);
        let page_size = req.page_size.clamp(1, 100);
        
        // Redis에서 방 목록 조회
        let (rooms, total_count) = self.room_redis.get_room_list(
            page, 
            page_size, 
            req.filter_private,
            req.filter_status
        ).await.map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
        
        // 각 방의 플레이어 정보 조회
        let mut room_infos = Vec::new();
        for room in rooms {
            let players = self.room_redis.get_room_users(room.room_id).await
                .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
            
            let mut room_info = room;
            room_info.players = players;
            room_info.current_players = room_info.players.len() as i32;
            room_infos.push(room_info);
        }
        
        Ok(Response::new(ListRoomsResponse {
            success: true,
            message: "방 목록을 조회했습니다.".to_string(),
            rooms: room_infos,
            total_count,
            current_page: page,
            total_pages: (total_count + page_size - 1) / page_size,
        }))
    }
    
    /// 방 참가
    async fn join_room(
        &self,
        request: Request<JoinRoomRequest>,
    ) -> Result<Response<JoinRoomResponse>, Status> {
        let auth_context = AuthContext::from_request(&request, AuthLevel::Required)?;
        let req = request.into_inner();
        
        // 사용자의 기존 방 확인
        if let Some(existing_room_id) = self.room_redis.get_user_current_room(auth_context.user_id).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))? {
            
            if existing_room_id == req.room_id {
                return Ok(Response::new(JoinRoomResponse {
                    success: false,
                    message: "이미 해당 방에 참가중입니다.".to_string(),
                    room_info: None,
                }));
            } else {
                return Ok(Response::new(JoinRoomResponse {
                    success: false,
                    message: format!("다른 방 {}에 참가중입니다. 먼저 나가주세요.", existing_room_id),
                    room_info: None,
                }));
            }
        }
        
        // 방 정보 조회
        let room_info = self.room_redis.get_room_info(req.room_id).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?
            .ok_or_else(|| Status::not_found("방을 찾을 수 없습니다."))?;
        
        // 방 참가 가능 여부 확인
        self.validate_room_join(&room_info, &req)?;
        
        // 비밀번호 확인 (비공개 방)
        if room_info.is_private {
            if !self.room_redis.verify_room_password(req.room_id, &req.password.unwrap_or_default()).await
                .map_err(|e| Status::internal(format!("Redis error: {}", e)))? {
                return Ok(Response::new(JoinRoomResponse {
                    success: false,
                    message: "잘못된 비밀번호입니다.".to_string(),
                    room_info: None,
                }));
            }
        }
        
        // 방에 사용자 추가
        self.room_redis.add_user_to_room(req.room_id, auth_context.user_id).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
        
        // 업데이트된 방 정보 조회
        let updated_room_info = self.room_redis.get_room_info(req.room_id).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?
            .unwrap();
        
        tracing::info!(
            "방 참가 완료: room_id={}, user_id={}", 
            req.room_id, auth_context.user_id
        );
        
        Ok(Response::new(JoinRoomResponse {
            success: true,
            message: "방에 성공적으로 참가했습니다.".to_string(),
            room_info: Some(updated_room_info),
        }))
    }
    
    /// 방 나가기
    async fn leave_room(
        &self,
        request: Request<LeaveRoomRequest>,
    ) -> Result<Response<LeaveRoomResponse>, Status> {
        let auth_context = AuthContext::from_request(&request, AuthLevel::Required)?;
        let req = request.into_inner();
        
        // 사용자가 해당 방에 있는지 확인
        let current_room_id = self.room_redis.get_user_current_room(auth_context.user_id).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?
            .ok_or_else(|| Status::failed_precondition("현재 참가중인 방이 없습니다."))?;
        
        if current_room_id != req.room_id {
            return Err(Status::failed_precondition("해당 방에 참가중이 아닙니다."));
        }
        
        // 방에서 사용자 제거
        let room_deleted = self.room_redis.remove_user_from_room(req.room_id, auth_context.user_id).await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;
        
        let message = if room_deleted {
            "방을 나갔습니다. (방이 삭제되었습니다)"
        } else {
            "방을 나갔습니다."
        };
        
        tracing::info!(
            "방 나가기 완료: room_id={}, user_id={}, room_deleted={}", 
            req.room_id, auth_context.user_id, room_deleted
        );
        
        Ok(Response::new(LeaveRoomResponse {
            success: true,
            message: message.to_string(),
            room_deleted,
        }))
    }
}

impl RoomServiceImpl {
    /// 방 생성 요청 검증
    fn validate_create_room_request(&self, req: &CreateRoomRequest) -> Result<(), Status> {
        if req.room_name.is_empty() || req.room_name.len() < 2 {
            return Err(Status::invalid_argument("방 이름은 2자 이상이어야 합니다."));
        }
        
        if req.max_players < 2 || req.max_players > 20 {
            return Err(Status::invalid_argument("최대 플레이어 수는 2~20명이어야 합니다."));
        }
        
        if req.is_private && req.password.as_ref().map_or(true, |p| p.len() < 4) {
            return Err(Status::invalid_argument("비공개 방의 비밀번호는 4자 이상이어야 합니다."));
        }
        
        Ok(())
    }
    
    /// 방 참가 가능 여부 검증
    fn validate_room_join(&self, room_info: &RoomInfo, req: &JoinRoomRequest) -> Result<(), Status> {
        if room_info.status != RoomStatus::Waiting as i32 {
            return Err(Status::failed_precondition("게임이 진행중인 방에는 참가할 수 없습니다."));
        }
        
        if room_info.current_players >= room_info.max_players {
            return Err(Status::failed_precondition("방이 가득 찼습니다."));
        }
        
        Ok(())
    }
    
    /// 데이터베이스에 방 기록 저장
    async fn save_room_to_db(
        &self, 
        room_id: i32, 
        req: &CreateRoomRequest, 
        host_id: i32
    ) -> Result<(), Status> {
        let query = r#"
            INSERT INTO room_history (room_id, room_name, host_id, max_players, is_private, created_at)
            VALUES (?, ?, ?, ?, ?, NOW())
        "#;
        
        sqlx::query(query)
            .bind(room_id)
            .bind(&req.room_name)
            .bind(host_id)
            .bind(req.max_players)
            .bind(req.is_private)
            .execute(&*self.db_pool)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;
        
        Ok(())
    }
}
```

### 4. JWT 인증 시스템

```rust
// src/auth/mod.rs
use tonic::{Request, Status, metadata::MetadataValue};
use shared::service::token::JwtService;

#[derive(Debug, Clone)]
pub enum AuthLevel {
    Required,    // 반드시 인증 필요
    Optional,    // 인증 선택적
    Conditional, // 조건부 인증
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: i32,
    pub username: String,
    pub role: Option<String>,
    pub is_authenticated: bool,
}

impl AuthContext {
    /// 요청에서 인증 컨텍스트 추출
    pub fn from_request<T>(request: &Request<T>, auth_level: AuthLevel) -> Result<Self, Status> {
        let metadata = request.metadata();
        
        match metadata.get("authorization") {
            Some(token_value) => {
                let token = token_value
                    .to_str()
                    .map_err(|_| Status::unauthenticated("Invalid token format"))?
                    .strip_prefix("Bearer ")
                    .ok_or_else(|| Status::unauthenticated("Invalid authorization header"))?;
                
                // JWT 토큰 검증
                let jwt_service = JwtService::new();
                let claims = jwt_service.verify_token(token)
                    .map_err(|_| Status::unauthenticated("Invalid or expired token"))?;
                
                Ok(AuthContext {
                    user_id: claims.sub,
                    username: claims.username,
                    role: claims.role,
                    is_authenticated: true,
                })
            }
            None => {
                match auth_level {
                    AuthLevel::Required => {
                        Err(Status::unauthenticated("인증이 필요합니다."))
                    }
                    AuthLevel::Optional => {
                        Ok(AuthContext {
                            user_id: 0,
                            username: "anonymous".to_string(),
                            role: None,
                            is_authenticated: false,
                        })
                    }
                    AuthLevel::Conditional => {
                        // 조건부 인증은 서비스별로 처리
                        Ok(AuthContext {
                            user_id: 0,
                            username: "anonymous".to_string(),
                            role: None,
                            is_authenticated: false,
                        })
                    }
                }
            }
        }
    }
    
    /// 관리자 권한 확인
    pub fn is_admin(&self) -> bool {
        self.role.as_ref().map_or(false, |r| r == "admin")
    }
    
    /// 인증된 사용자인지 확인
    pub fn is_authenticated(&self) -> bool {
        self.is_authenticated
    }
    
    /// 특정 사용자 접근 권한 확인
    pub fn can_access_user(&self, target_user_id: i32) -> bool {
        self.is_admin() || self.user_id == target_user_id
    }
}

/// 인증 인터셉터
pub struct AuthInterceptor {
    jwt_service: Arc<JwtService>,
}

impl AuthInterceptor {
    pub fn new(jwt_service: Arc<JwtService>) -> Self {
        Self { jwt_service }
    }
}

impl tonic::service::Interceptor for AuthInterceptor {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, Status> {
        // 공개 엔드포인트는 인증 스킵
        let uri = request.uri();
        if is_public_endpoint(uri.path()) {
            return Ok(request);
        }
        
        // JWT 토큰 검증
        let metadata = request.metadata();
        if let Some(token_value) = metadata.get("authorization") {
            let token = token_value
                .to_str()
                .map_err(|_| Status::unauthenticated("Invalid token format"))?
                .strip_prefix("Bearer ")
                .ok_or_else(|| Status::unauthenticated("Invalid authorization header"))?;
            
            self.jwt_service.verify_token(token)
                .map_err(|_| Status::unauthenticated("Invalid or expired token"))?;
        }
        
        Ok(request)
    }
}

/// 공개 엔드포인트 확인
fn is_public_endpoint(path: &str) -> bool {
    matches!(path, 
        "/user.UserService/RegisterUser" |
        "/user.UserService/LoginUser" |
        "/room.RoomService/ListRooms"
    )
}
```

이 gRPC 서버 아키텍처는 고성능 API 서버를 위한 견고한 기반을 제공합니다. Protocol Buffers를 통한 효율적인 직렬화, JWT 기반 다단계 인증, Redis 캐싱과 MariaDB 영구 저장소를 결합한 하이브리드 데이터 계층을 특징으로 합니다.