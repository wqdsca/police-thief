use anyhow::{Context, Result};
use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};

use crate::config::redis_config::RedisConfig;
use crate::service::redis::core::redis_get_key::KeyType;
use crate::service::redis::core::retry_operation::RETRY_OPT;

#[derive(Clone)]
pub struct HashHelper {
    conn: RedisConfig,
    key: KeyType,
    ttl: Option<u32>,
    limit: Option<u32>,
}

impl HashHelper {
    pub fn new(conn: RedisConfig, key: KeyType, ttl: Option<u32>, limit: Option<u32>) -> Self {
        Self { conn, key, ttl, limit }
    }

    /// HSET field <- JSON(value). ttl이 설정되어 있으면 HSET+EXPIRE 파이프라인.
    /// 반환: 추가된 필드 수(0 또는 1)
    pub async fn set_hash_field<T: Serialize>(
        &self,
        id: u16,
        field: &str,
        value: &T,
    ) -> Result<u64> {
        let key = self.key.get_key(&id);
        let json = serde_json::to_string(value).context("HashHelper: JSON 직렬화 실패")?;

        let ttl_opt = self.ttl;
        RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                let field = field.to_string();
                let json = json.clone();
                async move {
                    let mut conn = self.conn.get_connection();

                    if let Some(ttl_sec) = ttl_opt {
                        // HSET + EXPIRE 파이프라인
                        let mut p = redis::pipe();
                        p.atomic().hset(&key, &field, &json).expire(&key, ttl_sec as i64);
                        // 두 개의 응답(HSET: u64, EXPIRE: bool)
                        let (added, _exp_ok): (u64, bool) = p
                            .query_async(&mut conn)
                            .await
                            .context("HashHelper: PIPELINE(HSET+EXPIRE) 실패")?;
                        Ok(added)
                    } else {
                        let added: u64 = conn
                            .hset(&key, &field, &json)
                            .await
                            .context("HashHelper: HSET 실패")?;
                        Ok(added)
                    }
                }
            })
            .await
    }

    /// HGET field -> JSON 역직렬화하여 반환
    pub async fn get_hash_field<T: DeserializeOwned>(
        &self,
        id: u16,
        field: &str,
    ) -> Result<Option<T>> {
        let key = self.key.get_key(&id);

        let raw: Option<String> = RETRY_OPT
            .execute::<Option<String>, _, _>(|| {
                let key = key.clone();
                let field = field.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.hget(&key, field)
                        .await
                        .context("HashHelper: HGET 실패")
                }
            })
            .await?;

        if let Some(s) = raw {
            let v = serde_json::from_str::<T>(&s).context("HashHelper: JSON 역직렬화 실패")?;
            Ok(Some(v))
        } else {
            Ok(None)
        }
    }

    /// HGET field (raw string)
    pub async fn get_hash_field_raw(&self, id: u16, field: &str) -> Result<Option<String>> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<Option<String>, _, _>(|| {
                let key = key.clone();
                let field = field.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.hget(&key, field)
                        .await
                        .context("HashHelper: HGET 실패")
                }
            })
            .await
    }

    /// HGETALL -> HashMap<String, String>
    pub async fn get_all_hash_fields(&self, id: u16) -> Result<std::collections::HashMap<String, String>> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<std::collections::HashMap<String, String>, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.hgetall(&key)
                        .await
                        .context("HashHelper: HGETALL 실패")
                }
            })
            .await
    }

    /// HDEL field(s) -> 삭제된 필드 수
    pub async fn delete_hash_field(&self, id: u16, field: &str) -> Result<u64> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                let field = field.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.hdel(&key, field)
                        .await
                        .context("HashHelper: HDEL 실패")
                }
            })
            .await
    }

    /// DEL key -> 삭제된 키 수(0 또는 1)
    pub async fn delete_all_hash_fields(&self, id: u16) -> Result<u64> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.del(&key).await.context("HashHelper: DEL 실패")
                }
            })
            .await
    }

    /// 키 존재 여부 (EXISTS key)
    pub async fn is_exists_hash_id(&self, id: u16) -> Result<bool> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<bool, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.exists(&key)
                        .await
                        .context("HashHelper: EXISTS 실패")
                }
            })
            .await
    }

    /// EXPIRE key ttl -> true/false
    pub async fn update_ttl_hash(&self, id: u16, ttl: u32) -> Result<bool> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<bool, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.expire(&key, ttl as i64)
                        .await
                        .context("HashHelper: EXPIRE 실패")
                }
            })
            .await
    }

    /// HINCRBY -> 증가 후 값 반환
    pub async fn incr_hash_field(&self, id: u16, field: &str, value: i64) -> Result<i64> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<i64, _, _>(|| {
                let key = key.clone();
                let field = field.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.hincr(&key, field, value)
                        .await
                        .context("HashHelper: HINCR 실패")
                }
            })
            .await
    }

    /// HMGET fields -> Vec<Option<String>>
    pub async fn mget_hash_fields(
        &self,
        id: u16,
        fields: &[&str],
    ) -> Result<Vec<Option<String>>> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<Vec<Option<String>>, _, _>(|| {
                let key = key.clone();
                let fields = fields.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.hget(&key, fields)
                        .await
                        .context("HashHelper: HGET 실패")
                }
            })
            .await
    }

    /// 다건 HSET (HMSET 폐기→ hset_multiple 사용).
    /// 값이 JSON이어야 하면 호출 전 직렬화해서 넘기세요.
    pub async fn mset_hash_fields(
        &self,
        id: u16,
        fields: &[(&str, &str)],
    ) -> Result<()> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<(), _, _>(|| {
                let key = key.clone();
                let pairs = fields
                    .iter()
                    .map(|(f, v)| ((*f).to_string(), (*v).to_string()))
                    .collect::<Vec<(String, String)>>();

                async move {
                    let mut conn = self.conn.get_connection();
                    conn.hset_multiple(&key, &pairs)
                        .await
                        .context("HashHelper: HSET_MULTIPLE 실패")
                }
            })
            .await
    }

    /// HEXISTS field
    pub async fn hexists(&self, id: u16, field: &str) -> Result<bool> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<bool, _, _>(|| {
                let key = key.clone();
                let field = field.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.hexists(&key, field)
                        .await
                        .context("HashHelper: HEXISTS 실패")
                }
            })
            .await
    }

    /// 다건 필드 모두 존재하는지(모두 true면 true)
    pub async fn hexists_all(&self, id: u16, fields: &[&str]) -> Result<bool> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<bool, _, _>(|| {
                let key = key.clone();
                let fields = fields.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                async move {
                    let mut conn = self.conn.get_connection();
                    let mut p = redis::pipe();
                    for f in &fields {
                        p.cmd("HEXISTS").arg(&key).arg(f);
                    }
                    let results: Vec<bool> = p
                        .query_async(&mut conn)
                        .await
                        .context("HashHelper: PIPELINE(HEXISTS..) 실패")?;
                    Ok(results.into_iter().all(|b| b))
                }
            })
            .await
    }
}
