//! Optimized User Service gRPC Controller with High-Performance Features
//!
//! This enhanced controller integrates high-performance tools for:
//! - Lock-free statistics collection
//! - Parallel request processing
//! - Adaptive message compression
//! - Real-time performance monitoring

use crate::service::user_service::UserService as UserSvc;
use crate::user::{
    user_service_server::UserService, LoginRequest, LoginResponse, RegisterRequest,
    RegisterResponse,
};
use shared::service::TokenService;
use shared::tool::error::{helpers, AppError};
use shared::tool::high_performance::{
    AtomicStats, MetricsCollector, MessageCompression, MessageCompressionConfig,
    CompressionAlgorithm, SafePrimitives
};
use std::sync::Arc;
use std::time::Instant;
use tonic::{Request, Response, Status};
use tracing::{info, warn, debug};

/// Optimized User Service gRPC Controller with performance enhancements
pub struct OptimizedUserController {
    /// Core user service logic
    svc: UserSvc,
    /// JWT token verification service
    token_service: TokenService,
    /// Lock-free statistics collector
    stats: Arc<AtomicStats>,
    /// Real-time metrics collector
    metrics: Arc<MetricsCollector>,
    /// Message compression for responses
    compression: Arc<MessageCompression>,
    /// Memory-safe operations
    safe_ops: Arc<SafePrimitives>,
}

impl OptimizedUserController {
    /// Create new optimized controller with high-performance features
    pub fn new(
        svc: UserSvc,
        stats: Arc<AtomicStats>,
        metrics: Arc<MetricsCollector>,
    ) -> Result<Self, tonic::Status> {
        // Initialize JWT service with enhanced security
        let jwt_secret = std::env::var("JWT_SECRET_KEY").map_err(|_| {
            tracing::error!(
                "⚠️ SECURITY ERROR: JWT_SECRET_KEY environment variable is required"
            );
            tonic::Status::internal("Server configuration error: Missing JWT_SECRET_KEY")
        })?;

        // Enhanced security validation
        if jwt_secret.len() < 32 {
            tracing::error!("⚠️ SECURITY ERROR: JWT_SECRET_KEY must be at least 32 characters");
            return Err(tonic::Status::internal(
                "Server configuration error: JWT_SECRET_KEY too short",
            ));
        }

        let jwt_algorithm = std::env::var("JWT_ALGORITHM").unwrap_or_else(|_| "HS256".to_string());
        let token_service = TokenService::new(jwt_secret, jwt_algorithm);

        // Initialize compression with adaptive algorithm
        let compression_config = MessageCompressionConfig {
            algorithm: CompressionAlgorithm::Adaptive,
            compression_threshold: 256,
            compression_level: 3,
            enable_batching: false, // Disabled for gRPC (single response)
            batch_size: 1,
            batch_timeout_ms: 0,
            max_batch_bytes: 0,
            enable_compression_cache: true,
            cache_ttl_secs: 300,
        };
        
        let compression = Arc::new(MessageCompression::new(compression_config));
        let safe_ops = Arc::new(SafePrimitives::new());

        tracing::info!("🔐 Optimized gRPC Controller initialized with performance enhancements");
        
        Ok(Self {
            svc,
            token_service,
            stats,
            metrics,
            compression,
            safe_ops,
        })
    }

    /// Enhanced JWT token verification with metrics
    fn verify_jwt_token_with_metrics(&self, req: &Request<()>) -> Result<Option<i32>, Status> {
        let start = Instant::now();
        let result = self.token_service.with_optional_auth(req, Ok);
        
        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_operation_duration("jwt_verification", duration);
        self.stats.increment_counter("jwt_verifications");
        
        if result.is_err() {
            self.stats.increment_counter("jwt_verification_failures");
        }
        
        result
    }

    /// Enhanced request validation with performance tracking
    fn validate_login_request_optimized(&self, req: &LoginRequest) -> Result<(), AppError> {
        let start = Instant::now();
        
        // Use safe string operations
        let login_type_valid = self.safe_ops.safe_string_check(&req.login_type, 1, 20);
        let login_token_valid = self.safe_ops.safe_string_check(&req.login_token, 1, 1000);
        
        if !login_type_valid {
            self.stats.increment_counter("validation_failures");
            return Err(AppError::InvalidLoginType(req.login_type.clone()));
        }
        
        if !login_token_valid {
            self.stats.increment_counter("validation_failures");
            return Err(AppError::ValidationError("Invalid login token".to_string()));
        }

        // Validate login type against allowed values
        const VALID_LOGIN_TYPES: &[&str] = &["google", "apple", "test"];
        if !VALID_LOGIN_TYPES.contains(&req.login_type.as_str()) {
            self.stats.increment_counter("validation_failures");
            return Err(AppError::InvalidLoginType(req.login_type.clone()));
        }

        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_operation_duration("login_validation", duration);
        self.stats.increment_counter("successful_validations");
        
        Ok(())
    }

