// redis_config.rs
//! Redis 연결 설정(Production-Ready)
//!
//! - 전역 Mutex 제거: ConnectionManager는 Clone 가능 → 각 태스크가 복제본 사용(병목 ↓)
//! - 환경변수 로드 실패/연결 실패를 Result로 반환(운영 중 panic 회피)
//! - 헬스체크(PING) 제공
//! - 풍부한 주석으로 유지보수성 ↑
//
// 필요 크레이트 예시 (Cargo.toml)
// redis = { version = "0.23", features = ["tokio-comp", "connection-manager"] }
// dotenv = "0.15"          # 또는 dotenvy
// tracing = "0.1"          # 선택: 로깅/추적

use std::env;

use dotenv::dotenv;
use redis::{aio::ConnectionManager, Client, RedisError};
use tracing::{info, warn, error, instrument};

/// 고동시성 운영을 위한 권장 타입:
/// - Arc<Mutex<...>> 대신 **그냥 ConnectionManager 자체를 Clone**해서 사용.
/// - 각 비동기 태스크가 복제본을 로컬로 들고 네트워크 I/O 수행 → 전역 락 병목 제거.
pub type RedisConnection = ConnectionManager;

/// Redis 설정 및 커넥션 팩토리
#[derive(Clone)]
pub struct RedisConfig {
    /// 애플리케이션 전역에서 복제해 쓰는 커넥션 매니저
    conn: RedisConnection,
    /// 로깅/디버깅용 (선택)
    url: String,
}

impl RedisConfig {
    /// .env 로드 후 REDIS_URL을 읽어서 ConnectionManager 초기화
    ///
    /// - 실패 시 `RedisError`로 반환 (panic 없이 상위로 전파)
    /// - 예: REDIS_URL="redis://127.0.0.1:6379"
    #[instrument(name = "RedisConfig::new")]
    pub async fn new() -> Result<Self, RedisError> {
        // .env 로드 (없어도 ok)
        dotenv().ok();

        // 환경변수 읽기 (없으면 에러 반환)
        let url = match env::var("REDIS_URL") {
            Ok(v) => v,
            Err(_) => {
                // 운영 환경에서 명시적으로 실패시키고 상위에서 로깅/종료를 관리
                let err = RedisError::from((
                    redis::ErrorKind::InvalidClientConfig,
                    "REDIS_URL is missing",
                ));
                error!("환경변수 REDIS_URL 미설정");
                return Err(err);
            }
        };

        // Client 및 ConnectionManager 생성
        let client = Client::open(url.clone())?;
        let manager = ConnectionManager::new(client).await?;

        info!("Redis 연결 초기화 성공: {}", url);
        Ok(Self { conn: manager, url })
    }

    /// 커넥션 매니저 복제 반환
    ///
    /// - 각 작업은 `let mut conn = cfg.get_connection();` 후
    ///   `query_async(&mut conn).await?` 형태로 호출
    /// - 전역 락이 없어 동시성이 잘 나온다
    #[inline]
    pub fn get_connection(&self) -> RedisConnection {
        self.conn.clone()
    }

    /// 간단한 헬스체크: PING → PONG 확인
    ///
    /// - 애플리케이션 기동 시, 또는 주기적 상태 점검에 사용
    /// - 실패 시 RedisError로 반환
    #[instrument(level = "info", skip(self))]
    pub async fn health_check(&self) -> Result<(), RedisError> {
        let mut conn = self.get_connection();
        // PING 응답은 "PONG" (String) 을 기대
        let pong: String = redis::cmd("PING").query_async(&mut conn).await?;
        if pong == "PONG" {
            info!("Redis PING OK (url={})", self.url);
            Ok(())
        } else {
            warn!("Redis PING 비정상 응답: {}", pong);
            Err(RedisError::from((
                redis::ErrorKind::ResponseError,
                "unexpected PING response",
            )))
        }
    }
}

// ----------------------
// 사용 예
// ----------------------
//
// #[tokio::main]
// async fn main() -> anyhow::Result<()> {
//     // 초기화
//     let cfg = RedisConfig::new().await?;
//     cfg.health_check().await?; // PING 확인
//
//     // 사용
//     let mut conn = cfg.get_connection();
//     redis::cmd("SET").arg("k").arg("v").query_async::<_, ()>(&mut conn).await?;
//     let val: String = redis::cmd("GET").arg("k").query_async(&mut conn).await?;
//     println!("GET k = {}", val);
//
//     Ok(())
// }
