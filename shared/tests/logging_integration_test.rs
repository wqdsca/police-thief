//! 로깅 시스템 통합 테스트
//!
//! TDD 방식으로 작성된 로깅 시스템의 모든 기능을 통합적으로 테스트합니다.

use anyhow::Result;
use shared::logging::{
    config::{LoggingConfig, ServiceType},
    formatter::{LogFormatter, LogLevel, LogEntry},
    rotation::LogRotationManager,
    system::LoggingSystem,
    writer::{AsyncLogWriter, InMemoryLogWriter},
    init_logging,
};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs;

/// 로깅 시스템 기본 초기화 테스트
#[tokio::test]
async fn test_logging_system_initialization() -> Result<()> {
    let temp_dir = TempDir::new()?;
    
    // 로깅 시스템 생성 및 초기화
    let logger = init_logging(ServiceType::GrpcServer, Some(temp_dir.path())).await?;
    
    // 상태 확인
    assert_eq!(logger.get_state().await, shared::logging::system::LoggingState::Initialized);
    
    // 로그 디렉토리 생성 확인
    let grpc_dir = temp_dir.path().join("grpcserver");
    assert!(grpc_dir.exists());
    assert!(grpc_dir.is_dir());
    
    Ok(())
}

/// 전체 서비스 타입에 대한 로그 디렉토리 생성 테스트
#[tokio::test]
async fn test_all_service_directories_creation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config = LoggingConfig::default();
    let manager = LogRotationManager::new(temp_dir.path(), config);
    
    manager.initialize_directories().await?;
    
    // 모든 서비스 디렉토리가 생성되었는지 확인
    let service_types = [
        ServiceType::GrpcServer,
        ServiceType::TcpServer,
        ServiceType::RudpServer,
        ServiceType::GameCenter,
        ServiceType::Shared,
    ];
    
    for service_type in service_types {
        let service_dir = temp_dir.path().join(service_type.as_str());
        assert!(service_dir.exists(), "서비스 디렉토리가 생성되지 않음: {}", service_type.as_str());
        assert!(service_dir.is_dir());
    }
    
    Ok(())
}

/// 날짜별 로그 파일 생성 테스트
#[tokio::test]
async fn test_daily_log_file_creation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config = LoggingConfig::default();
    let mut manager = LogRotationManager::new(temp_dir.path(), config);
    
    // 초기 디렉토리 생성
    manager.initialize_directories().await?;
    
    // gRPC 서버 로그 파일 생성
    let log_file_path = manager.get_current_log_file(ServiceType::GrpcServer).await?;
    
    // 파일 경로 검증
    assert!(log_file_path.exists());
    assert!(log_file_path.to_string_lossy().contains("grpc"));
    assert!(log_file_path.extension().unwrap() == "log");
    
    // 파일 이름에 오늘 날짜가 포함되어 있는지 확인
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    assert!(log_file_path.file_name().unwrap().to_string_lossy().contains(&today));
    
    Ok(())
}

/// 로그 레벨별 메시지 작성 테스트
#[tokio::test]
async fn test_log_levels_writing() -> Result<()> {
    let mut system = LoggingSystem::new_test_mode().await?;
    system.init(ServiceType::TcpServer).await?;
    
    // 각 레벨별 로그 작성
    system.trace("Trace level message", &[("trace_data", "trace_value")]).await;
    system.debug("Debug level message", &[("debug_data", "debug_value")]).await;
    system.info("Info level message", &[("info_data", "info_value")]).await;
    system.warn("Warn level message", &[("warn_data", "warn_value")]).await;
    system.error("Error level message", &[("error_data", "error_value")]).await;
    system.fatal("Fatal level message", &[("fatal_data", "fatal_value")]).await;
    
    // 메모리 로그 확인
    let logs = system.get_memory_logs().await.unwrap();
    assert_eq!(logs.len(), 6);
    
    // 각 레벨이 올바르게 기록되었는지 확인
    let levels = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR", "FATAL"];
    let messages = [
        "Trace level message",
        "Debug level message", 
        "Info level message",
        "Warn level message",
        "Error level message",
        "Fatal level message",
    ];
    
    for (i, log) in logs.iter().enumerate() {
        assert!(log.contains(levels[i]), "로그에 레벨이 포함되지 않음: {}", levels[i]);
        assert!(log.contains(messages[i]), "로그에 메시지가 포함되지 않음: {}", messages[i]);
        assert!(log.contains("tcpserver"), "로그에 서비스명이 포함되지 않음");
    }
    
    Ok(())
}