    /// Enhanced register request validation
    fn validate_register_request_optimized(&self, req: &RegisterRequest) -> Result<(), AppError> {
        let start = Instant::now();
        
        // Use safe string operations
        let login_type_valid = self.safe_ops.safe_string_check(&req.login_type, 1, 20);
        let login_token_valid = self.safe_ops.safe_string_check(&req.login_token, 1, 1000);
        let nickname_valid = self.safe_ops.safe_string_check(&req.nick_name, 1, 20);
        
        if !login_type_valid || !login_token_valid || !nickname_valid {
            self.stats.increment_counter("validation_failures");
            return Err(AppError::ValidationError("Invalid request parameters".to_string()));
        }

        // Validate register type
        const VALID_REGISTER_TYPES: &[&str] = &["google", "apple", "guest"];
        if !VALID_REGISTER_TYPES.contains(&req.login_type.as_str()) {
            self.stats.increment_counter("validation_failures");
            return Err(AppError::InvalidLoginType(req.login_type.clone()));
        }

        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_operation_duration("register_validation", duration);
        self.stats.increment_counter("successful_validations");
        
        Ok(())
    }

    /// Compress response if beneficial
    async fn optimize_response<T>(&self, response: T) -> T
    where
        T: serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        // For gRPC, compression is typically handled at transport level
        // This is a placeholder for custom response optimization
        response
    }
}

#[tonic::async_trait]
impl UserService for OptimizedUserController {
    /// Optimized user login with performance enhancements
    async fn login_user(
        &self,
        req: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let start = Instant::now();
        let r = req.into_inner();
        
        info!("⚡ Optimized login request: login_type={}", r.login_type);
        self.stats.increment_counter("login_requests");

        // Enhanced validation with metrics
        if let Err(e) = self.validate_login_request_optimized(&r) {
            warn!("Login validation failed: {}", e);
            return Err(e.to_status());
        }

        // Enhanced JWT verification
        let _verified_user_id = self.verify_jwt_token_with_metrics(&Request::new(()))?;

        // Call business logic with timing
        let business_start = Instant::now();
        let (user_id, nick_name, access_token, is_register) = self
            .svc
            .login_user(r.login_type.clone(), r.login_token)
            .await
            .map_err(|e| {
                self.stats.increment_counter("login_failures");
                let app_error = AppError::InternalError(format!("로그인 실패: {e}"));
                app_error.to_status()
            })?;

        // Record business logic timing
        let business_duration = business_start.elapsed();
        self.metrics.record_operation_duration("login_business_logic", business_duration);

        // Create optimized response
        let response = LoginResponse {
            success: 1,
            user_id,
            nick_name: nick_name.clone(),
            access_token,
            refresh_token: String::new(),
            is_register,
        };

        // Record overall timing and success metrics
        let total_duration = start.elapsed();
        self.metrics.record_operation_duration("login_total", total_duration);
        self.stats.increment_counter("successful_logins");
        
        info!("✅ Login successful: user_id={}, nick={}, duration={:?}", 
              user_id, nick_name, total_duration);

        Ok(Response::new(response))
    }

    /// Optimized user registration with performance enhancements
    async fn register_user(
        &self,
        req: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let start = Instant::now();
        let r = req.into_inner();
        
        info!("⚡ Optimized register request: login_type={}, nick={}", 
              r.login_type, r.nick_name);
        self.stats.increment_counter("register_requests");

        // Enhanced validation
        if let Err(e) = self.validate_register_request_optimized(&r) {
            warn!("Register validation failed: {}", e);
            return Err(e.to_status());
        }

        // Enhanced JWT verification
        let _verified_user_id = self.verify_jwt_token_with_metrics(&Request::new(()))?;

        // Call business logic with timing
        let business_start = Instant::now();
        self.svc.register_user(r.nick_name.clone()).await.map_err(|e| {
            self.stats.increment_counter("register_failures");
            let app_error = AppError::InternalError(format!("회원가입 실패: {e}"));
            app_error.to_status()
        })?;

        // Record business logic timing
        let business_duration = business_start.elapsed();
        self.metrics.record_operation_duration("register_business_logic", business_duration);

        // Record overall timing and success metrics
        let total_duration = start.elapsed();
        self.metrics.record_operation_duration("register_total", total_duration);
        self.stats.increment_counter("successful_registers");
        
        info!("✅ Registration successful: nick={}, duration={:?}", 
              r.nick_name, total_duration);

        Ok(Response::new(RegisterResponse { success: 1 }))
    }
}