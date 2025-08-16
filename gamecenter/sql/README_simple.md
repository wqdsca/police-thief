# Simple Police Thief Game Database

## 🎯 Features
**3가지 핵심 기능만 구현**
- 🔐 **로그인 기능**: 사용자 인증, JWT 토큰, 통계 (NULL 허용)
- 👥 **친구 기능**: 친구 추가/관리, 온라인 상태
- 💬 **채팅 기능**: 1:1 채팅, 그룹 채팅, 실시간 메시지

## 📊 Database Schema

### Core Tables (6개)
```sql
users              -- 사용자 정보 + 게임 통계 (NULL 허용)
user_tokens        -- JWT 토큰 관리
friendships        -- 친구 관계
online_status      -- 실시간 온라인 상태
chat_rooms         -- 채팅방
chat_participants  -- 채팅방 참가자
chat_messages      -- 채팅 메시지 (파티션)
```

## 🚀 Quick Start

### 1. Database Setup
```bash
# 데이터베이스 생성
mysql -u root -p < sql/schema_simple.sql

# 테스트 데이터 생성 (선택사항)
mysql -u root -p police_thief_simple -e "CALL sp_create_test_data();"
```

### 2. Connection Info
```
Database: police_thief_simple
User: game_simple / game_password_123
Read-only: game_readonly / readonly_password_123
```

### 3. Rust Connection
```rust
let database_url = "mysql://game_simple:game_password_123@localhost/police_thief_simple";
let pool = MySqlPoolOptions::new()
    .max_connections(50)
    .connect(&database_url).await?;
```

## 🔐 Login System Features

### User Stats (NULL 허용)
```sql
level INT UNSIGNED NULL             -- 레벨 (기본값 없음)
experience BIGINT UNSIGNED NULL     -- 경험치 (기본값 없음)  
total_games INT UNSIGNED NULL       -- 총 게임 수
win_count INT UNSIGNED NULL         -- 승리 횟수
lose_count INT UNSIGNED NULL        -- 패배 횟수
win_rate DECIMAL(5,2) GENERATED     -- 자동 계산 승률
```

### Key Queries
```sql
-- 로그인
SELECT user_id, nickname, level, win_count, lose_count, win_rate 
FROM users WHERE username=? AND password_hash=?;

-- 통계 업데이트
CALL sp_update_user_stats(user_id, is_winner, exp_gained);
```

## 👥 Friends System Features

### Friendship Model
- **Requester/Addressee**: 명확한 요청/수락 구조
- **Bidirectional View**: `v_friendships` 뷰로 양방향 처리
- **Status**: pending, accepted, blocked, declined

### Key Queries
```sql
-- 친구 요청 보내기
CALL sp_send_friend_request(requester_id, addressee_id, @result);

-- 온라인 친구 목록
SELECT * FROM v_online_friends WHERE user_id = ?;

-- 친구 검색
SELECT * FROM users WHERE nickname LIKE ? AND user_id != ?;
```

## 💬 Chat System Features

### Chat Types
- **Private**: 1:1 채팅
- **Group**: 그룹 채팅 
- **Public**: 공개 채팅

### Key Features
- 파티션된 메시지 테이블 (월별)
- 읽지 않은 메시지 카운트
- 메시지 답장 기능
- 실시간 온라인 상태

### Key Queries
```sql
-- 1:1 채팅방 생성/찾기
CALL sp_get_or_create_private_chat(user1_id, user2_id, @room_id);

-- 메시지 전송
INSERT INTO chat_messages (room_id, sender_id, message_content) VALUES (?, ?, ?);

-- 안읽은 메시지 수
SELECT COUNT(*) FROM chat_messages WHERE created_at > last_read_at;
```

## 📈 Performance Features

### Indexes
```sql
-- 로그인 최적화
idx_login_credentials (username, password_hash, status)

-- 친구 시스템 최적화  
idx_friend_requests (addressee_id, status, created_at)
idx_mutual_friends (requester_id, addressee_id, status)

-- 채팅 최적화
idx_recent_messages (room_id, created_at DESC, is_deleted)
idx_user_chat_rooms (user_id, is_active, room_id)
```

