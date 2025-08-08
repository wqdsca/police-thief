use crate::config::redis_config::RedisConfig;
use crate::service::redis::core::redis_get_key::KeyType;
use crate::model::UserInfo;
use crate::tool::error::AppError;
use redis::Value;

#[derive(Debug, Clone)]
pub struct UserRedisServiceConfig {
    pub redis_config: RedisConfig,
    pub key_type: KeyType,
}

#[derive(Debug, Clone)]
pub struct UserRedisService {
    config: UserRedisServiceConfig,
}

impl UserRedisService {
    pub fn new(config: UserRedisServiceConfig) -> Self {
        Self { config }
    }

    // 로그인 성공 시 호출하는 서비스 
    pub async fn login_success_redis_service(&self, user_id: i32, user_info: &UserInfo) -> Result<bool, AppError> {
        let mut conn = self.config.redis_config.get_connection();
        let user_id_u16: u16 = user_id as u16; // i32 → u16 변환
        let user_key: String = self.config.key_type.get_key(&user_id_u16); // 키 한 번만 생성
        
        let mut p = redis::pipe();
        p.hset_multiple(&user_key, &[
            ("nick_name", &user_info.nick_name),
            ("access_token", &user_info.access_token),
        ]);
        p.expire(&user_key, 3600);
        
        let _resp: Vec<Value> = p.query_async(&mut conn).await
            .map_err(|e| AppError::RedisConnection(e.to_string()))?;
        Ok(true)
    }
    pub async fn logout_redis_service(&self, user_id: i32) -> Result<bool, AppError> {
        let mut conn = self.config.redis_config.get_connection();
        let user_id_u16 = user_id as u16; // i32 → u16 변환
        let user_key = self.config.key_type.get_key(&user_id_u16);
        
        let mut p = redis::pipe();
        p.del(&user_key);
        
        let _resp: Vec<Value> = p.query_async(&mut conn).await
            .map_err(|e| AppError::RedisConnection(e.to_string()))?;
        Ok(true)
    }
}