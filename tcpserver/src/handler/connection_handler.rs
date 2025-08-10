//! 연결 핸들러
//! 
//! 클라이언트 연결/해제 처리를 담당합니다.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::io::BufReader;
use tracing::{info, warn, debug, error};

use crate::service::{ConnectionService, HeartbeatService, MessageService};
use crate::protocol::GameMessage;
use crate::tool::{NetworkUtils, IpInfo, ConnectionQuality};
use shared::config::redis_config::RedisConfig;
use shared::service::redis::core::redis_get_key::KeyType;
use redis::AsyncCommands;

/// 연결 핸들러
pub struct ConnectionHandler {
    connection_service: Arc<ConnectionService>,
    heartbeat_service: Arc<HeartbeatService>,
    message_service: Arc<MessageService>,
    redis_config: Option<Arc<RedisConfig>>,
}

impl ConnectionHandler {
    /// 새로운 연결 핸들러 생성
    pub fn new(
        connection_service: Arc<ConnectionService>,
        heartbeat_service: Arc<HeartbeatService>,
        message_service: Arc<MessageService>,
    ) -> Self {
        Self {
            connection_service,
            heartbeat_service,
            message_service,
            redis_config: None,
        }
    }
    
    /// Redis 설정 추가
    pub async fn with_redis(&mut self) -> Result<()> {
        match RedisConfig::new().await {
            Ok(config) => {
                self.redis_config = Some(Arc::new(config));
                info!("Redis 연결 성공");
                Ok(())
            }
            Err(e) => {
                error!("Redis 연결 실패: {}", e);
                Err(anyhow!("Redis 연결 실패: {}", e))
            }
        }
    }
    
    /// 새로운 사용자 연결 처리
    pub async fn handle_new_connection(&self, stream: TcpStream, addr: String) -> Result<u32> {
        info!("새 사용자 연결 처리 시작: {}", addr);
        
        // IP 주소 검증
        let socket_addr = NetworkUtils::parse_socket_addr(&addr)?;
        let ip_info = IpInfo::from_socket_addr(&socket_addr);
        
        // 보안 검증 (예: 차단된 IP 확인)
        if let Err(e) = self.validate_user_connection(&ip_info).await {
            warn!("사용자 연결 거부: {} - {}", addr, e);
            return Err(e);
        }
        
        // 스트림 분리
        let (reader, writer) = stream.into_split();
        let mut buf_reader = BufReader::new(reader);
        
        // 클라이언트로부터 Connect 메시지 대기
        let connect_msg = match GameMessage::read_from_stream(&mut buf_reader).await {
            Ok(msg) => msg,
            Err(e) => {
                error!("Connect 메시지 읽기 실패: {}", e);
                return Err(anyhow!("Connect 메시지 읽기 실패: {}", e));
            }
        };
        
        // Connect 메시지 검증 및 처리
        let (room_id, user_id) = match connect_msg {
            GameMessage::Connect { room_id, user_id } => {
                info!("Connect 메시지 수신: room_id={}, user_id={}", room_id, user_id);
                (room_id, user_id)
            }
            _ => {
                warn!("잘못된 초기 메시지 타입: {:?}", connect_msg);
                return Err(anyhow!("첫 메시지는 Connect 메시지여야 합니다"));
            }
        };
        
        // TCP 호스트 정보를 Redis에 저장
        if let Some(redis_config) = &self.redis_config {
            if let Err(e) = self.store_tcp_host_to_redis(user_id, &addr, redis_config.as_ref()).await {
                error!("Redis에 TCP 호스트 정보 저장 실패: {}", e);
                // Redis 실패는 치명적이지 않으므로 계속 진행
            }
        } else {
            warn!("Redis가 설정되지 않아 TCP 호스트 정보를 저장할 수 없습니다");
        }
        
        // 연결 서비스에 등록 (reader와 writer를 다시 합침)
        let reader = buf_reader.into_inner();
        let reunited_stream = reader.reunite(writer)?;
        let registered_user_id = self.connection_service.handle_new_connection_with_id(reunited_stream, addr.clone(), user_id).await?;
        
        // 환영 메시지 전송
        if let Err(e) = self.send_welcome_message(registered_user_id).await {
            warn!("환영 메시지 전송 실패: {}", e);
        }
        
        info!("✅ 사용자 {} (room_id={}) 연결 처리 완료", user_id, room_id);
        Ok(registered_user_id)
    }
    
