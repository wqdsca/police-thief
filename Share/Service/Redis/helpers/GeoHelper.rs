// geo_helper.rs
use redis::AsyncCommands;

use crate::Share::Comman::error::{AppError, AppResult};
use crate::Share::Service::Redis::core::RedisConnection;

/// 위치 기반 매칭/검색
#[derive(Clone)]
pub struct GeoHelper {
    conn: RedisConnection,
    key: String,
    ttl: Option<u64>,
}

impl GeoHelper {
    pub fn new(conn: RedisConnection, key: impl Into<String>, ttl: Option<u64>) -> Self {
        Self { conn, key: key.into(), ttl }
    }

    /// GEOADD key lon lat member
    pub async fn addLocation(&self, lon: f64, lat: f64, member: &str) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let n: i64 = redis::cmd("GEOADD").arg(&self.key).arg(lon).arg(lat).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("GEOADD")))?;
        if let Some(sec) = self.ttl { 
            let _: bool = conn.expire(&self.key, sec as usize).await
                .map_err(|e| AppError::redis(e.to_string(), Some("EXPIRE")))?; 
        }
        Ok(n)
    }

    /// GEODIST key m1 m2 unit
    pub async fn getDistance(&self, m1: &str, m2: &str, unit: &str) -> AppResult<Option<f64>> {
        let mut conn = self.conn.clone();
        let d: Option<f64> = redis::cmd("GEODIST").arg(&self.key).arg(m1).arg(m2).arg(unit).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("GEODIST")))?;
        Ok(d)
    }

    /// GEOPOS key member…
    pub async fn getPositions(&self, members: &[&str]) -> AppResult<Vec<Option<(f64, f64)>>> {
        let mut conn = self.conn.clone();
        let mut cmd = redis::cmd("GEOPOS");
        cmd.arg(&self.key);
        for m in members { cmd.arg(m); }
        // 응답: [ [lon, lat], nil, [lon, lat], ... ]
        let raw: Vec<Option<(f64, f64)>> = cmd.query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("GEOPOS")))?;
        Ok(raw)
    }

    /// GEOSEARCH BYLONLAT … COUNT … WITHDIST (내부는 문자열 파싱 없이 원시 반환)
    pub async fn searchNearby(&self, lon: f64, lat: f64, radius: f64, unit: &str, count: usize, asc: bool) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        // 예: GEOSEARCH key FROMLONLAT 127.0 37.5 BYRADIUS 3 km ASC COUNT 50
        let order = if asc { "ASC" } else { "DESC" };
        let v: Vec<String> = redis::cmd("GEOSEARCH")
            .arg(&self.key)
            .arg("FROMLONLAT").arg(lon).arg(lat)
            .arg("BYRADIUS").arg(radius).arg(unit)
            .arg(order)
            .arg("COUNT").arg(count)
            .query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("GEOSEARCH")))?;
        Ok(v)
    }

    /// GEORADIUS key lon lat radius unit
    pub async fn getRadiusMembers(&self, lon: f64, lat: f64, radius: f64, unit: &str) -> AppResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let v: Vec<String> = redis::cmd("GEORADIUS")
            .arg(&self.key)
            .arg(lon).arg(lat).arg(radius).arg(unit)
            .query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("GEORADIUS")))?;
        Ok(v)
    }

    /// GEOREMOVE key member
    pub async fn removeLocation(&self, member: &str) -> AppResult<i64> {
        let mut conn = self.conn.clone();
        let n: i64 = redis::cmd("ZREM").arg(&self.key).arg(member).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("ZREM")))?;
        Ok(n)
    }

    /// ZCARD key (GEO는 내부적으로 ZSET 사용)
    pub async fn getLocationCount(&self) -> AppResult<usize> {
        let mut conn = self.conn.clone();
        let n: i64 = redis::cmd("ZCARD").arg(&self.key).query_async(&mut conn).await
            .map_err(|e| AppError::redis(e.to_string(), Some("ZCARD")))?;
        Ok(n.max(0) as usize)
    }
}
