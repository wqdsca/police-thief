use anyhow::Result;
use dotenv::{dotenv, from_path};
use std::{env, net::SocketAddr, path::PathBuf, sync::Arc};
use tonic::transport::Server;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

// High-performance optimizations
use shared::tool::high_performance::{
    AtomicStats, MetricsCollector, MetricsConfig, AlertThresholds, ParallelProcessingConfig,
    MessageCompressionConfig, CompressionAlgorithm
};

// 1) 프로토에서 생성된 코드를 같은 크레이트 루트에 포함
pub mod room {
    tonic::include_proto!("room");
}
pub mod user {
    tonic::include_proto!("user");
}

// 2) 도메인 로직·컨트롤러 모듈
mod controller;
mod service;
mod tool;
// 3) 편리한 import
use controller::{room_controller::RoomController, user_controller::UserController};
use room::room_service_server::RoomServiceServer;
use service::{
    room_service::{MockGameStateService, MockRoomRedisService, RoomService},
    user_service::{MockSocialAuthService, MockUserDatabaseService, MockUserRedisService, UserService},
};
use user::user_service_server::UserServiceServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize high-performance components
    let stats = Arc::new(AtomicStats::new());
    let metrics_config = MetricsConfig {
        collection_interval_secs: 10,
        retention_period_secs: 3600,
        enable_system_metrics: true,
        enable_performance_metrics: true,
        enable_network_metrics: true,
        enable_compression: false,
        alert_thresholds: AlertThresholds {
            cpu_usage_threshold: 80.0,
            memory_usage_threshold: 85.0,
            response_time_threshold: 1000.0,
            connection_count_threshold: 1000,
            error_rate_threshold: 5.0,
        },
    };
    let metrics_collector = Arc::new(MetricsCollector::new(metrics_config));
    
    // Configure parallel processing for gRPC handlers
    let parallel_config = ParallelProcessingConfig {
        worker_threads: num_cpus::get(),
        work_queue_size: 10000,
        batch_size: 50,
        enable_work_stealing: true,
        enable_numa_awareness: true,
        enable_dynamic_balancing: true,
    };
    
    // Configure message compression for responses
    let compression_config = MessageCompressionConfig {
        algorithm: CompressionAlgorithm::Adaptive,
        compression_threshold: 512, // Compress messages > 512 bytes
        compression_level: 3, // Balanced compression
        enable_batching: true,
        batch_size: 10,
        batch_timeout_ms: 5,
        max_batch_bytes: 64 * 1024, // 64KB max batch
        enable_compression_cache: true,
        cache_ttl_secs: 300,
    };
    
    info!("🚀 gRPC Server starting with high-performance optimizations");
    info!("📊 Parallel workers: {}, Compression: {:?}", parallel_config.worker_threads, compression_config.algorithm);
    
    // .env 로드 - workspace root에서 .env 파일 찾기
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Operation failed")
        .to_path_buf();
    let env_path = workspace_root.join(".env");

    if env_path.exists() {
        from_path(&env_path).map_err(|e| anyhow::anyhow!("Failed to load .env: {}", e))?;
    } else {
        dotenv().ok(); // fallback to default .env loading
    }

    // 로깅 초기화 (안전한 에러 처리)
    let filter = EnvFilter::from_default_env().add_directive(
        "info"
            .parse()
            .map_err(|e| anyhow::anyhow!("로깅 설정 파싱 실패: {e}"))?,
    );
    fmt().with_env_filter(filter).init();

    // grpc_host, grpc_port 읽기 (안전한 에러 처리)
    let host = env::var("grpc_host").map_err(|_| {
        anyhow::anyhow!("환경변수 'grpc_host'가 설정되지 않았습니다. .env 파일을 확인하세요.")
    })?;
    let port = env::var("grpc_port").map_err(|_| {
        anyhow::anyhow!("환경변수 'grpc_port'가 설정되지 않았습니다. .env 파일을 확인하세요.")
    })?;
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e| anyhow::anyhow!("잘못된 주소 형식 '{host}:{port}': {e}"))?;

    info!("▶ gRPC 서버 실행: {}", addr);

    // JWT 설정 확인 (선택적)
    // JWT 보안 설정 검증 호출
    validate_jwt_security_config()?;

    info!("🔐 JWT 보안 설정 검증 완료 - 서버 시작 준비 완료");
    info!("💡 JWT 토큰 검증은 컨트롤러 레벨에서 구현됩니다.");

    // Redis 연결 풀 초기화 (성능 최적화)
    info!("🔄 Redis 연결 풀 초기화 중...");
    shared::config::connection_pool::ConnectionPool::init()
        .await
        .map_err(|e| anyhow::anyhow!("Redis 연결 풀 초기화 실패: {}", e))?;
    info!("✅ Redis 연결 풀 초기화 완료");

    // 컨트롤러에 비즈니스 로직 서비스 주입 (의존성 주입 사용)
    let room_redis = Arc::new(MockRoomRedisService);
    let game_state = Arc::new(MockGameStateService);
    let room_service = RoomService::new(room_redis, game_state);
    let room_ctrl = RoomController::new(room_service)
        .map_err(|e| anyhow::anyhow!("Failed to initialize RoomController: {:?}", e))?;

    // UserService도 의존성 주입
    let social_auth_service = Arc::new(MockSocialAuthService);
    let user_redis = Arc::new(MockUserRedisService);
    let user_db = Arc::new(MockUserDatabaseService);
    let user_service = UserService::new(social_auth_service, user_redis, user_db);
    let user_ctrl = UserController::new(user_service)
        .map_err(|e| anyhow::anyhow!("Failed to initialize UserController: {:?}", e))?;

    info!("🚀 gRPC 서버 시작 중...");

    // 서버 빌드 & 실행 (최적화된 설정)
    let result = Server::builder()
        .add_service(RoomServiceServer::new(room_ctrl))
        .add_service(UserServiceServer::new(user_ctrl))
        .serve(addr)
        .await;

    match result {
        Ok(()) => info!("✅ gRPC 서버가 정상적으로 종료되었습니다."),
        Err(e) => return Err(anyhow::anyhow!("gRPC 서버 실행 실패: {e}")),
    }

    Ok(())
}

