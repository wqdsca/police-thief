// hash_helper.rs
use redis::{AsyncCommands, Pipeline};
use serde::{de::DeserializeOwned, Serialize};

use crate::share::comman::error::{AppError, AppResult};
use crate::share::service::redis::core::RedisConnection;

/// Hash 대량 연산을 위한 Lua 스크립트
const HASH_BATCH_UPDATE_LUA: &str = r#"
local key = KEYS[1]
local ttl = tonumber(ARGV[1])
local updates = cjson.decode(ARGV[2])

for field, value in pairs(updates) do
  redis.call('HSET', key, field, value)
end

if ttl > 0 then
  redis.call('EXPIRE', key, ttl)
end

return #updates
"#;

/// 운영용 Hash Helper:
/// - 필드 단건/배치 set/get
/// - struct 전체 JSON 저장/조회 편의 제공
#[derive(Clone)]
pub struct HashHelper {
    conn: RedisConnection,
    key: String,
    ttl: Option<u64>,
}

impl HashHelper {
    pub fn new(conn: RedisConnection, key: impl Into<String>, ttl: Option<u64>) -> Self {
        Self { conn, key: key.into(), ttl }
    }

    /// HSET field value
    pub async fn set_field(&self, field: &str, value: &str) -> AppResult<bool> {
        let mut conn = self.conn.clone();
        let n: i64 = conn.hset(&self.key, field, value).await
            .map_err(|e| AppError::redis(e.to_string(), Some("HSET")))?;
        if let Some(sec) = self.ttl { 
            let _: bool = conn.expire(&self.key, sec as usize).await
                .map_err(|e| AppError::redis(e.to_string(), Some("EXPIRE")))?; 
        }
        Ok(n == 1)
    }

    /// HGET field
    pub async fn get_field(&self, field: &str) -> AppResult<Option<String>> {
        let mut conn = self.conn.clone();
        let v: Option<String> = conn.hget(&self.key, field).await
            .map_err(|e| AppError::redis(e.to_string(), Some("HGET")))?;
        Ok(v)
    }

    /// HMSET (여러 필드) - Pipeline 사용
    pub async fn set_multiple_fields<T: AsRef<str>>(&self, pairs: &[(T, T)]) -> AppResult<()> {
        let mut conn = self.conn.clone();
        let mut pipe = redis::pipe();
        pipe.cmd("HMSET").arg(&self.key);
        for (k, v) in pairs {
            pipe.arg(k.as_ref()).arg(v.as_ref());
        }
        pipe.query_async::<_, ()>(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("HMSET")))?;
        if let Some(sec) = self.ttl { 
            let _: bool = conn.expire(&self.key, sec as usize).await
                .map_err(|e| AppError::redis(e.to_string(), Some("EXPIRE")))?; 
        }
        Ok(())
    }

    /// 대량 필드 업데이트 - Lua 스크립트 사용
    pub async fn batch_update_fields(&self, updates: &[(String, String)]) -> AppResult<usize> {
        let mut conn = self.conn.clone();
        let script = redis::Script::new(HASH_BATCH_UPDATE_LUA);
        let updates_json = serde_json::to_string(updates)
            .map_err(|e| AppError::serialization(e.to_string(), Some("JSON")))?;
        let ttl = self.ttl.unwrap_or(0) as i64;
        
        let result: i64 = script
            .key(&self.key)
            .arg(ttl)
            .arg(updates_json)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| AppError::redis(e.to_string(), Some("LUA_BATCH_UPDATE")))?;
        
        Ok(result as usize)
    }

    /// HGETALL
    pub async fn get_all_fields(&self) -> AppResult<Vec<(String, String)>> {
        let mut conn = self.conn.clone();
        let v: Vec<(String, String)> = redis::cmd("HGETALL").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("HGETALL")))?;
        Ok(v)
    }

    /// HINCRBY
    pub async fn increment_field(&self, field: &str, by: i64) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let v: i64 = redis::cmd("HINCRBY").arg(&self.key).arg(field).arg(by).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("HINCRBY")))?;
        if let Some(sec) = self.ttl { 
            let _: bool = conn.expire(&self.key, sec as usize).await
                .map_err(|e| AppError::redis(e.to_string(), Some("EXPIRE")))?; 
        }
        Ok(v)
    }

    /// HDEL field
    pub async fn delete_field(&self, field: &str) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let v: i64 = redis::cmd("HDEL").arg(&self.key).arg(field).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("HDEL")))?;
        Ok(v)
    }

    /// 다중 필드 삭제 - Pipeline 사용
    pub async fn delete_multiple_fields(&self, fields: &[&str]) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let mut pipe = redis::pipe();
        pipe.cmd("HDEL").arg(&self.key);
        for field in fields {
            pipe.arg(field);
        }
        let result: i64 = pipe.query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("HDEL_MULTIPLE")))?;
        Ok(result)
    }

    /// HEXISTS field
    pub async fn field_exists(&self, field: &str) -> AppResult<bool> {
        let mut conn = self.conn.clone();
        let v: i64 = redis::cmd("HEXISTS").arg(&self.key).arg(field).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("HEXISTS")))?;
        Ok(v == 1)
    }

    /// HLEN
    pub async fn get_field_count(&self) -> AppResult<usize> {
        let mut conn = self.conn.clone();
        let v: i64 = redis::cmd("HLEN").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("HLEN")))?;
        Ok(v.max(0) as usize)
    }

    // ---------- JSON 편의 ----------

    /// 키 자체에 JSON 통째로 저장 (Hash 대신 String) – 운영에서 자주 쓰는 패턴
    pub async fn set_json<T: Serialize>(&self, value: &T) -> AppResult<()> {
        let mut conn = self.conn.clone();
        let s = serde_json::to_string(value)
            .map_err(|e| AppError::serialization(e.to_string(), Some("JSON")))?;
        match self.ttl {
            Some(sec) => { 
                let _: () = redis::cmd("SET").arg(&self.key).arg(s).arg("EX").arg(sec).query_async(&mut conn).await
                    .map_err(|e| AppError::redis(e.to_string(), Some("SET")))?; 
            }
            None => { 
                let _: () = redis::cmd("SET").arg(&self.key).arg(s).query_async(&mut conn).await
                    .map_err(|e| AppError::redis(e.to_string(), Some("SET")))?; 
            }
        }
        Ok(())
    }

    pub async fn get_json<T: DeserializeOwned>(&self) -> AppResult<Option<T>> {
        let mut conn = self.conn.clone();
        let s: Option<String> = redis::cmd("GET").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("GET")))?;
        if let Some(raw) = s {
            Ok(Some(serde_json::from_str(&raw)
                .map_err(|e| AppError::serialization(e.to_string(), Some("JSON")))?))
        } else { 
            Ok(None) 
        }
    }
}
