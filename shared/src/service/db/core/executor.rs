//! 쿼리 실행 모듈
//!
//! 쿼리 빌듩, 실행, 결과 처리를 담당

use crate::service::db::core::config::QueryConfig;
use crate::service::db::core::connection::ConnectionManager;
use crate::service::db::core::types::{DbResult, QueryParams, QueryRow, QueryStats};
use crate::tool::error::AppError;
use sqlx::{mysql::MySqlRow, query, Column, Row, TypeInfo};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, warn};

/// 데이터베이스 작업을 위한 쿼리 실행기
pub struct QueryExecutor {
    /// 연결 관리자
    connection: ConnectionManager,
    
    /// 쿼리 설정
    config: QueryConfig,
    
    /// 쿼리 통계 수집기
    stats_enabled: bool,
}

impl QueryExecutor {
    /// 새 쿼리 실행기 생성
    pub fn new(connection: ConnectionManager, config: QueryConfig) -> Self {
        Self {
            connection,
            stats_enabled: config.enable_query_plan,
            config,
        }
    }
    
    /// SELECT 쿼리 실행
    pub async fn select(
        &self,
        sql: &str,
        params: Option<QueryParams>,
    ) -> Result<Vec<QueryRow>, AppError> {
        self.log_query(sql, params.as_ref());
        
        let start = Instant::now();
        let mut conn = self.connection.get_connection().await?;
        
        let query_builder = self.build_query(sql, params.as_ref());
        
        let rows = query_builder
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| {
                warn!("Query execution failed: {}", e);
                AppError::DatabaseQuery(format!("SELECT failed: {}", e))
            })?;
        
        self.connection.release_connection();
        self.connection.record_query();
        
        let elapsed = start.elapsed();
        self.check_slow_query(sql, elapsed.as_millis() as u64);
        
        // Convert rows to QueryRow format
        let mut results = Vec::new();
        for row in &rows {
            results.push(self.row_to_map(row)?);
        }
        
