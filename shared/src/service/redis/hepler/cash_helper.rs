use anyhow::{Context, Result};
use redis::{AsyncCommands, Value};
use serde::{de::DeserializeOwned, Serialize};

use crate::config::redis_config::RedisConfig;
use crate::service::redis::core::retry_operation::RETRY_OPT;
use crate::service::redis::core::redis_get_key::KeyType;

#[derive(Clone)]
pub struct CacheHelper {
    conn: RedisConfig,
    key: KeyType,
    ttl: Option<u32>,
    limit: Option<u32>,
}

impl CacheHelper {
    pub fn new(conn: RedisConfig, key: KeyType, ttl: Option<u32>, limit: Option<u32>) -> Self {
        Self { conn, key, ttl, limit }
    }

    /// 캐시에 JSON 값 저장 + LRU 목록 갱신
    /// 
    /// # 동작 방식
    /// 1. SET key JSON (TTL 설정)
    /// 2. LREM list_key 0 id (기존 항목 제거)
    /// 3. LPUSH list_key id (최근 목록 맨 앞에 추가)
    /// 4. LTRIM list_key 0 limit-1 (용량 제한)
    /// 
    /// # 예시
    /// ```rust
    /// let cache = CacheHelper::new(redis_config, KeyType::User, Some(3600), Some(100));
    /// cache.set_cache_field(42, &user_data).await?;
    /// ```
    pub async fn set_cache_field<T: Serialize>(&self, id: u16, value: &T) -> Result<()> {
        let data_key = self.key.get_key(&id);
        let list_key = self.key.get_index_key();
        let json = serde_json::to_string(value).context("CacheHelper: JSON 직렬화 실패")?;
        let ttl_opt = self.ttl;
        let cap = self.limit.unwrap_or(20);

        RETRY_OPT.execute::<Vec<Value>, _, _>(|| {
            let data_key = data_key.clone();
            let list_key = list_key.clone();
            let json = json.clone();

            async move {
                let mut conn = self.conn.get_connection();

                let mut p = redis::pipe();
                p.atomic()
                    .set(&data_key, &json);

                if let Some(ttl_sec) = ttl_opt {
                    p.expire(&data_key, ttl_sec as i64);
                }

                p.cmd("LREM").arg(&list_key).arg(0).arg(id as i64)
                 .cmd("LPUSH").arg(&list_key).arg(id as i64)
                 .cmd("LTRIM").arg(&list_key).arg(0).arg((cap.saturating_sub(1)) as i64);

                let resp: Vec<Value> = p
                    .query_async(&mut conn)
                    .await
                    .context("CacheHelper: PIPELINE(SET+EXPIRE+LREM+LPUSH+LTRIM) 실패")?;

                Ok(resp)
            }
        }).await?;

        Ok(())
    }

    /// 캐시에서 JSON 값 조회
    pub async fn get_cache_field<T: DeserializeOwned>(&self, id: u16) -> Result<Option<T>> {
        let data_key = self.key.get_key(&id);

        let raw: Option<String> = RETRY_OPT
            .execute::<Option<String>, _, _>(|| {
                let data_key = data_key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.get(&data_key)
                        .await
                        .context("CacheHelper: GET 실패")
                }
            })
            .await?;

        if let Some(s) = raw {
            let v = serde_json::from_str::<T>(&s).context("CacheHelper: JSON 역직렬화 실패")?;
            Ok(Some(v))
        } else {
            Ok(None)
        }
    }

    /// 캐시에서 raw string 조회
    pub async fn get_cache_field_raw(&self, id: u16) -> Result<Option<String>> {
        let data_key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<Option<String>, _, _>(|| {
                let data_key = data_key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.get(&data_key)
                        .await
                        .context("CacheHelper: GET 실패")
                }
            })
            .await
    }

    /// 캐시 항목 삭제 + LRU 목록에서 제거
    pub async fn delete_cache_field(&self, id: u16) -> Result<bool> {
        let data_key = self.key.get_key(&id);
        let list_key = self.key.get_index_key();

        let deleted: u64 = RETRY_OPT
            .execute::<u64, _, _>(|| {
                let data_key = data_key.clone();
                let list_key = list_key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    
                    // DEL + LREM 파이프라인
                    let mut p = redis::pipe();
                    p.atomic()
                        .del(&data_key)
                        .cmd("LREM").arg(&list_key).arg(0).arg(id as i64);
                    
                    let (del_count, _lrem_count): (u64, i64) = p
                        .query_async(&mut conn)
                        .await
                        .context("CacheHelper: PIPELINE(DEL+LREM) 실패")?;
                    
                    Ok(del_count)
                }
            })
            .await?;

        Ok(deleted > 0)
    }

    /// 최근 사용된 ID 목록 조회 (LRU 순서)
    pub async fn get_recent_list(&self) -> Result<Vec<u16>> {
        let list_key = self.key.get_index_key();

        let ids: Vec<i64> = RETRY_OPT
            .execute::<Vec<i64>, _, _>(|| {
                let list_key = list_key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.lrange(&list_key, 0, -1)
                        .await
                        .context("CacheHelper: LRANGE 실패")
                }
            })
            .await?;

        Ok(ids.into_iter().map(|id| id as u16).collect())
    }

    /// 캐시 항목 존재 여부 확인
    pub async fn exists_cache_field(&self, id: u16) -> Result<bool> {
        let data_key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<bool, _, _>(|| {
                let data_key = data_key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.exists(&data_key)
                        .await
                        .context("CacheHelper: EXISTS 실패")
                }
            })
            .await
    }

    /// TTL 갱신
    pub async fn update_ttl(&self, id: u16, ttl: u32) -> Result<bool> {
        let data_key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<bool, _, _>(|| {
                let data_key = data_key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.expire(&data_key, ttl as i64)
                        .await
                        .context("CacheHelper: EXPIRE 실패")
                }
            })
            .await
    }

    /// 캐시 통계 정보
    pub async fn get_cache_stats(&self) -> Result<(u64, u64)> {
        let data_key = self.key.get_key(&1); // 임시 키
        let list_key = self.key.get_index_key();

        let (data_count, list_len): (u64, i64) = RETRY_OPT
            .execute::<(u64, i64), _, _>(|| {
                let data_key = data_key.clone();
                let list_key = list_key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    
                    let mut p = redis::pipe();
                    p.atomic()
                        .cmd("DBSIZE") // 전체 키 수 (근사값)
                        .llen(&list_key);
                    
                    let (db_size, list_length): (u64, i64) = p
                        .query_async(&mut conn)
                        .await
                        .context("CacheHelper: PIPELINE(DBSIZE+LLEN) 실패")?;
                    
                    Ok((db_size, list_length))
                }
            })
            .await?;

        Ok((data_count, list_len as u64))
    }
}
