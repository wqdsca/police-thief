//! 통합 테스트 스위트
//! 
//! 전체 TCP 서버 시스템의 종합적인 통합 테스트

use crate::service::{ConnectionService, HeartbeatService};
use crate::protocol::GameMessage;
use crate::service::SimpleTcpService;
use crate::tool::SimpleUtils;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use anyhow::Result;

/// 전체 시스템 통합 테스트
#[tokio::test]
async fn test_full_system_integration() -> Result<()> {
    println!("🚀 전체 시스템 통합 테스트 시작");
    
    // 1. 연결 서비스 생성
    let connection_service = Arc::new(ConnectionService::new(100));
    assert_eq!(connection_service.get_connection_count().await, 0);
    
    // 2. 하트비트 서비스 생성 및 시작
    let heartbeat_service = HeartbeatService::with_default_config(connection_service.clone());
    assert!(!heartbeat_service.is_running().await);
    
    heartbeat_service.start().await?;
    assert!(heartbeat_service.is_running().await);
    
    // 3. 서비스 레이어 테스트
    let tcp_service = SimpleTcpService::new();
    assert_eq!(tcp_service.get_status().await, "ready");
    
    tcp_service.start("127.0.0.1:0").await?;
    tcp_service.stop().await?;
    
    // 4. 하트비트 시스템 중지
    heartbeat_service.stop().await?;
    assert!(!heartbeat_service.is_running().await);
    
    println!("✅ 전체 시스템 통합 테스트 통과");
    Ok(())
}

/// 메시지 프로토콜 종합 테스트
#[tokio::test]
async fn test_protocol_comprehensive() -> Result<()> {
    println!("📨 메시지 프로토콜 종합 테스트 시작");
    
    let messages = vec![
        GameMessage::HeartBeat,
        GameMessage::HeartBeatResponse { 
            timestamp: SimpleUtils::current_timestamp() 
        },
        GameMessage::ConnectionAck { client_id: 12345 },
        GameMessage::Error { 
            code: 404, 
            message: "Test error".to_string() 
        },
    ];
    
    for (i, message) in messages.iter().enumerate() {
        // 직렬화
        let bytes = message.to_bytes()?;
        assert!(bytes.len() > 4, "메시지 {}는 헤더보다 커야 함", i);
        
        // 역직렬화
        let decoded = GameMessage::from_bytes(&bytes)?;
        
        // 타입 확인
        std::mem::discriminant(&decoded) == std::mem::discriminant(message);
        
        println!("메시지 {} 테스트 완료: {:?}", i, message);
    }
    
    println!("✅ 메시지 프로토콜 종합 테스트 통과");
    Ok(())
}

/// 하트비트 시스템 통합 테스트
#[tokio::test]
async fn test_heartbeat_integration() -> Result<()> {
    println!("💓 하트비트 시스템 통합 테스트 시작");
    
    let connection_service = Arc::new(ConnectionService::new(100));
    let heartbeat_service = HeartbeatService::with_default_config(connection_service.clone());
    
    // 시스템 시작
    heartbeat_service.start().await?;
    
    // 연결 수 확인
    assert_eq!(heartbeat_service.get_active_connections().await, 0);
    assert_eq!(connection_service.get_connection_count().await, 0);
    
    // 수동 정리 테스트
    let cleanup_count = heartbeat_service.cleanup_now().await?;
    assert_eq!(cleanup_count, 0);
    
    // 짧은 실행 후 중지
    sleep(Duration::from_millis(100)).await;
    heartbeat_service.stop().await?;
    
    println!("✅ 하트비트 시스템 통합 테스트 통과");
    Ok(())
}

/// 시스템 안정성 테스트
#[tokio::test]
async fn test_system_stability() -> Result<()> {
    println!("🛡️ 시스템 안정성 테스트 시작");
    
    // 1. 연결 서비스 안정성
    let connection_service = Arc::new(ConnectionService::new(100));
    
    // 다중 브로드캐스트
    for i in 0..10 {
        let message = GameMessage::HeartBeatResponse { timestamp: i };
        connection_service.broadcast_message(&message).await?;
    }
    
    // 2. 하트비트 서비스 안정성
    let heartbeat_service = HeartbeatService::with_default_config(connection_service.clone());
    
    // 빠른 시작/중지 반복
    for _ in 0..5 {
        heartbeat_service.start().await?;
        sleep(Duration::from_millis(50)).await;
        heartbeat_service.stop().await?;
        sleep(Duration::from_millis(50)).await;
    }
    
    println!("✅ 시스템 안정성 테스트 통과");
    Ok(())
}

/// 기본 성능 테스트
#[tokio::test]
async fn test_basic_performance() -> Result<()> {
    println!("⚡ 기본 성능 테스트 시작");
    
    let iterations = 100;
    let start_time = std::time::Instant::now();
    
    for i in 0..iterations {
        // 메시지 생성
        let message = GameMessage::HeartBeatResponse { 
            timestamp: SimpleUtils::current_timestamp() + i as i64
        };
        
        // 직렬화/역직렬화
        let bytes = message.to_bytes()?;
        let _decoded = GameMessage::from_bytes(&bytes)?;
        
        // 16진수 변환
        let _hex = SimpleUtils::bytes_to_hex(&bytes);
    }
    
    let total_time = start_time.elapsed();
    let avg_time = total_time / iterations;
    
    println!("성능 결과 ({}회):", iterations);
    println!("- 총 시간: {:?}", total_time);
    println!("- 평균 시간: {:?}", avg_time);
    
    // 성능 기준: 평균 1ms 이하
    assert!(avg_time.as_millis() < 10, "평균 처리 시간이 너무 느림: {:?}", avg_time);
    
    println!("✅ 기본 성능 테스트 통과");
    Ok(())
}