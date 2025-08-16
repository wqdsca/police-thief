//! 이벤트 관리 도메인
//!
//! 게임 이벤트 및 보상 관리 기능을 담당합니다.

use actix_web::{web, HttpResponse, Result};
use chrono::{DateTime, Duration, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use shared::tool::error::AppError;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// 상수 정의
const EVENT_KEY_PREFIX: &str = "event:";
const ACTIVE_EVENTS_KEY: &str = "active_events";
const HOURS_TO_SECONDS: i64 = 3600;

/// 이벤트 보상 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventReward {
    pub event_id: String,
    pub event_name: String,
    pub reward_type: String,
    pub reward_amount: i32,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub is_active: bool,
    pub participants_count: usize,
}

/// 이벤트 생성 요청
#[derive(Debug, Deserialize)]
pub struct CreateEventRequest {
    pub event_name: String,
    pub reward_type: String,
    pub reward_amount: i32,
    pub duration_hours: u32,
}

/// 이벤트 저장소
pub struct EventRepository {
    redis_client: Option<Arc<redis::Client>>,
    active_events: Arc<RwLock<Vec<EventReward>>>,
}

impl EventRepository {
    /// 새 저장소 생성
    pub fn new(redis_client: Option<Arc<redis::Client>>) -> Self {
        Self {
            redis_client,
            active_events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Redis에서 이벤트 목록 로드
    pub async fn load_events(&self) -> Result<Vec<EventReward>, AppError> {
        let mut events = Vec::new();

        if let Some(ref client) = self.redis_client {
            let mut conn = client
                .get_multiplexed_tokio_connection()
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;

            let event_keys: Vec<String> = redis::cmd("KEYS")
                .arg(format!("{}*", EVENT_KEY_PREFIX))
                .query_async(&mut conn)
                .await
                .unwrap_or_default();

            for key in event_keys {
                if let Ok(event_data) = conn.get::<_, String>(&key).await {
                    if let Ok(event) = serde_json::from_str::<EventReward>(&event_data) {
                        events.push(event);
                    }
                }
            }
        }

        *self.active_events.write().await = events.clone();
        Ok(events)
    }

    /// 이벤트 생성
    pub async fn create_event(&self, request: CreateEventRequest) -> Result<EventReward, AppError> {
        let event = self.build_event(request);

        self.persist_event(&event).await?;
        self.add_to_active_events(&event).await?;
        self.add_to_memory_cache(event.clone()).await;

        Ok(event)
    }

    /// 이벤트 종료
    pub async fn end_event(&self, event_id: &str) -> Result<EventReward, AppError> {
        let mut events = self.active_events.write().await;

        let event = events
            .iter_mut()
            .find(|e| e.event_id == event_id)
            .ok_or_else(|| AppError::InternalError(format!("Event {} not found", event_id)))?;

        event.is_active = false;
        event.end_time = Utc::now();

        let updated_event = event.clone();
        self.update_event_in_redis(&updated_event).await?;

        Ok(updated_event)
    }

    /// 활성 이벤트 목록 조회
    pub async fn get_active_events(&self) -> Vec<EventReward> {
        let events = self.active_events.read().await;
        events
            .iter()
            .filter(|e| e.is_active && e.end_time > Utc::now())
            .cloned()
            .collect()
    }

    /// 모든 이벤트 목록 조회
    pub async fn get_all_events(&self) -> Vec<EventReward> {
        self.active_events.read().await.clone()
    }

    // Private helper methods

    fn build_event(&self, request: CreateEventRequest) -> EventReward {
        EventReward {
            event_id: Uuid::new_v4().to_string(),
            event_name: request.event_name,
            reward_type: request.reward_type,
            reward_amount: request.reward_amount,
            start_time: Utc::now(),
            end_time: Utc::now() + Duration::hours(request.duration_hours as i64),
            is_active: true,
            participants_count: 0,
        }
    }

    async fn persist_event(&self, event: &EventReward) -> Result<(), AppError> {
        if let Some(ref client) = self.redis_client {
            let mut conn = client
                .get_multiplexed_tokio_connection()
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;

            let event_key = format!("{}{}", EVENT_KEY_PREFIX, event.event_id);
            let event_json =
                serde_json::to_string(event).map_err(|e| AppError::InvalidFormat(e.to_string()))?;

            conn.set::<_, _, ()>(&event_key, &event_json)
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;

            // 이벤트 만료 시간 설정
            let ttl = (event.end_time - event.start_time).num_seconds();
            if ttl > 0 {
                conn.expire::<_, ()>(&event_key, ttl)
                    .await
                    .map_err(|e| AppError::RedisConnection(e.to_string()))?;
            }
        }
        Ok(())
    }

    async fn add_to_active_events(&self, event: &EventReward) -> Result<(), AppError> {
        if let Some(ref client) = self.redis_client {
            let mut conn = client
                .get_multiplexed_tokio_connection()
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;

            conn.sadd::<_, _, ()>(ACTIVE_EVENTS_KEY, &event.event_id)
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;
        }
        Ok(())
    }

    async fn update_event_in_redis(&self, event: &EventReward) -> Result<(), AppError> {
        self.persist_event(event).await?;

        if !event.is_active {
            if let Some(ref client) = self.redis_client {
                let mut conn = client
                    .get_multiplexed_tokio_connection()
                    .await
                    .map_err(|e| AppError::RedisConnection(e.to_string()))?;

                conn.srem::<_, _, ()>(ACTIVE_EVENTS_KEY, &event.event_id)
                    .await
                    .map_err(|e| AppError::RedisConnection(e.to_string()))?;
            }
        }

        Ok(())
    }

    async fn add_to_memory_cache(&self, event: EventReward) {
        let mut events = self.active_events.write().await;
        events.push(event);
    }
}

/// 이벤트 서비스
pub struct EventService {
    repository: Arc<EventRepository>,
}

impl EventService {
    /// 새 서비스 생성
    pub fn new(repository: Arc<EventRepository>) -> Self {
        Self { repository }
    }

    /// 이벤트 생성 (유효성 검사 포함)
    pub async fn create_event(&self, request: CreateEventRequest) -> Result<EventReward, AppError> {
        self.validate_create_request(&request)?;
        self.repository.create_event(request).await
    }

    /// 참가자 수 증가
    pub async fn increment_participants(&self, event_id: &str) -> Result<(), AppError> {
        let mut events = self.repository.active_events.write().await;

        if let Some(event) = events.iter_mut().find(|e| e.event_id == event_id) {
            event.participants_count += 1;
            // TODO: Redis 업데이트
        }

        Ok(())
    }

    // Private helper methods

    fn validate_create_request(&self, request: &CreateEventRequest) -> Result<(), AppError> {
        if request.event_name.is_empty() {
            return Err(AppError::InvalidInput(
                "Event name cannot be empty".to_string(),
            ));
        }

        if request.reward_amount <= 0 {
            return Err(AppError::InvalidInput(
                "Reward amount must be positive".to_string(),
            ));
        }

        if request.duration_hours == 0 {
            return Err(AppError::InvalidInput(
                "Duration must be at least 1 hour".to_string(),
            ));
        }

        Ok(())
    }
}

/// HTTP 핸들러
pub mod handlers {
    use super::*;

    /// 이벤트 목록 조회
    pub async fn get_events(repo: web::Data<Arc<EventRepository>>) -> Result<HttpResponse> {
        let events = repo.get_all_events().await;
        Ok(HttpResponse::Ok().json(events))
    }

    /// 이벤트 생성
    pub async fn create_event(
        service: web::Data<Arc<EventService>>,
        req: web::Json<CreateEventRequest>,
    ) -> Result<HttpResponse> {
        match service.create_event(req.into_inner()).await {
            Ok(event) => Ok(HttpResponse::Ok().json(event)),
            Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Failed to create event: {}", e)
            }))),
        }
    }

    /// 이벤트 종료
    pub async fn end_event(
        repo: web::Data<Arc<EventRepository>>,
        path: web::Path<String>,
    ) -> Result<HttpResponse> {
        let event_id = path.into_inner();

        match repo.end_event(&event_id).await {
            Ok(event) => Ok(HttpResponse::Ok().json(event)),
            Err(e) => Ok(HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Failed to end event: {}", e)
            }))),
        }
    }
}
