//! Transaction management module
//!
//! Handles database transactions with proper isolation and error handling

use crate::service::db::core::connection::ConnectionManager;
use crate::tool::error::AppError;
use sqlx::{MySql, Transaction};
use std::future::Future;
use std::pin::Pin;
use tracing::{debug, error, info, warn};

/// Transaction manager for database operations
pub struct TransactionManager {
    /// Connection manager
    connection: ConnectionManager,
}

impl TransactionManager {
    /// Create new transaction manager
    pub fn new(connection: ConnectionManager) -> Self {
        Self { connection }
    }
    
    /// Execute operation within a transaction
    pub async fn with_transaction<T, F>(&self, operation: F) -> Result<T, AppError>
    where
        F: for<'tx> FnOnce(
                &mut Transaction<'tx, MySql>,
            ) -> Pin<Box<dyn Future<Output = Result<T, AppError>> + Send + 'tx>>
            + Send,
        T: Send,
    {
        let pool = self.connection.pool();
        
        // Start transaction
        let mut tx = pool.begin().await.map_err(|e| {
            error!("Failed to start transaction: {}", e);
            AppError::DatabaseConnection(format!("Transaction start failed: {}", e))
        })?;
        
        debug!("Transaction started");
        
        // Execute operation
        let result = operation(&mut tx).await;
        
        match result {
            Ok(value) => {
                // Commit transaction
                tx.commit().await.map_err(|e| {
                    error!("Failed to commit transaction: {}", e);
                    AppError::DatabaseConnection(format!("Transaction commit failed: {}", e))
                })?;
                
                info!("Transaction committed successfully");
                Ok(value)
            }
            Err(err) => {
                // Rollback transaction
                tx.rollback().await.map_err(|e| {
                    error!("Failed to rollback transaction: {}", e);
                    AppError::DatabaseConnection(format!("Transaction rollback failed: {}", e))
                })?;
                
                warn!("Transaction rolled back due to error: {}", err);
                Err(err)
            }
        }
    }
    
    /// Execute operation with savepoint support
    pub async fn with_savepoint<T, F>(
        &self,
        tx: &mut Transaction<'_, MySql>,
        savepoint_name: &str,
        operation: F,
    ) -> Result<T, AppError>
    where
        F: for<'sp> FnOnce(
                &mut Transaction<'sp, MySql>,
            ) -> Pin<Box<dyn Future<Output = Result<T, AppError>> + Send + 'sp>>
            + Send,
        T: Send,
    {
        // Create savepoint
        let savepoint_sql = format!("SAVEPOINT {}", savepoint_name);
        sqlx::query(&savepoint_sql)
            .execute(&mut **tx)
            .await
            .map_err(|e| {
                error!("Failed to create savepoint '{}': {}", savepoint_name, e);
                AppError::DatabaseQuery(format!("Savepoint creation failed: {}", e))
            })?;
        
        debug!("Savepoint '{}' created", savepoint_name);
        
        // Execute operation
        let result = operation(tx).await;
        
        match result {
            Ok(value) => {
                debug!("Savepoint '{}' operation succeeded", savepoint_name);
                Ok(value)
            }
            Err(err) => {
                // Rollback to savepoint
                let rollback_sql = format!("ROLLBACK TO SAVEPOINT {}", savepoint_name);
                sqlx::query(&rollback_sql)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| {
                        error!("Failed to rollback to savepoint '{}': {}", savepoint_name, e);
                        AppError::DatabaseQuery(format!("Savepoint rollback failed: {}", e))
                    })?;
                
                warn!("Rolled back to savepoint '{}' due to error: {}", savepoint_name, err);
                Err(err)
            }
        }
    }
    
    /// Execute multiple operations in a single transaction
    pub async fn batch_transaction<T>(
        &self,
        operations: Vec<
            Box<
                dyn for<'tx> FnOnce(
                        &mut Transaction<'tx, MySql>,
                    ) -> Pin<Box<dyn Future<Output = Result<T, AppError>> + Send + 'tx>>
                    + Send,
            >,
        >,
    ) -> Result<Vec<T>, AppError>
    where
        T: Send + 'static,
    {
        let pool = self.connection.pool();
        
        // Start transaction
        let mut tx = pool.begin().await.map_err(|e| {
            error!("Failed to start batch transaction: {}", e);
            AppError::DatabaseConnection(format!("Batch transaction start failed: {}", e))
        })?;
        
        let mut results = Vec::new();
        
        for (index, operation) in operations.into_iter().enumerate() {
            debug!("Executing batch operation {}", index + 1);
            
            match operation(&mut tx).await {
                Ok(result) => results.push(result),
                Err(err) => {
                    // Rollback on error
                    tx.rollback().await.map_err(|e| {
                        error!("Failed to rollback batch transaction: {}", e);
                        AppError::DatabaseConnection(format!("Batch transaction rollback failed: {}", e))
                    })?;
                    
                    warn!("Batch transaction rolled back due to error in operation {}: {}", index + 1, err);
                    return Err(err);
                }
            }
        }
        
        // Commit transaction
        tx.commit().await.map_err(|e| {
            error!("Failed to commit batch transaction: {}", e);
            AppError::DatabaseConnection(format!("Batch transaction commit failed: {}", e))
        })?;
        
        info!("Batch transaction committed successfully with {} operations", results.len());
        Ok(results)
    }
    
    /// Execute operation with retry on deadlock
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
        let mut attempts = 0;
        
        loop {
            attempts += 1;
            
            let op = operation.clone();
            match self.with_transaction(op).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let error_str = e.to_string();
                    
                    // Check if it's a deadlock or lock timeout
                    if (error_str.contains("Deadlock") || error_str.contains("Lock wait timeout"))
                        && attempts < max_retries
                    {
                        warn!(
                            "Transaction failed due to lock conflict (attempt {}/{}), retrying...",
                            attempts, max_retries
                        );
                        
                        // Exponential backoff
                        let delay = std::time::Duration::from_millis(100 * 2_u64.pow(attempts - 1));
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    
                    return Err(e);
                }
            }
        }
    }
    
    /// Set transaction isolation level
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
        let pool = self.connection.pool();
        let mut tx = pool.begin().await.map_err(|e| {
            AppError::DatabaseConnection(format!("Transaction start failed: {}", e))
        })?;
        
        // Set isolation level
        let isolation_sql = format!("SET TRANSACTION ISOLATION LEVEL {}", isolation_level.as_str());
        sqlx::query(&isolation_sql)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::DatabaseQuery(format!("Failed to set isolation level: {}", e))
            })?;
        
        debug!("Transaction isolation level set to {:?}", isolation_level);
        
        // Execute operation
        let result = operation(&mut tx).await;
        
        match result {
            Ok(value) => {
                tx.commit().await.map_err(|e| {
                    AppError::DatabaseConnection(format!("Transaction commit failed: {}", e))
                })?;
                Ok(value)
            }
            Err(err) => {
                tx.rollback().await.map_err(|e| {
                    AppError::DatabaseConnection(format!("Transaction rollback failed: {}", e))
                })?;
                Err(err)
            }
        }
    }
}

/// Transaction isolation levels
#[derive(Debug, Clone, Copy)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl IsolationLevel {
    fn as_str(&self) -> &'static str {
        match self {
            IsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
            IsolationLevel::ReadCommitted => "READ COMMITTED",
            IsolationLevel::RepeatableRead => "REPEATABLE READ",
            IsolationLevel::Serializable => "SERIALIZABLE",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_isolation_level_string() {
        assert_eq!(IsolationLevel::ReadCommitted.as_str(), "READ COMMITTED");
        assert_eq!(IsolationLevel::Serializable.as_str(), "SERIALIZABLE");
    }
}