/// 컨텍스트 데이터 포함 테스트
#[tokio::test]
async fn test_context_data_inclusion() -> Result<()> {
    let mut system = LoggingSystem::new_test_mode().await?;
    system.init(ServiceType::RudpServer).await?;
    
    // 다양한 컨텍스트 데이터와 함께 로그 작성
    system.info("User authentication", &[
        ("user_id", "12345"),
        ("ip_address", "192.168.1.100"),
        ("action", "login"),
        ("success", "true"),
    ]).await;
    
    system.error("Database connection failed", &[
        ("database", "postgresql"),
        ("host", "localhost"),
        ("port", "5432"),
        ("timeout", "30s"),
    ]).await;
    
    let logs = system.get_memory_logs().await.unwrap();
    assert_eq!(logs.len(), 2);
    
    // 첫 번째 로그 - 사용자 인증
    let auth_log = &logs[0];
    assert!(auth_log.contains("User authentication"));
    assert!(auth_log.contains("user_id=12345"));
    assert!(auth_log.contains("ip_address=192.168.1.100"));
    assert!(auth_log.contains("action=login"));
    assert!(auth_log.contains("success=true"));
    
    // 두 번째 로그 - 데이터베이스 오류
    let db_log = &logs[1];
    assert!(db_log.contains("Database connection failed"));
    assert!(db_log.contains("database=postgresql"));
    assert!(db_log.contains("host=localhost"));
    assert!(db_log.contains("port=5432"));
    assert!(db_log.contains("timeout=30s"));
    
    Ok(())
}

/// 비동기 로그 작성 성능 테스트
#[tokio::test]
async fn test_async_logging_performance() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config = LoggingConfig::default();
    let formatter = Arc::new(LogFormatter::new(true, false)); // JSON 형식
    
    let log_path = temp_dir.path().join("performance_test.log");
    let writer = AsyncLogWriter::new(log_path.clone(), config, formatter).await?;
    
    let start_time = std::time::Instant::now();
    let num_logs = 1000;
    
    // 1000개의 로그 항목을 빠르게 작성
    for i in 0..num_logs {
        let entry = LogEntry::new(
            LogLevel::Info,
            "performance-test".to_string(),
            format!("Performance test message {}", i),
            &[("iteration", &i.to_string()), ("batch", "performance")],
        );
        writer.write_log(entry)?;
    }
    
    // 플러시 및 측정
    writer.flush()?;
    let duration = start_time.elapsed();
    
    println!("{}개 로그 작성 시간: {:?}", num_logs, duration);
    
    // 1초 내에 1000개 로그를 작성할 수 있어야 함 (성능 요구사항)
    assert!(duration < Duration::from_secs(1), "로그 작성 성능이 요구사항을 만족하지 않음");
    
    // 작성기 종료
    writer.shutdown().await?;
    
    // 파일이 생성되었고 내용이 있는지 확인
    assert!(log_path.exists());
    let file_size = fs::metadata(&log_path).await?.len();
    assert!(file_size > 0, "로그 파일이 비어있음");
    
    Ok(())
}

/// 로그 파일 순환(회전) 테스트
#[tokio::test]
async fn test_log_rotation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let mut config = LoggingConfig::default();
    config.max_file_size = 1024; // 1KB로 설정하여 빠른 순환
    
    let mut manager = LogRotationManager::new(temp_dir.path(), config);
    
    // 초기 디렉토리 생성
    manager.initialize_directories().await?;
    
    // 로그 파일 생성
    let log_path = manager.get_current_log_file(ServiceType::GrpcServer).await?;
    
    // 파일에 최대 크기를 초과하는 데이터 작성
    let large_content = "x".repeat(2048); // 2KB
    fs::write(&log_path, large_content).await?;
    
    // 순환이 필요한지 확인
    assert!(manager.should_rotate(&log_path).await?);
    
    // 순환 실행
    let rotated_file = manager.rotate_if_needed(ServiceType::GrpcServer).await?;
    assert!(rotated_file.is_some());
    
    // 새 로그 파일이 생성되었는지 확인
    assert!(log_path.exists());
    let new_size = manager.check_file_size(&log_path).await?;
    assert_eq!(new_size, 0); // 새 파일은 비어있어야 함
    
    // 순환된 파일이 존재하는지 확인
    let rotated_path = rotated_file.unwrap();
    assert!(rotated_path.exists());
    let rotated_size = manager.check_file_size(&rotated_path).await?;
    assert!(rotated_size > 1024); // 순환된 파일에는 원래 데이터가 있어야 함
    
    Ok(())
}

