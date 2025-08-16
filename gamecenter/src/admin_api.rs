//! 관리자 API 모듈
//!
//! 서버 모니터링, 유저 관리, 이벤트 관리를 위한 REST API 엔드포인트

use actix_web::{web, HttpResponse, Result};
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use shared::config::redis_config::RedisConfig;
use shared::tool::error::AppError;
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::RwLock;

/// CPU 사용량 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuUsage {
    pub timestamp: DateTime<Utc>,
    pub total_usage: f32,
    pub core_usage: Vec<f32>,
    pub process_usage: f32,
}

/// 서버 상태 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatus {
    pub server_name: String,
    pub is_running: bool,
    pub uptime_seconds: u64,
    pub connected_clients: usize,
    pub memory_usage_mb: f64,
    pub cpu_usage: CpuUsage,
}

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

/// 벤 요청 데이터
#[derive(Debug, Deserialize)]
pub struct BanRequest {
    pub user_id: String,
    pub reason: String,
    pub duration_hours: Option<u32>,
    pub admin_id: String,
}

/// 이벤트 생성 요청
#[derive(Debug, Deserialize)]
pub struct CreateEventRequest {
    pub event_name: String,
    pub reward_type: String,
    pub reward_amount: i32,
    pub duration_hours: u32,
}

/// 관리자 API 상태
pub struct AdminApiState {
    pub system: Arc<RwLock<System>>,
    pub banned_users: Arc<RwLock<Vec<UserBan>>>,
    pub active_events: Arc<RwLock<Vec<EventReward>>>,
    pub redis_client: Option<Arc<redis::Client>>,
    pub server_stats: Arc<RwLock<ServerStats>>,
}

/// 서버 통계
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerStats {
    pub total_connections: usize,
    pub active_rooms: usize,
    pub total_messages: u64,
    pub uptime_start: DateTime<Utc>,
}

impl AdminApiState {
    pub fn new() -> Self {
        Self {
            system: Arc::new(RwLock::new(System::new_all())),
            banned_users: Arc::new(RwLock::new(Vec::new())),
            active_events: Arc::new(RwLock::new(Vec::new())),
            redis_client: None,
            server_stats: Arc::new(RwLock::new(ServerStats {
                uptime_start: Utc::now(),
                ..Default::default()
            })),
        }
    }

    /// Redis 연결 초기화
    pub async fn with_redis(mut self) -> Result<Self, AppError> {
        let redis_config = RedisConfig::new()
            .await
            .map_err(|e| AppError::RedisConnection(e.to_string()))?;
        let client = redis::Client::open(format!(
            "redis://{}:{}",
            redis_config.host, redis_config.port
        ))
        .map_err(|e| AppError::RedisConnection(e.to_string()))?;

        self.redis_client = Some(Arc::new(client));

        // Redis에서 기존 벤 목록 로드
        if let Some(ref client) = self.redis_client {
            let mut conn = client
                .get_multiplexed_tokio_connection()
                .await
                .map_err(|e| AppError::RedisConnection(e.to_string()))?;

            // 벤 유저 목록 로드
            let ban_keys: Vec<String> = redis::cmd("KEYS")
                .arg("ban:*")
                .query_async(&mut conn)
                .await
                .unwrap_or_default();

            let mut banned_users = Vec::new();
            for key in ban_keys {
                let ban_data: Result<String, _> = conn.get(&key).await;
                if let Ok(data) = ban_data {
                    if let Ok(user_ban) = serde_json::from_str::<UserBan>(&data) {
                        banned_users.push(user_ban);
                    }
                }
            }
            *self.banned_users.write().await = banned_users;

            // 이벤트 목록 로드
            let event_keys: Vec<String> = redis::cmd("KEYS")
                .arg("event:*")
                .query_async(&mut conn)
                .await
                .unwrap_or_default();

            let mut events = Vec::new();
            for key in event_keys {
                let event_data: Result<String, _> = conn.get(&key).await;
                if let Ok(data) = event_data {
                    if let Ok(event) = serde_json::from_str::<EventReward>(&data) {
                        events.push(event);
                    }
                }
            }
            *self.active_events.write().await = events;
        }

        Ok(self)
    }
}

