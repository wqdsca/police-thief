//! Comprehensive test suite for shared module

use shared::security::{input_validator, jwt, rate_limiter};
use shared::service::redis::core::redis_get_key::get_value_from_redis;
use shared::tool::error::AppError;
use shared::tool::high_performance::{
    dashmap_optimizer::DashMapOptimizer, memory_pool::MemoryPool,
    parallel_processing::ParallelProcessor,
};

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_app_error_conversion() {
        let redis_error = redis::RedisError::from((redis::ErrorKind::IoError, "test error"));
        let app_error = AppError::from(redis_error);
        assert!(matches!(app_error, AppError::RedisError(_)));
    }

    #[test]
    fn test_error_display() {
        let error = AppError::InvalidInput("test input".to_string());
        assert_eq!(error.to_string(), "Invalid input: test input");
    }

    #[test]
    fn test_error_severity() {
        let critical = AppError::DatabaseConnection("connection failed".to_string());
        assert_eq!(
            critical.severity(),
            shared::tool::error::ErrorSeverity::Critical
        );

        let low = AppError::InvalidInput("bad input".to_string());
        assert_eq!(low.severity(), shared::tool::error::ErrorSeverity::Medium);
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;
    use shared::security::input_validator::InputValidator;
    use shared::security::jwt::JwtService;
    use shared::security::rate_limiter::RateLimiter;

    #[tokio::test]
    async fn test_jwt_generation_and_validation() {
        let jwt_service = JwtService::new("test_secret_key_256_bits_minimum_required");

        // Generate token
        let user_id = 123;
        let token = jwt_service.generate_token(user_id).expect("Safe unwrap");
        assert!(!token.is_empty());

        // Validate token
        let decoded_user_id = jwt_service.validate_token(&token).expect("Safe unwrap");
        assert_eq!(decoded_user_id, user_id);
    }

    #[test]
    fn test_input_validation() {
        let validator = InputValidator::new();

        // Test SQL injection
        assert!(!validator.is_safe_sql_input("'; DROP TABLE users; --"));
        assert!(validator.is_safe_sql_input("normal_username"));

        // Test XSS
        assert!(!validator.is_safe_html_input("<script>alert('xss')</script>"));
        assert!(validator.is_safe_html_input("Normal text"));

        // Test path traversal
        assert!(!validator.is_safe_path("../../../etc/passwd"));
        assert!(validator.is_safe_path("documents/file.txt"));
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let rate_limiter = RateLimiter::new(5, 60); // 5 requests per minute
        let client_id = "test_client";

        // First 5 requests should pass
        for _ in 0..5 {
            assert!(rate_limiter.check_rate_limit(client_id).await.is_ok());
        }

        // 6th request should fail
        assert!(rate_limiter.check_rate_limit(client_id).await.is_err());
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use rayon::ThreadPoolBuilder;
    use std::sync::Arc;

    #[test]
    fn test_memory_pool() {
        let pool: MemoryPool<Vec<u8>> = MemoryPool::new(10, || vec![0u8; 1024]);

        // Get objects from pool
        let obj1 = pool.get();
        assert_eq!(obj1.len(), 1024);

        // Return to pool
        pool.put(obj1);

        // Verify reuse
        let obj2 = pool.get();
        assert_eq!(obj2.len(), 1024);
    }

    #[test]
    fn test_dashmap_optimizer() {
        let optimizer = DashMapOptimizer::new();
        let map = optimizer.create_optimized_map::<String, i32>();

        // Concurrent operations
        map.insert("key1".to_string(), 100);
        map.insert("key2".to_string(), 200);

        assert_eq!(*map.get("key1").expect("Safe unwrap"), 100);
        assert_eq!(*map.get("key2").expect("Safe unwrap"), 200);
    }

    #[test]
    fn test_parallel_processing() {
        let thread_pool = ThreadPoolBuilder::new().num_threads(4).build().expect("Safe unwrap");
        let processor = ParallelProcessor::new(thread_pool);

        let data = vec![1, 2, 3, 4, 5];
        let results = processor.process_parallel(data, |x| Ok(x * 2));

        assert_eq!(results, vec![2, 4, 6, 8, 10]);
    }
}

#[cfg(test)]
mod redis_tests {
    use super::*;
    use shared::service::redis::user_redis_service::UserRedisService;

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_redis_operations() {
        let service = UserRedisService::new().await.expect("Safe unwrap");

        // Set and get
        service.set_user_session(1, "session_data").await.expect("Safe unwrap");
        let session = service.get_user_session(1).await.expect("Safe unwrap");
        assert_eq!(session, Some("session_data".to_string()));

        // Delete
        service.delete_user_session(1).await.expect("Safe unwrap");
        let deleted = service.get_user_session(1).await.expect("Safe unwrap");
        assert_eq!(deleted, None);
    }

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_redis_pipeline() {
        let service = UserRedisService::new().await?;

        // Pipeline operations for performance
        let operations = vec![
            ("user:1", "data1"),
            ("user:2", "data2"),
            ("user:3", "data3"),
        ];

        service.batch_set(operations).await?;

        // Verify all were set
        for i in 1..=3 {
            let data = service.get_user_session(i).await?;
            assert!(data.is_some());
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_full_authentication_flow() {
        let jwt_service = JwtService::new("test_secret");
        let validator = InputValidator::new();
        let rate_limiter = RateLimiter::new(10, 60);

        // Validate input
        let username = "test_user";
        assert!(validator.is_safe_sql_input(username));

        // Check rate limit
        assert!(rate_limiter.check_rate_limit(username).await.is_ok());

        // Generate token
        let token = jwt_service.generate_token(123).expect("Safe unwrap");

        // Validate token
        let user_id = jwt_service.validate_token(&token).expect("Safe unwrap");
        assert_eq!(user_id, 123);
    }
}

#[cfg(test)]
mod logging_tests {
    use shared::logging::config::LogConfig;
    use shared::logging::system::LoggingSystem;

    #[test]
    fn test_logging_configuration() {
        let config = LogConfig::default();
        assert_eq!(config.level, "info");
        assert!(config.enable_file_output);
        assert_eq!(config.max_file_size_mb, 100);
    }

    #[tokio::test]
    async fn test_logging_system_init() {
        let config = LogConfig::default();
        let system = LoggingSystem::init(config);
        assert!(system.is_ok());

        // Test logging
        tracing::info!("Test log message");
        tracing::error!("Test error message");
    }
}
