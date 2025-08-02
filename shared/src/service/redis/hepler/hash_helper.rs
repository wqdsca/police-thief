use redis::AsyncCommands;
use serde::Serialize;
use anyhow::{Context, Result};

use crate::config::redis_config::RedisConfig;
use crate::service::redis::core::redis_get_key::KeyType;
use crate::service::redis::core::retry_operation::RETRY_OPT;

#[derive(Clone)]
pub struct HashHelper {
    conn: RedisConfig,
    id: u16,
    ttl: Option<u32>,
    limit: Option<u32>,
}

impl HashHelper {
    /// 생성자
    pub fn new(conn: RedisConfig, id: u16, ttl: Option<u32>, _limit: Option<u32>) -> Self {
        Self { conn, id, ttl, limit: _limit }
    }

    /// Hash에 필드 추가 (value를 JSON으로 직렬화)
    pub async fn set_hash_field<T>(&self, field: &str, value: &T) -> Result<String>
    where
        T: Serialize,
    {
        // 1) Connection 가져오기 (사용하지 않으므로 제거)

        // 2) 키 생성
        let key = KeyType::User.get_key(&self.id);

        // 3) 값 JSON 직렬화
        let json = serde_json::to_string(value)
            .context("HashHelper: JSON 직렬화 실패")?;
        
        // 4) Redis에 저장
        RETRY_OPT.execute::<(), _, _>(|| {
            let json_clone = json.clone();
            let key_clone = key.clone();
            let field_clone = field.to_string();
            async move {
                let mut conn = self.conn.get_connection();
                conn.hset(&key_clone, field_clone, json_clone)
                    .await
                    .context("HashHelper: HSET 실패")
            }
        })
        .await?;
        
        // 5) TTL 설정 (옵션)
        if let Some(ttl_sec) = self.ttl {
            let key_clone = key.clone();
            RETRY_OPT.execute::<(), _, _>(|| {
                let key_clone = key_clone.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.expire(&key_clone, ttl_sec as i64)
                        .await
                        .context("HashHelper: EXPIRE 실패")
                }
            })
            .await?;
        }

        Ok(key)
    }

    /// Hash에서 필드 가져오기
    pub async fn get_hash_field(&self, field: &str) -> Result<Option<String>> {
        let key = KeyType::User.get_key(&self.id);

        let val: Option<String> = RETRY_OPT.execute::<Option<String>, _, _>(|| {
            let key_clone = key.clone();
            let field_clone = field.to_string();
            async move {
                let mut conn = self.conn.get_connection();
                conn.hget(&key_clone, field_clone)
                    .await
                    .context("HashHelper: HGET 실패")
            }
        })
        .await?;

        Ok(val)
    }
}