/// 서버 상태 조회 API
pub async fn get_server_status(data: web::Data<AdminApiState>) -> Result<HttpResponse> {
    let mut system = data.system.write().await;
    system.refresh_all();

    let total_cpu = system.global_cpu_info().cpu_usage();
    let core_usage: Vec<f32> = system.cpus().iter().map(|cpu| cpu.cpu_usage()).collect();

    let process_usage = 0.0; // sysinfo 0.30에서는 프로세스별 CPU 사용량을 다르게 처리
    let memory_usage = system.used_memory() as f64 / 1024.0 / 1024.0;

    // Redis에서 실시간 통계 가져오기
    let mut connected_clients = 0;
    if let Some(ref client) = data.redis_client {
        if let Ok(mut conn) = client.get_multiplexed_tokio_connection().await {
            // 활성 유저 수 카운트
            if let Ok(user_keys) = redis::cmd("KEYS")
                .arg("user:*")
                .query_async::<_, Vec<String>>(&mut conn)
                .await
            {
                connected_clients = user_keys.len();
            }
        }
    }

    let stats = data.server_stats.read().await;
    let uptime_seconds = (Utc::now() - stats.uptime_start).num_seconds() as u64;

    let status = ServerStatus {
        server_name: "GameCenter Unified Server".to_string(),
        is_running: true,
        uptime_seconds,
        connected_clients,
        memory_usage_mb: memory_usage,
        cpu_usage: CpuUsage {
            timestamp: Utc::now(),
            total_usage: total_cpu,
            core_usage,
            process_usage,
        },
    };

    Ok(HttpResponse::Ok().json(status))
}

/// 모든 서버 상태 조회
pub async fn get_all_servers_status(data: web::Data<AdminApiState>) -> Result<HttpResponse> {
    let mut system = data.system.write().await;
    system.refresh_all();

    let mut servers = Vec::new();

    // 간단한 서버 상태 (sysinfo 0.30에서는 프로세스별 조회가 복잡함)
    servers.push(ServerStatus {
        server_name: "TCP Server".to_string(),
        is_running: true,
        uptime_seconds: 0,
        connected_clients: 0,
        memory_usage_mb: 0.0,
        cpu_usage: CpuUsage {
            timestamp: Utc::now(),
            total_usage: system.global_cpu_info().cpu_usage(),
            core_usage: vec![],
            process_usage: 0.0,
        },
    });

    servers.push(ServerStatus {
        server_name: "gRPC Server".to_string(),
        is_running: true,
        uptime_seconds: 0,
        connected_clients: 0,
        memory_usage_mb: 0.0,
        cpu_usage: CpuUsage {
            timestamp: Utc::now(),
            total_usage: system.global_cpu_info().cpu_usage(),
            core_usage: vec![],
            process_usage: 0.0,
        },
    });

    servers.push(ServerStatus {
        server_name: "RUDP Server".to_string(),
        is_running: true,
        uptime_seconds: 0,
        connected_clients: 0,
        memory_usage_mb: 0.0,
        cpu_usage: CpuUsage {
            timestamp: Utc::now(),
            total_usage: system.global_cpu_info().cpu_usage(),
            core_usage: vec![],
            process_usage: 0.0,
        },
    });

    Ok(HttpResponse::Ok().json(servers))
}

/// 유저 벤 목록 조회
pub async fn get_banned_users(data: web::Data<AdminApiState>) -> Result<HttpResponse> {
    let banned_users = data.banned_users.read().await;
    Ok(HttpResponse::Ok().json(banned_users.clone()))
}

/// 유저 벤 처리
pub async fn ban_user(
    data: web::Data<AdminApiState>,
    req: web::Json<BanRequest>,
) -> Result<HttpResponse> {
    // 실제 유저명 조회
    let mut username = format!("User_{}", req.user_id);
    if let Some(ref client) = data.redis_client {
        if let Ok(mut conn) = client.get_multiplexed_tokio_connection().await {
            let user_key = format!("user:{}", req.user_id);
            if let Ok(name) = conn.hget::<_, _, String>(&user_key, "username").await {
                username = name;
            }
        }
    }

    let ban = UserBan {
        user_id: req.user_id.clone(),
        username,
        ban_reason: req.reason.clone(),
        banned_at: Utc::now(),
        banned_until: req
            .duration_hours
            .map(|hours| Utc::now() + chrono::Duration::hours(hours as i64)),
        banned_by: req.admin_id.clone(),
    };

    // Redis에 벤 정보 저장
    if let Some(ref client) = data.redis_client {
        if let Ok(mut conn) = client.get_multiplexed_tokio_connection().await {
            let ban_key = format!("ban:{}", req.user_id);
            let ban_json = serde_json::to_string(&ban).unwrap_or_default();

            // 벤 정보 저장
            let _: Result<(), _> = conn.set(&ban_key, &ban_json).await;

            // 만료 시간 설정 (있는 경우)
            if let Some(duration_hours) = req.duration_hours {
                let _: Result<(), _> = conn.expire(&ban_key, (duration_hours * 3600) as i64).await;
            }

            // 유저 세션 무효화
            let user_key = format!("user:{}", req.user_id);
            let _: Result<(), _> = conn.del(&user_key).await;
        }
    }

    // 메모리에도 저장
    let mut banned_users = data.banned_users.write().await;
    banned_users.push(ban.clone());

    Ok(HttpResponse::Ok().json(ban))
}