/// JSON 포맷터 테스트
#[tokio::test] 
async fn test_json_formatter() -> Result<()> {
    let formatter = LogFormatter::new(true, false); // JSON 형식, 색상 없음
    
    let entry = LogEntry::new(
        LogLevel::Error,
        "json-test".to_string(),
        "JSON formatting test".to_string(),
        &[("error_code", "E001"), ("module", "authentication")],
    );
    
    let formatted = formatter.format(&entry)?;
    
    // JSON 파싱이 가능한지 확인
    let parsed: serde_json::Value = serde_json::from_str(&formatted)?;
    
    // JSON 필드 확인
    assert_eq!(parsed["level"], "Error");
    assert_eq!(parsed["service"], "json-test");
    assert_eq!(parsed["message"], "JSON formatting test");
    assert_eq!(parsed["context"]["error_code"], "E001");
    assert_eq!(parsed["context"]["module"], "authentication");
    
    // 타임스탬프 필드 존재 확인
    assert!(parsed["timestamp"].is_string());
    
    Ok(())
}

/// 텍스트 포맷터 테스트
#[tokio::test]
async fn test_text_formatter() -> Result<()> {
    let formatter = LogFormatter::new(false, false); // 텍스트 형식, 색상 없음
    
    let entry = LogEntry::new(
        LogLevel::Warn,
        "text-test".to_string(),
        "Text formatting test".to_string(),
        &[("warning_type", "deprecated_api"), ("version", "1.0")],
    );
    
    let formatted = formatter.format(&entry)?;
    
    // 텍스트 형식 확인
    assert!(formatted.contains("[WARN]"));
    assert!(formatted.contains("[text-test]"));
    assert!(formatted.contains("Text formatting test"));
    assert!(formatted.contains("warning_type=deprecated_api"));
    assert!(formatted.contains("version=1.0"));
    
    // 타임스탬프가 포함되어 있는지 확인 (YYYY-MM-DD 형식)
    let current_year = chrono::Utc::now().format("%Y").to_string();
    assert!(formatted.contains(&current_year));
    
    Ok(())
}

/// 메모리 로그 작성기 테스트
#[tokio::test]
async fn test_memory_writer() -> Result<()> {
    let formatter = Arc::new(LogFormatter::new(false, false));
    let writer = InMemoryLogWriter::new(formatter);
    
    // 여러 로그 항목 작성
    let entries = vec![
        ("INFO", "First message", vec![("key1", "value1")]),
        ("ERROR", "Second message", vec![("key2", "value2")]),
        ("DEBUG", "Third message", vec![("key3", "value3")]),
    ];
    
    for (level_str, message, context) in entries {
        let level = level_str.parse().unwrap();
        let context_slice: Vec<(&str, &str)> = context.iter().map(|(k, v)| (*k, *v)).collect();
        
        let entry = LogEntry::new(
            level,
            "memory-test".to_string(),
            message.to_string(),
            &context_slice,
        );
        
        writer.write_log(entry).await?;
    }
    
    // 로그 확인
    let logs = writer.get_logs().await;
    assert_eq!(logs.len(), 3);
    assert_eq!(writer.len().await, 3);
    
    assert!(logs[0].contains("[INFO]") && logs[0].contains("First message"));
    assert!(logs[1].contains("[ERROR]") && logs[1].contains("Second message"));
    assert!(logs[2].contains("[DEBUG]") && logs[2].contains("Third message"));
    
    // 로그 지우기 테스트
    writer.clear().await;
    assert_eq!(writer.len().await, 0);
    
    Ok(())
}

/// 환경변수 설정 테스트
#[tokio::test]
async fn test_config_from_env() -> Result<()> {
    // 환경변수 설정
    std::env::set_var("LOG_RETENTION_DAYS", "14");
    std::env::set_var("LOG_MAX_FILE_SIZE", "52428800"); // 50MB
    std::env::set_var("LOG_JSON_FORMAT", "false");
    std::env::set_var("LOG_DEBUG_MODE", "true");
    
    let config = LoggingConfig::from_env();
    
    assert_eq!(config.retention_days, 14);
    assert_eq!(config.max_file_size, 52428800);
    assert!(!config.json_format);
    assert!(config.debug_mode);
    
    // 환경변수 정리
    std::env::remove_var("LOG_RETENTION_DAYS");
    std::env::remove_var("LOG_MAX_FILE_SIZE");
    std::env::remove_var("LOG_JSON_FORMAT");
    std::env::remove_var("LOG_DEBUG_MODE");
    
    Ok(())
}

