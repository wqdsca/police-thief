//! 서버 모니터링 도메인
//!
//! 서버 상태, CPU, 메모리 등의 모니터링 기능을 담당합니다.

use actix_web::{web, HttpResponse, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::RwLock;

// 상수 정의
const BYTES_TO_MB: f64 = 1024.0 * 1024.0;
const REFRESH_INTERVAL_SECS: u64 = 5;
const DEFAULT_SERVER_NAME: &str = "GameCenter Unified Server";

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

/// 서버 통계
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerStats {
    pub total_connections: usize,
    pub active_rooms: usize,
    pub total_messages: u64,
    pub uptime_start: DateTime<Utc>,
}

/// 모니터링 서비스
pub struct MonitoringService {
    system: Arc<RwLock<System>>,
    stats: Arc<RwLock<ServerStats>>,
    redis_client: Option<Arc<redis::Client>>,
}

impl MonitoringService {
    /// 새 모니터링 서비스 생성
    pub fn new(redis_client: Option<Arc<redis::Client>>) -> Self {
        Self {
            system: Arc::new(RwLock::new(System::new_all())),
            stats: Arc::new(RwLock::new(ServerStats {
                uptime_start: Utc::now(),
                ..Default::default()
            })),
            redis_client,
        }
    }

    /// 서버 상태 조회
    pub async fn get_server_status(&self) -> Result<ServerStatus> {
        let cpu_usage = self.collect_cpu_metrics().await;
        let memory_mb = self.collect_memory_metrics().await;
        let connected_clients = self.count_connected_clients().await;
        let uptime_seconds = self.calculate_uptime().await;

        Ok(ServerStatus {
            server_name: DEFAULT_SERVER_NAME.to_string(),
            is_running: true,
            uptime_seconds,
            connected_clients,
            memory_usage_mb: memory_mb,
            cpu_usage,
        })
    }

    /// CPU 메트릭 수집
    async fn collect_cpu_metrics(&self) -> CpuUsage {
        let mut system = self.system.write().await;
        system.refresh_all();

        CpuUsage {
            timestamp: Utc::now(),
            total_usage: system.global_cpu_info().cpu_usage(),
            core_usage: system.cpus().iter().map(|cpu| cpu.cpu_usage()).collect(),
            process_usage: 0.0, // sysinfo 0.30에서는 프로세스별 CPU 사용량이 복잡함
        }
    }

    /// 메모리 메트릭 수집
    async fn collect_memory_metrics(&self) -> f64 {
        let system = self.system.read().await;
        system.used_memory() as f64 / BYTES_TO_MB
    }

    /// 연결된 클라이언트 수 조회
    async fn count_connected_clients(&self) -> usize {
        if let Some(ref client) = self.redis_client {
            if let Ok(mut conn) = client.get_multiplexed_tokio_connection().await {
                if let Ok(keys) = redis::cmd("KEYS")
                    .arg("user:*")
                    .query_async::<_, Vec<String>>(&mut conn)
                    .await
                {
                    return keys.len();
                }
            }
        }
        0
    }

    /// 서버 가동 시간 계산
    async fn calculate_uptime(&self) -> u64 {
        let stats = self.stats.read().await;
        (Utc::now() - stats.uptime_start).num_seconds() as u64
    }

    /// 통계 업데이트
    pub async fn update_stats(&self, delta: ServerStatsDelta) {
        let mut stats = self.stats.write().await;
        stats.total_connections += delta.new_connections;
        if delta.room_change > 0 {
            stats.active_rooms += delta.room_change as usize;
        } else if delta.room_change < 0 && stats.active_rooms > 0 {
            let decrease = (-delta.room_change) as usize;
            stats.active_rooms = stats.active_rooms.saturating_sub(decrease);
        }
        stats.total_messages += delta.new_messages;
    }
}

/// 서버 통계 변경 사항
pub struct ServerStatsDelta {
    pub new_connections: usize,
    pub room_change: isize,
    pub new_messages: u64,
}

/// HTTP 핸들러
pub mod handlers {
    use super::*;

    /// 서버 상태 조회 API
    pub async fn get_server_status(
        service: web::Data<Arc<MonitoringService>>,
    ) -> Result<HttpResponse> {
        match service.get_server_status().await {
            Ok(status) => Ok(HttpResponse::Ok().json(status)),
            Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to get server status: {}", e)
            }))),
        }
    }
}
