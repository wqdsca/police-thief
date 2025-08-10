//! Room Service Business Logic
//! 
//! 방 생성 및 조회 기능을 담당하는 비즈니스 로직입니다.
//! Redis 연결 풀과 최적화된 서비스 인스턴스를 사용하여 고성능을 달성합니다.

use tracing::info;
use std::sync::Arc;
use tokio::sync::OnceCell;
use shared::tool::error::AppError;
use shared::config::connection_pool::ConnectionPool;
use shared::service::redis::core::redis_get_key::KeyType;
use shared::service::redis::room_redis_service::{RoomRedisService, RoomRedisServiceConfig};
use shared::model::RoomInfo;

/// 최적화된 Redis 서비스 인스턴스 (싱글톤)
static ROOM_REDIS_SERVICE: OnceCell<Arc<RoomRedisService>> = OnceCell::const_new();

/// Room Service 비즈니스 로직
/// 
/// 방 생성 및 조회 기능을 처리하는 최적화된 서비스입니다.
/// Redis 연결 풀과 서비스 인스턴스를 재사용하여 성능을 극대화합니다.
pub struct RoomService;

impl RoomService {
    /// 새로운 RoomService 인스턴스를 생성합니다.
    /// 
    /// # Returns
    /// * `Self` - 초기화된 RoomService 인스턴스
    pub fn new() -> Self { 
        Self 
    }

    /// 최적화된 Redis 서비스 인스턴스를 가져옵니다.
    /// 
    /// 싱글톤 패턴으로 한 번만 초기화하고 재사용합니다.
    /// 
    /// # Returns
    /// * `Result<Arc<RoomRedisService>, AppError>` - Redis 서비스 인스턴스
    async fn get_redis_service(&self) -> Result<Arc<RoomRedisService>, AppError> {
        ROOM_REDIS_SERVICE
            .get_or_try_init(|| async {
                let redis_config = ConnectionPool::get_config().await
                    .map_err(|e| AppError::RedisConnection(e.to_string()))?;
                
                let room_redis_service = RoomRedisService::new(RoomRedisServiceConfig {
                    redis_config,
                    key_type: KeyType::RoomInfo,
                });
                
                Ok(Arc::new(room_redis_service))
            })
            .await
            .cloned()
    }

    /// 새로운 방을 생성합니다.
    /// 
    /// 최적화된 Redis 서비스 인스턴스를 재사용하여 방을 생성합니다.
    /// 
    /// # Arguments
    /// * `room_info` - 생성할 방의 정보
    /// 
    /// # Returns
    /// * `Result<i32, AppError>` - 생성된 방의 ID
    pub async fn make_room(
        &self,
        room_info: RoomInfo,
    ) -> Result<i32, AppError> {
        info!("방 생성 서비스 호출: room_info={:?}", room_info);
        
        // 최적화된 Redis 서비스 인스턴스 사용
        let room_redis_service = self.get_redis_service().await?;
        let room_id = room_info.room_id as i32;
        
        let success: bool = room_redis_service.make_room(room_info).await?;
        if !success {
            return Err(AppError::DatabaseConnection("방 생성 실패".to_string()));
        } 

        info!("방 생성 완료: room_id={}", room_id);
        Ok(room_id)
    }

    /// 방 리스트를 조회합니다.
    /// 
    /// 최적화된 Redis 서비스 인스턴스를 재사용하여 방 목록을 조회합니다.
    /// 
    /// # Arguments
    /// * `last_room_id` - 마지막으로 조회한 방의 ID (페이징 처리용)
    /// 
    /// # Returns
    /// * `Result<Vec<RoomInfo>, AppError>` - 방 정보 리스트
    pub async fn get_room_list(
        &self,
        last_room_id: i32,
    ) -> Result<Vec<RoomInfo>, AppError> {
        info!("방 리스트 조회 서비스 호출: last_room_id={}", last_room_id);
  
        // i32 → u16 변환 (음수 처리)
        let last_room_id_u16 = if last_room_id < 0 { 0 } else { last_room_id as u16 };
  
        // 최적화된 Redis 서비스 인스턴스 사용
        let room_redis_service = self.get_redis_service().await?;
        
        let room_list: Vec<RoomInfo> = room_redis_service
            .get_room_list(last_room_id_u16)
            .await?;
  
        if room_list.is_empty() {
            info!("방 리스트 조회 완료: 0개 방");
            return Ok(vec![]);
        }
  
        info!("방 리스트 조회 완료: {}개 방", room_list.len());
        Ok(room_list)
    }
}
