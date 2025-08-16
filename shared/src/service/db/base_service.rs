//! 기본 데이터베이스 서비스 - Clean Architecture 구현
//!
//! 다양한 책임을 전문 모듈에 위임하는 메인 오케스트레이터 서비스

use crate::service::db::core::{
    config::DbServiceConfig,
    connection::ConnectionManager,
    executor::QueryExecutor,
    metadata::MetadataProvider,
    transaction::{IsolationLevel, TransactionManager},
    types::*,
};
use crate::tool::error::AppError;
use async_trait::async_trait;
use sqlx::{MySql, Transaction};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, info};

/// 기본 데이터베이스 서비스 trait - 인터페이스 정의
#[async_trait]
pub trait BaseDbService: Send + Sync {
    // === 메타데이터 작업 ===
    async fn get_databases(&self) -> Result<Vec<DatabaseInfo>, AppError>;
    async fn get_tables(&self, database: Option<&str>) -> Result<Vec<TableInfo>, AppError>;
    async fn get_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, AppError>;
    async fn get_indexes(&self, table: &str) -> Result<Vec<IndexInfo>, AppError>;
    async fn table_exists(&self, table: &str) -> Result<bool, AppError>;
    
    // === 쿼리 작업 ===
    async fn select(
        &self,
        table: &str,
        where_clause: Option<&str>,
        params: Option<QueryParams>,
    ) -> Result<Vec<QueryRow>, AppError>;
    
    async fn insert(&self, table: &str, data: QueryParams) -> Result<u64, AppError>;
    
    async fn update(
        &self,
        table: &str,
        data: QueryParams,
        where_clause: &str,
        where_params: Option<QueryParams>,
    ) -> Result<u64, AppError>;
    
    async fn delete(
        &self,
        table: &str,
        where_clause: &str,
        params: Option<QueryParams>,
    ) -> Result<u64, AppError>;
    
    // === 원시 쿼리 실행 ===
    async fn execute_query(
        &self,
        sql: &str,
        params: Option<QueryParams>,
    ) -> Result<Vec<QueryRow>, AppError>;
    
    async fn execute_non_query(
        &self,
        sql: &str,
        params: Option<QueryParams>,
    ) -> Result<u64, AppError>;
    
    // === 트랜잭션 작업 ===
    async fn with_transaction<T, F>(&self, operation: F) -> Result<T, AppError>
    where
        F: for<'tx> FnOnce(
                &mut Transaction<'tx, MySql>,
            ) -> Pin<Box<dyn Future<Output = Result<T, AppError>> + Send + 'tx>>
            + Send,
        T: Send;
    
    // === 상태 확인 및 통계 ===
    async fn health_check(&self) -> Result<bool, AppError>;
    async fn get_connection_stats(&self) -> Result<ConnectionStats, AppError>;
}

/// 기본 데이터베이스 서비스 구현체
pub struct BaseDbServiceImpl {
    /// 서비스 설정
    config: Arc<DbServiceConfig>,
    
    /// 연결 관리자
    connection: Arc<ConnectionManager>,
    
    /// 쿼리 실행기
    executor: Arc<QueryExecutor>,
    
    /// 메타데이터 제공자
    metadata: Arc<MetadataProvider>,
    
    /// 트랜잭션 관리자
    transaction: Arc<TransactionManager>,
}

impl BaseDbServiceImpl {
    /// 새 서비스 인스턴스 생성
    pub async fn new(config: DbServiceConfig) -> Result<Self, AppError> {
        info!("기본 데이터베이스 서비스 초기화 중");
        
        // 연결 관리자 생성
        let connection = Arc::new(ConnectionManager::new(&config).await?);
        
        // 설정된 경우 연결 풀 워밍업
        if config.pool_config.min_connections > 0 {
            connection.warmup().await?;
        }
        
        // 연결 관리자로 실행기 생성
        let executor = Arc::new(QueryExecutor::new(
            (*connection).clone(),
            config.query_config.clone(),
        ));
        
        // 자체 실행기로 메타데이터 제공자 생성
        let metadata = Arc::new(MetadataProvider::new(
            QueryExecutor::new(
                (*connection).clone(),
                config.query_config.clone(),
            ),
            config.db_config.database.clone(),
        ));
        
        // 트랜잭션 관리자 생성
        let transaction = Arc::new(TransactionManager::new((*connection).clone()));
        
        Ok(Self {
            config: Arc::new(config),
            connection,
            executor,
            metadata,
            transaction,
        })
    }
    