/// 동시성 테스트 - 여러 스레드에서 동시 로그 작성
#[tokio::test]
async fn test_concurrent_logging() -> Result<()> {
    let mut system = LoggingSystem::new_test_mode().await?;
    system.init(ServiceType::GameCenter).await?;
    
    let system = Arc::new(tokio::sync::Mutex::new(system));
    let mut handles = vec![];
    
    // 10개의 동시 태스크 생성
    for i in 0..10 {
        let system_clone = system.clone();
        let handle = tokio::spawn(async move {
            let system = system_clone.lock().await;
            for j in 0..10 {
                system.info(&format!("Concurrent message {} from task {}", j, i), &[
                    ("task_id", &i.to_string()),
                    ("message_id", &j.to_string()),
                ]).await;
            }
        });
        handles.push(handle);
    }
    
    // 모든 태스크 완료 대기
    for handle in handles {
        handle.await?;
    }
    
    // 결과 확인
    let system = system.lock().await;
    let logs = system.get_memory_logs().await.unwrap();
    assert_eq!(logs.len(), 100); // 10 태스크 × 10 메시지
    
    // 모든 태스크의 메시지가 포함되어 있는지 확인
    for i in 0..10 {
        let task_logs: Vec<&String> = logs.iter()
            .filter(|log| log.contains(&format!("task_id={}", i)))
            .collect();
        assert_eq!(task_logs.len(), 10);
    }
    
    Ok(())
}

/// 시스템 종료 및 리소스 정리 테스트
#[tokio::test]
async fn test_system_shutdown() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let logger = init_logging(ServiceType::Shared, Some(temp_dir.path())).await?;
    
    // 로그 작성
    logger.info("Before shutdown", &[("status", "running")]).await;
    
    // 시스템 종료
    logger.shutdown().await?;
    
    // 종료 후 상태 확인 (실제로는 시스템이 소멸되므로 직접 확인하기 어려움)
    // 여기서는 종료 과정에서 에러가 발생하지 않았는지만 확인
    
    Ok(())
}

/// 통합 시나리오 테스트 - 실제 사용 패턴 모방
#[tokio::test]
async fn test_realistic_usage_scenario() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let mut logger = LoggingSystem::new(temp_dir.path()).await?;
    logger.init(ServiceType::GrpcServer).await?;
    
    // 서버 시작 시나리오
    logger.info("gRPC 서버 시작", &[
        ("port", "50051"),
        ("version", "1.0.0"),
        ("environment", "test"),
    ]).await;
    
    // 사용자 인증 시나리오
    logger.debug("사용자 인증 시도", &[
        ("user_id", "test_user"),
        ("ip", "127.0.0.1"),
    ]).await;
    
    logger.info("사용자 인증 성공", &[
        ("user_id", "test_user"),
        ("session_id", "sess_123"),
        ("auth_method", "jwt"),
    ]).await;
    
    // 비즈니스 로직 실행
    logger.info("방 생성 요청", &[
        ("user_id", "test_user"),
        ("room_name", "테스트방"),
        ("max_players", "4"),
    ]).await;
    
    logger.warn("방 인원 거의 찬 상태", &[
        ("room_id", "room_456"),
        ("current_players", "3"),
        ("max_players", "4"),
    ]).await;
    
    // 오류 상황
    logger.error("데이터베이스 연결 실패", &[
        ("database", "postgresql"),
        ("error", "connection timeout"),
        ("retry_attempt", "3"),
    ]).await;
    
    // 시스템 모니터링 정보
    logger.info("시스템 상태", &[
        ("cpu_usage", "45%"),
        ("memory_usage", "67%"),
        ("active_connections", "23"),
    ]).await;
    
    // 플러시 실행
    logger.flush().await?;
    
    // 비동기 작성 완료 대기
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // 로그 파일이 생성되었는지 확인
    let grpc_dir = temp_dir.path().join("grpcserver");
    assert!(grpc_dir.exists());
    
    let mut log_files = Vec::new();
    let mut entries = fs::read_dir(&grpc_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        log_files.push(entry);
    }
    assert!(!log_files.is_empty(), "로그 파일이 생성되지 않음");
    
    // 로그 파일 내용 확인 (첫 번째 파일)
    if let Some(log_file) = log_files.first() {
        let content = fs::read_to_string(log_file.path()).await?;
        assert!(content.contains("gRPC 서버 시작"));
        assert!(content.contains("사용자 인증 성공"));
        assert!(content.contains("방 생성 요청"));
        assert!(content.contains("데이터베이스 연결 실패"));
    }
    
    Ok(())
}