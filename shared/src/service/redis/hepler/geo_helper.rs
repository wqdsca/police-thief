use anyhow::{anyhow, Context, Result};
use redis::{AsyncCommands, FromRedisValue, Value};
use serde::{de::DeserializeOwned, Serialize};

use crate::config::redis_config::RedisConfig;
use crate::service::redis::core::redis_get_key::KeyType;
use crate::service::redis::core::retry_operation::RETRY_OPT;

/// 타입 별칭들
type GeoScoreRange = Vec<(String, f64)>;

#[derive(Clone)]
pub struct GeoHelper {
    conn: RedisConfig,
    key: KeyType,
    ttl: Option<u32>,
}

impl GeoHelper {
    pub fn new(conn: RedisConfig, key: KeyType, ttl: Option<u32>, _limit: Option<u32>) -> Self {
        Self { conn, key, ttl }
    }

    /// GEOADD(위치 정보) + (옵션)EXPIRE
    pub async fn add_location<T: Serialize>(
        &self,
        id: u16,
        member: &str,
        longitude: f64,
        latitude: f64,
        data: &T,
    ) -> Result<u64> {
        let key = self.key.get_key(&id);
        let json = serde_json::to_string(data).context("GeoHelper: JSON 직렬화 실패")?;
        let ttl_opt = self.ttl;

        let added = RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                let _member = member.to_string();
                let json = json.clone();
                async move {
                    let mut conn = self.conn.get_connection();

                    let mut p = redis::pipe();
                    p.geo_add(&key, &[(longitude, latitude, &json)]);

                    if let Some(ttl_sec) = ttl_opt {
                        p.expire(&key, ttl_sec as i64);
                    }

                    let resp: Vec<Value> = p
                        .query_async(&mut conn)
                        .await
                        .context("GeoHelper: PIPELINE(GEOADD+EXPIRE) 실패")?;

                    let first = resp
                        .first()
                        .ok_or_else(|| anyhow!("파이프라인 응답 비어있음"))?;
                    let added: u64 =
                        FromRedisValue::from_redis_value(first).context("GEOADD 응답 파싱 실패")?;
                    Ok(added)
                }
            })
            .await?;

        Ok(added)
    }

    /// 위치 정보 조회 (JSON 역직렬화)
    pub async fn get_location<T: DeserializeOwned>(
        &self,
        id: u16,
        member: &str,
    ) -> Result<Option<(T, f64, f64)>> {
        let key = self.key.get_key(&id);

        let result: Option<(String, f64, f64)> = RETRY_OPT
            .execute::<Option<(String, f64, f64)>, _, _>(|| {
                let key = key.clone();
                let member = member.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.geo_pos(&key, member)
                        .await
                        .context("GeoHelper: GEOPOS 실패")
                }
            })
            .await?;

        if let Some((json, longitude, latitude)) = result {
            let data = serde_json::from_str::<T>(&json).context("GeoHelper: JSON 역직렬화 실패")?;
            Ok(Some((data, longitude, latitude)))
        } else {
            Ok(None)
        }
    }

    /// 위치 정보 raw 조회
    pub async fn get_location_raw(
        &self,
        id: u16,
        member: &str,
    ) -> Result<Option<(String, f64, f64)>> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<Option<(String, f64, f64)>, _, _>(|| {
                let key = key.clone();
                let member = member.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.geo_pos(&key, member)
                        .await
                        .context("GeoHelper: GEOPOS 실패")
                }
            })
            .await
    }

    /// 반경 내 위치 조회 (JSON 역직렬화)
    pub async fn get_locations_in_radius<T: DeserializeOwned>(
        &self,
        id: u16,
        longitude: f64,
        latitude: f64,
        radius: f64,
        unit: &str,
    ) -> Result<Vec<(T, f64, f64, f64)>> {
        let key = self.key.get_key(&id);

        let results: Vec<(String, f64, f64, f64)> = RETRY_OPT
            .execute::<Vec<(String, f64, f64, f64)>, _, _>(|| {
                let key = key.clone();
                let unit = unit.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    redis::cmd("GEORADIUS")
                        .arg(&key)
                        .arg(longitude)
                        .arg(latitude)
                        .arg(radius)
                        .arg(unit)
                        .arg("WITHCOORD")
                        .arg("WITHDIST")
                        .query_async(&mut conn)
                        .await
                        .context("GeoHelper: GEORADIUS 실패")
                }
            })
            .await?;

        let mut out = Vec::with_capacity(results.len());
        for (json, longitude, latitude, distance) in results {
            let data = serde_json::from_str::<T>(&json).context("GeoHelper: JSON 역직렬화 실패")?;
            out.push((data, longitude, latitude, distance));
        }
        Ok(out)
    }

    /// 반경 내 위치 raw 조회
    pub async fn get_locations_in_radius_raw(
        &self,
        id: u16,
        longitude: f64,
        latitude: f64,
        radius: f64,
        unit: &str,
    ) -> Result<Vec<(String, f64, f64, f64)>> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<Vec<(String, f64, f64, f64)>, _, _>(|| {
                let key = key.clone();
                let unit = unit.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    redis::cmd("GEORADIUS")
                        .arg(&key)
                        .arg(longitude)
                        .arg(latitude)
                        .arg(radius)
                        .arg(unit)
                        .arg("WITHCOORD")
                        .arg("WITHDIST")
                        .query_async(&mut conn)
                        .await
                        .context("GeoHelper: GEORADIUS 실패")
                }
            })
            .await
    }

    /// 특정 멤버 기준 반경 내 위치 조회
    pub async fn get_locations_by_member<T: DeserializeOwned>(
        &self,
        id: u16,
        member: &str,
        radius: f64,
        unit: &str,
    ) -> Result<Vec<(T, f64, f64, f64)>> {
        let key = self.key.get_key(&id);

        let results: Vec<(String, f64, f64, f64)> = RETRY_OPT
            .execute::<Vec<(String, f64, f64, f64)>, _, _>(|| {
                let key = key.clone();
                let member = member.to_string();
                let unit = unit.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    redis::cmd("GEORADIUSBYMEMBER")
                        .arg(&key)
                        .arg(member)
                        .arg(radius)
                        .arg(unit)
                        .arg("WITHCOORD")
                        .arg("WITHDIST")
                        .query_async(&mut conn)
                        .await
                        .context("GeoHelper: GEORADIUSBYMEMBER 실패")
                }
            })
            .await?;

        let mut out = Vec::with_capacity(results.len());
        for (json, longitude, latitude, distance) in results {
            let data = serde_json::from_str::<T>(&json).context("GeoHelper: JSON 역직렬화 실패")?;
            out.push((data, longitude, latitude, distance));
        }
        Ok(out)
    }

    /// 두 위치 간 거리 계산
    pub async fn get_distance(
        &self,
        id: u16,
        member1: &str,
        member2: &str,
        unit: &str,
    ) -> Result<Option<f64>> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<Option<f64>, _, _>(|| {
                let key = key.clone();
                let member1 = member1.to_string();
                let member2 = member2.to_string();
                let unit = unit.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    redis::cmd("GEODIST")
                        .arg(&key)
                        .arg(member1)
                        .arg(member2)
                        .arg(unit)
                        .query_async(&mut conn)
                        .await
                        .context("GeoHelper: GEODIST 실패")
                }
            })
            .await
    }

    /// 위치 해시값 조회 (GeoHash)
    pub async fn get_geohash(&self, id: u16, member: &str) -> Result<Option<String>> {
        let key = self.key.get_key(&id);

        let hashes: Vec<Option<String>> = RETRY_OPT
            .execute::<Vec<Option<String>>, _, _>(|| {
                let key = key.clone();
                let member = member.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.geo_hash(&key, member)
                        .await
                        .context("GeoHelper: GEOHASH 실패")
                }
            })
            .await?;

        Ok(hashes.into_iter().next().flatten())
    }

    /// 여러 멤버의 위치 해시값 조회
    pub async fn get_geohashes(&self, id: u16, members: &[&str]) -> Result<Vec<Option<String>>> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<Vec<Option<String>>, _, _>(|| {
                let key = key.clone();
                let members = members.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.geo_hash(&key, members)
                        .await
                        .context("GeoHelper: GEOHASH 실패")
                }
            })
            .await
    }

    /// 위치 정보 삭제
    pub async fn remove_location(&self, id: u16, member: &str) -> Result<u64> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                let member = member.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.zrem(&key, member)
                        .await
                        .context("GeoHelper: ZREM 실패")
                }
            })
            .await
    }

    /// 여러 위치 정보 삭제
    pub async fn remove_locations(&self, id: u16, members: &[&str]) -> Result<u64> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                let members = members.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.zrem(&key, members)
                        .await
                        .context("GeoHelper: ZREM 실패")
                }
            })
            .await
    }

    /// 모든 위치 정보 삭제
    pub async fn delete_all_locations(&self, id: u16) -> Result<u64> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.del(&key).await.context("GeoHelper: DEL 실패")
                }
            })
            .await
    }

    /// 위치 정보 존재 여부 확인
    pub async fn exists_location(&self, id: u16, member: &str) -> Result<bool> {
        let key = self.key.get_key(&id);

        let score: Option<f64> = RETRY_OPT
            .execute::<Option<f64>, _, _>(|| {
                let key = key.clone();
                let member = member.to_string();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.zscore(&key, member)
                        .await
                        .context("GeoHelper: ZSCORE 실패")
                }
            })
            .await?;

        Ok(score.is_some())
    }

    /// 전체 위치 정보 수 조회
    pub async fn get_location_count(&self, id: u16) -> Result<u64> {
        let key = self.key.get_key(&id);

        RETRY_OPT
            .execute::<u64, _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();
                    conn.zcard(&key).await.context("GeoHelper: ZCARD 실패")
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
                        .context("GeoHelper: EXPIRE 실패")
                }
            })
            .await
    }

    /// Geo 통계 정보
    pub async fn get_geo_stats(&self, id: u16) -> Result<(u64, f64, f64)> {
        let key = self.key.get_key(&id);

        let (location_count, min_score, max_score): (u64, Option<f64>, Option<f64>) = RETRY_OPT
            .execute::<(u64, Option<f64>, Option<f64>), _, _>(|| {
                let key = key.clone();
                async move {
                    let mut conn = self.conn.get_connection();

                    let mut p = redis::pipe();
                    p.atomic()
                        .zcard(&key)
                        .cmd("ZRANGE")
                        .arg(&key)
                        .arg(0)
                        .arg(0)
                        .arg("WITHSCORES")
                        .cmd("ZREVRANGE")
                        .arg(&key)
                        .arg(0)
                        .arg(0)
                        .arg("WITHSCORES");

                    let (card, min_range, max_range): (u64, GeoScoreRange, GeoScoreRange) = p
                        .query_async(&mut conn)
                        .await
                        .context("GeoHelper: PIPELINE(ZCARD+ZRANGE+ZREVRANGE) 실패")?;

                    let min_score = min_range.first().map(|(_, score)| *score);
                    let max_score = max_range.first().map(|(_, score)| *score);

                    Ok((card, min_score, max_score))
                }
            })
            .await?;

        Ok((
            location_count,
            min_score.unwrap_or(0.0),
            max_score.unwrap_or(0.0),
        ))
    }
}
