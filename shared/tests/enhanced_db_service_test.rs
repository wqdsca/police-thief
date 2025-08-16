//! Integration tests for Enhanced Database Service
//! 
//! Tests ID management, pagination, search, bulk operations, and other enhanced features.

use shared::config::db::{DbConfig, DbConnection};
use shared::service::db::{
    EnhancedBaseDbService, EnhancedBaseDbServiceImpl,
    IdStrategy, PaginationParams, SearchParams, SearchMatchType, SortOrder,
    BulkOperationResult, LockType, SnowflakeIdGenerator,
};
use std::collections::HashMap;
use std::sync::Arc;
use sqlx::mysql::MySqlPool;

/// Test database configuration
async fn setup_test_db() -> Arc<MySqlPool> {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "mysql://test:test@localhost:3306/test_police_thief".to_string());
    
    let pool = MySqlPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");
    
    Arc::new(pool)
}

/// Setup test table
async fn setup_test_table(pool: &MySqlPool) {
    // Drop existing test tables
    sqlx::query("DROP TABLE IF EXISTS test_users")
        .execute(pool)
        .await
        .ok();
    
    sqlx::query("DROP TABLE IF EXISTS test_orders")
        .execute(pool)
        .await
        .ok();
    
    // Create test users table
    sqlx::query(
        r#"
        CREATE TABLE test_users (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            email VARCHAR(255) UNIQUE NOT NULL,
            username VARCHAR(100) NOT NULL,
            full_name VARCHAR(255),
            status VARCHAR(50) DEFAULT 'active',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            deleted_at TIMESTAMP NULL,
            INDEX idx_username (username),
            INDEX idx_status (status)
        )
        "#
    )
    .execute(pool)
    .await
    .expect("Failed to create test_users table");
    
    // Create test orders table
    sqlx::query(
        r#"
        CREATE TABLE test_orders (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            user_id BIGINT NOT NULL,
            amount DECIMAL(10, 2) NOT NULL,
            status VARCHAR(50) DEFAULT 'pending',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            INDEX idx_user_id (user_id),
            INDEX idx_status (status)
        )
        "#
    )
    .execute(pool)
    .await
    .expect("Failed to create test_orders table");
    
    // Insert test data
    sqlx::query(
        r#"
        INSERT INTO test_users (email, username, full_name, status) VALUES
        ('john@example.com', 'john_doe', 'John Doe', 'active'),
        ('jane@example.com', 'jane_smith', 'Jane Smith', 'active'),
        ('bob@example.com', 'bob_johnson', 'Bob Johnson', 'inactive'),
        ('alice@example.com', 'alice_wong', 'Alice Wong', 'active'),
        ('charlie@example.com', 'charlie_brown', 'Charlie Brown', 'active')
        "#
    )
    .execute(pool)
    .await
    .expect("Failed to insert test users");
    
    // Insert test orders
    sqlx::query(
        r#"
        INSERT INTO test_orders (user_id, amount, status) VALUES
        (1, 100.50, 'completed'),
        (1, 200.00, 'completed'),
        (2, 150.75, 'pending'),
        (3, 300.00, 'completed'),
        (4, 50.25, 'cancelled')
        "#
    )
    .execute(pool)
    .await
    .expect("Failed to insert test orders");
}

/// Cleanup test data
async fn cleanup_test_data(pool: &MySqlPool) {
    sqlx::query("DROP TABLE IF EXISTS test_users")
        .execute(pool)
        .await
        .ok();
    
    sqlx::query("DROP TABLE IF EXISTS test_orders")
        .execute(pool)
        .await
        .ok();
}

