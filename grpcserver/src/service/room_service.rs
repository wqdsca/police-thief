//! Room Service Business Logic
//! 
//! 방 생성 및 조회 기능을 담당하는 비즈니스 로직입니다.
//! 실제 데이터베이스 연동 및 방 관련 비즈니스 규칙을 처리합니다.

use tracing::info;
use shared::tool::error::AppError;
// use shared::tool::get_id::RoomIdGenerator;
use shared::config::connection_pool::ConnectionPool;
use shared::service::redis::core::redis_get_key::KeyType;
use shared::service::redis::room_redis_service::{RoomRedisService, RoomRedisServiceConfig};
use shared::model::RoomInfo;

/// Room Service 비즈니스 로직
/// 
/// 방 생성 및 조회 기능을 처리하는 서비스입니다.
/// 현재는 더미 데이터를 반환하지만, 향후 실제 데이터베이스 연동이 추가될 예정입니다.
#[derive(Default)]
pub struct RoomService;

impl RoomService {
    /// 새로운 RoomService 인스턴스를 생성합니다.
    /// 
    /// # Returns
    /// * `Self` - 초기화된 RoomService 인스턴스
    pub fn new() -> Self { 
        Self 
    }

    /// 새로운 방을 생성합니다.
    /// 
    /// 사용자가 방을 생성할 때 호출되는 메서드입니다.
    /// 현재는 더미 데이터를 반환하지만, 향후 실제 데이터베이스에 방 정보를 저장할 예정입니다.
    /// 
    /// # Arguments
    /// * `user_id` - 방을 생성하는 사용자의 ID
    /// * `nick_name` - 방을 생성하는 사용자의 닉네임
    /// * `room_name` - 생성할 방의 이름
    /// * `max_player_num` - 방의 최대 플레이어 수
    /// 
    /// # Returns
    /// * `Result<i32, AppError>` - 생성된 방의 ID
    pub async fn make_room(
        &self,
        room_info: RoomInfo,
    ) -> Result<i32, AppError> {
        info!("방 생성 서비스 호출: room_info={:?}", room_info);
        let redis_config = ConnectionPool::get_config().await
            .map_err(|e| AppError::RedisConnection(e.to_string()))?;
        let room_redis_service = RoomRedisService::new(RoomRedisServiceConfig {
            redis_config,
            key_type: KeyType::RoomInfo,
        });
        let room_id = room_info.room_id as i32;
        let success: bool = room_redis_service.make_room(room_info).await?;
        if !success {
            return Err(AppError::DatabaseConnection("방 생성 실패".to_string()));
        } 

        // TODO: 실제 DB/Redis 로직 구현 필요
        // - 방 정보를 데이터베이스에 저장
        // - 방 생성자 정보 기록
        // - 방 상태 초기화
        info!("방 생성 완료: room_id={}", room_id);
        Ok(room_id)
    }

    /// 방 리스트를 조회합니다.
    /// 
    /// 사용자가 방 목록을 조회할 때 호출되는 메서드입니다.
    /// 마지막으로 조회한 방 ID 이후의 방들을 반환합니다.
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
        println!("DEBUG RoomService: 방 리스트 조회 서비스 호출: last_room_id={}", last_room_id);
        info!("방 리스트 조회 서비스 호출: last_room_id={}", last_room_id);
  
        // i32 → u16 변환 (음수 처리)
        let last_room_id_u16 = if last_room_id < 0 { 0 } else { last_room_id as u16 };
  
        // Redis 연결을 한 번만 생성 (optimized with connection pool)
        let redis_config = ConnectionPool::get_config().await
            .map_err(|e| AppError::RedisConnection(e.to_string()))?;
  
        let room_redis_service = RoomRedisService::new(RoomRedisServiceConfig {
            redis_config,
            key_type: KeyType::RoomInfo, // 방 정보 조회를 위해 RoomInfo 사용
        });
  
        println!("DEBUG RoomService: last_room_id_u16={}, Redis 서비스 호출 전", last_room_id_u16);
        let room_list: Vec<RoomInfo> = room_redis_service
            .get_room_list(last_room_id_u16)
            .await?;
        println!("DEBUG RoomService: Redis 서비스 호출 후, room_list.len()={}", room_list.len());
  
        if room_list.is_empty() {
            info!("방 리스트 조회 완료: 0개 방");
            return Ok(vec![]);
        }
  
        info!("방 리스트 조회 완료: {}개 방", room_list.len());
        Ok(room_list)
    }
}
