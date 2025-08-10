//! Police Thief 게임 서버 로깅 시스템 사용 예제
//!
//! 이 예제는 로깅 시스템의 다양한 기능과 사용 패턴을 보여줍니다.

use anyhow::Result;
use shared::logging::{
    config::{LoggingConfig, ServiceType},
    system::LoggingSystem,
    init_logging,
};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🎮 Police Thief 로깅 시스템 예제 시작");

    // === 예제 1: 기본 사용법 ===
    println!("\n📝 예제 1: 기본 로깅 시스템 사용");
    basic_logging_example().await?;

    // === 예제 2: 커스텀 설정 ===
    println!("\n⚙️ 예제 2: 커스텀 설정으로 로깅 시스템 구성");
    custom_config_example().await?;

    // === 예제 3: 게임 서버 시나리오 ===
    println!("\n🎯 예제 3: 실제 게임 서버 시나리오");
    game_server_scenario().await?;

    // === 예제 4: 에러 처리 및 디버깅 ===
    println!("\n🔍 예제 4: 에러 처리 및 디버깅 로그");
    error_handling_example().await?;

    // === 예제 5: 성능 테스트 ===
    println!("\n⚡ 예제 5: 고성능 로깅 테스트");
    performance_example().await?;

    println!("\n✅ 모든 예제 완료! logs/ 디렉토리에서 생성된 로그를 확인하세요.");
    Ok(())
}

/// 예제 1: 기본적인 로깅 시스템 사용법
async fn basic_logging_example() -> Result<()> {
    // 간편한 초기화 함수 사용
    let logger = init_logging(ServiceType::GrpcServer, Some("./logs")).await?;

    // 다양한 로그 레벨로 메시지 작성
    logger.trace("상세한 추적 정보", &[("function", "basic_example")]).await;
    logger.debug("디버깅 정보", &[("step", "1"), ("status", "processing")]).await;
    logger.info("일반 정보", &[("action", "server_start"), ("port", "50051")]).await;
    logger.warn("경고 메시지", &[("memory_usage", "85%"), ("threshold", "80%")]).await;
    logger.error("오류 발생", &[("error_code", "E001"), ("component", "database")]).await;
    logger.fatal("심각한 오류", &[("reason", "system_failure")]).await;

    // 즉시 디스크에 기록
    logger.flush().await?;
    println!("   ✓ gRPC 서버 로그가 logs/grpcserver/ 에 생성됨");

    Ok(())
}

/// 예제 2: 커스텀 설정으로 로깅 시스템 구성
async fn custom_config_example() -> Result<()> {
    // 커스텀 설정 생성
    let mut config = LoggingConfig::default();
    config.json_format = false; // 텍스트 형식 사용
    config.retention_days = 3;  // 3일 보관
    config.max_file_size = 10 * 1024 * 1024; // 10MB
    config.debug_mode = true;

    // 수동으로 시스템 생성
    let mut logger = LoggingSystem::new("./logs").await?;
    logger.init(ServiceType::TcpServer).await?;

    // 게임 서버 시작 시뮬레이션
    logger.info("TCP 서버 초기화 시작", &[
        ("config", "custom"),
        ("format", "text"),
        ("retention_days", "3"),
    ]).await;

    logger.info("서버 바인딩 성공", &[
        ("address", "0.0.0.0:4000"),
        ("protocol", "tcp"),
    ]).await;

    logger.info("플레이어 풀 초기화", &[
        ("initial_capacity", "1000"),
        ("max_connections", "5000"),
    ]).await;

    println!("   ✓ TCP 서버 로그가 텍스트 형식으로 생성됨");

    Ok(())
}

/// 예제 3: 실제 게임 서버 시나리오 시뮬레이션
async fn game_server_scenario() -> Result<()> {
    let logger = init_logging(ServiceType::RudpServer, Some("./logs")).await?;

    // 서버 시작
    logger.info("RUDP 게임 서버 시작", &[
        ("version", "1.0.0"),
        ("environment", "example"),
        ("max_players", "500"),
    ]).await;

    // 플레이어 연결 시뮬레이션
    for i in 1..=5 {
        let player_id = format!("player_{:03}", i);
        let session_id = format!("sess_{}", i);

        logger.info("플레이어 연결", &[
            ("player_id", &player_id),
            ("session_id", &session_id),
            ("ip_address", "127.0.0.1"),
            ("connection_type", "rudp"),
        ]).await;

        // 인증 과정
        logger.debug("플레이어 인증 시작", &[
            ("player_id", &player_id),
            ("auth_method", "jwt"),
        ]).await;

        if i % 4 != 0 {
            logger.info("인증 성공", &[
                ("player_id", &player_id),
                ("permissions", "player"),
                ("auth_time_ms", "150"),
            ]).await;
        } else {
            logger.warn("인증 실패", &[
                ("player_id", &player_id),
                ("reason", "invalid_token"),
                ("retry_count", "1"),
            ]).await;
            continue;
        }

        // 게임 방 입장
        let room_id = "room_001";
        logger.info("방 입장 요청", &[
            ("player_id", &player_id),
            ("room_id", room_id),
            ("room_type", "normal"),
        ]).await;

        logger.info("방 입장 완료", &[
            ("player_id", &player_id),
            ("room_id", room_id),
            ("position", "spawn_point_1"),
            ("current_players", &i.to_string()),
        ]).await;
    }

    // 게임 이벤트들
    logger.info("게임 시작", &[
        ("room_id", "room_001"),
        ("game_mode", "police_thief"),
        ("duration_minutes", "10"),
        ("players_count", "4"),
    ]).await;

    // 게임 중 이벤트
    logger.info("플레이어 이동", &[
        ("player_id", "player_001"),
        ("from", "spawn_point_1"),
        ("to", "building_a"),
        ("speed", "5.2"),
    ]).await;

    logger.warn("플레이어 의심스러운 활동", &[
        ("player_id", "player_002"),
        ("activity", "rapid_movement"),
        ("detection_confidence", "0.85"),
    ]).await;

    logger.info("아이템 획득", &[
        ("player_id", "player_003"),
        ("item_id", "keycard_001"),
        ("location", "office_desk"),
        ("rarity", "common"),
    ]).await;

    logger.info("게임 종료", &[
        ("room_id", "room_001"),
        ("winner", "thief"),
        ("duration_seconds", "342"),
        ("final_score", "1500"),
    ]).await;

    println!("   ✓ 게임 시나리오 로그가 완료됨");

    Ok(())
}