    /// 기본 설정으로 생성
    pub async fn from_env() -> Result<Self, AppError> {
        let config = DbServiceConfig::from_env().await
            .map_err(|e| AppError::Configuration(e.to_string()))?;
        Self::new(config).await
    }
    
    /// 최적화된 설정으로 생성
    pub async fn optimized() -> Result<Self, AppError> {
        let config = DbServiceConfig::from_env().await
            .map_err(|e| AppError::Configuration(e.to_string()))?
            .optimized();
        Self::new(config).await
    }
    
    /// 서비스 설정 가져오기
    pub fn config(&self) -> &DbServiceConfig {
        &self.config
    }
    
    /// 배치 삽입 작업
    pub async fn batch_insert(
        &self,
        table: &str,
        data_list: Vec<QueryParams>,
    ) -> Result<u64, AppError> {
        if data_list.is_empty() {
            return Ok(0);
        }
        
        let batch_size = self.config.performance_config.default_batch_size;
        let mut total_affected = 0u64;
        
        // 배치로 처리
        for chunk in data_list.chunks(batch_size) {
            let mut chunk_affected = 0u64;
            
            // 각 배치에 대해 트랜잭션 사용
            let pool = self.connection.pool();
            let mut tx = pool.begin().await
                .map_err(|e| AppError::DatabaseConnection(format!("Transaction start failed: {}", e)))?;
            
            for data in chunk {
                if !data.is_empty() {
                    let columns: Vec<String> = data.keys().cloned().collect();
                    let placeholders: Vec<String> =
                        (0..columns.len()).map(|_| "?".to_string()).collect();
                    
                    let sql = format!(
                        "INSERT INTO {} ({}) VALUES ({})",
                        table,
                        columns.join(", "),
                        placeholders.join(", ")
                    );
                    
                    // 파라미터로 쿼리 빌드
                    let mut query_builder = sqlx::query(&sql);
                    for value in data.values() {
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
                            serde_json::Value::Null => {
                                query_builder.bind(Option::<String>::None)
                            }
                            _ => query_builder.bind(value.to_string()),
                        };
                    }
                    
                    let result = query_builder.execute(&mut *tx).await
                        .map_err(|e| AppError::DatabaseQuery(format!("Batch insert failed: {}", e)))?;
                    
                    chunk_affected += result.rows_affected();
                }
            }
            
            // 이 배치에 대한 트랜잭션 커밋
            tx.commit().await
                .map_err(|e| AppError::DatabaseConnection(format!("Transaction commit failed: {}", e)))?;
            
            total_affected += chunk_affected;
            debug!("Batch processed: {} rows affected", chunk_affected);
        }
        
        info!("Batch insert completed: {} total rows inserted", total_affected);
        Ok(total_affected)
    }
    
    /// 선택적 where 절로 행 개수 가져오기
    pub async fn get_row_count(
        &self,
        table: &str,
        where_clause: Option<&str>,
    ) -> Result<u64, AppError> {
        let mut sql = format!("SELECT COUNT(*) as count FROM {}", table);
        if let Some(where_clause) = where_clause {
            sql.push_str(&format!(" WHERE {}", where_clause));
        }
        
        let count: Option<i64> = self.executor.fetch_scalar(&sql, None).await?;
        Ok(count.unwrap_or(0) as u64)
    }
    
    /// 사용자 정의 격리 수준으로 실행
    pub async fn with_isolation_level<T, F>(
        &self,
        isolation_level: IsolationLevel,
        operation: F,
    ) -> Result<T, AppError>
    where
        F: for<'tx> FnOnce(
                &mut Transaction<'tx, MySql>,
            ) -> Pin<Box<dyn Future<Output = Result<T, AppError>> + Send + 'tx>>
            + Send,
        T: Send,
    {
        self.transaction
            .with_isolation_level(isolation_level, operation)
            .await
    }
    
    /// 데드락 시 재시도하며 실행
    pub async fn with_retry<T, F>(
        &self,
        operation: F,
        max_retries: u32,
    ) -> Result<T, AppError>
    where
        F: for<'tx> Fn(
                &mut Transaction<'tx, MySql>,
            ) -> Pin<Box<dyn Future<Output = Result<T, AppError>> + Send + 'tx>>
            + Send
            + Clone,
        T: Send,
    {
        self.transaction.with_retry(operation, max_retries).await
    }
}