### Partitioning
```sql
-- 메시지 테이블 월별 파티션
PARTITION BY RANGE (UNIX_TIMESTAMP(created_at))
```

### Memory Tables
```sql
-- 온라인 상태는 메모리 테이블로 고속 처리
online_status ... ENGINE=MEMORY
```

## 🛠️ API Examples

### Rust Implementation Examples

#### 로그인
```rust
async fn login(pool: &MySqlPool, username: &str, password: &str) -> Result<User> {
    let user = sqlx::query_as!(
        User,
        "SELECT user_id, username, nickname, avatar_url, 
                COALESCE(level, 1) as level,
                COALESCE(total_games, 0) as total_games,
                COALESCE(win_count, 0) as win_count,
                COALESCE(lose_count, 0) as lose_count,
                COALESCE(win_rate, 0.00) as win_rate
         FROM users 
         WHERE username = ? AND password_hash = ? AND status = 'active'",
        username, password
    ).fetch_one(pool).await?;
    
    Ok(user)
}
```

#### 친구 목록
```rust
async fn get_friends(pool: &MySqlPool, user_id: i64) -> Result<Vec<Friend>> {
    let friends = sqlx::query_as!(
        Friend,
        "SELECT f.friend_id, u.nickname, u.avatar_url,
                COALESCE(u.level, 1) as level,
                COALESCE(os.status, 'offline') as online_status
         FROM v_friendships f
         INNER JOIN users u ON f.friend_id = u.user_id  
         LEFT JOIN online_status os ON f.friend_id = os.user_id
         WHERE f.user_id = ?",
        user_id
    ).fetch_all(pool).await?;
    
    Ok(friends)
}
```

#### 채팅 메시지
```rust
async fn send_message(pool: &MySqlPool, room_id: i64, sender_id: i64, content: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO chat_messages (room_id, sender_id, message_content) VALUES (?, ?, ?)",
        room_id, sender_id, content
    ).execute(pool).await?;
    
    Ok(())
}
```

## 📊 Database Size Estimates

### Expected Usage
- **Users**: ~10K users
- **Messages**: ~1M messages/month  
- **Friendships**: ~50K relationships

### Storage Requirements
- **Total DB Size**: ~500MB/year
- **Message Partition**: ~50MB/month
- **Index Overhead**: ~30%

## 🔧 Maintenance

### Automated Tasks
```sql
-- 토큰 정리 (매시간)
cleanup_expired_tokens

-- 오프라인 상태 업데이트 (5분마다)  
update_offline_status

-- 오래된 메시지 정리 (매주)
cleanup_old_messages
```

### Manual Tasks
- 파티션 추가 (매월)
- 통계 분석 (필요시)
- 인덱스 최적화 (분기별)

## 🧪 Testing

### Test Data Creation
```sql
CALL sp_create_test_data();  -- 10명 테스트 사용자 + 친구관계 생성
```

### Performance Testing
```sql
-- 테이블 크기 확인
SELECT table_name, ROUND(data_length/1024/1024,2) as data_mb 
FROM information_schema.tables 
WHERE table_schema='police_thief_simple';

-- 활성 사용자 통계
SELECT COUNT(*) FROM users WHERE status='active';
SELECT COUNT(*) FROM friendships WHERE status='accepted';  
SELECT COUNT(*) FROM chat_messages WHERE created_at > DATE_SUB(NOW(), INTERVAL 1 DAY);
```

## 🔒 Security Features

### Data Protection
- 암호화된 패스워드 (password_hash)
- JWT 토큰 관리
- SQL Injection 방지 (Prepared Statements)

### Access Control  
- 애플리케이션 전용 사용자
- 읽기 전용 사용자 분리
- 외래키 제약으로 데이터 무결성

## 🚀 Next Steps

1. **Unity Client 연동**
   - gRPC API 개발
   - 실시간 알림 구현

2. **Redis 캐싱 추가**
   - 온라인 상태 캐싱
   - 세션 관리

3. **확장성 개선**  
   - 읽기 전용 복제본
   - 샤딩 전략

---
**Simple하지만 Production-Ready한 데이터베이스!** 🎉