/// 예제 4: 에러 처리 및 디버깅 시나리오
async fn error_handling_example() -> Result<()> {
    let logger = init_logging(ServiceType::GameCenter, Some("./logs")).await?;

    logger.info("게임 센터 서비스 시작", &[
        ("service", "game_center"),
        ("version", "2.1.0"),
    ]).await;

    // 다양한 에러 상황들
    logger.error("Redis 연결 실패", &[
        ("redis_host", "localhost:6379"),
        ("error_type", "connection_refused"),
        ("retry_count", "3"),
        ("next_retry_sec", "5"),
    ]).await;

    logger.warn("데이터베이스 응답 지연", &[
        ("query", "SELECT * FROM players WHERE active = true"),
        ("response_time_ms", "2500"),
        ("timeout_ms", "3000"),
        ("affected_operations", "player_lookup"),
    ]).await;

    logger.error("메모리 부족 경고", &[
        ("available_mb", "128"),
        ("required_mb", "256"),
        ("process", "game_state_manager"),
        ("action", "garbage_collection_triggered"),
    ]).await;

    // 복구 과정
    logger.info("시스템 복구 시작", &[
        ("recovery_type", "automatic"),
        ("estimated_time_sec", "30"),
    ]).await;

    logger.info("Redis 재연결 성공", &[
        ("redis_host", "localhost:6379"),
        ("connection_pool_size", "10"),
        ("ping_time_ms", "2"),
    ]).await;

    logger.info("시스템 정상화 완료", &[
        ("recovery_time_sec", "25"),
        ("health_check", "passed"),
        ("active_connections", "150"),
    ]).await;

    println!("   ✓ 에러 처리 시나리오 로그 완료");

    Ok(())
}

/// 예제 5: 고성능 로깅 테스트
async fn performance_example() -> Result<()> {
    let logger = init_logging(ServiceType::Shared, Some("./logs")).await?;

    logger.info("성능 테스트 시작", &[
        ("test_type", "high_throughput"),
        ("target_logs", "1000"),
    ]).await;

    let start_time = std::time::Instant::now();

    // 1000개의 로그를 빠르게 작성
    for i in 0..1000 {
        let iteration = i.to_string();
        let batch_id = (i / 100).to_string();

        match i % 5 {
            0 => {
                logger.trace("성능 테스트 추적", &[
                    ("iteration", &iteration),
                    ("batch", &batch_id),
                    ("type", "trace"),
                ]).await;
            }
            1 => {
                logger.debug("성능 테스트 디버그", &[
                    ("iteration", &iteration),
                    ("batch", &batch_id),
                    ("type", "debug"),
                ]).await;
            }
            2 => {
                logger.info("성능 테스트 정보", &[
                    ("iteration", &iteration),
                    ("batch", &batch_id),
                    ("type", "info"),
                ]).await;
            }
            3 => {
                logger.warn("성능 테스트 경고", &[
                    ("iteration", &iteration),
                    ("batch", &batch_id),
                    ("type", "warn"),
                ]).await;
            }
            4 => {
                logger.error("성능 테스트 에러", &[
                    ("iteration", &iteration),
                    ("batch", &batch_id),
                    ("type", "error"),
                ]).await;
            }
            _ => unreachable!(),
        }
    }

    // 플러시 및 측정 완료
    logger.flush().await?;
    let duration = start_time.elapsed();

    logger.info("성능 테스트 완료", &[
        ("total_logs", "1000"),
        ("duration_ms", &duration.as_millis().to_string()),
        ("logs_per_second", &(1000.0 / duration.as_secs_f64()).to_string()),
    ]).await;

    println!("   ✓ 1000개 로그 작성 완료: {:?}", duration);
    println!("   ✓ 초당 로그 처리량: {:.0} logs/sec", 1000.0 / duration.as_secs_f64());

    // 비동기 작성 완료 대기
    sleep(Duration::from_millis(100)).await;

    Ok(())
}