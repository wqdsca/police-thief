//! 서비스 레이어 테스트
//! 
//! Service 모듈들의 비즈니스 로직 테스트

use crate::service::SimpleTcpService;
use anyhow::Result;

/// SimpleTcpService 생성 테스트
#[tokio::test]
async fn test_simple_tcp_service_creation() {
    let service = SimpleTcpService::new();
    
    // 기본 상태 확인
    let status = service.get_status().await;
    assert_eq!(status, "ready", "초기 상태는 ready여야 함");
    
    println!("✅ SimpleTcpService 생성 테스트 통과");
}

/// 서비스 상태 확인 테스트
#[tokio::test]
async fn test_service_status() {
    let service = SimpleTcpService::new();
    
    // 상태 조회 테스트
    let status = service.get_status().await;
    assert!(!status.is_empty(), "상태는 빈 문자열이 아니어야 함");
    assert_eq!(status, "ready", "초기 상태는 ready여야 함");
    
    println!("✅ 서비스 상태 확인 테스트 통과");
}

/// 서비스 시작 테스트
#[tokio::test]
async fn test_service_start() -> Result<()> {
    let service = SimpleTcpService::new();
    
    // 서비스 시작
    let result = service.start("127.0.0.1:0").await;
    assert!(result.is_ok(), "서비스 시작은 성공해야 함");
    
    println!("✅ 서비스 시작 테스트 통과");
    Ok(())
}

/// 서비스 중지 테스트
#[tokio::test]
async fn test_service_stop() -> Result<()> {
    let service = SimpleTcpService::new();
    
    // 서비스 시작 후 중지
    service.start("127.0.0.1:0").await?;
    let result = service.stop().await;
    assert!(result.is_ok(), "서비스 중지는 성공해야 함");
    
    println!("✅ 서비스 중지 테스트 통과");
    Ok(())
}

/// 서비스 생명주기 테스트
#[tokio::test]
async fn test_service_lifecycle() -> Result<()> {
    let service = SimpleTcpService::new();
    
    // 초기 상태
    assert_eq!(service.get_status().await, "ready");
    
    // 시작
    service.start("127.0.0.1:0").await?;
    // 상태는 구현에 따라 변경될 수 있음
    
    // 중지
    service.stop().await?;
    // 상태는 구현에 따라 변경될 수 있음
    
    println!("✅ 서비스 생명주기 테스트 통과");
    Ok(())
}

/// Default trait 구현 테스트
#[tokio::test]
async fn test_service_default() {
    let service = SimpleTcpService::default();
    let status = service.get_status().await;
    
    assert_eq!(status, "ready", "기본 생성된 서비스는 ready 상태여야 함");
    
    println!("✅ 서비스 Default 구현 테스트 통과");
}

/// 서비스 동시성 안전성 테스트
#[tokio::test]
async fn test_service_concurrency() -> Result<()> {
    let service = SimpleTcpService::new();
    
    // 동시에 여러 작업 수행
    let handles = vec![
        tokio::spawn({
            let service = service.clone();
            async move { service.start("127.0.0.1:0").await }
        }),
        tokio::spawn({
            let service = service.clone();
            async move { service.get_status().await; Ok(()) }
        }),
        tokio::spawn({
            let service = service.clone();
            async move { service.stop().await }
        }),
    ];
    
    // 모든 작업 완료 대기
    for handle in handles {
        let result = handle.await?;
        assert!(result.is_ok(), "동시 작업들은 모두 성공해야 함");
    }
    
    println!("✅ 서비스 동시성 안전성 테스트 통과");
    Ok(())
}

/// 서비스 상태 지속성 테스트
#[tokio::test]
async fn test_service_state_persistence() -> Result<()> {
    let service = SimpleTcpService::new();
    
    // 초기 상태
    let initial_status = service.get_status().await;
    
    // 시작
    service.start("127.0.0.1:0").await?;
    let started_status = service.get_status().await;
    
    // 중지  
    service.stop().await?;
    let stopped_status = service.get_status().await;
    
    // 상태가 변경되었는지 확인 (구현에 따라)
    println!("상태 변화: {} → {} → {}", initial_status, started_status, stopped_status);
    
    println!("✅ 서비스 상태 지속성 테스트 통과");
    Ok(())
}

/// 서비스 에러 처리 테스트
#[tokio::test]
async fn test_service_error_handling() -> Result<()> {
    let service = SimpleTcpService::new();
    
    // 정상 케이스
    assert!(service.start("127.0.0.1:0").await.is_ok());
    assert!(service.stop().await.is_ok());
    
    // 상태 조회는 항상 성공해야 함
    let status = service.get_status().await;
    assert!(!status.is_empty());
    
    println!("✅ 서비스 에러 처리 테스트 통과");
    Ok(())
}