    /// 사용자 연결 해제 처리
    pub async fn handle_disconnection(&self, user_id: u32, reason: &str) -> Result<()> {
        info!("사용자 {} 연결 해제 처리: {}", user_id, reason);
        
        // 다른 사용자들에게 알림 (필요한 경우)
        let disconnect_message = GameMessage::Error {
            code: 1001,
            message: format!("사용자 {}가 연결을 해제했습니다", user_id),
        };
        
        // 브로드캐스트는 선택적으로 (필요한 경우에만)
        if let Err(e) = self.message_service.broadcast(&disconnect_message).await {
            warn!("연결 해제 알림 브로드캐스트 실패: {}", e);
        }
        
        // 연결 서비스에서 제거
        let removed = self.connection_service.remove_connection(user_id).await;
        
        if removed {
            info!("✅ 사용자 {} 연결 해제 완료", user_id);
        } else {
            warn!("사용자 {}가 이미 해제되었습니다", user_id);
        }
        
        Ok(())
    }
    
    /// 사용자 연결 유효성 검증
    async fn validate_user_connection(&self, ip_info: &IpInfo) -> Result<()> {
        // 기본 검증
        if ip_info.address.is_empty() {
            return Err(anyhow!("유효하지 않은 IP 주소"));
        }
        
        // 로컬호스트는 항상 허용
        if ip_info.is_localhost {
            debug!("로컬호스트 연결 허용: {}", ip_info.address);
            return Ok(());
        }
        
        // 사설 IP 확인
        if ip_info.is_private {
            debug!("사설 IP 연결: {}", ip_info.address);
        }
        
        // 연결 수 제한 확인
        let current_count = self.connection_service.get_connection_count().await;
        let connection_stats = self.connection_service.get_connection_stats().await;
        
        if current_count >= 1000 { // 임시 하드코딩된 제한
            return Err(anyhow!("서버가 가득 참: {}/1000", current_count));
        }
        
        // IP별 연결 수 제한 (향후 구현)
        // TODO: IP별 연결 수 추적 및 제한
        
        debug!("사용자 연결 검증 통과: {}", ip_info.address);
        Ok(())
    }
    
    /// TCP 호스트 정보를 Redis에 저장
    async fn store_tcp_host_to_redis(&self, user_id: u32, addr: &str, redis_config: &RedisConfig) -> Result<()> {
        let mut conn = redis_config.get_connection();
        let key_type = KeyType::User;
        let user_key = key_type.get_key(&(user_id as u16));
        
        // TCP 호스트 정보를 user_info 해시에 저장
        let _: () = conn.hset(&user_key, "tcp_host", addr).await
            .map_err(|e| anyhow!("Redis HSET 실패: {}", e))?;
        
        // TTL 갱신 (1시간)
        let _: () = conn.expire(&user_key, 3600).await
            .map_err(|e| anyhow!("Redis EXPIRE 실패: {}", e))?;
        
        debug!("사용자 {} TCP 호스트 정보 Redis 저장 완료: {}", user_id, addr);
        Ok(())
    }
    
    /// 환영 메시지 전송
    async fn send_welcome_message(&self, user_id: u32) -> Result<()> {
        let welcome_message = GameMessage::ConnectionAck { user_id };
        
        self.connection_service.send_to_user(user_id, &welcome_message).await?;
        
        debug!("환영 메시지 전송 완료: 사용자 {}", user_id);
        Ok(())
    }
    
