//! 연결 핸들러
//! 
//! 클라이언트 연결/해제 처리를 담당합니다.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use tokio::net::TcpStream;
use tracing::{info, warn, debug};

use crate::service::{ConnectionService, HeartbeatService, MessageService};
use crate::protocol::GameMessage;
use crate::tool::{NetworkUtils, IpInfo, ConnectionQuality};

/// 연결 핸들러
pub struct ConnectionHandler {
    connection_service: Arc<ConnectionService>,
    heartbeat_service: Arc<HeartbeatService>,
    message_service: Arc<MessageService>,
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
        }
    }
    
    /// 새로운 클라이언트 연결 처리
    pub async fn handle_new_connection(&self, stream: TcpStream, addr: String) -> Result<u32> {
        info!("새 클라이언트 연결 처리 시작: {}", addr);
        
        // IP 주소 검증
        let socket_addr = NetworkUtils::parse_socket_addr(&addr)?;
        let ip_info = IpInfo::from_socket_addr(&socket_addr);
        
        // 보안 검증 (예: 차단된 IP 확인)
        if let Err(e) = self.validate_client_connection(&ip_info).await {
            warn!("클라이언트 연결 거부: {} - {}", addr, e);
            return Err(e);
        }
        
        // 연결 서비스에 등록
        let client_id = self.connection_service.handle_new_connection(stream, addr.clone()).await?;
        
        // 환영 메시지 전송 (선택적)
        if let Err(e) = self.send_welcome_message(client_id).await {
            warn!("환영 메시지 전송 실패: {}", e);
        }
        
        info!("✅ 클라이언트 {} 연결 처리 완료", client_id);
        Ok(client_id)
    }
    
    /// 클라이언트 연결 해제 처리
    pub async fn handle_disconnection(&self, client_id: u32, reason: &str) -> Result<()> {
        info!("클라이언트 {} 연결 해제 처리: {}", client_id, reason);
        
        // 다른 클라이언트들에게 알림 (게임 로직에 따라)
        let disconnect_message = GameMessage::Error {
            code: 1001,
            message: format!("클라이언트 {}가 연결을 해제했습니다", client_id),
        };
        
        // 브로드캐스트는 선택적으로 (게임 상황에 따라)
        if let Err(e) = self.message_service.broadcast(&disconnect_message).await {
            warn!("연결 해제 알림 브로드캐스트 실패: {}", e);
        }
        
        // 연결 서비스에서 제거
        let removed = self.connection_service.remove_connection(client_id).await;
        
        if removed {
            info!("✅ 클라이언트 {} 연결 해제 완료", client_id);
        } else {
            warn!("클라이언트 {}가 이미 해제되었습니다", client_id);
        }
        
        Ok(())
    }
    
    /// 클라이언트 연결 유효성 검증
    async fn validate_client_connection(&self, ip_info: &IpInfo) -> Result<()> {
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
        
        debug!("클라이언트 연결 검증 통과: {}", ip_info.address);
        Ok(())
    }
    
    /// 환영 메시지 전송
    async fn send_welcome_message(&self, client_id: u32) -> Result<()> {
        let welcome_message = GameMessage::ConnectionAck { client_id };
        
        self.connection_service.send_to_client(client_id, &welcome_message).await?;
        
        debug!("환영 메시지 전송 완료: 클라이언트 {}", client_id);
        Ok(())
    }
    
    /// 연결 품질 확인
    pub async fn check_connection_quality(&self, client_id: u32) -> Result<ConnectionQuality> {
        if let Some(client_info) = self.connection_service.get_client_info(client_id).await {
            let uptime = client_info.uptime_seconds;
            let last_heartbeat_secs = client_info.last_heartbeat.elapsed().as_secs();
            
            let quality = match (uptime, last_heartbeat_secs) {
                (u, h) if u > 3600 && h < 10 => ConnectionQuality::Excellent, // 1시간+ 연결, 최근 하트비트
                (u, h) if u > 300 && h < 20 => ConnectionQuality::Good,        // 5분+ 연결, 양호한 하트비트
                (u, h) if u > 60 && h < 30 => ConnectionQuality::Fair,         // 1분+ 연결, 보통 하트비트
                (_, h) if h < 60 => ConnectionQuality::Poor,                   // 최근 연결이지만 하트비트 지연
                _ => ConnectionQuality::VeryPoor,                              // 문제 있는 연결
            };
            
            Ok(quality)
        } else {
            Err(anyhow!("클라이언트 {}를 찾을 수 없습니다", client_id))
        }
    }
    
    /// 모든 클라이언트 연결 상태 요약
    pub async fn get_connections_summary(&self) -> ConnectionsSummary {
        let clients = self.connection_service.get_all_clients().await;
        let stats = self.connection_service.get_connection_stats().await;
        
        let mut quality_counts = std::collections::HashMap::new();
        for client in &clients {
            if let Ok(quality) = self.check_connection_quality(client.client_id).await {
                *quality_counts.entry(format!("{:?}", quality)).or_insert(0) += 1;
            }
        }
        
        ConnectionsSummary {
            total_connections: clients.len(),
            quality_distribution: quality_counts,
            peak_connections: stats.peak_connections,
            total_lifetime_connections: stats.total_connections,
            timeout_disconnections: stats.timeout_disconnections,
        }
    }
    
    /// 문제 있는 연결들 식별
    pub async fn identify_problematic_connections(&self) -> Vec<ProblematicConnection> {
        let clients = self.connection_service.get_all_clients().await;
        let mut problematic = Vec::new();
        
        for client in clients {
            let last_heartbeat_secs = client.last_heartbeat.elapsed().as_secs();
            
            if last_heartbeat_secs > 25 { // 타임아웃 임박
                problematic.push(ProblematicConnection {
                    client_id: client.client_id,
                    addr: client.addr,
                    issue: "하트비트 지연".to_string(),
                    severity: if last_heartbeat_secs > 30 { "높음" } else { "보통" }.to_string(),
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
    pub client_id: u32,
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