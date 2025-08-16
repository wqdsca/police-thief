use anyhow::{anyhow, Context, Result};
use redis::{AsyncCommands, FromRedisValue, Value};
use serde::{de::DeserializeOwned, Serialize};

use crate::config::redis_config::RedisConfig;
use crate::service::redis::core::redis_get_key::KeyType;
use crate::service::redis::core::retry_operation::RETRY_OPT;

#[derive(Clone)]
pub struct SetHelper {
    conn: RedisConfig,
    key: KeyType,
    ttl: Option<u32>,
}

impl SetHelper {
    pub fn new(conn: RedisConfig, key: KeyType, ttl: Option<u32>, _limit: Option<u32>) -> Self {
        Self { conn, key, ttl }
    }

    /// SADD(JSON member) + (옵션)EXPIRE
    pub async fn add_member<T: Serialize>(&self, id: u16, member: &str, data: &T) -> Result<u64> {
        let key = self.key.get_key(&id);
        let json = serde_json::to_string(data).context("SetHelper: JSON 직렬화 실패")?;
        let ttl_opt = self.ttl;

        let added = RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                let _member = member.to_string();
                let json = json.clone();
                async move {
                    let mut conn = self.conn.get_connection();

                    let mut p = redis::pipe();
                    p.sadd(&key, &json);

                    if let Some(ttl_sec) = ttl_opt {
                        p.expire(&key, ttl_sec as i64);
                    }

                    let resp: Vec<Value> = p
                        .query_async(&mut conn)
                        .await
                        .context("SetHelper: PIPELINE(SADD+EXPIRE) 실패")?;

                    let first = resp
                        .first()
                        .ok_or_else(|| anyhow!("파이프라인 응답 비어있음"))?;
                    let added: u64 =
                        FromRedisValue::from_redis_value(first).context("SADD 응답 파싱 실패")?;
                    Ok(added)
                }
            })
            .await?;

        Ok(added)
    }

    /// 멤버 조회 (JSON 역직렬화)
    pub async fn get_member<T: DeserializeOwned>(
        &self,
        id: u16,
        member: &str,
    ) -> Result<Option<T>> {
        let key = self.key.get_key(&id);

        let exists: bool = RETRY_OPT
            .execute::<bool, _, _>(|| {
                let key = key.clone();
                let _member = member.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.sismember(&key, member)
                        .await
                        .context("SetHelper: SISMEMBER 실패")
                }
            })
            .await?;

        if exists {
            // 멤버가 존재하면 raw 데이터 조회
            let raw: Option<String> = RETRY_OPT
                .execute::<Option<String>, _, _>(|| {
                    let key = key.clone();
                    async move {
                        let mut conn = self.conn.get_connection();
                        conn.srandmember(&key)
                            .await
                            .context("SetHelper: SRANDMEMBER 실패")
                    }
                })
                .await?;

            if let Some(json) = raw {
                let data =
                    serde_json::from_str::<T>(&json).context("SetHelper: JSON 역직렬화 실패")?;
                Ok(Some(data))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// 멤버 raw 조회
    pub async fn get_member_raw(&self, id: u16, member: &str) -> Result<Option<String>> {
        let key = self.key.get_key(&id);

        let exists: bool = RETRY_OPT
            .execute::<bool, _, _>(|| {
                let key = key.clone();
                let _member = member.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.sismember(&key, member)
                        .await
                        .context("SetHelper: SISMEMBER 실패")
                }
            })
            .await?;

        if exists {
            RETRY_OPT
                .execute::<Option<String>, _, _>(|| {
                    let key = key.clone();
                    async move {
                        let mut conn = self.conn.get_connection();
                        conn.srandmember(&key)
                            .await
                            .context("SetHelper: SRANDMEMBER 실패")
                    }
                })
                .await
        } else {
            Ok(None)
        }
    }

    /// 모든 멤버 조회 (JSON 역직렬화)
    pub async fn get_all_members<T: DeserializeOwned>(&self, id: u16) -> Result<Vec<T>> {
        let key = self.key.get_key(&id);

        let members: Vec<String> = RETRY_OPT
            .execute::<Vec<String>, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.smembers(&key)
                        .await
                        .context("SetHelper: SMEMBERS 실패")
                }
            })
            .await?;

        let mut out = Vec::with_capacity(members.len());
        for json in members {
            let data = serde_json::from_str::<T>(&json).context("SetHelper: JSON 역직렬화 실패")?;
            out.push(data);
        }
        Ok(out)
    }

    /// 모든 멤버 raw 조회
    pub async fn get_all_members_raw(&self, id: u16) -> Result<Vec<String>> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<Vec<String>, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.smembers(&key)
                        .await
                        .context("SetHelper: SMEMBERS 실패")
                }
            })
            .await
    }

    /// 랜덤 멤버 조회 (JSON 역직렬화)
    pub async fn get_random_member<T: DeserializeOwned>(&self, id: u16) -> Result<Option<T>> {
        let key = self.key.get_key(&id);

        let member: Option<String> = RETRY_OPT
            .execute::<Option<String>, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.srandmember(&key)
                        .await
                        .context("SetHelper: SRANDMEMBER 실패")
                }
            })
            .await?;

        if let Some(json) = member {
            let data = serde_json::from_str::<T>(&json).context("SetHelper: JSON 역직렬화 실패")?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    /// 랜덤 멤버들 조회 (JSON 역직렬화)
    pub async fn get_random_members<T: DeserializeOwned>(
        &self,
        id: u16,
        count: i64,
    ) -> Result<Vec<T>> {
        let key = self.key.get_key(&id);

        let members: Vec<String> = RETRY_OPT
            .execute::<Vec<String>, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.srandmember_multiple(&key, count.try_into().unwrap_or(1))
                        .await
                        .context("SetHelper: SRANDMEMBER 실패")
                }
            })
            .await?;

        let mut out = Vec::with_capacity(members.len());
        for json in members {
            let data = serde_json::from_str::<T>(&json).context("SetHelper: JSON 역직렬화 실패")?;
            out.push(data);
        }
        Ok(out)
    }

    /// 멤버 삭제
    pub async fn remove_member(&self, id: u16, member: &str) -> Result<u64> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                let member = member.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.srem(&key, member)
                        .await
                        .context("SetHelper: SREM 실패")
                }
            })
            .await
    }

    /// 여러 멤버 삭제
    pub async fn remove_members(&self, id: u16, members: &[&str]) -> Result<u64> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                let members = members.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.srem(&key, members)
                        .await
                        .context("SetHelper: SREM 실패")
                }
            })
            .await
    }

    /// 모든 멤버 삭제
    pub async fn delete_all_members(&self, id: u16) -> Result<u64> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.del(&key).await.context("SetHelper: DEL 실패")
                }
            })
            .await
    }

    /// 멤버 존재 여부 확인
    pub async fn exists_member(&self, id: u16, member: &str) -> Result<bool> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<bool, _, _>(|| {
                let key = key.clone();
                let member = member.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.sismember(&key, member)
                        .await
                        .context("SetHelper: SISMEMBER 실패")
                }
            })
            .await
    }

    /// 전체 멤버 수 조회
    pub async fn get_member_count(&self, id: u16) -> Result<u64> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.scard(&key).await.context("SetHelper: SCARD 실패")
                }
            })
            .await
    }

    /// TTL 갱신
    pub async fn update_ttl(&self, id: u16, ttl: u32) -> Result<bool> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<bool, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.expire(&key, ttl as i64)
                        .await
                        .context("SetHelper: EXPIRE 실패")
                }
            })
            .await
    }

    /// Set 연산: 교집합
    pub async fn intersection<T: DeserializeOwned>(
        &self,
        id: u16,
        other_keys: &[&str],
    ) -> Result<Vec<T>> {
        let key = self.key.get_key(&id);
        let all_keys = std::iter::once(key.as_str())
            .chain(other_keys.iter().copied())
            .collect::<Vec<_>>();

        let members: Vec<String> = RETRY_OPT
            .execute::<Vec<String>, _, _>(|| {
                let all_keys = all_keys.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.sinter(&all_keys)
                        .await
                        .context("SetHelper: SINTER 실패")
                }
            })
            .await?;

        let mut out = Vec::with_capacity(members.len());
        for json in members {
            let data = serde_json::from_str::<T>(&json).context("SetHelper: JSON 역직렬화 실패")?;
            out.push(data);
        }
        Ok(out)
    }

    /// Set 연산: 합집합
    pub async fn union<T: DeserializeOwned>(&self, id: u16, other_keys: &[&str]) -> Result<Vec<T>> {
        let key = self.key.get_key(&id);
        let all_keys = std::iter::once(key.as_str())
            .chain(other_keys.iter().copied())
            .collect::<Vec<_>>();

        let members: Vec<String> = RETRY_OPT
            .execute::<Vec<String>, _, _>(|| {
                let all_keys = all_keys.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.sunion(&all_keys)
                        .await
                        .context("SetHelper: SUNION 실패")
                }
            })
            .await?;

        let mut out = Vec::with_capacity(members.len());
        for json in members {
            let data = serde_json::from_str::<T>(&json).context("SetHelper: JSON 역직렬화 실패")?;
            out.push(data);
        }
        Ok(out)
    }

    /// Set 연산: 차집합
    pub async fn difference<T: DeserializeOwned>(
        &self,
        id: u16,
        other_keys: &[&str],
    ) -> Result<Vec<T>> {
        let key = self.key.get_key(&id);
        let all_keys = std::iter::once(key.as_str())
            .chain(other_keys.iter().copied())
            .collect::<Vec<_>>();

        let members: Vec<String> = RETRY_OPT
            .execute::<Vec<String>, _, _>(|| {
                let all_keys = all_keys.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.sdiff(&all_keys).await.context("SetHelper: SDIFF 실패")
                }
            })
            .await?;

        let mut out = Vec::with_capacity(members.len());
        for json in members {
            let data = serde_json::from_str::<T>(&json).context("SetHelper: JSON 역직렬화 실패")?;
            out.push(data);
        }
        Ok(out)
    }

    /// Set 통계 정보
    pub async fn get_set_stats(&self, id: u16) -> Result<u64> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.scard(&key).await.context("SetHelper: SCARD 실패")
                }
            })
            .await
    }
}
