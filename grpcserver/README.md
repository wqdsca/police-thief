# Police Thief gRPC Server

Police Thief 게임을 위한 gRPC 서버입니다. RoomService와 UserService를 제공하며, JWT 토큰 기반 인증 시스템과 체계적인 에러 처리를 포함합니다.

## 🚀 Features

### Core Services
- **Room Service**: 방 생성 및 조회 기능
- **User Service**: 사용자 인증 및 회원가입 기능
- **JWT Authentication**: 토큰 기반 인증 시스템
- **Error Management**: 체계적인 에러 처리 및 로깅

### Advanced Features
- **Common Authentication**: 모든 컨트롤러에서 재사용 가능한 인증 함수
- **Integration Tests**: gRPC 클라이언트 테스트
- **Structured Logging**: tracing 기반 로깅 시스템
- **Environment Configuration**: 환경변수 기반 설정

## 📁 Project Structure

```
grpcserver/
├── src/
│   ├── controller/           # gRPC 컨트롤러
│   │   ├── room_controller.rs
│   │   └── user_controller.rs
│   ├── service/             # 비즈니스 로직
│   │   ├── room_service.rs
│   │   └── user_service.rs
│   ├── tool/               # 유틸리티 모듈
│   │   ├── error.rs        # 에러 관리 시스템
│   │   ├── token.rs        # JWT 토큰 서비스
│   │   └── intercepter.rs  # gRPC 인터셉터
│   ├── test/               # 테스트 모듈
│   │   ├── test_interceptor.rs
│   │   └── test_client.rs
│   ├── server.rs           # 서버 설정
│   ├── main.rs             # 애플리케이션 진입점
│   └── lib.rs              # 라이브러리 모듈 정의
├── proto/                  # Protocol Buffer 정의
│   ├── room.proto
│   └── user.proto
├── tests/                  # 통합 테스트
│   └── integration_test.rs
└── README.md
```

## 🛠 Installation & Setup

### Prerequisites
- Rust 1.70+ 
- Cargo
- Protocol Buffers compiler

### Environment Variables
`.env` 파일을 프로젝트 루트에 생성하세요:

```env
# gRPC Server Configuration
grpc_host=127.0.0.1
grpc_port=50051

# JWT Configuration
JWT_SECRET_KEY=your_secret_key_here
JWT_ALGORITHM=HS256
```

### Build & Run

```bash
# 의존성 설치
cargo build

# 서버 실행
grpc_host=127.0.0.1 grpc_port=50051 cargo run

# 테스트 실행
cargo test --lib --test test_interceptor -- --nocapture
```

## 🔧 Core Components

### 1. TokenService - 공통 인증 시스템

#### `with_auth<T, F>` - 필수 인증
```rust
// JWT 토큰이 반드시 필요한 엔드포인트
self.token_service.with_auth(&req, |user_id| {
    // 비즈니스 로직 실행
    Ok(response)
})
```

#### `with_optional_auth<T, F>` - 선택적 인증
```rust
// 토큰이 있으면 검증, 없으면 통과
self.token_service.with_optional_auth(&req, |user_id| {
    match user_id {
        Some(id) => get_personalized_data(id),
        None => get_public_data(),
    }
})
```

#### `with_conditional_auth<T, F>` - 조건부 인증
```rust
// 공개/보호 엔드포인트 자동 구분
self.token_service.with_conditional_auth(&req, |user_id| {
    // 경로에 따라 인증 방식 결정
    Ok(response)
})
```

### 2. Error Management System

#### AppError Types
```rust
#[derive(Error, Debug, Clone)]
pub enum AppError {
    AuthError(String),           // 인증 실패
    UserNotFound(String),        // 사용자 없음
    InvalidInput(String),        // 입력값 오류
    DatabaseConnection(String),   // DB 연결 실패
    NicknameExists(String),      // 닉네임 중복
    // ... 기타 에러 타입들
}
```

#### Error Severity Levels
- **Critical**: 시스템 장애
- **High**: 비즈니스 로직 실패
- **Medium**: 사용자 입력 오류
- **Low**: 일반적인 경고

### 3. Service Layer

