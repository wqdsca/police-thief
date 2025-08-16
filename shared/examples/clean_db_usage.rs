//! Clean Architecture Database Service Usage Example
//!
//! Demonstrates the modular, clean architecture approach to database operations

use shared::service::db::{
    BaseDbService, BaseDbServiceImpl, ConnectionStats, DbServiceConfig, IsolationLevel,
    MonitoringConfig, PerformanceConfig, PoolConfig, QueryConfig, QueryParams,
};
use std::collections::HashMap;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ========== 1. Configuration with Builder Pattern ==========
    println!("=== Clean Architecture Database Service Demo ===\n");

    // Create configuration with fine-grained control
    let config = DbServiceConfig::from_env()?
        .with_query_config(QueryConfig {
            enable_query_logging: true,
            slow_query_threshold_ms: 500,
            default_timeout: Duration::from_secs(30),
            max_result_size: 10000,
            enable_query_plan: true,
        })
        .with_pool_config(PoolConfig {
            min_connections: 5,
            max_connections: 100,
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(3600),
            enable_retry: true,
            max_retries: 3,
            retry_delay: Duration::from_millis(100),
        })
        .with_monitoring(MonitoringConfig {
            enable_metrics: true,
            metrics_interval: Duration::from_secs(60),
            enable_tracing: true,
            enable_pool_monitoring: true,
            alert_on_errors: true,
            error_threshold: 10,
        })
        .with_performance(PerformanceConfig {
            enable_query_cache: true,
            query_cache_size: 1000,
            query_cache_ttl: Duration::from_secs(300),
            use_prepared_statements: true,
            prepared_cache_size: 100,
            optimize_batch_operations: true,
            default_batch_size: 1000,
        });

    // Or use pre-configured optimized settings
    // let config = DbServiceConfig::from_env()?.optimized().with_full_monitoring();

    // ========== 2. Service Initialization ==========
    let db_service = BaseDbServiceImpl::new(config).await?;
    println!("✅ Database service initialized with clean architecture\n");

    // ========== 3. Health Check and Statistics ==========
    if db_service.health_check().await? {
        println!("✅ Database connection healthy");
    }

    let stats: ConnectionStats = db_service.get_connection_stats().await?;
    println!("📊 Connection Statistics:");
    println!("   Active: {}/{}", stats.active_connections, stats.max_connections);
    println!("   Idle: {}", stats.idle_connections);
    println!("   Total Queries: {}", stats.total_queries);
    println!("   Errors: {}\n", stats.connection_errors);

    // ========== 4. Metadata Operations ==========
    println!("=== Metadata Operations ===\n");

    // Get database list
    let databases = db_service.get_databases().await?;
    println!("📁 Available databases: {}", databases.len());
    for db in databases.iter().take(3) {
        println!("   - {} (charset: {}, collation: {})", db.name, db.charset, db.collation);
    }

    // Check table existence
    if db_service.table_exists("users").await? {
        println!("\n✅ Table 'users' exists");

        // Get table columns
        let columns = db_service.get_columns("users").await?;
        println!("📋 Columns in 'users' table:");
        for col in columns.iter().take(5) {
            println!(
                "   - {} ({}) {}{}",
                col.name,
                col.data_type,
                if col.is_nullable { "NULL" } else { "NOT NULL" },
                if col.is_primary_key { " [PRIMARY]" } else { "" }
            );
        }

        // Get indexes
        let indexes = db_service.get_indexes("users").await?;
        println!("\n🔍 Indexes on 'users' table:");
        for idx in indexes.iter().take(3) {
            println!(
                "   - {} on [{}] {}",
                idx.name,
                idx.column_names.join(", "),
                if idx.is_unique { "(UNIQUE)" } else { "" }
            );
        }
    }

    // ========== 5. Query Operations ==========
    println!("\n=== Query Operations ===\n");

    // Simple SELECT
    let users = db_service
        .select("users", Some("status = 'active'"), None)
        .await?;
    println!("👥 Active users found: {}", users.len());

    // INSERT with parameters
    let mut new_user = HashMap::new();
    new_user.insert("email".to_string(), serde_json::json!("test@example.com"));
    new_user.insert("username".to_string(), serde_json::json!("testuser"));
    new_user.insert("status".to_string(), serde_json::json!("active"));

    // Note: This would actually insert - commented for safety
    // let inserted_id = db_service.insert("users", new_user).await?;
    // println!("✅ User inserted with {} rows affected", inserted_id);

    // UPDATE with parameters
    let mut update_data = HashMap::new();
    update_data.insert("last_login".to_string(), serde_json::json!(chrono::Utc::now()));

    let mut where_params = HashMap::new();
    where_params.insert("id".to_string(), serde_json::json!(1));

    // Note: This would actually update - commented for safety
    // let updated = db_service.update(
    //     "users",
    //     update_data,
    //     "id = ?",
    //     Some(where_params)
    // ).await?;
    // println!("✅ Updated {} rows", updated);

    // ========== 6. Transaction Management ==========
    println!("\n=== Transaction Management ===\n");

    // Standard transaction
    let result = db_service
        .with_transaction(|tx| {
            Box::pin(async move {
                // Multiple operations in a single transaction
                sqlx::query("SELECT COUNT(*) as count FROM users")
                    .fetch_one(&mut **tx)
                    .await
                    .map_err(|e| shared::tool::error::AppError::DatabaseQuery(e.to_string()))?;

                println!("   📝 Transaction operation executed");

                // Return value from transaction
                Ok("Transaction completed successfully".to_string())
            })
        })
        .await?;

    println!("✅ {}", result);

    // Transaction with custom isolation level
    let isolated_result = db_service
        .with_isolation_level(IsolationLevel::ReadCommitted, |tx| {
            Box::pin(async move {
                // Operations with READ COMMITTED isolation
                println!("   🔒 Running with READ COMMITTED isolation");
                Ok(())
            })
        })
        .await?;

    // Transaction with retry on deadlock
    let retry_result = db_service
        .with_retry(
            |tx| {
                Box::pin(async move {
                    // Operations that might deadlock
                    println!("   🔄 Operation with automatic retry on deadlock");
                    Ok(42)
                })
            },
            3, // max retries
        )
        .await?;

    println!("✅ Retry-enabled transaction returned: {}", retry_result);

    // ========== 7. Batch Operations ==========
    println!("\n=== Batch Operations ===\n");

    // Prepare batch data
    let batch_data: Vec<QueryParams> = (1..=5)
        .map(|i| {
            let mut data = HashMap::new();
            data.insert("email".to_string(), serde_json::json!(format!("user{}@example.com", i)));
            data.insert("username".to_string(), serde_json::json!(format!("user{}", i)));
            data.insert("status".to_string(), serde_json::json!("pending"));
            data
        })
        .collect();

    // Note: This would actually insert - commented for safety
    // let batch_result = db_service.batch_insert("users", batch_data).await?;
    // println!("✅ Batch insert completed: {} rows", batch_result);
    println!("📦 Prepared {} items for batch insert (skipped for safety)", batch_data.len());

    // ========== 8. Raw Query Execution ==========
    println!("\n=== Raw Query Execution ===\n");

    // Complex SELECT query
    let sql = r#"
        SELECT 
            u.username,
            COUNT(o.id) as order_count,
            MAX(o.created_at) as last_order
        FROM users u
        LEFT JOIN orders o ON u.id = o.user_id
        WHERE u.status = ?
        GROUP BY u.id, u.username
        LIMIT 10
    "#;

    let mut params = HashMap::new();
    params.insert("status".to_string(), serde_json::json!("active"));

    // Note: Adjust SQL based on your actual schema
    // let results = db_service.execute_query(sql, Some(params)).await?;
    // println!("📊 Query returned {} rows", results.len());

    // ========== 9. Performance Features ==========
    println!("\n=== Performance Features ===\n");

    // The service automatically:
    println!("⚡ Performance optimizations enabled:");
    println!("   ✓ Query result caching");
    println!("   ✓ Prepared statement caching");
    println!("   ✓ Connection pooling with warmup");
    println!("   ✓ Batch operation optimization");
    println!("   ✓ Automatic retry on connection failure");
    println!("   ✓ Slow query detection and logging");

    // ========== 10. Clean Shutdown ==========
    println!("\n=== Service Information ===\n");

    let config = db_service.config();
    println!("⚙️  Configuration Summary:");
    println!("   Pool: {}-{} connections", config.pool_config.min_connections, config.pool_config.max_connections);
    println!("   Query timeout: {:?}", config.query_config.default_timeout);
    println!("   Slow query threshold: {} ms", config.query_config.slow_query_threshold_ms);
    println!("   Monitoring: {}", if config.monitoring_config.enable_metrics { "Enabled" } else { "Disabled" });
    println!("   Query cache: {}", if config.performance_config.enable_query_cache { "Enabled" } else { "Disabled" });

    println!("\n✅ Clean Architecture Database Service Demo Complete!");
    println!("\n📚 Benefits of this architecture:");
    println!("   1. Separation of Concerns - Each module has a single responsibility");
    println!("   2. Testability - Each component can be tested independently");
    println!("   3. Maintainability - Changes are isolated to specific modules");
    println!("   4. Flexibility - Easy to swap implementations or add features");
    println!("   5. Performance - Optimizations are centralized and configurable");

    Ok(())
}