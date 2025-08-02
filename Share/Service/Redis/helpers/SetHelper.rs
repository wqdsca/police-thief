// set_helper.rs
use redis::AsyncCommands;

use crate::Share::Comman::error::{AppError, AppResult};
use crate::Share::Service::Redis::core::RedisConnection;

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
    pub async fn addMember(&self, member: &str) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let n: i64 = redis::cmd("SADD").arg(&self.key).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SADD")))?;
        if let Some(sec) = self.ttl { 
            let _: bool = conn.expire(&self.key, sec as usize).await
                .map_err(|e| AppError::redis(e.to_string(), Some("EXPIRE")))?; 
        }
        Ok(n)
    }

    /// SADD key member1 member2 member3...
    pub async fn addMultipleMembers(&self, members: &[&str]) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let mut cmd = redis::cmd("SADD");
        cmd.arg(&self.key);
        for member in members {
            cmd.arg(member);
        }
        let n: i64 = cmd.query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SADD_MULTIPLE")))?;
        if let Some(sec) = self.ttl { 
            let _: bool = conn.expire(&self.key, sec as usize).await
                .map_err(|e| AppError::redis(e.to_string(), Some("EXPIRE")))?; 
        }
        Ok(n)
    }

    /// SREM key member
    pub async fn removeMember(&self, member: &str) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let n: i64 = redis::cmd("SREM").arg(&self.key).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SREM")))?;
        Ok(n)
    }

    /// SISMEMBER key member
    pub async fn memberExists(&self, member: &str) -> AppResult<bool> {
        let mut conn = self.conn.clone();
        let exists: i64 = redis::cmd("SISMEMBER").arg(&self.key).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SISMEMBER")))?;
        Ok(exists == 1)
    }

    /// SMEMBERS key
    pub async fn getAllMembers(&self) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let members: Vec<String> = redis::cmd("SMEMBERS").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SMEMBERS")))?;
        Ok(members)
    }

    /// SCARD key
    pub async fn getMemberCount(&self) -> AppResult<usize> {
        let mut conn = self.conn.clone();
        let count: i64 = redis::cmd("SCARD").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SCARD")))?;
        Ok(count.max(0) as usize)
    }

    /// SPOP key [count]
    pub async fn popRandomMember(&self) -> AppResult<Option<String>> {
        let mut conn = self.conn.clone();
        let member: Option<String> = redis::cmd("SPOP").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SPOP")))?;
        Ok(member)
    }

    /// SPOP key count
    pub async fn popRandomMembers(&self, count: usize) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let members: Vec<String> = redis::cmd("SPOP").arg(&self.key).arg(count).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SPOP_MULTIPLE")))?;
        Ok(members)
    }

    /// SRANDMEMBER key [count]
    pub async fn getRandomMember(&self) -> AppResult<Option<String>> {
        let mut conn = self.conn.clone();
        let member: Option<String> = redis::cmd("SRANDMEMBER").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SRANDMEMBER")))?;
        Ok(member)
    }

    /// SRANDMEMBER key count
    pub async fn getRandomMembers(&self, count: usize) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let members: Vec<String> = redis::cmd("SRANDMEMBER").arg(&self.key).arg(count).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SRANDMEMBER_MULTIPLE")))?;
        Ok(members)
    }

    /// SUNION key1 key2 key3...
    pub async fn getUnion(&self, other_keys: &[&str]) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let mut cmd = redis::cmd("SUNION");
        cmd.arg(&self.key);
        for key in other_keys {
            cmd.arg(key);
        }
        let members: Vec<String> = cmd.query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SUNION")))?;
        Ok(members)
    }

    /// SINTER key1 key2 key3...
    pub async fn getIntersection(&self, other_keys: &[&str]) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let mut cmd = redis::cmd("SINTER");
        cmd.arg(&self.key);
        for key in other_keys {
            cmd.arg(key);
        }
        let members: Vec<String> = cmd.query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SINTER")))?;
        Ok(members)
    }

    /// SDIFF key1 key2 key3...
    pub async fn getDifference(&self, other_keys: &[&str]) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let mut cmd = redis::cmd("SDIFF");
        cmd.arg(&self.key);
        for key in other_keys {
            cmd.arg(key);
        }
        let members: Vec<String> = cmd.query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("SDIFF")))?;
        Ok(members)
    }

    /// DEL key
    pub async fn deleteSet(&self) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let n: i64 = redis::cmd("DEL").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("DEL")))?;
        Ok(n)
    }
}