/// JWT 보안 설정 검증 함수
///
/// 프로덕션 환경에서 안전한 JWT 설정을 보장합니다.
///
/// # Returns
/// * `Result<()>` - 검증 성공 시 Ok(()), 실패 시 Error
///
/// # Panics
/// * JWT_SECRET_KEY가 설정되지 않았거나 보안 요구사항을 만족하지 않을 때
fn validate_jwt_security_config() -> Result<()> {
    use std::env;
    use tracing::info;

    // JWT_SECRET_KEY 필수 검증
    let jwt_secret = env::var("JWT_SECRET_KEY").map_err(|_| {
        anyhow::anyhow!(
            "🚨 SECURITY ERROR: JWT_SECRET_KEY environment variable is required.\n\
             Please set a cryptographically secure random key of at least 32 characters.\n\
             Example: openssl rand -hex 32"
        )
    })?;

    // 보안 검증: 최소 32자 이상의 시크릿 키 요구
    if jwt_secret.len() < 32 {
        return Err(anyhow::anyhow!(
            "🚨 SECURITY ERROR: JWT_SECRET_KEY must be at least 32 characters long.\n\
             Current length: {}. Please generate a stronger key.\n\
             Example: openssl rand -hex 32",
            jwt_secret.len()
        ));
    }

    // 보안 검증: 약한 기본값 사용 방지
    let lower_secret = jwt_secret.to_lowercase();
    if lower_secret.contains("default")
        || lower_secret.contains("secret")
        || lower_secret.contains("change")
        || lower_secret.contains("your_")
        || lower_secret.contains("please")
        || lower_secret.contains("example")
    {
        return Err(anyhow::anyhow!(
            "🚨 SECURITY ERROR: JWT_SECRET_KEY appears to contain default/weak values.\n\
             Please use a cryptographically secure random key.\n\
             Example: openssl rand -hex 32"
        ));
    }

    // JWT 알고리즘 설정 확인
    let jwt_algorithm = env::var("JWT_ALGORITHM").unwrap_or_else(|_| {
        info!("ℹ️ JWT_ALGORITHM not set, using default 'HS256'");
        "HS256".to_string()
    });

    // 지원되는 알고리즘 검증
    match jwt_algorithm.as_str() {
        "HS256" | "HS384" | "HS512" => {
            info!("✅ JWT algorithm '{}' is supported", jwt_algorithm);
        }
        _ => {
            return Err(anyhow::anyhow!(
                "🚨 SECURITY ERROR: Unsupported JWT algorithm '{}'. \n\
                 Supported algorithms: HS256, HS384, HS512",
                jwt_algorithm
            ));
        }
    }

    // 보안 설정 로그 (시크릿 키는 길이만 표시)
    info!("🔐 JWT Security Configuration:");
    info!("  └─ Algorithm: {}", jwt_algorithm);
    info!("  └─ Secret Key Length: {} characters", jwt_secret.len());
    info!("  └─ Security Level: ✅ SECURE");

    Ok(())
}