    /// 연결 품질 확인
    pub async fn check_connection_quality(&self, user_id: u32) -> Result<ConnectionQuality> {
        if let Some(user_info) = self.connection_service.get_user_info(user_id).await {
            let uptime = user_info.uptime_seconds;
            let last_heartbeat_secs = user_info.last_heartbeat.elapsed().as_secs();
            
            let quality = match (uptime, last_heartbeat_secs) {
                (u, h) if u > 3600 && h < 600 => ConnectionQuality::Excellent,  // 1시간+ 연결, 10분 내 하트비트
                (u, h) if u > 1800 && h < 900 => ConnectionQuality::Good,       // 30분+ 연결, 15분 내 하트비트
                (u, h) if u > 600 && h < 1200 => ConnectionQuality::Fair,       // 10분+ 연결, 20분 내 하트비트
                (_, h) if h < 1500 => ConnectionQuality::Poor,                  // 25분 내 하트비트
                _ => ConnectionQuality::VeryPoor,                               // 문제 있는 연결
            };
            
            Ok(quality)
        } else {
            Err(anyhow!("사용자 {}를 찾을 수 없습니다", user_id))
        }
    }
    
    /// 모든 사용자 연결 상태 요약
    pub async fn get_connections_summary(&self) -> ConnectionsSummary {
        let users = self.connection_service.get_all_users().await;
        let stats = self.connection_service.get_connection_stats().await;
        
        let mut quality_counts = std::collections::HashMap::new();
        for user in &users {
            if let Ok(quality) = self.check_connection_quality(user.user_id).await {
                *quality_counts.entry(format!("{:?}", quality)).or_insert(0) += 1;
            }
        }
        
        ConnectionsSummary {
            total_connections: users.len(),
            quality_distribution: quality_counts,
            peak_connections: stats.peak_connections,
            total_lifetime_connections: stats.total_connections,
            timeout_disconnections: stats.timeout_disconnections,
        }
    }
    
    /// 문제 있는 연결들 식별
    pub async fn identify_problematic_connections(&self) -> Vec<ProblematicConnection> {
        let users = self.connection_service.get_all_users().await;
        let mut problematic = Vec::new();
        
        for user in users {
            let last_heartbeat_secs = user.last_heartbeat.elapsed().as_secs();
            
            if last_heartbeat_secs > 1500 { // 25분 - 타임아웃 임박 (30분 타임아웃 전 5분)
                problematic.push(ProblematicConnection {
                    user_id: user.user_id,
                    addr: user.addr,
                    issue: "하트비트 지연".to_string(),
                    severity: if last_heartbeat_secs > 1800 { "높음" } else { "보통" }.to_string(),
                    last_heartbeat_secs,
                });
            }
        }
        
        problematic
    }
}


/// 연결 요약 정보
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectionsSummary {
    pub total_connections: usize,
    pub quality_distribution: std::collections::HashMap<String, u32>,
    pub peak_connections: u32,
    pub total_lifetime_connections: u64,
    pub timeout_disconnections: u64,
}

/// 문제 있는 연결 정보
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProblematicConnection {
    pub user_id: u32,
    pub addr: String,
    pub issue: String,
    pub severity: String,
    pub last_heartbeat_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_connection_handler() {
        let connection_service = Arc::new(ConnectionService::new(100));
        let heartbeat_service = Arc::new(HeartbeatService::with_default_config(connection_service.clone()));
        let message_service = Arc::new(MessageService::new(connection_service.clone()));
        
        let handler = ConnectionHandler::new(
            connection_service,
            heartbeat_service,
            message_service,
        );
        
        // 연결 요약 조회
        let summary = handler.get_connections_summary().await;
        assert_eq!(summary.total_connections, 0);
        
        // 문제 연결 식별
        let problematic = handler.identify_problematic_connections().await;
        assert!(problematic.is_empty());
    }
}