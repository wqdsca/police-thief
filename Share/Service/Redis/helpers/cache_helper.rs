// cache_helper.rs
use redis::{aio::ConnectionManager, Script};
use serde::{de::DeserializeOwned, Serialize};
use tracing::{error, info, instrument};

use crate::share::comman::error::{AppError, AppResult};
use crate::share::service::redis::core::{retry_operation, RetryOptions, RETRY_OPT, RedisConnection};

/// LRU + 아이템 저장을 원자적으로 처리하는 Lua (SET/LREM/LPUSH/RPOP/EXPIRE)
const LRU_ATOMIC_ADD_LUA: &str = r#"
local item_key = KEYS[1]
local list_key = KEYS[2]
local value    = ARGV[1]
local id       = ARGV[2]
local limit    = tonumber(ARGV[3])
local ttl      = tonumber(ARGV[4])
local prefix   = ARGV[5]

if ttl > 0 then
  redis.call('SET', item_key, value, 'EX', ttl)
else
  redis.call('SET', item_key, value)
end

redis.call('LREM', list_key, 0, id)
redis.call('LPUSH', list_key, id)

local len = redis.call('LLEN', list_key)
if len > limit then
  local old_id = redis.call('RPOP', list_key)
  if old_id then
    redis.call('DEL', prefix .. ':' .. old_id)
  end
end

if ttl > 0 then
  redis.call('EXPIRE', list_key, ttl)
end

return 1
"#;

/// 운영용 캐시: JSON 아이템 저장 + 최근 목록(LRU) 유지
#[derive(Clone)]
pub struct CacheHelper {
    conn: RedisConnection,
    ttl: Option<u64>,  // 아이템/리스트 TTL(초)
    limit: usize,      // LRU 유지 개수
    item_prefix: String, // 아이템 키 프리픽스 (예: "room:list", "user")
    list_key: String,    // 리스트 키 (예: "room:list", "room:user", "room:list:time")
}

impl CacheHelper {
    /// 예: `CacheHelper::new(conn, Some(60), 50, "room:list", "room:list")`
    pub fn new(conn: RedisConnection, ttl: Option<u64>, limit: usize, item_prefix: impl Into<String>, list_key: impl Into<String>) -> Self {
        Self { conn, ttl, limit: limit.max(1), item_prefix: item_prefix.into(), list_key: list_key.into() }
    }

    /// JSON 저장 + LRU 갱신(원자)
    #[instrument(skip(self, value))]
    pub async fn setItem<T: Serialize>(&self, id: u32, value: &T) -> AppResult<()> {
        let item_key = format!("{}:{}", self.item_prefix, id);
        let list_key = self.list_key.clone();
        let raw = serde_json::to_string(value).map_err(|e| AppError::serialization(e.to_string(), Some("JSON")))?;
        let ttl = self.ttl.unwrap_or(0) as i64;
        let limit = self.limit as i64;
        let prefix = self.item_prefix.clone();

        retry_operation(
            || {
                let mut conn = self.conn.clone();
                let script = Script::new(LRU_ATOMIC_ADD_LUA);
                let id_str = id.to_string();
                async move {
                    script
                        .key(item_key.clone())
                        .key(list_key.clone())
                        .arg(raw.clone())
                        .arg(id_str.clone())
                        .arg(limit)
                        .arg(ttl)
                        .arg(prefix.clone())
                        .invoke_async::<_, i32>(&mut conn)
                        .await
                        .map_err(|e| AppError::redis(e.to_string(), Some("Lua LRU 실행")))
                        .map(|_| ())
                }
            },
            RETRY_OPT,
        ).await
    }

    /// 아이템 조회(JSON → T)
    pub async fn getItem<T: DeserializeOwned>(&self, id: u32) -> AppResult<Option<T>> {
        let mut conn = self.conn.clone();
        let key = format!("{}:{}", self.item_prefix, id);
        let s: Option<String> = redis::cmd("GET").arg(&key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("GET")))?;
        match s {
            Some(v) => Ok(Some(serde_json::from_str(&v).map_err(|e| AppError::serialization(e.to_string(), Some("JSON")))?)),
            None => Ok(None),
        }
    }

    /// 최근 목록(LRU) 조회: 상위 `count`개 id 반환
    pub async fn getRecentList(&self, count: usize) -> AppResult<Vec<u32>> {
        let mut conn = self.conn.clone();
        let end = (count.saturating_sub(1)) as isize;
        let vals: Vec<String> = redis::cmd("LRANGE")
            .arg(&self.list_key).arg(0).arg(end)
            .query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("LRANGE")))?;
        let mut out = Vec::with_capacity(vals.len());
        for v in vals {
            if let Ok(n) = v.parse::<u32>() { out.push(n); }
        }
        Ok(out)
    }

    /// 아이템/목록에서 제거 (아이템 키 삭제 + 리스트에서 ID 제거)
    pub async fn removeItem(&self, id: u32) -> AppResult<()> {
        let mut conn = self.conn.clone();
        let key = format!("{}:{}", self.item_prefix, id);
        redis::pipe()
            .cmd("DEL").arg(&key)
            .cmd("LREM").arg(&self.list_key).arg(0).arg(id.to_string())
            .query_async::<_, ()>(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("REMOVE")))
    }

    /// 전체 목록 조회
    pub async fn getAllItems(&self) -> AppResult<Vec<u32>> {
        let mut conn = self.conn.clone();
        let vals: Vec<String> = redis::cmd("LRANGE")
            .arg(&self.list_key).arg(0).arg(-1)
            .query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("LRANGE_ALL")))?;
        let mut out = Vec::with_capacity(vals.len());
        for v in vals {
            if let Ok(n) = v.parse::<u32>() { out.push(n); }
        }
        Ok(out)
    }

    /// 목록 크기 조회
    pub async fn getListSize(&self) -> AppResult<usize> {
        let mut conn = self.conn.clone();
        let size: i64 = redis::cmd("LLEN")
            .arg(&self.list_key)
            .query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("LLEN")))?;
        Ok(size.max(0) as usize)
    }
}
