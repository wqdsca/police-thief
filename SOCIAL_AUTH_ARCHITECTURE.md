# 소셜로그인 아키텍처 정리

## 현재 문제점

### 1. 과도한 중복
- `gamecenter/src/service/social_auth_service.rs` - 468줄
- `gamecenter/src/social_auth_handler.rs` - 177줄  
- `grpcserver/src/service/user_service.rs` - 불완전한 구현
- 동일한 OAuth 로직이 여러 곳에 분산

### 2. Arc<dyn Trait> 남용
```rust
// 현재 코드 - 과도한 추상화
pub struct UserService {
    auth_service: Arc<dyn AuthService>,
    user_redis: Arc<dyn UserRedisServiceTrait>,
    user_db: Arc<dyn UserDatabaseService>,
}
```

**문제점:**
- 불필요한 동적 디스패치로 성능 저하
- 코드 가독성 저하
- 컴파일 타임 타입 체크 불가
- 실제로는 구현체가 하나뿐

### 3. 복잡한 의존성
```
grpcserver → shared/traits → 여러 서비스들
gamecenter → 자체 구현
tcpserver → 별도 인증
```

## 개선된 아키텍처

### 1. 단순화된 구조
```
┌─────────────────────────────────────────┐
│            Client (Mobile/Web)           │
└────────────┬───────────────┬────────────┘
             │               │
         gRPC:50051      REST:8080
             │               │
    ┌────────▼───────┐ ┌────▼──────┐
    │  gRPC Handler  │ │REST Handler│
    └────────┬───────┘ └────┬──────┘
             │               │
         ┌───▼───────────────▼───┐
         │  Unified Social Auth  │
         │    (shared crate)     │
         └───┬───────────────┬───┘
             │               │
        ┌────▼────┐    ┌────▼────┐
        │  Redis  │    │MariaDB  │
        └─────────┘    └─────────┘
```

### 2. 파일 구조 (개선안)

```
shared/
├── src/
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── social_auth.rs      # 핵심 OAuth 로직 (통합)
│   │   ├── token.rs            # JWT 처리
│   │   └── types.rs            # 공통 타입
│   └── lib.rs

grpcserver/
├── src/
│   ├── handlers/
│   │   └── auth_handler.rs     # gRPC 엔드포인트만
│   └── main.rs

gamecenter/
├── src/
│   ├── api/
│   │   └── auth_api.rs         # REST 엔드포인트만
│   └── main.rs
```

### 3. 개선된 코드 구조

#### Before (과도한 추상화)
```rust
// 불필요한 Arc<dyn Trait>
pub struct UserService {
    auth_service: Arc<dyn AuthService>,
    user_redis: Arc<dyn UserRedisServiceTrait>,
    user_db: Arc<dyn UserDatabaseService>,
}

// 사용할 때마다 동적 디스패치
self.auth_service.login(&credentials).await?;
```

#### After (구체 타입 사용)
```rust
// 직접 구체 타입 사용
pub struct SocialAuthService {
    pool: MySqlPool,
    redis: RedisConnection,
    jwt_secret: String,
}

// 컴파일 타임에 타입 확정
impl SocialAuthService {
    pub async fn login(&self, provider: Provider, code: &str) -> Result<TokenPair> {
        // 직접 호출, 인라이닝 가능
    }
}
```

## OAuth 2.0 플로우

### 1. 인증 시작
```
Client → Server: /auth/social/start {provider: "google"}
Server → Client: {auth_url: "https://google.com/oauth...", state: "uuid"}
```

### 2. 사용자 인증
```
Client → OAuth Provider: 사용자 로그인
OAuth Provider → Client: code=xxx&state=uuid
```

### 3. 토큰 교환
```
Client → Server: /auth/social/callback {code: "xxx", state: "uuid"}
Server → OAuth Provider: Exchange code for token
OAuth Provider → Server: {access_token: "..."}
Server → OAuth Provider: Get user info
OAuth Provider → Server: {email: "user@gmail.com", ...}
```

### 4. JWT 발급
```
Server → DB: Create/Update user
Server → Redis: Store session
Server → Client: {access_token: "jwt...", refresh_token: "..."}
```

## 통합 서비스 사용법

### gRPC에서 사용
```rust
// grpcserver/src/handlers/auth_handler.rs
use shared::auth::SocialAuthService;

impl AuthHandler {
    async fn social_login(&self, req: SocialLoginRequest) -> Result<LoginResponse> {
        let auth = SocialAuthService::new(self.pool.clone());
        let tokens = auth.login(req.provider, &req.code).await?;
        
        Ok(LoginResponse {
            access_token: tokens.access,
            refresh_token: tokens.refresh,
        })
    }
}
```

### REST에서 사용
```rust
// gamecenter/src/api/auth_api.rs
use shared::auth::SocialAuthService;

async fn social_callback(
    Query(params): Query<CallbackParams>,
    State(pool): State<MySqlPool>,
) -> Result<Json<TokenResponse>> {
    let auth = SocialAuthService::new(pool);
    let tokens = auth.login(params.provider, &params.code).await?;
    
    Ok(Json(TokenResponse::from(tokens)))
}
```

## 성능 개선

### Before
- Arc<dyn Trait> 동적 디스패치: ~10ns 오버헤드
- 여러 서비스 간 호출: 복잡한 콜 스택
- 중복 Redis 연결: 연결 풀 낭비

### After  
- 구체 타입 직접 호출: 인라이닝 가능
- 단일 서비스: 간단한 콜 스택
- 공유 Redis 풀: 효율적인 연결 관리

## 보안 고려사항

1. **CSRF 보호**: state 토큰 검증
2. **토큰 만료**: Access 1시간, Refresh 30일
3. **HTTPS 필수**: 모든 OAuth 통신
4. **시크릿 관리**: 환경변수 사용

## 필요한 환경변수

```env
# OAuth Providers
GOOGLE_CLIENT_ID=xxx
GOOGLE_CLIENT_SECRET=xxx
KAKAO_CLIENT_ID=xxx
KAKAO_CLIENT_SECRET=xxx
APPLE_CLIENT_ID=xxx
APPLE_TEAM_ID=xxx
APPLE_KEY_ID=xxx

# JWT
JWT_SECRET=minimum_256_bits_key
JWT_EXPIRE_HOURS=1
JWT_REFRESH_DAYS=30

# Database
DATABASE_URL=mysql://user:pass@localhost/police

# Redis  
REDIS_URL=redis://localhost:6379
```

## 마이그레이션 계획

### Phase 1: 통합 서비스 생성
1. `shared/src/auth/` 디렉토리 생성
2. 핵심 OAuth 로직 통합
3. 테스트 작성

### Phase 2: gRPC 연동
1. gRPC 핸들러를 통합 서비스 사용으로 변경
2. 기존 미완성 코드 제거
3. 통합 테스트

### Phase 3: REST 연동
1. gamecenter의 중복 코드 제거
2. 통합 서비스 사용
3. E2E 테스트

### Phase 4: 정리
1. 불필요한 Arc<dyn Trait> 제거
2. 사용하지 않는 trait 삭제
3. 문서화