#[async_trait]
impl BaseDbService for BaseDbServiceImpl {
    // === 메타데이터 작업 ===
    
    async fn get_databases(&self) -> Result<Vec<DatabaseInfo>, AppError> {
        self.metadata.get_databases().await
    }
    
    async fn get_tables(&self, database: Option<&str>) -> Result<Vec<TableInfo>, AppError> {
        self.metadata.get_tables(database).await
    }
    
    async fn get_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, AppError> {
        self.metadata.get_columns(table).await
    }
    
    async fn get_indexes(&self, table: &str) -> Result<Vec<IndexInfo>, AppError> {
        self.metadata.get_indexes(table).await
    }
    
    async fn table_exists(&self, table: &str) -> Result<bool, AppError> {
        self.metadata.table_exists(table).await
    }
    
    // === 쿼리 작업 ===
    
    async fn select(
        &self,
        table: &str,
        where_clause: Option<&str>,
        params: Option<QueryParams>,
    ) -> Result<Vec<QueryRow>, AppError> {
        let mut sql = format!("SELECT * FROM {}", table);
        if let Some(where_clause) = where_clause {
            sql.push_str(&format!(" WHERE {}", where_clause));
        }
        
        self.executor.select(&sql, params).await
    }
    
    async fn insert(&self, table: &str, data: QueryParams) -> Result<u64, AppError> {
        let result = self.executor.insert(table, data).await?;
        Ok(result.affected_rows)
    }
    
    async fn update(
        &self,
        table: &str,
        data: QueryParams,
        where_clause: &str,
        where_params: Option<QueryParams>,
    ) -> Result<u64, AppError> {
        self.executor.update(table, data, where_clause, where_params).await
    }
    
    async fn delete(
        &self,
        table: &str,
        where_clause: &str,
        params: Option<QueryParams>,
    ) -> Result<u64, AppError> {
        self.executor.delete(table, where_clause, params).await
    }
    
    // === 원시 쿼리 실행 ===
    
    async fn execute_query(
        &self,
        sql: &str,
        params: Option<QueryParams>,
    ) -> Result<Vec<QueryRow>, AppError> {
        self.executor.select(sql, params).await
    }
    
    async fn execute_non_query(
        &self,
        sql: &str,
        params: Option<QueryParams>,
    ) -> Result<u64, AppError> {
        let result = self.executor.execute(sql, params).await?;
        Ok(result.affected_rows)
    }
    
    // === 트랜잭션 작업 ===
    
    async fn with_transaction<T, F>(&self, operation: F) -> Result<T, AppError>
    where
        F: for<'tx> FnOnce(
                &mut Transaction<'tx, MySql>,
            ) -> Pin<Box<dyn Future<Output = Result<T, AppError>> + Send + 'tx>>
            + Send,
        T: Send,
    {
        self.transaction.with_transaction(operation).await
    }
    
    // === 상태 확인 및 통계 ===
    
    async fn health_check(&self) -> Result<bool, AppError> {
        self.connection.health_check().await
    }
    
    async fn get_connection_stats(&self) -> Result<ConnectionStats, AppError> {
        Ok(self.connection.get_stats().await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_service_creation() {
        // 실제 DB 연결 없이 서비스 생성 테스트
        // 실제 통합 테스트는 별도의 테스트 파일에 있어야 함
    }
}