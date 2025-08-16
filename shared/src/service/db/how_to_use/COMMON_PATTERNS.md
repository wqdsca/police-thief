# 일반적인 사용 패턴

실제 프로젝트에서 자주 사용되는 Enhanced Database Service 패턴 모음입니다.

## 목차
1. [사용자 관리 패턴](#사용자-관리-패턴)
2. [게임 방 관리 패턴](#게임-방-관리-패턴)
3. [채팅 시스템 패턴](#채팅-시스템-패턴)
4. [통계 및 분석 패턴](#통계-및-분석-패턴)
5. [캐싱 패턴](#캐싱-패턴)
6. [배치 처리 패턴](#배치-처리-패턴)
7. [에러 처리 패턴](#에러-처리-패턴)

## 사용자 관리 패턴

### 사용자 등록 with 중복 체크

```rust
use shared::service::db::{EnhancedBaseDbService, IdStrategy};
use shared::tool::error::AppError;
use std::collections::HashMap;

async fn register_user(
    db: &impl EnhancedBaseDbService,
    email: &str,
    username: &str,
    password_hash: &str,
) -> Result<String, AppError> {
    // 1. 이메일 중복 체크
    let mut params = HashMap::new();
    params.insert("email".to_string(), serde_json::json!(email));
    
    if db.exists("users", "email = ?", Some(params)).await? {
        return Err(AppError::InvalidInput("이메일이 이미 사용 중입니다".to_string()));
    }
    
    // 2. 사용자명 중복 체크
    let mut params = HashMap::new();
    params.insert("username".to_string(), serde_json::json!(username));
    
    if db.exists("users", "username = ?", Some(params)).await? {
        return Err(AppError::InvalidInput("사용자명이 이미 사용 중입니다".to_string()));
    }
    
    // 3. UUID로 사용자 ID 생성
    let user_id = db.generate_id(&IdStrategy::Uuid).await?;
    
    // 4. 트랜잭션으로 사용자 생성
    let result = db.with_transaction(|tx| {
        let user_id = user_id.clone();
        Box::pin(async move {
            // 사용자 생성
            sqlx::query(
                "INSERT INTO users (id, email, username, password_hash, created_at) 
                 VALUES (?, ?, ?, ?, NOW())"
            )
            .bind(&user_id)
            .bind(email)
            .bind(username)
            .bind(password_hash)
            .execute(&mut **tx)
            .await?;
            
            // 프로필 생성
            sqlx::query(
                "INSERT INTO user_profiles (user_id, display_name) VALUES (?, ?)"
            )
            .bind(&user_id)
            .bind(username)
            .execute(&mut **tx)
            .await?;
            
            // 통계 초기화
            sqlx::query(
                "INSERT INTO user_stats (user_id, games_played, games_won) VALUES (?, 0, 0)"
            )
            .bind(&user_id)
            .execute(&mut **tx)
            .await?;
            
            Ok(user_id)
        })
    }).await?;
    
    Ok(result)
}
```

### 로그인 with 실패 횟수 추적

```rust
async fn login_user(
    db: &impl EnhancedBaseDbService,
    email: &str,
    password: &str,
) -> Result<LoginResult, AppError> {
    // 1. 사용자 조회
    let mut params = HashMap::new();
    params.insert("email".to_string(), serde_json::json!(email));
    
    let user = db.get_one("users", "email = ? AND deleted_at IS NULL", Some(params.clone()))
        .await?
        .ok_or_else(|| AppError::NotFound("사용자를 찾을 수 없습니다".to_string()))?;
    
    let user_id = user.get("id").unwrap().as_str().unwrap();
    let stored_hash = user.get("password_hash").unwrap().as_str().unwrap();
    let failed_attempts = user.get("failed_login_attempts").unwrap().as_i64().unwrap_or(0);
    
    // 2. 계정 잠금 확인
    if failed_attempts >= 5 {
        let locked_until = user.get("locked_until").and_then(|v| v.as_str());
        if let Some(locked_time) = locked_until {
            // 잠금 시간 확인
            return Err(AppError::Unauthorized("계정이 일시적으로 잠겼습니다".to_string()));
        }
    }
    
    // 3. 비밀번호 검증
    if !verify_password(password, stored_hash) {
        // 실패 횟수 증가
        let mut update = HashMap::new();
        update.insert("failed_login_attempts".to_string(), 
            serde_json::json!(failed_attempts + 1));
        
        if failed_attempts + 1 >= 5 {
            // 계정 잠금 (30분)
            update.insert("locked_until".to_string(), 
                serde_json::json!(chrono::Utc::now() + chrono::Duration::minutes(30)));
        }
        
        db.update("users", update, "id = ?", Some({
            let mut p = HashMap::new();
            p.insert("id".to_string(), serde_json::json!(user_id));
            p
        })).await?;
        
        return Err(AppError::Unauthorized("비밀번호가 일치하지 않습니다".to_string()));
    }
    
    // 4. 로그인 성공 - 실패 횟수 초기화, 마지막 로그인 시간 업데이트
    let mut update = HashMap::new();
    update.insert("failed_login_attempts".to_string(), serde_json::json!(0));
    update.insert("last_login_at".to_string(), 
        serde_json::json!(chrono::Utc::now()));
    
    db.update("users", update, "id = ?", Some({
        let mut p = HashMap::new();
        p.insert("id".to_string(), serde_json::json!(user_id));
        p
    })).await?;
    
    // 5. 로그인 기록 생성
    let mut login_log = HashMap::new();
    login_log.insert("user_id".to_string(), serde_json::json!(user_id));
    login_log.insert("ip_address".to_string(), serde_json::json!(client_ip));
    login_log.insert("user_agent".to_string(), serde_json::json!(user_agent));
    
    db.insert("login_logs", login_log).await?;
    
    Ok(LoginResult {
        user_id: user_id.to_string(),
        username: user.get("username").unwrap().as_str().unwrap().to_string(),
        // ... 기타 필드
    })
}
```

## 게임 방 관리 패턴

### 방 생성 with Snowflake ID

```rust
async fn create_game_room(
    db: &impl EnhancedBaseDbService,
    creator_id: &str,
    room_name: &str,
    max_players: i32,
) -> Result<String, AppError> {
    // 1. Snowflake ID로 방 ID 생성 (시간순 정렬 가능)
    let room_id = db.generate_id(&IdStrategy::Snowflake {
        machine_id: 1,
        datacenter_id: 1,
    }).await?;
    
    // 2. 친숙한 방 코드 생성
    let room_code = db.generate_id(&IdStrategy::PrefixedSequence {
        prefix: "ROOM-".to_string(),
        padding: 4,
    }).await?;
    
    // 3. 트랜잭션으로 방 생성
    db.with_transaction(|tx| {
        let room_id = room_id.clone();
        let room_code = room_code.clone();
        Box::pin(async move {
            // 방 생성
            sqlx::query(
                "INSERT INTO game_rooms (id, code, name, creator_id, max_players, status) 
                 VALUES (?, ?, ?, ?, ?, 'waiting')"
            )
            .bind(&room_id)
            .bind(&room_code)
            .bind(room_name)
            .bind(creator_id)
            .bind(max_players)
            .execute(&mut **tx)
            .await?;
            
            // 방장을 첫 번째 플레이어로 추가
            sqlx::query(
                "INSERT INTO room_players (room_id, player_id, is_host, joined_at) 
                 VALUES (?, ?, true, NOW())"
            )
            .bind(&room_id)
            .bind(creator_id)
            .execute(&mut **tx)
            .await?;
            
            Ok(room_code)
        })
    }).await
}
```

### 방 참여 with 동시성 제어

```rust
async fn join_game_room(
    db: &impl EnhancedBaseDbService,
    room_code: &str,
    player_id: &str,
) -> Result<(), AppError> {
    // 트랜잭션으로 동시성 제어
    db.with_transaction(|tx| {
        Box::pin(async move {
            // 1. 방 정보 조회 (FOR UPDATE로 락)
            let room = sqlx::query(
                "SELECT id, max_players, status FROM game_rooms 
                 WHERE code = ? AND status = 'waiting' FOR UPDATE"
            )
            .bind(room_code)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| AppError::NotFound("방을 찾을 수 없습니다".to_string()))?;
            
            let room_id: String = room.get("id");
            let max_players: i32 = room.get("max_players");
            
            // 2. 현재 플레이어 수 확인
            let count_row = sqlx::query(
                "SELECT COUNT(*) as count FROM room_players WHERE room_id = ?"
            )
            .bind(&room_id)
            .fetch_one(&mut **tx)
            .await?;
            
            let current_players: i64 = count_row.get("count");
            
            if current_players >= max_players as i64 {
                return Err(AppError::InvalidInput("방이 가득 찼습니다".to_string()));
            }
            
            // 3. 중복 참여 확인
            let exists = sqlx::query(
                "SELECT 1 FROM room_players WHERE room_id = ? AND player_id = ?"
            )
            .bind(&room_id)
            .bind(player_id)
            .fetch_optional(&mut **tx)
            .await?;
            
            if exists.is_some() {
                return Err(AppError::InvalidInput("이미 참여한 방입니다".to_string()));
            }
            
            // 4. 플레이어 추가
            sqlx::query(
                "INSERT INTO room_players (room_id, player_id, is_host, joined_at) 
                 VALUES (?, ?, false, NOW())"
            )
            .bind(&room_id)
            .bind(player_id)
            .execute(&mut **tx)
            .await?;
            
            // 5. 방이 가득 찼으면 상태 변경
            if current_players + 1 == max_players as i64 {
                sqlx::query("UPDATE game_rooms SET status = 'ready' WHERE id = ?")
                    .bind(&room_id)
                    .execute(&mut **tx)
                    .await?;
            }
            
            Ok(())
        })
    }).await
}
```

## 채팅 시스템 패턴

### 메시지 저장 with 페이지네이션

```rust
use shared::service::db::{PaginationParams, SortOrder};

async fn save_chat_message(
    db: &impl EnhancedBaseDbService,
    room_id: &str,
    sender_id: &str,
    content: &str,
) -> Result<String, AppError> {
    // Snowflake ID로 시간순 정렬 가능
    let message_id = db.generate_id(&IdStrategy::Snowflake {
        machine_id: 2,  // 채팅 서버용
        datacenter_id: 1,
    }).await?;
    
    let mut message = HashMap::new();
    message.insert("id".to_string(), serde_json::json!(message_id));
    message.insert("room_id".to_string(), serde_json::json!(room_id));
    message.insert("sender_id".to_string(), serde_json::json!(sender_id));
    message.insert("content".to_string(), serde_json::json!(content));
    message.insert("created_at".to_string(), serde_json::json!(chrono::Utc::now()));
    
    db.insert("chat_messages", message).await?;
    
    Ok(message_id)
}

async fn get_chat_history(
    db: &impl EnhancedBaseDbService,
    room_id: &str,
    page: u64,
    page_size: u64,
) -> Result<PaginationResult, AppError> {
    let mut params = HashMap::new();
    params.insert("room_id".to_string(), serde_json::json!(room_id));
    
    // Snowflake ID는 시간순이므로 ID로 정렬
    db.paginate(
        "chat_messages",
        PaginationParams {
            page,
            page_size,
            sort_by: Some("id".to_string()),  // Snowflake ID
            sort_order: Some(SortOrder::Desc), // 최신 메시지 먼저
        },
        Some("room_id = ?"),
        Some(params),
    ).await
}
```

### 메시지 검색

```rust
use shared::service::db::{SearchParams, SearchMatchType};

async fn search_messages(
    db: &impl EnhancedBaseDbService,
    keyword: &str,
    room_id: Option<&str>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let search = SearchParams {
        keyword: keyword.to_string(),
        fields: vec!["content".to_string()],
        match_type: SearchMatchType::Contains,
        case_sensitive: false,
    };
    
    let where_clause = room_id.map(|_| "room_id = ?");
    let params = room_id.map(|rid| {
        let mut p = HashMap::new();
        p.insert("room_id".to_string(), serde_json::json!(rid));
        p
    });
    
    let result = db.search_with_filter(
        "chat_messages",
        search,
        where_clause,
        params,
        Some(PaginationParams {
            page: 1,
            page_size: 100,
            sort_by: Some("created_at".to_string()),
            sort_order: Some(SortOrder::Desc),
        }),
    ).await?;
    
    Ok(result.data)
}
```

## 통계 및 분석 패턴

### 실시간 통계 집계

```rust
async fn get_game_statistics(
    db: &impl EnhancedBaseDbService,
    player_id: &str,
) -> Result<PlayerStats, AppError> {
    let mut params = HashMap::new();
    params.insert("player_id".to_string(), serde_json::json!(player_id));
    
    // 총 게임 수
    let total_games = db.count(
        "game_history",
        Some("player_id = ?"),
        Some(params.clone())
    ).await?;
    
    // 승리 수
    let wins = db.count(
        "game_history",
        Some("player_id = ? AND result = 'win'"),
        Some(params.clone())
    ).await?;
    
    // 평균 점수
    let avg_score = db.avg(
        "game_history",
        "score",
        Some("player_id = ?"),
        Some(params.clone())
    ).await?;
    
    // 최고 점수
    let max_score: Option<f64> = db.max(
        "game_history",
        "score",
        Some("player_id = ?"),
        Some(params.clone())
    ).await?;
    
    // 총 플레이 시간 (초)
    let total_playtime = db.sum(
        "game_history",
        "duration_seconds",
        Some("player_id = ?"),
        Some(params.clone())
    ).await?;
    
    Ok(PlayerStats {
        total_games,
        wins,
        win_rate: if total_games > 0 { 
            (wins as f64 / total_games as f64) * 100.0 
        } else { 
            0.0 
        },
        avg_score,
        max_score: max_score.unwrap_or(0.0),
        total_playtime_hours: total_playtime / 3600.0,
    })
}
```

### 리더보드 with 캐싱

```rust
async fn get_leaderboard(
    db: &impl EnhancedBaseDbService,
    metric: &str,  // "score", "wins", "playtime"
    limit: u64,
) -> Result<Vec<LeaderboardEntry>, AppError> {
    // 복잡한 쿼리는 직접 실행
    let sql = match metric {
        "score" => {
            "SELECT u.id, u.username, u.avatar, 
                    MAX(gh.score) as value,
                    RANK() OVER (ORDER BY MAX(gh.score) DESC) as rank
             FROM users u
             JOIN game_history gh ON u.id = gh.player_id
             WHERE u.deleted_at IS NULL
             GROUP BY u.id, u.username, u.avatar
             ORDER BY value DESC
             LIMIT ?"
        },
        "wins" => {
            "SELECT u.id, u.username, u.avatar,
                    COUNT(CASE WHEN gh.result = 'win' THEN 1 END) as value,
                    RANK() OVER (ORDER BY COUNT(CASE WHEN gh.result = 'win' THEN 1 END) DESC) as rank
             FROM users u
             JOIN game_history gh ON u.id = gh.player_id
             WHERE u.deleted_at IS NULL
             GROUP BY u.id, u.username, u.avatar
             ORDER BY value DESC
             LIMIT ?"
        },
        _ => return Err(AppError::InvalidInput("Invalid metric".to_string())),
    };
    
    let mut params = HashMap::new();
    params.insert("limit".to_string(), serde_json::json!(limit));
    
    let results = db.execute_query(sql, Some(params)).await?;
    
    // 결과 매핑
    let leaderboard: Vec<LeaderboardEntry> = results.iter().map(|row| {
        LeaderboardEntry {
            rank: row.get("rank").unwrap().as_i64().unwrap() as u32,
            player_id: row.get("id").unwrap().as_str().unwrap().to_string(),
            username: row.get("username").unwrap().as_str().unwrap().to_string(),
            avatar: row.get("avatar").and_then(|v| v.as_str()).map(String::from),
            value: row.get("value").unwrap().as_f64().unwrap(),
        }
    }).collect();
    
    Ok(leaderboard)
}
```

## 캐싱 패턴

### Read-Through 캐시

```rust
use std::sync::Arc;
use std::time::Duration;

struct CachedDbService {
    db: Arc<dyn EnhancedBaseDbService>,
    cache: Arc<dyn CacheService>,
}

impl CachedDbService {
    async fn get_user_cached(
        &self,
        user_id: &str,
    ) -> Result<Option<serde_json::Value>, AppError> {
        let cache_key = format!("user:{}", user_id);
        
        // 1. 캐시 확인
        if let Some(cached) = self.cache.get(&cache_key).await? {
            return Ok(Some(cached));
        }
        
        // 2. DB에서 조회
        let user = self.db.get_by_id("users", "id", user_id).await?;
        
        // 3. 캐시 저장 (1시간)
        if let Some(ref user_data) = user {
            self.cache.set(&cache_key, user_data, Duration::from_secs(3600)).await?;
        }
        
        Ok(user)
    }
    
    async fn update_user_cached(
        &self,
        user_id: &str,
        updates: HashMap<String, serde_json::Value>,
    ) -> Result<u64, AppError> {
        // 1. DB 업데이트
        let mut params = HashMap::new();
        params.insert("id".to_string(), serde_json::json!(user_id));
        
        let affected = self.db.update(
            "users",
            updates,
            "id = ?",
            Some(params)
        ).await?;
        
        // 2. 캐시 무효화
        let cache_key = format!("user:{}", user_id);
        self.cache.delete(&cache_key).await?;
        
        Ok(affected)
    }
}
```

## 배치 처리 패턴

### 대량 데이터 임포트

```rust
async fn import_users_batch(
    db: &impl EnhancedBaseDbService,
    csv_data: Vec<UserCsvRow>,
) -> Result<BulkOperationResult, AppError> {
    // 1000개씩 배치 처리
    const BATCH_SIZE: usize = 1000;
    let mut total_result = BulkOperationResult::default();
    
    for chunk in csv_data.chunks(BATCH_SIZE) {
        // 데이터 변환
        let users: Vec<HashMap<String, serde_json::Value>> = chunk.iter().map(|row| {
            let mut user = HashMap::new();
            user.insert("email".to_string(), serde_json::json!(row.email));
            user.insert("username".to_string(), serde_json::json!(row.username));
            user.insert("full_name".to_string(), serde_json::json!(row.full_name));
            user
        }).collect();
        
        // 배치 upsert
        let result = db.bulk_upsert(
            "users",
            users,
            vec!["email".to_string()]  // 이메일로 중복 체크
        ).await?;
        
        total_result.total += result.total;
        total_result.success += result.success;
        total_result.failed += result.failed;
        total_result.errors.extend(result.errors);
        
        // 진행 상황 로깅
        println!("Processed {} / {} users", total_result.total, csv_data.len());
    }
    
    Ok(total_result)
}
```

### 정기 정리 작업

```rust
async fn cleanup_old_data(
    db: &impl EnhancedBaseDbService,
) -> Result<(), AppError> {
    // 1. 오래된 로그인 기록 삭제
    let deleted_logs = db.delete(
        "login_logs",
        "created_at < DATE_SUB(NOW(), INTERVAL 90 DAY)",
        None
    ).await?;
    println!("Deleted {} old login logs", deleted_logs);
    
    // 2. 비활성 계정 soft delete
    let soft_deleted = db.soft_delete(
        "users",
        "last_login_at < DATE_SUB(NOW(), INTERVAL 1 YEAR) AND status = 'inactive'",
        None,
        "deleted_at"
    ).await?;
    println!("Soft deleted {} inactive users", soft_deleted);
    
    // 3. 임시 데이터 정리
    db.truncate("temp_calculations").await?;
    
    // 4. 테이블 최적화
    for table in &["users", "game_history", "chat_messages"] {
        db.optimize_table(table).await?;
        println!("Optimized table: {}", table);
    }
    
    Ok(())
}
```

## 에러 처리 패턴

### 상세한 에러 처리

```rust
use shared::tool::error::AppError;

async fn safe_database_operation(
    db: &impl EnhancedBaseDbService,
) -> Result<(), AppError> {
    match perform_operation(db).await {
        Ok(result) => Ok(result),
        Err(e) => {
            match e {
                AppError::DatabaseConnection(msg) => {
                    // 연결 재시도
                    eprintln!("DB 연결 실패: {}", msg);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    perform_operation(db).await
                },
                AppError::DatabaseQuery(msg) if msg.contains("Duplicate entry") => {
                    // 중복 에러 처리
                    Err(AppError::InvalidInput("이미 존재하는 데이터입니다".to_string()))
                },
                AppError::DatabaseQuery(msg) if msg.contains("Lock wait timeout") => {
                    // 데드락 처리
                    eprintln!("데드락 감지: {}", msg);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    perform_operation(db).await
                },
                _ => Err(e),
            }
        }
    }
}
```

### 트랜잭션 재시도 패턴

```rust
async fn retry_transaction<T, F>(
    db: &impl EnhancedBaseDbService,
    operation: F,
    max_retries: u32,
) -> Result<T, AppError>
where
    F: Fn() -> Pin<Box<dyn Future<Output = Result<T, AppError>> + Send>>,
{
    let mut attempts = 0;
    
    loop {
        attempts += 1;
        
        match db.with_transaction(|tx| operation()).await {
            Ok(result) => return Ok(result),
            Err(e) if attempts < max_retries => {
                match e {
                    AppError::DatabaseQuery(msg) 
                        if msg.contains("Deadlock") || msg.contains("Lock wait timeout") => {
                        // 데드락이면 재시도
                        let delay = Duration::from_millis(100 * attempts as u64);
                        tokio::time::sleep(delay).await;
                        continue;
                    },
                    _ => return Err(e),
                }
            },
            Err(e) => return Err(e),
        }
    }
}
```

## 모니터링 패턴

### 쿼리 성능 추적

```rust
use std::time::Instant;

struct MonitoredDbService {
    db: Arc<dyn EnhancedBaseDbService>,
    metrics: Arc<dyn MetricsCollector>,
}

impl MonitoredDbService {
    async fn monitored_query<T>(
        &self,
        operation_name: &str,
        operation: impl Future<Output = Result<T, AppError>>,
    ) -> Result<T, AppError> {
        let start = Instant::now();
        
        let result = operation.await;
        
        let duration = start.elapsed();
        
        // 메트릭 수집
        self.metrics.record_duration(operation_name, duration);
        
        // 느린 쿼리 경고
        if duration > Duration::from_secs(1) {
            eprintln!("Slow query detected: {} took {:?}", operation_name, duration);
        }
        
        match &result {
            Ok(_) => self.metrics.increment_success(operation_name),
            Err(_) => self.metrics.increment_failure(operation_name),
        }
        
        result
    }
}
```

## 최적화 팁

### 1. N+1 쿼리 방지
```rust
// ❌ 나쁜 예: N+1 쿼리
let users = db.select("users", None, None).await?;
for user in users {
    let user_id = user.get("id").unwrap();
    let orders = db.select("orders", Some("user_id = ?"), Some(params)).await?;
}

// ✅ 좋은 예: JOIN 사용
let sql = "SELECT u.*, o.* FROM users u 
           LEFT JOIN orders o ON u.id = o.user_id";
let results = db.execute_query(sql, None).await?;
```

### 2. 인덱스 활용
```sql
-- 자주 검색하는 컬럼에 인덱스 생성
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_status_created ON users(status, created_at);
CREATE INDEX idx_messages_room_created ON chat_messages(room_id, created_at);
```

### 3. 커넥션 풀 튜닝
```env
# .env 파일
db_max_connections=100  # 최대 연결 수
db_min_connections=10   # 최소 연결 수
db_connect_timeout=30   # 연결 타임아웃 (초)
db_idle_timeout=600     # 유휴 연결 타임아웃 (초)
```