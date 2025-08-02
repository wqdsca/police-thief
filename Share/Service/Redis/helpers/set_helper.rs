// set_helper.rs
use redis::{AsyncCommands, Pipeline};
use serde_json;

use crate::share::comman::error::{AppError, AppResult};
use crate::share::service::redis::core::RedisConnection;

/// Set 대량 연산을 위한 Lua 스크립트
const SET_BATCH_OPERATIONS_LUA: &str = r#"
local key = KEYS[1]
local ttl = tonumber(ARGV[1])
local operation = ARGV[2]
local members = cjson.decode(ARGV[3])

local result = 0

if operation == "add" then
  for _, member in ipairs(members) do
    result = result + redis.call('SADD', key, member)
  end
elseif operation == "remove" then
  for _, member in ipairs(members) do
    result = result + redis.call('SREM', key, member)
  end
end

if ttl > 0 then
  redis.call('EXPIRE', key, ttl)
end

return result
"#;

/// Set 데이터 타입을 위한 헬퍼
#[derive(Clone)]
pub struct SetHelper {
    conn: RedisConnection,
    key: String,
    ttl: Option<u64>,
}

impl SetHelper {
    pub fn new(conn: RedisConnection, key: impl Into<String>, ttl: Option<u64>) -> Self {
        Self { conn, key: key.into(), ttl }
    }

    /// SADD key member
    pub async fn add_member(&self, member: &str) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let n: i64 = redis::cmd("SADD").arg(&self.key).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SADD")))?;
        if let Some(sec) = self.ttl { 
            let _: bool = conn.expire(&self.key, sec as usize).await
                .map_err(|e| AppError::redis(e.to_string(), Some("EXPIRE")))?; 
        }
        Ok(n)
    }

    /// SADD key member1 member2 member3... - Pipeline 사용
    pub async fn add_multiple_members(&self, members: &[&str]) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let mut pipe = redis::pipe();
        pipe.cmd("SADD").arg(&self.key);
        for member in members {
            pipe.arg(member);
        }
        let n: i64 = pipe.query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SADD_MULTIPLE")))?;
        if let Some(sec) = self.ttl { 
            let _: bool = conn.expire(&self.key, sec as usize).await
                .map_err(|e| AppError::redis(e.to_string(), Some("EXPIRE")))?; 
        }
        Ok(n)
    }

    /// 대량 멤버 추가 - Lua 스크립트 사용
    pub async fn batch_add_members(&self, members: &[String]) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let script = redis::Script::new(SET_BATCH_OPERATIONS_LUA);
        let members_json = serde_json::to_string(members)
            .map_err(|e| AppError::serialization(e.to_string(), Some("JSON")))?;
        let ttl = self.ttl.unwrap_or(0) as i64;
        
        let result: i64 = script
            .key(&self.key)
            .arg(ttl)
            .arg("add")
            .arg(members_json)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| AppError::redis(e.to_string(), Some("LUA_BATCH_ADD")))?;
        
        Ok(result)
    }

    /// SREM key member
    pub async fn remove_member(&self, member: &str) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let n: i64 = redis::cmd("SREM").arg(&self.key).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SREM")))?;
        Ok(n)
    }

    /// 대량 멤버 제거 - Lua 스크립트 사용
    pub async fn batch_remove_members(&self, members: &[String]) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let script = redis::Script::new(SET_BATCH_OPERATIONS_LUA);
        let members_json = serde_json::to_string(members)
            .map_err(|e| AppError::serialization(e.to_string(), Some("JSON")))?;
        let ttl = self.ttl.unwrap_or(0) as i64;
        
        let result: i64 = script
            .key(&self.key)
            .arg(ttl)
            .arg("remove")
            .arg(members_json)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| AppError::redis(e.to_string(), Some("LUA_BATCH_REMOVE")))?;
        
        Ok(result)
    }

    /// SISMEMBER key member
    pub async fn member_exists(&self, member: &str) -> AppResult<bool> {
        let mut conn = self.conn.clone();
        let exists: i64 = redis::cmd("SISMEMBER").arg(&self.key).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SISMEMBER")))?;
        Ok(exists == 1)
    }

    /// SMEMBERS key
    pub async fn get_all_members(&self) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let members: Vec<String> = redis::cmd("SMEMBERS").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SMEMBERS")))?;
        Ok(members)
    }

    /// SCARD key
    pub async fn get_member_count(&self) -> AppResult<usize> {
        let mut conn = self.conn.clone();
        let count: i64 = redis::cmd("SCARD").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SCARD")))?;
        Ok(count.max(0) as usize)
    }

    /// SPOP key [count]
    pub async fn pop_random_member(&self) -> AppResult<Option<String>> {
        let mut conn = self.conn.clone();
        let member: Option<String> = redis::cmd("SPOP").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SPOP")))?;
        Ok(member)
    }

    /// SPOP key count
    pub async fn pop_random_members(&self, count: usize) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let members: Vec<String> = redis::cmd("SPOP").arg(&self.key).arg(count).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SPOP_MULTIPLE")))?;
        Ok(members)
    }

    /// SRANDMEMBER key [count]
    pub async fn get_random_member(&self) -> AppResult<Option<String>> {
        let mut conn = self.conn.clone();
        let member: Option<String> = redis::cmd("SRANDMEMBER").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SRANDMEMBER")))?;
        Ok(member)
    }

    /// SRANDMEMBER key count
    pub async fn get_random_members(&self, count: usize) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let members: Vec<String> = redis::cmd("SRANDMEMBER").arg(&self.key).arg(count).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SRANDMEMBER_MULTIPLE")))?;
        Ok(members)
    }

    /// SUNION key1 key2 key3...
    pub async fn get_union(&self, other_keys: &[&str]) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let mut pipe = redis::pipe();
        pipe.cmd("SUNION").arg(&self.key);
        for key in other_keys {
            pipe.arg(key);
        }
        let members: Vec<String> = pipe.query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SUNION")))?;
        Ok(members)
    }

    /// SINTER key1 key2 key3...
    pub async fn get_intersection(&self, other_keys: &[&str]) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let mut pipe = redis::pipe();
        pipe.cmd("SINTER").arg(&self.key);
        for key in other_keys {
            pipe.arg(key);
        }
        let members: Vec<String> = pipe.query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SINTER")))?;
        Ok(members)
    }

    /// SDIFF key1 key2 key3...
    pub async fn get_difference(&self, other_keys: &[&str]) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let mut pipe = redis::pipe();
        pipe.cmd("SDIFF").arg(&self.key);
        for key in other_keys {
            pipe.arg(key);
        }
        let members: Vec<String> = pipe.query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SDIFF")))?;
        Ok(members)
    }

    /// DEL key
    pub async fn delete_set(&self) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let n: i64 = redis::cmd("DEL").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("DEL")))?;
        Ok(n)
    }
}