#[tokio::test]
async fn test_id_generation_strategies() {
    let pool = setup_test_db().await;
    setup_test_table(&pool).await;
    
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .with_snowflake(1, 1)
        .await
        .expect("Failed to create db service");
    
    // Test UUID generation
    let uuid_id = db_service.generate_id(&IdStrategy::Uuid)
        .await
        .expect("Failed to generate UUID");
    assert!(uuid_id.len() == 36); // Standard UUID length
    println!("Generated UUID: {}", uuid_id);
    
    // Test Snowflake ID generation
    let snowflake_id = db_service.generate_id(&IdStrategy::Snowflake {
        machine_id: 1,
        datacenter_id: 1,
    }).await.expect("Failed to generate Snowflake ID");
    assert!(snowflake_id.parse::<u64>().is_ok());
    println!("Generated Snowflake ID: {}", snowflake_id);
    
    // Test Prefixed Sequence
    let prefixed_id = db_service.generate_id(&IdStrategy::PrefixedSequence {
        prefix: "USR_".to_string(),
        padding: 6,
    }).await.expect("Failed to generate prefixed ID");
    assert!(prefixed_id.starts_with("USR_"));
    println!("Generated Prefixed ID: {}", prefixed_id);
    
    // Test Timestamp-based ID
    let timestamp_id = db_service.generate_id(&IdStrategy::TimestampBased {
        prefix: Some("TS_".to_string()),
    }).await.expect("Failed to generate timestamp ID");
    assert!(timestamp_id.starts_with("TS_"));
    println!("Generated Timestamp ID: {}", timestamp_id);
    
    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_snowflake_id_uniqueness() {
    let generator = SnowflakeIdGenerator::new(1, 1);
    
    let mut ids = Vec::new();
    for _ in 0..1000 {
        ids.push(generator.generate().await);
    }
    
    // Check all IDs are unique
    let unique_count = ids.iter().collect::<std::collections::HashSet<_>>().len();
    assert_eq!(unique_count, 1000, "Snowflake IDs should be unique");
    
    // Check IDs are increasing
    for i in 1..ids.len() {
        assert!(ids[i] >= ids[i-1], "Snowflake IDs should be monotonically increasing");
    }
}

#[tokio::test]
async fn test_common_query_operations() {
    let pool = setup_test_db().await;
    setup_test_table(&pool).await;
    
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .await
        .expect("Failed to create db service");
    
    // Test exists
    let mut params = HashMap::new();
    params.insert("email".to_string(), serde_json::json!("john@example.com"));
    
    let exists = db_service.exists("test_users", "email = ?", Some(params.clone()))
        .await
        .expect("Failed to check existence");
    assert!(exists, "User should exist");
    
    // Test get_one
    let user = db_service.get_one("test_users", "email = ?", Some(params.clone()))
        .await
        .expect("Failed to get user")
        .expect("User not found");
    assert_eq!(user.get("username").expect("Safe unwrap"), "john_doe");
    
    // Test get_by_id
    let user_by_id = db_service.get_by_id("test_users", "id", "1")
        .await
        .expect("Failed to get user by ID")
        .expect("User not found");
    assert_eq!(user_by_id.get("email").expect("Safe unwrap"), "john@example.com");
    
    // Test count
    let total_count = db_service.count("test_users", None, None)
        .await
        .expect("Failed to count users");
    assert_eq!(total_count, 5);
    
    let active_count = db_service.count("test_users", Some("status = 'active'"), None)
        .await
        .expect("Failed to count active users");
    assert_eq!(active_count, 4);
    
    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_pagination() {
    let pool = setup_test_db().await;
    setup_test_table(&pool).await;
    
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .await
        .expect("Failed to create db service");
    
    // Test pagination with sorting
    let pagination = PaginationParams {
        page: 1,
        page_size: 2,
        sort_by: Some("username".to_string()),
        sort_order: Some(SortOrder::Asc),
    };
    
    let result = db_service.paginate(
        "test_users",
        pagination,
        Some("status = 'active'"),
        None
    ).await.expect("Failed to paginate");
    
    assert_eq!(result.page, 1);
    assert_eq!(result.page_size, 2);
    assert_eq!(result.total, 4);
    assert_eq!(result.total_pages, 2);
    assert!(result.has_next);
    assert!(!result.has_prev);
    assert_eq!(result.data.len(), 2);
    
    // Test page 2
    let pagination2 = PaginationParams {
        page: 2,
        page_size: 2,
        sort_by: Some("username".to_string()),
        sort_order: Some(SortOrder::Asc),
    };
    
    let result2 = db_service.paginate(
        "test_users",
        pagination2,
        Some("status = 'active'"),
        None
    ).await.expect("Failed to paginate page 2");
    
    assert_eq!(result2.page, 2);
    assert!(!result2.has_next);
    assert!(result2.has_prev);
    
    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_search() {
    let pool = setup_test_db().await;
    setup_test_table(&pool).await;
    
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .await
        .expect("Failed to create db service");
    
    // Test search with Contains match
    let search_params = SearchParams {
        keyword: "john".to_string(),
        fields: vec!["username".to_string(), "email".to_string(), "full_name".to_string()],
        match_type: SearchMatchType::Contains,
        case_sensitive: false,
    };
    
    let result = db_service.search(
        "test_users",
        search_params,
        None
    ).await.expect("Failed to search");
    
    assert_eq!(result.total, 2); // john_doe and bob_johnson
    
    // Test search with StartsWith match
    let search_params2 = SearchParams {
        keyword: "alice".to_string(),
        fields: vec!["username".to_string()],
        match_type: SearchMatchType::StartsWith,
        case_sensitive: false,
    };
    
    let result2 = db_service.search(
        "test_users",
        search_params2,
        None
    ).await.expect("Failed to search with StartsWith");
    
    assert_eq!(result2.total, 1); // alice_wong
    
    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_upsert_operations() {
    let pool = setup_test_db().await;
    setup_test_table(&pool).await;
    
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .await
        .expect("Failed to create db service");
    
    // Test upsert - insert new record
    let mut new_user = HashMap::new();
    new_user.insert("email".to_string(), serde_json::json!("newuser@example.com"));
    new_user.insert("username".to_string(), serde_json::json!("new_user"));
    new_user.insert("status".to_string(), serde_json::json!("active"));
    
    let affected = db_service.upsert(
        "test_users",
        new_user.clone(),
        vec!["email".to_string()]
    ).await.expect("Failed to upsert new user");
    
    assert!(affected > 0);
    
    // Test upsert - update existing record
    new_user.insert("username".to_string(), serde_json::json!("updated_user"));
    
    let affected2 = db_service.upsert(
        "test_users",
        new_user,
        vec!["email".to_string()]
    ).await.expect("Failed to upsert existing user");
    
    assert!(affected2 > 0);
    
    // Verify update
    let mut params = HashMap::new();
    params.insert("email".to_string(), serde_json::json!("newuser@example.com"));
    
    let user = db_service.get_one("test_users", "email = ?", Some(params))
        .await
        .expect("Failed to get user")
        .expect("User not found");
    
    assert_eq!(user.get("username").expect("Safe unwrap"), "updated_user");
    
    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_bulk_operations() {
    let pool = setup_test_db().await;
    setup_test_table(&pool).await;
    
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .await
        .expect("Failed to create db service");
    
    // Prepare bulk data
    let bulk_data = vec![
        {
            let mut data = HashMap::new();
            data.insert("email".to_string(), serde_json::json!("bulk1@example.com"));
            data.insert("username".to_string(), serde_json::json!("bulk_user1"));
            data.insert("status".to_string(), serde_json::json!("active"));
            data
        },
        {
            let mut data = HashMap::new();
            data.insert("email".to_string(), serde_json::json!("bulk2@example.com"));
            data.insert("username".to_string(), serde_json::json!("bulk_user2"));
            data.insert("status".to_string(), serde_json::json!("active"));
            data
        },
        {
            let mut data = HashMap::new();
            data.insert("email".to_string(), serde_json::json!("john@example.com")); // Existing
            data.insert("username".to_string(), serde_json::json!("john_updated"));
            data.insert("status".to_string(), serde_json::json!("inactive"));
            data
        },
    ];
    
    // Test bulk upsert
    let result: BulkOperationResult = db_service.bulk_upsert(
        "test_users",
        bulk_data,
        vec!["email".to_string()]
    ).await.expect("Failed to bulk upsert");
    
    assert_eq!(result.total, 3);
    assert_eq!(result.success, 3);
    assert_eq!(result.failed, 0);
    
    // Verify bulk operation results
    let total_count = db_service.count("test_users", None, None)
        .await
        .expect("Failed to count after bulk");
    
    assert_eq!(total_count, 7); // 5 original + 2 new
    
    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_soft_delete_and_restore() {
    let pool = setup_test_db().await;
    setup_test_table(&pool).await;
    
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .await
        .expect("Failed to create db service");
    
    // Test soft delete
    let deleted = db_service.soft_delete(
        "test_users",
        "status = 'inactive'",
        None,
        "deleted_at"
    ).await.expect("Failed to soft delete");
    
    assert_eq!(deleted, 1); // Bob Johnson
    
    // Verify soft delete
    let active_count = db_service.count("test_users", Some("deleted_at IS NULL"), None)
        .await
        .expect("Failed to count non-deleted");
    
    assert_eq!(active_count, 4);
    
    // Test restore
    let restored = db_service.restore(
        "test_users",
        "email = 'bob@example.com'",
        None,
        "deleted_at"
    ).await.expect("Failed to restore");
    
    assert_eq!(restored, 1);
    
    // Verify restore
    let total_active = db_service.count("test_users", Some("deleted_at IS NULL"), None)
        .await
        .expect("Failed to count after restore");
    
    assert_eq!(total_active, 5);
    
    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_aggregate_functions() {
    let pool = setup_test_db().await;
    setup_test_table(&pool).await;
    
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .await
        .expect("Failed to create db service");
    
    // Test sum
    let total_amount = db_service.sum(
        "test_orders",
        "amount",
        Some("status = 'completed'"),
        None
    ).await.expect("Failed to calculate sum");
    
    assert_eq!(total_amount, 600.50); // 100.50 + 200.00 + 300.00
    
    // Test average
    let avg_amount = db_service.avg(
        "test_orders",
        "amount",
        None,
        None
    ).await.expect("Failed to calculate average");
    
    assert!((avg_amount - 160.30).abs() < 0.01); // (100.50 + 200.00 + 150.75 + 300.00 + 50.25) / 5
    
    // Test min
    let min_amount: Option<f64> = db_service.min(
        "test_orders",
        "amount",
        None,
        None
    ).await.expect("Failed to get min");
    
    assert_eq!(min_amount, Some(50.25));
    
    // Test max
    let max_amount: Option<f64> = db_service.max(
        "test_orders",
        "amount",
        None,
        None
    ).await.expect("Failed to get max");
    
    assert_eq!(max_amount, Some(300.00));
    
    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_transaction_operations() {
    let pool = setup_test_db().await;
    setup_test_table(&pool).await;
    
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .await
        .expect("Failed to create db service");
    
    // Test successful transaction
    let result = db_service.with_transaction(|tx| {
        Box::pin(async move {
            // Insert new user
            sqlx::query(
                "INSERT INTO test_users (email, username, status) VALUES (?, ?, ?)"
            )
            .bind("transaction@example.com")
            .bind("tx_user")
            .bind("active")
            .execute(&mut **tx)
            .await
            .map_err(|e| shared::tool::error::AppError::DatabaseQuery(e.to_string()))?;
            
            // Get the inserted ID
            let row = sqlx::query("SELECT LAST_INSERT_ID() as id")
                .fetch_one(&mut **tx)
                .await
                .map_err(|e| shared::tool::error::AppError::DatabaseQuery(e.to_string()))?;
            
            let user_id: u64 = row.get("id");
            
            // Insert order for the user
            sqlx::query(
                "INSERT INTO test_orders (user_id, amount, status) VALUES (?, ?, ?)"
            )
            .bind(user_id as i64)
            .bind(999.99)
            .bind("pending")
            .execute(&mut **tx)
            .await
            .map_err(|e| shared::tool::error::AppError::DatabaseQuery(e.to_string()))?;
            
            Ok(user_id)
        })
    }).await.expect("Transaction failed");
    
    assert!(result > 0);
    
    // Verify transaction results
    let user_count = db_service.count("test_users", None, None)
        .await
        .expect("Failed to count users");
    assert_eq!(user_count, 6);
    
    let order_count = db_service.count("test_orders", None, None)
        .await
        .expect("Failed to count orders");
    assert_eq!(order_count, 6);
    
    // Test failed transaction (should rollback)
    let failed_result = db_service.with_transaction(|tx| {
        Box::pin(async move {
            // Try to insert duplicate email
            sqlx::query(
                "INSERT INTO test_users (email, username, status) VALUES (?, ?, ?)"
            )
            .bind("john@example.com") // Duplicate
            .bind("duplicate_user")
            .bind("active")
            .execute(&mut **tx)
            .await
            .map_err(|e| shared::tool::error::AppError::DatabaseQuery(e.to_string()))?;
            
            Ok(())
        })
    }).await;
    
    assert!(failed_result.is_err());
    
    // Verify rollback (count should remain the same)
    let user_count_after = db_service.count("test_users", None, None)
        .await
        .expect("Failed to count users after rollback");
    assert_eq!(user_count_after, 6);
    
    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_utility_functions() {
    let pool = setup_test_db().await;
    setup_test_table(&pool).await;
    
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .await
        .expect("Failed to create db service");
    
    // Test table size
    let table_size = db_service.get_table_size("test_users")
        .await
        .expect("Failed to get table size");
    
    assert!(table_size > 0);
    println!("Table size: {} bytes", table_size);
    
    // Test backup table
    let backed_up = db_service.backup_table("test_users", "test_users_backup")
        .await
        .expect("Failed to backup table");
    
    assert_eq!(backed_up, 5);
    
    // Verify backup
    let backup_count = db_service.count("test_users_backup", None, None)
        .await
        .expect("Failed to count backup table");
    
    assert_eq!(backup_count, 5);
    
    // Cleanup backup table
    sqlx::query("DROP TABLE IF EXISTS test_users_backup")
        .execute(&**pool)
        .await
        .ok();
    
    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_insert_returning_id() {
    let pool = setup_test_db().await;
    setup_test_table(&pool).await;
    
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .await
        .expect("Failed to create db service");
    
    // Test insert returning ID
    let mut new_user = HashMap::new();
    new_user.insert("email".to_string(), serde_json::json!("return_id@example.com"));
    new_user.insert("username".to_string(), serde_json::json!("return_id_user"));
    new_user.insert("status".to_string(), serde_json::json!("active"));
    
    let new_id = db_service.insert_returning_id(
        "test_users",
        new_user,
        "id"
    ).await.expect("Failed to insert and return ID");
    
    assert!(new_id.parse::<u64>().expect("Safe unwrap") > 5);
    println!("New user ID: {}", new_id);
    
    // Verify the insert
    let user = db_service.get_by_id("test_users", "id", &new_id)
        .await
        .expect("Failed to get inserted user")
        .expect("User not found");
    
    assert_eq!(user.get("email").expect("Safe unwrap"), "return_id@example.com");
    
    cleanup_test_data(&pool).await;
}

#[tokio::test]
async fn test_get_next_auto_increment_id() {
    let pool = setup_test_db().await;
    setup_test_table(&pool).await;
    
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .await
        .expect("Failed to create db service");
    
    // Get next auto-increment ID
    let next_id = db_service.get_next_auto_increment_id("test_users")
        .await
        .expect("Failed to get next auto-increment ID");
    
    assert!(next_id > 5);
    println!("Next auto-increment ID: {}", next_id);
    
    // Insert a new record and verify the ID matches
    let mut new_user = HashMap::new();
    new_user.insert("email".to_string(), serde_json::json!("auto_inc@example.com"));
    new_user.insert("username".to_string(), serde_json::json!("auto_inc_user"));
    
    let inserted_id = db_service.insert_returning_id(
        "test_users",
        new_user,
        "id"
    ).await.expect("Failed to insert");
    
    assert_eq!(inserted_id.parse::<u64>().expect("Safe unwrap"), next_id);
    
    cleanup_test_data(&pool).await;
}