// zset_helper.rs
use redis::AsyncCommands;

use crate::share::comman::error::{AppError, AppResult};
use crate::share::service::redis::core::RedisConnection;

/// 점수 기반 랭킹/정렬에 사용
#[derive(Clone)]
pub struct ZSetHelper {
    conn: RedisConnection,
    key: String,
    ttl: Option<u64>,
}

impl ZSetHelper {
    pub fn new(conn: RedisConnection, key: impl Into<String>, ttl: Option<u64>) -> Self {
        Self { conn, key: key.into(), ttl }
    }

    /// ZADD key score member
    pub async fn addMember(&self, member: &str, score: f64) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let n: i64 = redis::cmd("ZADD").arg(&self.key).arg(score).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("ZADD")))?;
        if let Some(sec) = self.ttl { 
            let _: bool = conn.expire(&self.key, sec as usize).await
                .map_err(|e| AppError::redis(e.to_string(), Some("EXPIRE")))?; 
        }
        Ok(n)
    }

    pub async fn removeMember(&self, member: &str) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let n: i64 = redis::cmd("ZREM").arg(&self.key).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("ZREM")))?;
        Ok(n)
    }

    pub async fn incrementScore(&self, member: &str, by: f64) -> AppResult<f64> {
        let mut conn = self.conn.clone();
        let s: f64 = redis::cmd("ZINCRBY").arg(&self.key).arg(by).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("ZINCRBY")))?;
        if let Some(sec) = self.ttl { 
            let _: bool = conn.expire(&self.key, sec as usize).await
                .map_err(|e| AppError::redis(e.to_string(), Some("EXPIRE")))?; 
        }
        Ok(s)
    }

    pub async fn getScore(&self, member: &str) -> AppResult<Option<f64>> {
        let mut conn = self.conn.clone();
        let v: Option<f64> = redis::cmd("ZSCORE").arg(&self.key).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("ZSCORE")))?;
        Ok(v)
    }

    /// 상위 N (점수 내림차순)
    pub async fn getTopRankings(&self, start: isize, stop: isize) -> AppResult<Vec<(String, f64)>> {
        let mut conn = self.conn.clone();
        let v: Vec<(String, f64)> = redis::cmd("ZREVRANGE").arg(&self.key).arg(start).arg(stop).arg("WITHSCORES").query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("ZREVRANGE")))?;
        Ok(v)
    }

    /// 하위 N (점수 오름차순)
    pub async fn getBottomRankings(&self, start: isize, stop: isize) -> AppResult<Vec<(String, f64)>> {
        let mut conn = self.conn.clone();
        let v: Vec<(String, f64)> = redis::cmd("ZRANGE").arg(&self.key).arg(start).arg(stop).arg("WITHSCORES").query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("ZRANGE")))?;
        Ok(v)
    }

    pub async fn getMemberCount(&self) -> AppResult<usize> {
        let mut conn = self.conn.clone();
        let n: i64 = redis::cmd("ZCARD").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("ZCARD")))?;
        Ok(n.max(0) as usize)
    }

    /// ZRANK key member (0부터 시작하는 순위)
    pub async fn getRank(&self, member: &str) -> AppResult<Option<usize>> {
        let mut conn = self.conn.clone();
        let rank: Option<i64> = redis::cmd("ZRANK").arg(&self.key).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("ZRANK")))?;
        Ok(rank.map(|r| r.max(0) as usize))
    }

    /// ZREVRANK key member (역순 순위)
    pub async fn getReverseRank(&self, member: &str) -> AppResult<Option<usize>> {
        let mut conn = self.conn.clone();
        let rank: Option<i64> = redis::cmd("ZREVRANK").arg(&self.key).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("ZREVRANK")))?;
        Ok(rank.map(|r| r.max(0) as usize))
    }

    /// ZRANGEBYSCORE key min max
    pub async fn getMembersByScore(&self, min: f64, max: f64) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let members: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(&self.key).arg(min).arg(max)
            .query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("ZRANGEBYSCORE")))?;
        Ok(members)
    }
} 