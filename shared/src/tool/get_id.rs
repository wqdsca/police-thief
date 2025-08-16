//! 방 ID 생성 유틸리티

use crate::config::redis_config::RedisConfig;
use crate::service::redis::core::redis_get_key::KeyType;
use anyhow::{anyhow, Result};
use redis::AsyncCommands;

/// 방 ID 생성기
#[derive(Clone)]
pub struct RoomIdGenerator {
    redis_config: RedisConfig,
    key_type: KeyType,
}

impl RoomIdGenerator {
    /// 환경변수를 사용하여 생성기를 생성합니다.
    pub async fn from_env() -> Result<Self> {
        let redis_config = RedisConfig::new()
            .await
            .map_err(|e| anyhow!("Redis 설정 생성 실패: {}", e))?;
        Ok(Self {
            redis_config,
            key_type: KeyType::RoomId,
        })
    }

    /// 방 ID를 생성합니다.
    pub async fn get_room_id(&mut self) -> Result<u16> {
        let mut conn = self.redis_config.get_connection();

        // 재활용 풀에서 ID 가져오기 시도
        let recycle_key = self.key_type.get_index_key();

        match conn.lpop::<&str, Option<u16>>(&recycle_key, None).await {
            Ok(Some(recycled_id)) => Ok(recycled_id),
            _ => {
                // 새로운 ID 생성
                let counter_key = "room_counter:id";
                let new_id: u16 = conn
                    .incr(counter_key, 1)
                    .await
                    .map_err(|e| anyhow!("ID 카운터 증가 실패: {}", e))?;
                Ok(new_id)
            }
        }
    }

    /// 방 ID를 반납합니다.
    pub async fn return_room_id(&mut self, room_id: u16) -> Result<()> {
        let mut conn = self.redis_config.get_connection();
        let recycle_key = self.key_type.get_index_key();

        let _: () = conn
            .lpush(&recycle_key, room_id)
            .await
            .map_err(|e| anyhow!("재활용 풀에 ID 반납 실패: {}", e))?;

        Ok(())
    }
}

mod tests {

    #[tokio::test]
    async fn test_room_id_generation() {
        let generator_result = RoomIdGenerator::from_env().await;

        let mut generator = match generator_result {
            Ok(g) => g,
            Err(_) => {
                println!("Redis 서버가 실행되지 않아 테스트를 건너뜁니다.");
                return;
            }
        };

        // 방 ID 생성 테스트
        if let Ok(room_id) = generator.get_room_id().await {
            println!("방 ID 생성 성공: {}", room_id);

            // 방 ID 반납 테스트
            let _ = generator.return_room_id(room_id).await;
        }
    }
}
