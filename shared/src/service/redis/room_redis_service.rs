use crate::config::{redis_config::RedisConfig, connection_pool::ConnectionPool};
use crate::service::redis::core::redis_get_key::KeyType;
use crate::service::redis::hepler::zset_helper::ZSetHelper;
use crate::model::RoomInfo;
use crate::tool::error::AppError;
use redis::Value;
use crate::tool::current_time::CurrentTime;

#[derive(Debug, Clone)]
pub struct RoomRedisServiceConfig {
    pub redis_config: RedisConfig,
    pub key_type: KeyType,
}

#[derive(Debug, Clone)]
pub struct RoomRedisService {
    pub config: RoomRedisServiceConfig,
}

impl RoomRedisService {
    pub fn new(config: RoomRedisServiceConfig) -> Self {
        Self {
            config,
        }
    }

    // 방 만들기 서비스 
    pub async fn make_room(&self, room_info: RoomInfo) -> Result<bool, AppError> {
        let mut conn = self.config.redis_config.get_connection();
        let mut p = redis::pipe();
        let room_id = room_info.room_id;
        if room_id == 0 {
            return Err(AppError::InvalidInput("룸 아이디가 필요합니다".to_string()));
        }
       p.hset_multiple(self.config.key_type.get_key(&room_id), &[
        ("room_name", &room_info.room_name),
        ("max_player_num", &room_info.max_player_num.to_string()),
        ("current_player_num", &room_info.current_player_num.to_string()),
        ("create_at", &room_info.create_at),
       ]);
       p.expire(self.config.key_type.get_key(&room_id), 3600);  
       let zset_key = KeyType::RoomListByTime.get_index_key();
       let current_time_instance = CurrentTime::new();

       // 2. time_string_to_int() 메서드를 호출하여 '점수'로 사용할 숫자를 가져옵니다.
       //    (주의: 이 메서드는 이름과 달리 저장된 시간이 아닌 '새로운' 현재 시간의 타임스탬프를 반환합니다.)
       let score = current_time_instance.time_string_to_int();
       
       // 3. zadd 명령어에 (멤버, 점수) 순서로 올바른 값을 전달합니다.
       //    i32 타입의 score를 i64로 캐스팅하여 전달하는 것이 안전합니다.
       p.zadd(
           zset_key,
           room_id.to_string(), // 멤버 (Member)
           score,               // 점수 (Score)
       );
        let _resp: Vec<Value> = p.query_async(&mut conn).await
            .map_err(|e| AppError::RedisConnection(e.to_string()))?;
        Ok(true)
        
    }

    // 방조회 하는 서비스 lastId 기반으로 그 이상되는 걸 조회 (최신순 정렬, 20개 제한)
    pub async fn get_room_list(&self, last_id: u16) -> Result<Vec<RoomInfo>, AppError> {
        let zset_helper = ZSetHelper::new(
            self.config.redis_config.clone(), 
            KeyType::RoomListByTime, 
            None, 
            Some(20)
        );
        
        // 1. last_id의 타임스탬프 조회 (페이징 기준점)
        let last_id_time = if last_id == 0 {
            // 첫 페이지는 현재 시간부터
            f64::INFINITY
        } else {
            zset_helper.get_member_score(last_id as i64)
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?
                .unwrap_or(0.0)
        };
        
        // 2. 해당 시간 이전의 방 목록 조회 (최신순, 20개 제한)
        // ZREVRANGEBYSCORE를 위해 min_score는 -inf, max_score는 타임스탬프 기준점 사용
        println!("DEBUG RoomRedisService: last_id={}, last_id_time={}", last_id, last_id_time);
        println!("DEBUG RoomRedisService: NEG_INFINITY={}, last_id_time={}", f64::NEG_INFINITY, last_id_time);
        
        // 페이징에서 중복 방지를 위해 last_id보다 작은 타임스탬프만 조회
        let max_score = if last_id == 0 {
            last_id_time  // 첫 페이지는 현재 시간(inf) 기준
        } else {
            last_id_time - 0.0001  // 이전 페이지 경계값 제외 (마이크로초 단위 차이)
        };
        
        let room_id_list: Vec<String> = zset_helper
            .get_range_by_score(f64::NEG_INFINITY, max_score)
            .await
            .map_err(|e| AppError::RedisConnection(e.to_string()))?;
            
        println!("DEBUG RoomRedisService: room_id_list={:?}", room_id_list);
        println!("DEBUG RoomRedisService: room_id_list.len()={}", room_id_list.len());
        
        // 3. 파이프라인으로 모든 방 정보를 한 번에 조회 (성능 최적화 - connection pool)
        let mut room_list = Vec::with_capacity(room_id_list.len());
        let mut conn = ConnectionPool::get_connection().await
            .map_err(|e| AppError::RedisConnection(e.to_string()))?;
        
        if !room_id_list.is_empty() {
            let mut pipe = redis::pipe();
            let mut valid_room_ids = Vec::new();
            
            // 파이프라인에 모든 HGETALL 명령 추가
            for room_id_str in &room_id_list {
                if let Ok(room_id) = room_id_str.parse::<u16>() {
                    let room_key = self.config.key_type.get_key(&room_id);
                    pipe.hgetall(&room_key);
                    valid_room_ids.push(room_id);
                }
            }
            
            // 파이프라인 실행 - 모든 방 정보를 한 번에 가져오기
            let results: Vec<std::collections::HashMap<String, String>> = pipe
                .query_async(&mut conn)
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;
            
            // 결과를 RoomInfo로 변환
            for (i, room_data) in results.into_iter().enumerate() {
                if !room_data.is_empty() && i < valid_room_ids.len() {
                    let room_id = valid_room_ids[i];
                    let room_info = RoomInfo {
                        room_id,
                        room_name: room_data.get("room_name").cloned().unwrap_or_default(),
                        max_player_num: room_data
                            .get("max_player_num")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0),
                        current_player_num: room_data
                            .get("current_player_num")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0),
                        create_at: room_data.get("create_at").cloned().unwrap_or_default(),
                    };
                    room_list.push(room_info);
                }
            }
        }
        
        Ok(room_list)
    }
}
