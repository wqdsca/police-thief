//! Database metadata operations module
//!
//! Provides functions to query database, table, column, and index metadata

use crate::service::db::core::executor::QueryExecutor;
use crate::service::db::core::types::{ColumnInfo, DatabaseInfo, IndexInfo, TableInfo};
use crate::tool::error::AppError;
use std::collections::HashMap;
use tracing::{debug, info};

/// Metadata provider for database schema information
pub struct MetadataProvider {
    /// Query executor
    executor: QueryExecutor,
    
    /// Current database name
    database_name: String,
}

impl MetadataProvider {
    /// Create new metadata provider
    pub fn new(executor: QueryExecutor, database_name: String) -> Self {
        Self {
            executor,
            database_name,
        }
    }
    
    /// Get list of all databases
    pub async fn get_databases(&self) -> Result<Vec<DatabaseInfo>, AppError> {
        let sql = r#"
            SELECT 
                SCHEMA_NAME as name,
                DEFAULT_CHARACTER_SET_NAME as charset,
                DEFAULT_COLLATION_NAME as collation,
                0 as size_bytes
            FROM information_schema.SCHEMATA
            WHERE SCHEMA_NAME NOT IN ('information_schema', 'performance_schema', 'mysql', 'sys')
            ORDER BY SCHEMA_NAME
        "#;
        
        let rows = self.executor.select(sql, None).await?;
        
        let databases: Vec<DatabaseInfo> = rows.into_iter()
            .map(|row| DatabaseInfo {
                name: row.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                charset: row.get("charset")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                collation: row.get("collation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                size_bytes: row.get("size_bytes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
            })
            .collect();
        
        info!("Retrieved {} databases", databases.len());
        Ok(databases)
    }
    
    /// Get list of tables in a database
    pub async fn get_tables(&self, database: Option<&str>) -> Result<Vec<TableInfo>, AppError> {
        let db_name = database.unwrap_or(&self.database_name);
        
        let sql = r#"
            SELECT 
                TABLE_NAME as name,
                ENGINE as engine,
                TABLE_ROWS as rows,
                DATA_LENGTH as data_length,
                INDEX_LENGTH as index_length,
                TABLE_COMMENT as comment,
                CREATE_TIME as created_at
            FROM information_schema.TABLES
            WHERE TABLE_SCHEMA = ? AND TABLE_TYPE = 'BASE TABLE'
            ORDER BY TABLE_NAME
        "#;
        
        let mut params = HashMap::new();
        params.insert("database".to_string(), serde_json::json!(db_name));
        
        let rows = self.executor.select(sql, Some(params)).await?;
        
        let tables: Vec<TableInfo> = rows.into_iter()
            .map(|row| TableInfo {
                name: row.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                engine: row.get("engine")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                rows: row.get("rows")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                data_length: row.get("data_length")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                index_length: row.get("index_length")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                comment: row.get("comment")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                created_at: row.get("created_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok())
                    .map(|dt| chrono::DateTime::from_naive_utc_and_offset(dt, chrono::Utc)),
            })
            .collect();
        
        info!("Retrieved {} tables from database '{}'", tables.len(), db_name);
        Ok(tables)
    }
    
    /// Get column information for a table
    pub async fn get_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, AppError> {
        let sql = r#"
            SELECT 
                c.COLUMN_NAME as name,
                c.DATA_TYPE as data_type,
                c.IS_NULLABLE as is_nullable,
                c.COLUMN_DEFAULT as default_value,
                c.COLUMN_KEY as column_key,
                c.COLUMN_COMMENT as comment,
                CASE 
                    WHEN c.COLUMN_KEY = 'PRI' THEN 1 
                    ELSE 0 
                END as is_primary_key,
                CASE 
                    WHEN c.COLUMN_KEY IN ('PRI', 'UNI') THEN 1 
                    ELSE 0 
                END as is_unique,
                CASE 
                    WHEN c.COLUMN_KEY IN ('PRI', 'UNI', 'MUL') THEN 1 
                    ELSE 0 
                END as is_indexed
            FROM information_schema.COLUMNS c
            WHERE c.TABLE_SCHEMA = ? AND c.TABLE_NAME = ?
            ORDER BY c.ORDINAL_POSITION
        "#;
        
        let mut params = HashMap::new();
        params.insert("database".to_string(), serde_json::json!(&self.database_name));
        params.insert("table".to_string(), serde_json::json!(table));
        
        let rows = self.executor.select(sql, Some(params)).await?;
        
        let columns: Vec<ColumnInfo> = rows.into_iter()
            .map(|row| {
                let is_nullable = row.get("is_nullable")
                    .and_then(|v| v.as_str())
                    .map(|s| s == "YES")
                    .unwrap_or(false);
                
                ColumnInfo {
                    name: row.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    data_type: row.get("data_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    is_nullable,
                    default_value: row.get("default_value")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    is_primary_key: row.get("is_primary_key")
                        .and_then(|v| v.as_i64())
                        .map(|v| v == 1)
                        .unwrap_or(false),
                    is_unique: row.get("is_unique")
                        .and_then(|v| v.as_i64())
                        .map(|v| v == 1)
                        .unwrap_or(false),
                    is_indexed: row.get("is_indexed")
                        .and_then(|v| v.as_i64())
                        .map(|v| v == 1)
                        .unwrap_or(false),
                    comment: row.get("comment")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                }
            })
            .collect();
        
        debug!("Retrieved {} columns from table '{}'", columns.len(), table);
        Ok(columns)
    }
    
    /// Get index information for a table
    pub async fn get_indexes(&self, table: &str) -> Result<Vec<IndexInfo>, AppError> {
        let sql = r#"
            SELECT 
                INDEX_NAME as name,
                TABLE_NAME as table_name,
                GROUP_CONCAT(COLUMN_NAME ORDER BY SEQ_IN_INDEX) as column_names,
                NON_UNIQUE as non_unique,
                INDEX_TYPE as index_type,
                CARDINALITY as cardinality,
                CASE 
                    WHEN INDEX_NAME = 'PRIMARY' THEN 1 
                    ELSE 0 
                END as is_primary
            FROM information_schema.STATISTICS
            WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?
            GROUP BY INDEX_NAME, TABLE_NAME, NON_UNIQUE, INDEX_TYPE, CARDINALITY
            ORDER BY INDEX_NAME
        "#;
        
        let mut params = HashMap::new();
        params.insert("database".to_string(), serde_json::json!(&self.database_name));
        params.insert("table".to_string(), serde_json::json!(table));
        
        let rows = self.executor.select(sql, Some(params)).await?;
        
        let indexes: Vec<IndexInfo> = rows.into_iter()
            .map(|row| {
                let non_unique = row.get("non_unique")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1);
                
                let column_names_str = row.get("column_names")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                
                let column_names: Vec<String> = column_names_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                
                IndexInfo {
                    name: row.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    table_name: row.get("table_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    column_names,
                    is_unique: non_unique == 0,
                    is_primary: row.get("is_primary")
                        .and_then(|v| v.as_i64())
                        .map(|v| v == 1)
                        .unwrap_or(false),
                    index_type: row.get("index_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    cardinality: row.get("cardinality")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0),
                }
            })
            .collect();
        
        debug!("Retrieved {} indexes from table '{}'", indexes.len(), table);
        Ok(indexes)
    }
    
    /// Check if a table exists
    pub async fn table_exists(&self, table: &str) -> Result<bool, AppError> {
        let sql = r#"
            SELECT COUNT(*) as count
            FROM information_schema.TABLES
            WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND TABLE_TYPE = 'BASE TABLE'
        "#;
        
        let mut params = HashMap::new();
        params.insert("database".to_string(), serde_json::json!(&self.database_name));
        params.insert("table".to_string(), serde_json::json!(table));
        
        let count: Option<i64> = self.executor
            .fetch_scalar(sql, Some(params))
            .await?;
        
        Ok(count.unwrap_or(0) > 0)
    }
    
    /// Get table statistics
    pub async fn get_table_stats(&self, table: &str) -> Result<HashMap<String, serde_json::Value>, AppError> {
        let sql = r#"
            SELECT 
                TABLE_ROWS as row_count,
                AVG_ROW_LENGTH as avg_row_length,
                DATA_LENGTH as data_size,
                INDEX_LENGTH as index_size,
                DATA_FREE as free_space,
                AUTO_INCREMENT as auto_increment,
                CREATE_TIME as created_at,
                UPDATE_TIME as updated_at
            FROM information_schema.TABLES
            WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?
        "#;
        
        let mut params = HashMap::new();
        params.insert("database".to_string(), serde_json::json!(&self.database_name));
        params.insert("table".to_string(), serde_json::json!(table));
        
        let result = self.executor.fetch_one(sql, Some(params)).await?;
        
        match result {
            Some(stats) => Ok(stats),
            None => Err(AppError::NotFound(format!("Table '{}' not found", table))),
        }
    }
    
    /// Get database size
    pub async fn get_database_size(&self, database: Option<&str>) -> Result<u64, AppError> {
        let db_name = database.unwrap_or(&self.database_name);
        
        let sql = r#"
            SELECT 
                SUM(DATA_LENGTH + INDEX_LENGTH) as total_size
            FROM information_schema.TABLES
            WHERE TABLE_SCHEMA = ?
        "#;
        
        let mut params = HashMap::new();
        params.insert("database".to_string(), serde_json::json!(db_name));
        
        let size: Option<i64> = self.executor
            .fetch_scalar(sql, Some(params))
            .await?;
        
        Ok(size.unwrap_or(0) as u64)
    }
}