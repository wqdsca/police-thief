use anyhow::{anyhow, Context, Result};
use redis::{AsyncCommands, FromRedisValue, Value};
use serde::{de::DeserializeOwned, Serialize};

use crate::config::redis_config::RedisConfig;
use crate::service::redis::core::redis_get_key::KeyType;
use crate::service::redis::core::retry_operation::RETRY_OPT;

#[derive(Clone)]
pub struct ListHelper {
    conn: RedisConfig,
    key: KeyType,
    ttl: Option<u32>,
    limit: Option<u32>,
}

impl ListHelper {
    pub fn new(conn: RedisConfig, key: KeyType, ttl: Option<u32>, limit: Option<u32>) -> Self {
        Self { conn, key, ttl, limit }
    }

    /// LPUSH(JSON value) + (옵션)EXPIRE
    pub async fn push_front<T: Serialize>(&self, id: u16, value: &T) -> Result<u64> {
        let key = self.key.get_key(&id);
        let json = serde_json::to_string(value).context("ListHelper: JSON 직렬화 실패")?;
        let ttl_opt = self.ttl;

        let added = RETRY_OPT.execute::<u64, _, _>(|| {
            let key = key.clone();
            let json = json.clone();
            async move {
                let mut conn = self.conn.get_connection();

                let mut p = redis::pipe();
                p.lpush(&key, &json);

                if let Some(ttl_sec) = ttl_opt {
                    p.expire(&key, ttl_sec as i64);
                }

                let resp: Vec<Value> = p
                    .query_async(&mut conn)
                    .await
                    .context("ListHelper: PIPELINE(LPUSH+EXPIRE) 실패")?;

                let first = resp.get(0).ok_or_else(|| anyhow!("파이프라인 응답 비어있음"))?;
                let added: u64 = FromRedisValue::from_redis_value(first)
                    .context("LPUSH 응답 파싱 실패")?;
                Ok(added)
            }
        }).await?;

        Ok(added)
    }

    /// RPUSH(JSON value) + (옵션)EXPIRE
    pub async fn push_back<T: Serialize>(&self, id: u16, value: &T) -> Result<u64> {
        let key = self.key.get_key(&id);
        let json = serde_json::to_string(value).context("ListHelper: JSON 직렬화 실패")?;
        let ttl_opt = self.ttl;

        let added = RETRY_OPT.execute::<u64, _, _>(|| {
            let key = key.clone();
            let json = json.clone();
            async move {
                let mut conn = self.conn.get_connection();

                let mut p = redis::pipe();
                p.rpush(&key, &json);

                if let Some(ttl_sec) = ttl_opt {
                    p.expire(&key, ttl_sec as i64);
                }

                let resp: Vec<Value> = p
                    .query_async(&mut conn)
                    .await
                    .context("ListHelper: PIPELINE(RPUSH+EXPIRE) 실패")?;

                let first = resp.get(0).ok_or_else(|| anyhow!("파이프라인 응답 비어있음"))?;
                let added: u64 = FromRedisValue::from_redis_value(first)
                    .context("RPUSH 응답 파싱 실패")?;
                Ok(added)
            }
        }).await?;

        Ok(added)
    }

    /// LPOP 조회 (JSON 역직렬화)
    pub async fn pop_front<T: DeserializeOwned>(&self, id: u16) -> Result<Option<T>> {
        let key = self.key.get_key(&id);

        let result: Option<String> = RETRY_OPT.execute::<Option<String>, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                conn.lpop(&key, None)
                    .await
                    .context("ListHelper: LPOP 실패")
            }
        }).await?;

        if let Some(json) = result {
            let data = serde_json::from_str::<T>(&json)
                .context("ListHelper: JSON 역직렬화 실패")?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    /// RPOP 조회 (JSON 역직렬화)
    pub async fn pop_back<T: DeserializeOwned>(&self, id: u16) -> Result<Option<T>> {
        let key = self.key.get_key(&id);

        let result: Option<String> = RETRY_OPT.execute::<Option<String>, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                conn.rpop(&key, None)
                    .await
                    .context("ListHelper: RPOP 실패")
            }
        }).await?;

        if let Some(json) = result {
            let data = serde_json::from_str::<T>(&json)
                .context("ListHelper: JSON 역직렬화 실패")?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    /// 모든 요소 조회 (JSON 역직렬화)
    pub async fn get_all<T: DeserializeOwned>(&self, id: u16) -> Result<Vec<T>> {
        let key = self.key.get_key(&id);

        let items: Vec<String> = RETRY_OPT.execute::<Vec<String>, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                conn.lrange(&key, 0, -1)
                    .await
                    .context("ListHelper: LRANGE 실패")
            }
        }).await?;

        let mut out = Vec::with_capacity(items.len());
        for json in items {
            let data = serde_json::from_str::<T>(&json)
                .context("ListHelper: JSON 역직렬화 실패")?;
            out.push(data);
        }
        Ok(out)
    }

    /// 리스트 길이 조회
    pub async fn get_length(&self, id: u16) -> Result<u64> {
        let key = self.key.get_key(&id);

        RETRY_OPT.execute::<u64, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                conn.llen(&key)
                    .await
                    .context("ListHelper: LLEN 실패")
            }
        }).await
    }

    /// TTL 갱신
    pub async fn update_ttl(&self, id: u16, ttl: u32) -> Result<bool> {
        let key = self.key.get_key(&id);

        RETRY_OPT.execute::<bool, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                conn.expire(&key, ttl as i64)
                    .await
                    .context("ListHelper: EXPIRE 실패")
            }
        }).await
    }
} 