/// 유저 벤 해제
pub async fn unban_user(
    data: web::Data<AdminApiState>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let user_id = path.into_inner();

    // Redis에서 벤 정보 삭제
    if let Some(ref client) = data.redis_client {
        if let Ok(mut conn) = client.get_multiplexed_tokio_connection().await {
            let ban_key = format!("ban:{}", user_id);
            let _: Result<(), _> = conn.del(&ban_key).await;
        }
    }

    // 메모리에서도 삭제
    let mut banned_users = data.banned_users.write().await;
    banned_users.retain(|ban| ban.user_id != user_id);

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("User {} has been unbanned", user_id)
    })))
}

/// 이벤트 목록 조회
pub async fn get_events(data: web::Data<AdminApiState>) -> Result<HttpResponse> {
    let events = data.active_events.read().await;
    Ok(HttpResponse::Ok().json(events.clone()))
}

/// 이벤트 생성
pub async fn create_event(
    data: web::Data<AdminApiState>,
    req: web::Json<CreateEventRequest>,
) -> Result<HttpResponse> {
    let event = EventReward {
        event_id: uuid::Uuid::new_v4().to_string(),
        event_name: req.event_name.clone(),
        reward_type: req.reward_type.clone(),
        reward_amount: req.reward_amount,
        start_time: Utc::now(),
        end_time: Utc::now() + chrono::Duration::hours(req.duration_hours as i64),
        is_active: true,
        participants_count: 0,
    };

    // Redis에 이벤트 정보 저장
    if let Some(ref client) = data.redis_client {
        if let Ok(mut conn) = client.get_multiplexed_tokio_connection().await {
            let event_key = format!("event:{}", event.event_id);
            let event_json = serde_json::to_string(&event).unwrap_or_default();

            // 이벤트 정보 저장
            let _: Result<(), _> = conn.set(&event_key, &event_json).await;

            // 이벤트 만료 시간 설정
            let ttl = (req.duration_hours * 3600) as i64;
            let _: Result<(), _> = conn.expire(&event_key, ttl).await;

            // 활성 이벤트 목록에 추가
            let _: Result<(), _> = conn.sadd("active_events", &event.event_id).await;
        }
    }

    // 메모리에도 저장
    let mut events = data.active_events.write().await;
    events.push(event.clone());

    Ok(HttpResponse::Ok().json(event))
}

/// 이벤트 종료
pub async fn end_event(
    data: web::Data<AdminApiState>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let event_id = path.into_inner();
    let mut events = data.active_events.write().await;

    if let Some(event) = events.iter_mut().find(|e| e.event_id == event_id) {
        event.is_active = false;
        event.end_time = Utc::now();

        // TODO: Redis에서 이벤트 상태 업데이트

        Ok(HttpResponse::Ok().json(event.clone()))
    } else {
        Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Event not found"
        })))
    }
}

/// 관리자 API 라우트 설정
pub fn configure_admin_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/admin")
            .route("/status", web::get().to(get_server_status))
            .route("/servers", web::get().to(get_all_servers_status))
            .route("/users/banned", web::get().to(get_banned_users))
            .route("/users/ban", web::post().to(ban_user))
            .route("/users/unban/{user_id}", web::delete().to(unban_user))
            .route("/events", web::get().to(get_events))
            .route("/events", web::post().to(create_event))
            .route("/events/{event_id}/end", web::put().to(end_event)),
    );
}
