//! Enhanced Database Service Usage Examples
//! 
//! This example demonstrates how to use the enhanced database service
//! with ID management and common database functions.

use shared::config::db::{DbConfig, DbConnection};
use shared::service::db::{
    EnhancedBaseDbService, EnhancedBaseDbServiceImpl,
    IdStrategy, PaginationParams, SearchParams, SearchMatchType, SortOrder,
    BulkOperationResult, LockType,
};
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize database connection
    let db_config = DbConfig::from_env()?;
    let pool = Arc::new(db_config.create_pool().await?);
    
    // Create enhanced database service
    let db_service = EnhancedBaseDbServiceImpl::new(pool.clone())
        .with_snowflake(1, 1) // Machine ID: 1, Datacenter ID: 1
        .await?;
    
    // ========== ID Management Examples ==========
    
    println!("=== ID Management Examples ===\n");
    
    // 1. Generate different types of IDs
    let uuid_id = db_service.generate_id(&IdStrategy::Uuid).await?;
    println!("Generated UUID: {}", uuid_id);
    
    let snowflake_id = db_service.generate_id(&IdStrategy::Snowflake {
        machine_id: 1,
        datacenter_id: 1,
    }).await?;
    println!("Generated Snowflake ID: {}", snowflake_id);
    
    let prefixed_id = db_service.generate_id(&IdStrategy::PrefixedSequence {
        prefix: "USER_".to_string(),
        padding: 6,
    }).await?;
    println!("Generated Prefixed ID: {}", prefixed_id);
    
    let timestamp_id = db_service.generate_id(&IdStrategy::TimestampBased {
        prefix: Some("TS_".to_string()),
    }).await?;
    println!("Generated Timestamp ID: {}", timestamp_id);
    
    // 2. Get ID information for a table
    let id_info = db_service.get_id_info("users").await?;
    println!("\nID Info for 'users' table:");
    println!("  Column: {}", id_info.column);
    println!("  Strategy: {:?}", id_info.strategy);
    println!("  Last Value: {:?}", id_info.last_value);
    
    // 3. Get next auto-increment ID
    let next_id = db_service.get_next_auto_increment_id("users").await?;
    println!("\nNext auto-increment ID for 'users': {}", next_id);
    
    // ========== Common Query Operations ==========
    
    println!("\n=== Common Query Operations ===\n");
    
    // 1. Check if record exists
    let mut params = HashMap::new();
    params.insert("email".to_string(), serde_json::json!("user@example.com"));
    
    let exists = db_service.exists("users", "email = ?", Some(params.clone())).await?;
    println!("User exists: {}", exists);
    
    // 2. Get single record
    if let Some(user) = db_service.get_one("users", "email = ?", Some(params.clone())).await? {
        println!("Found user: {:?}", user);
    }
    
    // 3. Get record by ID
    if let Some(user) = db_service.get_by_id("users", "id", "1").await? {
        println!("User with ID 1: {:?}", user);
    }
    
    // 4. Count records
    let total_users = db_service.count("users", None, None).await?;
    println!("Total users: {}", total_users);
    
    let active_users = db_service.count("users", Some("status = 'active'"), None).await?;
    println!("Active users: {}", active_users);
    
    // ========== Pagination Examples ==========
    
    println!("\n=== Pagination Examples ===\n");
    
    let pagination = PaginationParams {
        page: 1,
        page_size: 10,
        sort_by: Some("created_at".to_string()),
        sort_order: Some(SortOrder::Desc),
    };
    
    let page_result = db_service.paginate(
        "users",
        pagination,
        Some("status = 'active'"),
        None
    ).await?;
    
    println!("Pagination Result:");
    println!("  Page: {}/{}", page_result.page, page_result.total_pages);
    println!("  Records: {}/{}", page_result.data.len(), page_result.total);
    println!("  Has Next: {}", page_result.has_next);
    println!("  Has Prev: {}", page_result.has_prev);
    
    // ========== Search Examples ==========
    
    println!("\n=== Search Examples ===\n");
    
    let search_params = SearchParams {
        keyword: "john".to_string(),
        fields: vec!["username".to_string(), "email".to_string(), "full_name".to_string()],
        match_type: SearchMatchType::Contains,
        case_sensitive: false,
    };
    
    let search_result = db_service.search(
        "users",
        search_params,
        Some(PaginationParams {
            page: 1,
            page_size: 20,
            sort_by: None,
            sort_order: None,
        })
    ).await?;
    
    println!("Search results for 'john': {} matches", search_result.total);
    
    // ========== Advanced Operations ==========
    
    println!("\n=== Advanced Operations ===\n");
    
    // 1. Upsert (INSERT or UPDATE)
    let mut user_data = HashMap::new();
    user_data.insert("email".to_string(), serde_json::json!("newuser@example.com"));
    user_data.insert("username".to_string(), serde_json::json!("newuser"));
    user_data.insert("status".to_string(), serde_json::json!("active"));
    
    let affected = db_service.upsert(
        "users",
        user_data.clone(),
        vec!["email".to_string()] // Unique keys
    ).await?;
    println!("Upsert affected rows: {}", affected);
    
    // 2. Insert and return ID
    let new_id = db_service.insert_returning_id(
        "users",
        user_data.clone(),
        "id"
    ).await?;
    println!("New record ID: {}", new_id);
    
    // 3. Bulk upsert
    let bulk_data = vec![
        {
            let mut data = HashMap::new();
            data.insert("email".to_string(), serde_json::json!("user1@example.com"));
            data.insert("username".to_string(), serde_json::json!("user1"));
            data
        },
        {
            let mut data = HashMap::new();
            data.insert("email".to_string(), serde_json::json!("user2@example.com"));
            data.insert("username".to_string(), serde_json::json!("user2"));
            data
        },
    ];
    
    let bulk_result: BulkOperationResult = db_service.bulk_upsert(
        "users",
        bulk_data,
        vec!["email".to_string()]
    ).await?;
    
    println!("Bulk upsert result:");
    println!("  Total: {}", bulk_result.total);
    println!("  Success: {}", bulk_result.success);
    println!("  Failed: {}", bulk_result.failed);
    
    // 4. Soft delete
    let deleted = db_service.soft_delete(
        "users",
        "status = 'inactive'",
        None,
        "deleted_at"
    ).await?;
    println!("Soft deleted {} records", deleted);
    
    // 5. Restore soft deleted
    let restored = db_service.restore(
        "users",
        "id = 1",
        None,
        "deleted_at"
    ).await?;
    println!("Restored {} records", restored);
    
    // ========== Aggregate Functions ==========
    
    println!("\n=== Aggregate Functions ===\n");
    
    // 1. Sum
    let total_amount = db_service.sum(
        "orders",
        "amount",
        Some("status = 'completed'"),
        None
    ).await?;
    println!("Total completed orders amount: ${:.2}", total_amount);
    
    // 2. Average
    let avg_amount = db_service.avg(
        "orders",
        "amount",
        None,
        None
    ).await?;
    println!("Average order amount: ${:.2}", avg_amount);
    
    // 3. Min/Max
    let min_amount: Option<f64> = db_service.min(
        "orders",
        "amount",
        None,
        None
    ).await?;
    println!("Minimum order amount: {:?}", min_amount);
    
    let max_amount: Option<f64> = db_service.max(
        "orders",
        "amount",
        None,
        None
    ).await?;
    println!("Maximum order amount: {:?}", max_amount);
    
    // ========== Utility Functions ==========
    
    println!("\n=== Utility Functions ===\n");
    
    // 1. Get table size
    let table_size = db_service.get_table_size("users").await?;
    println!("Users table size: {} bytes", table_size);
    
    // 2. Table locking
    db_service.lock_tables(vec![
        ("users".to_string(), LockType::Write),
        ("orders".to_string(), LockType::Read),
    ]).await?;
    println!("Tables locked");
    
    // Do some operations...
    
    db_service.unlock_tables().await?;
    println!("Tables unlocked");
    
    // 3. Backup table
    let backed_up = db_service.backup_table("users", "users_backup_20240115").await?;
    println!("Backed up {} rows", backed_up);
    
    // 4. Optimize and analyze
    db_service.optimize_table("users").await?;
    println!("Table optimized");
    
    db_service.analyze_table("users").await?;
    println!("Table analyzed");
    
    // ========== Transaction Example ==========
    
    println!("\n=== Transaction Example ===\n");
    
    // Complex transaction with multiple operations
    let result = db_service.with_transaction(|tx| {
        Box::pin(async move {
            // Insert user
            sqlx::query("INSERT INTO users (email, username) VALUES (?, ?)")
                .bind("transaction@example.com")
                .bind("txuser")
                .execute(&mut **tx)
                .await?;
            
            // Get the inserted ID
            let row = sqlx::query("SELECT LAST_INSERT_ID() as id")
                .fetch_one(&mut **tx)
                .await?;
            let user_id: u64 = row.get("id");
            
            // Insert related data
            sqlx::query("INSERT INTO user_profiles (user_id, bio) VALUES (?, ?)")
                .bind(user_id)
                .bind("Transaction test user")
                .execute(&mut **tx)
                .await?;
            
            Ok(user_id)
        })
    }).await?;
    
    println!("Transaction completed. New user ID: {}", result);
    
    println!("\n=== All examples completed successfully! ===");
    
    Ok(())
}

// ========== Helper Functions ==========

fn print_separator() {
    println!("{}", "=".repeat(50));
}

fn print_json_value(value: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(value));
}