//! Common type definitions for database service
//!
//! Shared types used across database service modules

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Query result row type - generic key-value map
pub type QueryRow = HashMap<String, serde_json::Value>;

/// Query parameters type - used for parameterized queries
pub type QueryParams = HashMap<String, serde_json::Value>;

/// Database operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbResult<T> {
    pub data: Option<T>,
    pub affected_rows: u64,
    pub last_insert_id: Option<u64>,
}

/// Database error types
#[derive(Debug, Clone)]
pub enum DbError {
    Connection(String),
    Query(String),
    Transaction(String),
    Timeout(String),
    InvalidInput(String),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Connection(msg) => write!(f, "Connection error: {}", msg),
            DbError::Query(msg) => write!(f, "Query error: {}", msg),
            DbError::Transaction(msg) => write!(f, "Transaction error: {}", msg),
            DbError::Timeout(msg) => write!(f, "Timeout error: {}", msg),
            DbError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
        }
    }
}

impl std::error::Error for DbError {}

/// Database metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub charset: String,
    pub collation: String,
    pub size_bytes: u64,
}

/// Table metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub engine: String,
    pub rows: u64,
    pub data_length: u64,
    pub index_length: u64,
    pub comment: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Column metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub default_value: Option<String>,
    pub is_primary_key: bool,
    pub is_unique: bool,
    pub is_indexed: bool,
    pub comment: String,
}

/// Index metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub table_name: String,
    pub column_names: Vec<String>,
    pub is_unique: bool,
    pub is_primary: bool,
    pub index_type: String,
    pub cardinality: u64,
}

/// Query statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryStats {
    pub execution_time_ms: u64,
    pub rows_examined: u64,
    pub rows_affected: u64,
    pub query_plan: Option<String>,
}

/// Connection statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub active_connections: u32,
    pub idle_connections: u32,
    pub total_connections: u32,
    pub max_connections: u32,
    pub connection_errors: u64,
    pub total_queries: u64,
}

/// Batch operation options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOptions {
    pub batch_size: usize,
    pub continue_on_error: bool,
    pub transaction_per_batch: bool,
    pub parallel_execution: bool,
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            continue_on_error: false,
            transaction_per_batch: true,
            parallel_execution: false,
        }
    }
}