        debug!("Query returned {} rows in {:?}", results.len(), elapsed);
        Ok(results)
    }
    
    /// Execute INSERT/UPDATE/DELETE query
    pub async fn execute(
        &self,
        sql: &str,
        params: Option<QueryParams>,
    ) -> Result<DbResult<()>, AppError> {
        self.log_query(sql, params.as_ref());
        
        let start = Instant::now();
        let mut conn = self.connection.get_connection().await?;
        
        let query_builder = self.build_query(sql, params.as_ref());
        
        let result = query_builder
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                warn!("Query execution failed: {}", e);
                AppError::DatabaseQuery(format!("Execute failed: {}", e))
            })?;
        
        self.connection.release_connection();
        self.connection.record_query();
        
        let elapsed = start.elapsed();
        self.check_slow_query(sql, elapsed.as_millis() as u64);
        
        Ok(DbResult {
            data: Some(()),
            affected_rows: result.rows_affected(),
            last_insert_id: Some(result.last_insert_id() as u64),
        })
    }
    
    /// Execute query and return single row
    pub async fn fetch_one(
        &self,
        sql: &str,
        params: Option<QueryParams>,
    ) -> Result<Option<QueryRow>, AppError> {
        self.log_query(sql, params.as_ref());
        
        let mut conn = self.connection.get_connection().await?;
        let query_builder = self.build_query(sql, params.as_ref());
        
        match query_builder.fetch_optional(&mut *conn).await {
            Ok(Some(row)) => {
                self.connection.release_connection();
                self.connection.record_query();
                Ok(Some(self.row_to_map(&row)?))
            }
            Ok(None) => {
                self.connection.release_connection();
                Ok(None)
            }
            Err(e) => {
                self.connection.release_connection();
                Err(AppError::DatabaseQuery(format!("Fetch one failed: {}", e)))
            }
        }
    }
    
    /// Execute scalar query (returns single value)
    pub async fn fetch_scalar<T>(
        &self,
        sql: &str,
        params: Option<QueryParams>,
    ) -> Result<Option<T>, AppError>
    where
        T: for<'r> sqlx::decode::Decode<'r, sqlx::MySql>
            + sqlx::Type<sqlx::MySql>
            + Send
            + Unpin,
    {
        self.log_query(sql, params.as_ref());
        
        let mut conn = self.connection.get_connection().await?;
        let query_builder = self.build_query(sql, params.as_ref());
        
        match query_builder.fetch_optional(&mut *conn).await {
            Ok(Some(row)) => {
                self.connection.release_connection();
                self.connection.record_query();
                
                // Get first column value
                if row.columns().is_empty() {
                    return Ok(None);
                }
                
                let value: T = row.try_get(0).map_err(|e| {
                    AppError::DatabaseQuery(format!("Failed to get scalar value: {}", e))
                })?;
                
                Ok(Some(value))
            }
            Ok(None) => {
                self.connection.release_connection();
                Ok(None)
            }
            Err(e) => {
                self.connection.release_connection();
                Err(AppError::DatabaseQuery(format!("Fetch scalar failed: {}", e)))
            }
        }
    }
    
    /// Build INSERT query
    pub async fn insert(
        &self,
        table: &str,
        data: QueryParams,
    ) -> Result<DbResult<u64>, AppError> {
        if data.is_empty() {
            return Err(AppError::InvalidInput("INSERT data is empty".to_string()));
        }
        
        let columns: Vec<String> = data.keys().cloned().collect();
        let placeholders: Vec<String> = (0..columns.len())
            .map(|_| "?".to_string())
            .collect();
        
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            table,
            columns.join(", "),
            placeholders.join(", ")
        );
        
        let result = self.execute(&sql, Some(data)).await?;
        
        Ok(DbResult {
            data: result.last_insert_id,
            affected_rows: result.affected_rows,
            last_insert_id: result.last_insert_id,
        })
    }
    
    /// Build UPDATE query
    pub async fn update(
        &self,
        table: &str,
        data: QueryParams,
        where_clause: &str,
        where_params: Option<QueryParams>,
    ) -> Result<u64, AppError> {
        if data.is_empty() {
            return Err(AppError::InvalidInput("UPDATE data is empty".to_string()));
        }
        
        let set_clause: Vec<String> = data.keys()
            .map(|k| format!("{} = ?", k))
            .collect();
        
        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            table,
            set_clause.join(", "),
            where_clause
        );
        
        // Combine data and where parameters
        let mut combined_params = data;
        if let Some(where_params) = where_params {
            combined_params.extend(where_params);
        }
        
        let result = self.execute(&sql, Some(combined_params)).await?;
        Ok(result.affected_rows)
    }
    
    /// Build DELETE query
    pub async fn delete(
        &self,
        table: &str,
        where_clause: &str,
        params: Option<QueryParams>,
    ) -> Result<u64, AppError> {
        let sql = format!("DELETE FROM {} WHERE {}", table, where_clause);
        let result = self.execute(&sql, params).await?;
        Ok(result.affected_rows)
    }
    
    /// Build parameterized query
    fn build_query<'q>(
        &self,
        sql: &'q str,
        params: Option<&QueryParams>,
    ) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
        let mut query_builder = query(sql);
        
        if let Some(params) = params {
            for value in params.values() {
                query_builder = match value {
                    serde_json::Value::String(s) => query_builder.bind(s.clone()),
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            query_builder.bind(i)
                        } else if let Some(f) = n.as_f64() {
                            query_builder.bind(f)
                        } else {
                            query_builder.bind(n.to_string())
                        }
                    }
                    serde_json::Value::Bool(b) => query_builder.bind(*b),
                    serde_json::Value::Null => query_builder.bind(Option::<String>::None),
                    _ => query_builder.bind(value.to_string()),
                };
            }
        }
        
        query_builder
    }
    
    /// Convert database row to HashMap
    fn row_to_map(&self, row: &MySqlRow) -> Result<QueryRow, AppError> {
        let mut result = HashMap::new();
        
        for column in row.columns() {
            let column_name = column.name().to_string();
            
            let value = match column.type_info().name() {
                "INT" | "BIGINT" | "SMALLINT" | "TINYINT" | "MEDIUMINT" => {
                    if let Ok(val) = row.try_get::<Option<i64>, _>(column_name.as_str()) {
                        val.map(|v| serde_json::Value::Number(serde_json::Number::from(v)))
                            .unwrap_or(serde_json::Value::Null)
                    } else {
                        serde_json::Value::Null
                    }
                }
                "VARCHAR" | "TEXT" | "CHAR" | "TINYTEXT" | "MEDIUMTEXT" | "LONGTEXT" => {
                    if let Ok(val) = row.try_get::<Option<String>, _>(column_name.as_str()) {
                        val.map(serde_json::Value::String)
                            .unwrap_or(serde_json::Value::Null)
                    } else {
                        serde_json::Value::Null
                    }
                }
                "FLOAT" | "DOUBLE" | "DECIMAL" | "NUMERIC" => {
                    if let Ok(val) = row.try_get::<Option<f64>, _>(column_name.as_str()) {
                        val.and_then(serde_json::Number::from_f64)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null)
                    } else {
                        serde_json::Value::Null
                    }
                }
                "BOOLEAN" | "BOOL" | "TINYINT(1)" => {
                    if let Ok(val) = row.try_get::<Option<bool>, _>(column_name.as_str()) {
                        val.map(serde_json::Value::Bool)
                            .unwrap_or(serde_json::Value::Null)
                    } else {
                        serde_json::Value::Null
                    }
                }
                "DATE" | "DATETIME" | "TIMESTAMP" => {
                    if let Ok(val) = row.try_get::<Option<chrono::NaiveDateTime>, _>(column_name.as_str()) {
                        val.map(|v| serde_json::Value::String(v.to_string()))
                            .unwrap_or(serde_json::Value::Null)
                    } else {
                        serde_json::Value::Null
                    }
                }
                _ => {
                    // Default: try to get as string
                    if let Ok(val) = row.try_get::<Option<String>, _>(column_name.as_str()) {
                        val.map(serde_json::Value::String)
                            .unwrap_or(serde_json::Value::Null)
                    } else {
                        serde_json::Value::Null
                    }
                }
            };
            
            result.insert(column_name, value);
        }
        
        Ok(result)
    }
    
    /// Log query if enabled
    fn log_query(&self, sql: &str, params: Option<&QueryParams>) {
        if self.config.enable_query_logging {
            if let Some(params) = params {
                debug!("Executing query: {} | Params: {:?}", sql, params);
            } else {
                debug!("Executing query: {}", sql);
            }
        }
    }
    
    /// Check for slow queries
    fn check_slow_query(&self, sql: &str, elapsed_ms: u64) {
        if elapsed_ms > self.config.slow_query_threshold_ms {
            warn!(
                "Slow query detected ({} ms): {}",
                elapsed_ms,
                sql.chars().take(200).collect::<String>()
            );
        }
    }
    
    /// Get query statistics (if enabled)
    pub async fn get_query_stats(&self, sql: &str) -> Result<Option<QueryStats>, AppError> {
        if !self.stats_enabled {
            return Ok(None);
        }
        
        let explain_sql = format!("EXPLAIN {}", sql);
        let rows = self.select(&explain_sql, None).await?;
        
        if rows.is_empty() {
            return Ok(None);
        }
        
        // Parse EXPLAIN output
        let stats = QueryStats {
            execution_time_ms: 0, // Would need actual timing
            rows_examined: rows.first()
                .and_then(|r| r.get("rows"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            rows_affected: 0,
            query_plan: Some(serde_json::to_string(&rows).unwrap_or_default()),
        };
        
        Ok(Some(stats))
    }
}