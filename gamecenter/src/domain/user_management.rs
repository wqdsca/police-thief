//! 유저 관리 도메인
//!
//! 유저 벤/언벤 등의 관리 기능을 담당합니다.

use actix_web::{web, HttpResponse, Result};
use chrono::{DateTime, Duration, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use shared::tool::error::AppError;
use std::sync::Arc;
use tokio::sync::RwLock;

// 상수 정의
const BAN_KEY_PREFIX: &str = "ban:";
const USER_KEY_PREFIX: &str = "user:";
const DEFAULT_USERNAME_PREFIX: &str = "User_";
const HOURS_TO_SECONDS: i64 = 3600;

/// 유저 벤 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBan {
    pub user_id: String,
    pub username: String,
    pub ban_reason: String,
    pub banned_at: DateTime<Utc>,
    pub banned_until: Option<DateTime<Utc>>,
    pub banned_by: String,
}

/// 벤 요청 데이터
#[derive(Debug, Deserialize)]
pub struct BanRequest {
    pub user_id: String,
    pub reason: String,
    pub duration_hours: Option<u32>,
    pub admin_id: String,
}

/// 유저 관리 저장소
pub struct UserRepository {
    redis_client: Option<Arc<redis::Client>>,
    banned_users: Arc<RwLock<Vec<UserBan>>>,
}

impl UserRepository {
    /// 새 저장소 생성
    pub fn new(redis_client: Option<Arc<redis::Client>>) -> Self {
        Self {
            redis_client,
            banned_users: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Redis에서 벤 목록 로드
    pub async fn load_banned_users(&self) -> Result<Vec<UserBan>, AppError> {
        let mut banned_users = Vec::new();

        if let Some(ref client) = self.redis_client {
            let mut conn = client
                .get_multiplexed_tokio_connection()
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;

            let ban_keys: Vec<String> = redis::cmd("KEYS")
                .arg(format!("{}*", BAN_KEY_PREFIX))
                .query_async(&mut conn)
                .await
                .unwrap_or_default();

            for key in ban_keys {
                if let Ok(ban_data) = conn.get::<_, String>(&key).await {
                    if let Ok(user_ban) = serde_json::from_str::<UserBan>(&ban_data) {
                        banned_users.push(user_ban);
                    }
                }
            }
        }

        *self.banned_users.write().await = banned_users.clone();
        Ok(banned_users)
    }

    /// 유저 벤 처리
    pub async fn ban_user(&self, request: BanRequest) -> Result<UserBan, AppError> {
        let username = self.fetch_username(&request.user_id).await?;
        let ban = self.create_ban_record(request, username);

        self.persist_ban(&ban).await?;
        self.invalidate_user_session(&ban.user_id).await?;
        self.add_to_memory_cache(ban.clone()).await;

        Ok(ban)
    }

    /// 유저 벤 해제
    pub async fn unban_user(&self, user_id: &str) -> Result<(), AppError> {
        self.remove_ban_from_redis(user_id).await?;
        self.remove_from_memory_cache(user_id).await;
        Ok(())
    }

    /// 벤 목록 조회
    pub async fn get_banned_users(&self) -> Vec<UserBan> {
        self.banned_users.read().await.clone()
    }

    // Private helper methods

    async fn fetch_username(&self, user_id: &str) -> Result<String, AppError> {
        if let Some(ref client) = self.redis_client {
            if let Ok(mut conn) = client.get_multiplexed_tokio_connection().await {
                let user_key = format!("{}{}", USER_KEY_PREFIX, user_id);
                if let Ok(name) = conn.hget::<_, _, String>(&user_key, "username").await {
                    return Ok(name);
                }
            }
        }
        Ok(format!("{}{}", DEFAULT_USERNAME_PREFIX, user_id))
    }

    fn create_ban_record(&self, request: BanRequest, username: String) -> UserBan {
        UserBan {
            user_id: request.user_id,
            username,
            ban_reason: request.reason,
            banned_at: Utc::now(),
            banned_until: request
                .duration_hours
                .map(|hours| Utc::now() + Duration::hours(hours as i64)),
            banned_by: request.admin_id,
        }
    }

    async fn persist_ban(&self, ban: &UserBan) -> Result<(), AppError> {
        if let Some(ref client) = self.redis_client {
            let mut conn = client
                .get_multiplexed_tokio_connection()
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;

            let ban_key = format!("{}{}", BAN_KEY_PREFIX, ban.user_id);
            let ban_json =
                serde_json::to_string(ban).map_err(|e| AppError::InvalidFormat(e.to_string()))?;

            conn.set::<_, _, ()>(&ban_key, &ban_json)
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;

            if let Some(banned_until) = ban.banned_until {
                let duration = (banned_until - ban.banned_at).num_seconds();
                conn.expire::<_, ()>(&ban_key, duration)
                    .await
                    .map_err(|e| AppError::RedisConnection(e.to_string()))?;
            }
        }
        Ok(())
    }

    async fn invalidate_user_session(&self, user_id: &str) -> Result<(), AppError> {
        if let Some(ref client) = self.redis_client {
            let mut conn = client
                .get_multiplexed_tokio_connection()
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;

            let user_key = format!("{}{}", USER_KEY_PREFIX, user_id);
            conn.del::<_, ()>(&user_key)
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;
        }
        Ok(())
    }

    async fn remove_ban_from_redis(&self, user_id: &str) -> Result<(), AppError> {
        if let Some(ref client) = self.redis_client {
            let mut conn = client
                .get_multiplexed_tokio_connection()
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;

            let ban_key = format!("{}{}", BAN_KEY_PREFIX, user_id);
            conn.del::<_, ()>(&ban_key)
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;
        }
        Ok(())
    }

    async fn add_to_memory_cache(&self, ban: UserBan) {
        let mut banned_users = self.banned_users.write().await;
        banned_users.push(ban);
    }

    async fn remove_from_memory_cache(&self, user_id: &str) {
        let mut banned_users = self.banned_users.write().await;
        banned_users.retain(|ban| ban.user_id != user_id);
    }
}

/// HTTP 핸들러
pub mod handlers {
    use super::*;

    /// 유저 벤 목록 조회
    pub async fn get_banned_users(repo: web::Data<Arc<UserRepository>>) -> Result<HttpResponse> {
        let banned_users = repo.get_banned_users().await;
        Ok(HttpResponse::Ok().json(banned_users))
    }

    /// 유저 벤 처리
    pub async fn ban_user(
        repo: web::Data<Arc<UserRepository>>,
        req: web::Json<BanRequest>,
    ) -> Result<HttpResponse> {
        match repo.ban_user(req.into_inner()).await {
            Ok(ban) => Ok(HttpResponse::Ok().json(ban)),
            Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to ban user: {}", e)
            }))),
        }
    }

    /// 유저 벤 해제
    pub async fn unban_user(
        repo: web::Data<Arc<UserRepository>>,
        path: web::Path<String>,
    ) -> Result<HttpResponse> {
        let user_id = path.into_inner();

        match repo.unban_user(&user_id).await {
            Ok(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
                "message": format!("User {} has been unbanned", user_id)
            }))),
            Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to unban user: {}", e)
            }))),
        }
    }
}
