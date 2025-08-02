use anyhow::{anyhow, Context, Result};
use redis::{AsyncCommands, FromRedisValue, Value};
use serde::{de::DeserializeOwned, Serialize};

use crate::config::redis_config::RedisConfig;
use crate::service::redis::core::redis_get_key::KeyType;
use crate::service::redis::core::retry_operation::RETRY_OPT;

#[derive(Clone)]
pub struct ZSetHelper {
    conn: RedisConfig,
    key: KeyType,
    ttl: Option<u32>,
    limit: Option<u32>,
}

impl ZSetHelper {
    pub fn new(conn: RedisConfig, key: KeyType, ttl: Option<u32>, limit: Option<u32>) -> Self {
        Self { conn, key, ttl, limit }
    }

    /// ZADD(JSON member, score) + (옵션)EXPIRE + ZREMRANGEBYRANK로 용량 유지
    pub async fn add_member<T: Serialize>(&self, id: u16, score: f64, value: &T) -> Result<u64> {
        let key = self.key.get_key(&id);
        let json = serde_json::to_string(value).context("ZSetHelper: JSON 직렬화 실패")?;
        let ttl_opt = self.ttl;
        let limit = self.limit.unwrap_or(100);

        let added = RETRY_OPT.execute::<u64, _, _>(|| {
            let key = key.clone();
            let json = json.clone();
            async move {
                let mut conn = self.conn.get_connection();

                let mut p = redis::pipe();
                p.atomic().zadd(&key, &json, score);
                if let Some(ttl_sec) = ttl_opt {
                    p.expire(&key, ttl_sec as i64);
                }
                // 낮은 점수(앞)부터 제거 → 마지막 limit개만 보존
                p.cmd("ZREMRANGEBYRANK").arg(&key).arg(0).arg(-(limit as i64 + 1));

                let resp: Vec<Value> = p
                    .query_async(&mut conn)
                    .await
                    .context("ZSetHelper: PIPELINE(ZADD..TRIM) 실패")?;

                let first = resp.get(0).ok_or_else(|| anyhow!("파이프라인 응답 비어있음"))?;
                let added: u64 =
                    FromRedisValue::from_redis_value(first).context("ZADD 응답 파싱 실패")?;
                Ok(added)
            }
        }).await?;

        Ok(added)
    }

