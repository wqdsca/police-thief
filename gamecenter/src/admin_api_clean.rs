//! 관리자 API 모듈 (클린 코드 버전)
//!
//! 도메인별로 분리된 관리자 API 엔드포인트

use actix_web::{web, HttpResponse, Result};
use shared::config::redis_config::RedisConfig;
use shared::tool::error::AppError;
use std::sync::Arc;

use crate::domain::{
    event_management::{handlers as event_handlers, EventRepository, EventService},
    monitoring::{handlers as monitoring_handlers, MonitoringService},
    user_management::{handlers as user_handlers, UserRepository},
};

/// 관리자 API 애플리케이션 상태
pub struct AdminApiState {
    pub monitoring_service: Arc<MonitoringService>,
    pub user_repository: Arc<UserRepository>,
    pub event_repository: Arc<EventRepository>,
    pub event_service: Arc<EventService>,
}

impl AdminApiState {
    /// 새 상태 생성
    pub fn new() -> Self {
        let monitoring = Arc::new(MonitoringService::new(None));
        let user_repo = Arc::new(UserRepository::new(None));
        let event_repo = Arc::new(EventRepository::new(None));
        let event_service = Arc::new(EventService::new(event_repo.clone()));

        Self {
            monitoring_service: monitoring,
            user_repository: user_repo,
            event_repository: event_repo,
            event_service,
        }
    }

    /// Redis 연결 초기화
    pub async fn with_redis(self) -> Result<Self, AppError> {
        let redis_config = RedisConfig::new()
            .await
            .map_err(|e| AppError::RedisConnection(e.to_string()))?;

        let client = redis::Client::open(format!(
            "redis://{}:{}",
            redis_config.host, redis_config.port
        ))
        .map_err(|e| AppError::RedisConnection(e.to_string()))?;

        let redis_client = Arc::new(client);

        // 각 서비스에 Redis 클라이언트 주입
        let monitoring = Arc::new(MonitoringService::new(Some(redis_client.clone())));
        let user_repo = Arc::new(UserRepository::new(Some(redis_client.clone())));
        let event_repo = Arc::new(EventRepository::new(Some(redis_client.clone())));

        // 기존 데이터 로드
        user_repo.load_banned_users().await?;
        event_repo.load_events().await?;

        let event_service = Arc::new(EventService::new(event_repo.clone()));

        Ok(Self {
            monitoring_service: monitoring,
            user_repository: user_repo,
            event_repository: event_repo,
            event_service,
        })
    }
}

/// 관리자 API 라우트 설정
pub fn configure_admin_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/admin")
            // 모니터링 라우트
            .service(web::scope("/monitoring").route(
                "/status",
                web::get().to(monitoring_handlers::get_server_status),
            ))
            // 유저 관리 라우트
            .service(
                web::scope("/users")
                    .route("/banned", web::get().to(user_handlers::get_banned_users))
                    .route("/ban", web::post().to(user_handlers::ban_user))
                    .route(
                        "/unban/{user_id}",
                        web::delete().to(user_handlers::unban_user),
                    ),
            )
            // 이벤트 관리 라우트
            .service(
                web::scope("/events")
                    .route("", web::get().to(event_handlers::get_events))
                    .route("", web::post().to(event_handlers::create_event))
                    .route("/{event_id}/end", web::put().to(event_handlers::end_event)),
            ),
    );
}

/// 헬스체크 엔드포인트
pub async fn health_check() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now()
    })))
}

/// 메트릭 엔드포인트
pub async fn metrics() -> Result<HttpResponse> {
    // TODO: Prometheus 메트릭 구현
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "metrics": "not_implemented"
    })))
}