#### RoomService
```rust
impl RoomService {
    pub async fn make_room(
        &self, 
        user_id: i32, 
        nick_name: String, 
        room_name: String, 
        max_player_num: i32
    ) -> Result<i32, AppError>
    
    pub async fn get_room_list(
        &self, 
        last_room_id: i32
    ) -> Result<Vec<RoomInfo>, AppError>
}
```

#### UserService
```rust
impl UserService {
    pub async fn login_user(
        &self, 
        login_type: String, 
        login_token: String
    ) -> Result<(i32, String, String, String, bool), AppError>
    
    pub async fn register_user(
        &self, 
        login_type: String, 
        login_token: String, 
        nick_name: String
    ) -> Result<(), AppError>
}
```

## 🧪 Testing

### Integration Tests
```bash
# gRPC 연결 테스트
cargo test test_grpc_connection --lib -- --nocapture

# 에러 시스템 테스트
cargo test test_error_system --lib -- --nocapture

# 전체 테스트
cargo test --lib --test test_interceptor -- --nocapture
```

### Test Coverage
- ✅ **Room Service**: 방 생성/조회 정상/에러 케이스
- ✅ **User Service**: 로그인/회원가입 정상/에러 케이스
- ✅ **Error System**: 에러 변환 및 통계
- ✅ **Authentication**: JWT 토큰 검증

## 📊 API Endpoints

### Room Service
| Method | Endpoint | Description | Auth Required |
|--------|----------|-------------|---------------|
| `make_room` | `/room.RoomService/MakeRoom` | 방 생성 | Optional |
| `get_room_list` | `/room.RoomService/GetRoomList` | 방 리스트 조회 | Optional |

### User Service
| Method | Endpoint | Description | Auth Required |
|--------|----------|-------------|---------------|
| `login_user` | `/user.UserService/LoginUser` | 로그인 | No |
| `register_user` | `/user.UserService/RegisterUser` | 회원가입 | No |

## 🔐 Authentication

### JWT Token Format
```
Authorization: Bearer <jwt_token>
```

### Token Claims
```json
{
  "sub": 123,        // 사용자 ID
  "exp": 1735689600  // 만료 시간 (Unix timestamp)
}
```

### Public Endpoints
- `/user.UserService/LoginUser`
- `/user.UserService/RegisterUser`

### Protected Endpoints
- 모든 Room Service 엔드포인트 (선택적 인증)

## 🚨 Error Handling

### Error Response Format
```json
{
  "code": "Internal",
  "message": "에러 메시지",
  "details": []
}
```

### Common Error Codes
- `Unauthenticated`: 인증 실패
- `InvalidArgument`: 잘못된 입력값
- `NotFound`: 리소스 없음
- `AlreadyExists`: 중복된 리소스
- `Internal`: 내부 서버 오류

## 📝 Logging

### Log Levels
- **INFO**: 일반적인 작업 로그
- **WARN**: 경고 메시지
- **ERROR**: 에러 메시지

### Log Format
```
2025-08-07T13:27:16.681379Z  INFO grpcserver::controller::room_controller: 방 생성 요청: user_id=123, room_name=테스트 방
```

## 🔧 Configuration

### Environment Variables
| Variable | Default | Description |
|----------|---------|-------------|
| `grpc_host` | `127.0.0.1` | gRPC 서버 호스트 |
| `grpc_port` | `50051` | gRPC 서버 포트 |
| `JWT_SECRET_KEY` | `default_secret` | JWT 서명 키 |
| `JWT_ALGORITHM` | `HS256` | JWT 알고리즘 |

## 🚀 Deployment

### Development
```bash
# 개발 모드 실행
cargo run
```

### Production
```bash
# 릴리즈 빌드
cargo build --release

# 프로덕션 실행
./target/release/grpcserver
```

## 📚 Dependencies

### Core Dependencies
- `tonic`: gRPC 프레임워크
- `tokio`: 비동기 런타임
- `tracing`: 로깅 시스템
- `jsonwebtoken`: JWT 토큰 처리
- `thiserror`: 에러 처리
- `anyhow`: 에러 전파

### Development Dependencies
- `tonic-build`: Protocol Buffer 컴파일
- `prost`: Protocol Buffer 런타임

## 🤝 Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🆘 Support

문제가 발생하거나 질문이 있으시면 이슈를 생성해주세요.

---

**Police Thief gRPC Server** - 안전하고 확장 가능한 게임 서버 🎮 