    /// raw 조회: 전체(오름차순)
    pub async fn get_all_members(&self, id: u16) -> Result<Vec<(String, f64)>> {
        let key = self.key.get_key(&id);
        RETRY_OPT.execute::<Vec<(String, f64)>, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zrange_withscores(&key, 0, -1)
                    .await
                    .context("ZSetHelper: ZRANGE WITHSCORES 실패")
            }
        }).await
    }

    /// raw 조회: 상위 N(내림차순)
    pub async fn get_top_members(&self, id: u16, count: i64) -> Result<Vec<(String, f64)>> {
        let key = self.key.get_key(&id);
        RETRY_OPT.execute::<Vec<(String, f64)>, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zrevrange_withscores(&key, 0, (count - 1).try_into().unwrap())
                    .await
                    .context("ZSetHelper: ZREVRANGE WITHSCORES 실패")
            }
        }).await
    }

    /// raw 조회: 하위 N(오름차순)
    pub async fn get_bottom_members(&self, id: u16, count: i64) -> Result<Vec<(String, f64)>> {
        let key = self.key.get_key(&id);
        RETRY_OPT.execute::<Vec<(String, f64)>, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zrange_withscores(&key, 0, (count - 1).try_into().unwrap())
                    .await
                    .context("ZSetHelper: ZRANGE WITHSCORES 실패")
            }
        }).await
    }

    /// 제네릭 조회 유틸: 구간 + 정렬방향(desc=true면 상위부터)
    pub async fn range_with_scores<T: DeserializeOwned>(
        &self,
        id: u16,
        start: i64,
        stop: i64,
        desc: bool,
    ) -> Result<Vec<(T, f64)>> {
        let key = self.key.get_key(&id);

        let rows: Vec<(String, f64)> = RETRY_OPT.execute::<Vec<(String, f64)>, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                if desc {
                    conn.zrevrange_withscores(&key, start.try_into().unwrap(), stop.try_into().unwrap())
                        .await
                        .context("ZSetHelper: ZREVRANGE WITHSCORES 실패")
                } else {
                    conn.zrange_withscores(&key, start.try_into().unwrap(), stop.try_into().unwrap())
                        .await
                        .context("ZSetHelper: ZRANGE WITHSCORES 실패")
                }
            }
        }).await?;

        let mut out = Vec::with_capacity(rows.len());
        for (m, s) in rows {
            let v = serde_json::from_str::<T>(&m).context("ZSetHelper: JSON 역직렬화 실패(range)")?;
            out.push((v, s));
        }
        Ok(out)
    }

    /// 상위 N개 제네릭
    pub async fn get_top_members_typed<T: DeserializeOwned>(
        &self,
        id: u16,
        count: i64,
    ) -> Result<Vec<(T, f64)>> {
        self.range_with_scores(id, 0, count - 1, true).await
    }

    /// 하위 N개 제네릭
    pub async fn get_bottom_members_typed<T: DeserializeOwned>(
        &self,
        id: u16,
        count: i64,
    ) -> Result<Vec<(T, f64)>> {
        self.range_with_scores(id, 0, count - 1, false).await
    }

    /// 멤버 삭제
    pub async fn remove_member(&self, id: u16, member: &str) -> Result<u64> {
        let key = self.key.get_key(&id);
        RETRY_OPT.execute::<u64, _, _>(|| {
            let key = key.clone();
            let member = member.to_string();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zrem(&key, member).await.context("ZSetHelper: ZREM 실패")
            }
        }).await
    }

    /// 멤버 다건 삭제
    pub async fn remove_members(&self, id: u16, members: &[&str]) -> Result<u64> {
        let key = self.key.get_key(&id);
        RETRY_OPT.execute::<u64, _, _>(|| {
            let key = key.clone();
            let members = members.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zrem(&key, members).await.context("ZSetHelper: ZREM 실패")
            }
        }).await
    }

    /// 점수 범위 삭제
    pub async fn remove_by_score_range(&self, id: u16, min_score: f64, max_score: f64) -> Result<u64> {
        let key = self.key.get_key(&id);
        RETRY_OPT.execute::<u64, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                redis::cmd("ZREMRANGEBYSCORE")
                    .arg(&key)
                    .arg(min_score)
                    .arg(max_score)
                    .query_async(&mut conn)
                    .await
                    .context("ZSetHelper: ZREMRANGEBYSCORE 실패")
            }
        }).await
    }

    /// 랭크 범위 삭제
    pub async fn remove_by_rank_range(&self, id: u16, start: i64, stop: i64) -> Result<u64> {
        let key = self.key.get_key(&id);
        RETRY_OPT.execute::<u64, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zremrangebyrank(&key, start.try_into().unwrap(), stop.try_into().unwrap())
                    .await
                    .context("ZSetHelper: ZREMRANGEBYRANK 실패")
            }
        }).await
    }

    /// 전체 삭제(키 삭제)
    pub async fn delete_all_members(&self, id: u16) -> Result<u64> {
        let key = self.key.get_key(&id);
        RETRY_OPT.execute::<u64, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                conn.del(&key).await.context("ZSetHelper: DEL 실패")
            }
        }).await
    }

    /// 존재 여부(점수 유무로 판단)
    pub async fn exists_member(&self, id: u16, member: &str) -> Result<bool> {
        let key = self.key.get_key(&id);
        let score: Option<f64> = RETRY_OPT.execute::<Option<f64>, _, _>(|| {
            let key = key.clone();
            let member = member.to_string();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zscore(&key, member).await.context("ZSetHelper: ZSCORE 실패")
            }
        }).await?;
        Ok(score.is_some())
    }

    /// 점수 조회
    pub async fn get_member_score(&self, id: u16, member: &str) -> Result<Option<f64>> {
        let key = self.key.get_key(&id);
        RETRY_OPT.execute::<Option<f64>, _, _>(|| {
            let key = key.clone();
            let member = member.to_string();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zscore(&key, member).await.context("ZSetHelper: ZSCORE 실패")
            }
        }).await
    }

    /// 순위 조회(높은 점수 순, 0부터)
    pub async fn get_member_rank(&self, id: u16, member: &str) -> Result<Option<u64>> {
        let key = self.key.get_key(&id);
        let rank: Option<i64> = RETRY_OPT.execute::<Option<i64>, _, _>(|| {
            let key = key.clone();
            let member = member.to_string();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zrevrank(&key, member).await.context("ZSetHelper: ZREVRANK 실패")
            }
        }).await?;
        Ok(rank.map(|r| r as u64))
    }

    /// 순위 조회(낮은 점수 순, 0부터)
    pub async fn get_member_rank_asc(&self, id: u16, member: &str) -> Result<Option<u64>> {
        let key = self.key.get_key(&id);
        let rank: Option<i64> = RETRY_OPT.execute::<Option<i64>, _, _>(|| {
            let key = key.clone();
            let member = member.to_string();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zrank(&key, member).await.context("ZSetHelper: ZRANK 실패")
            }
        }).await?;
        Ok(rank.map(|r| r as u64))
    }

    /// 점수 증가
    pub async fn increment_score(&self, id: u16, member: &str, increment: f64) -> Result<f64> {
        let key = self.key.get_key(&id);
        RETRY_OPT.execute::<f64, _, _>(|| {
            let key = key.clone();
            let member = member.to_string();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zincr(&key, increment, member)
                    .await
                    .context("ZSetHelper: ZINCR 실패")
            }
        }).await
    }

    /// 점수 범위 내 멤버 수
    pub async fn count_by_score_range(&self, id: u16, min_score: f64, max_score: f64) -> Result<u64> {
        let key = self.key.get_key(&id);
        RETRY_OPT.execute::<u64, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zcount(&key, min_score, max_score)
                    .await
                    .context("ZSetHelper: ZCOUNT 실패")
            }
        }).await
    }

    /// 전체 멤버 수
    pub async fn get_member_count(&self, id: u16) -> Result<u64> {
        let key = self.key.get_key(&id);
        RETRY_OPT.execute::<u64, _, _>(|| {
            let key = key.clone();
            async move {
                let mut conn = self.conn.get_connection();
                conn.zcard(&key).await.context("ZSetHelper: ZCARD 실패")
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
                    .context("ZSetHelper: EXPIRE 실패")
            }
        }).await
    }

    /// 통계: (멤버 수, 최소 점수, 최대 점수)
    pub async fn get_zset_stats(&self, id: u16) -> Result<(u64, f64, f64)> {
        let key = self.key.get_key(&id);

        let (member_count, min_range, max_range): (u64, Vec<(String, f64)>, Vec<(String, f64)>) =
            RETRY_OPT.execute::<(u64, Vec<(String, f64)>, Vec<(String, f64)>), _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    
                    let mut p = redis::pipe();
                    p.atomic()
                        .zcard(&key)
                        .cmd("ZRANGE").arg(&key).arg(0).arg(0).arg("WITHSCORES")
                        .cmd("ZREVRANGE").arg(&key).arg(0).arg(0).arg("WITHSCORES");
                    
                    p.query_async(&mut conn)
                        .await
                        .context("ZSetHelper: PIPELINE(ZCARD+ZRANGE+ZREVRANGE) 실패")
                }
            }).await?;

        let min_score = min_range.first().map(|(_, s)| *s).unwrap_or(0.0);
        let max_score = max_range.first().map(|(_, s)| *s).unwrap_or(0.0);

        Ok((member_count, min_score, max_score))
    }
}
