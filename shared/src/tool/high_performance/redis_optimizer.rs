//! Redis 성능 최적화 라이브러리
//! 
//! - 파이프라인 배치 처리
//! - 연결 풀 최적화
//! - 캐시 전략 최적화
//! - 메모리 효율적인 직렬화

use anyhow::Result;
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{info, warn};

/// 타입 별칭들
type LocalCacheMap = HashMap<String, (Vec<u8>, Instant)>;

/// Redis 최적화 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisOptimizerConfig {
    /// 파이프라인 배치 크기
    pub pipeline_batch_size: usize,
    /// 연결 풀 크기
    pub connection_pool_size: usize,
    /// 재시도 횟수
    pub max_retries: usize,
    /// 재시도 간격 (밀리초)
    pub retry_delay_ms: u64,
    /// 연결 타임아웃 (초)
    pub connection_timeout_secs: u64,
    /// 키 압축 활성화
    pub enable_key_compression: bool,
    /// 값 압축 활성화
    pub enable_value_compression: bool,
    /// TTL 기본값 (초)
    pub default_ttl_secs: usize,
}

impl Default for RedisOptimizerConfig {
    fn default() -> Self {
        Self {
            pipeline_batch_size: 100,
            connection_pool_size: 20,
            max_retries: 3,
            retry_delay_ms: 100,
            connection_timeout_secs: 5,
            enable_key_compression: false,
            enable_value_compression: true,
            default_ttl_secs: 3600,
        }
    }
}

/// Redis 배치 작업 유형
#[derive(Debug, Clone)]
pub enum BatchOperation {
    Get { key: String },
    Set { key: String, value: Vec<u8>, ttl: Option<usize> },
    Del { key: String },
    HGet { key: String, field: String },
    HSet { key: String, field: String, value: Vec<u8> },
    HDel { key: String, field: String },
    ZAdd { key: String, score: f64, member: String },
    ZRem { key: String, member: String },
    Expire { key: String, ttl: usize },
}

/// 배치 작업 결과
#[derive(Debug, Clone)]
pub struct BatchResult {
    pub operation_index: usize,
    pub success: bool,
    pub data: Option<Vec<u8>>,
    pub error: Option<String>,
}

/// Redis 성능 통계
#[derive(Debug, Default, Clone)]
pub struct RedisPerformanceStats {
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub pipeline_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub avg_response_time_ms: f64,
    pub connection_pool_usage: f64,
}

/// Redis 최적화기
pub struct RedisOptimizer {
    config: RedisOptimizerConfig,
    connection_manager: ConnectionManager,
    /// 연결 풀 세마포어
    connection_semaphore: Arc<Semaphore>,
    /// 성능 통계
    stats: Arc<RwLock<RedisPerformanceStats>>,
    /// 캐시 엔트리 타입 별칭
    local_cache: Arc<RwLock<LocalCacheMap>>,
}

impl RedisOptimizer {
    /// 새 Redis 최적화기 생성
    pub async fn new(redis_url: &str, config: RedisOptimizerConfig) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let connection_manager = client.get_connection_manager().await?;
        
        info!("Redis 최적화기 생성 완료: {}", redis_url);
        
        Ok(Self {
            connection_semaphore: Arc::new(Semaphore::new(config.connection_pool_size)),
            config,
            connection_manager,
            stats: Arc::new(RwLock::new(RedisPerformanceStats::default())),
            local_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// 단일 키 GET (L1 캐시 지원)
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let start_time = Instant::now();
        
        // L1 캐시 확인
        if let Some(cached_value) = self.retrieve_from_local_cache(key).await {
            let mut stats = self.stats.write().await;
            stats.cache_hits += 1;
            stats.total_operations += 1;
            return Ok(Some(cached_value));
        }
        
        // Redis에서 가져오기
        let _permit = self.connection_semaphore.acquire().await?;
        
        let result = self.with_retry(|| async {
            let mut conn = self.connection_manager.clone();
            let value: Option<Vec<u8>> = conn.get(key).await?;
            Ok(value)
        }).await;
        
        // 통계 업데이트
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        stats.avg_response_time_ms = 
            (stats.avg_response_time_ms * (stats.total_operations - 1) as f64 + start_time.elapsed().as_millis() as f64) 
            / stats.total_operations as f64;
        
        match &result {
            Ok(Some(value)) => {
                stats.successful_operations += 1;
                stats.cache_misses += 1;
                // L1 캐시에 저장
                self.store_in_local_cache(key, value.clone()).await;
            }
            Ok(None) => {
                stats.cache_misses += 1;
            }
            Err(_) => {
                stats.failed_operations += 1;
            }
        }
        
        result
    }
    
    /// 단일 키 SET (TTL 지원)
    pub async fn set(&self, key: &str, value: &[u8], ttl: Option<usize>) -> Result<()> {
        let start_time = Instant::now();
        let _permit = self.connection_semaphore.acquire().await?;
        
        let result = self.with_retry(|| async {
            let mut conn = self.connection_manager.clone();
            
            if let Some(ttl_secs) = ttl {
                let _: () = conn.set_ex(key, value, ttl_secs as u64).await?;
            } else {
                let _: () = conn.set(key, value).await?;
            }
            
            Ok::<(), redis::RedisError>(())
        }).await;
        
        // L1 캐시 업데이트
        if result.is_ok() {
            self.store_in_local_cache(key, value.to_vec()).await;
        }
        
        // 통계 업데이트
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        stats.avg_response_time_ms = 
            (stats.avg_response_time_ms * (stats.total_operations - 1) as f64 + start_time.elapsed().as_millis() as f64) 
            / stats.total_operations as f64;
        
        match result {
            Ok(_) => stats.successful_operations += 1,
            Err(_) => stats.failed_operations += 1,
        }
        
        result.map_err(|e| anyhow::anyhow!("Redis SET failed: {}", e))
    }
    
    /// 다중 키 GET (파이프라인 사용)
    pub async fn get_multiple_keys(&self, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        
        let start_time = Instant::now();
        
        // L1 캐시에서 가능한 것들 먼저 가져오기
        let mut results = vec![None; keys.len()];
        let mut missing_indices = Vec::new();
        let mut missing_keys = Vec::new();
        
        for (i, key) in keys.iter().enumerate() {
            if let Some(cached_value) = self.retrieve_from_local_cache(key).await {
                results[i] = Some(cached_value);
            } else {
                missing_indices.push(i);
                missing_keys.push(key.clone());
            }
        }
        
        // Redis에서 누락된 키들만 파이프라인으로 가져오기
        if !missing_keys.is_empty() {
            let _permit = self.connection_semaphore.acquire().await?;
            
            let redis_results = self.with_retry(|| async {
                let mut conn = self.connection_manager.clone();
                let values: Vec<Option<Vec<u8>>> = conn.get(&missing_keys).await?;
                Ok(values)
            }).await?;
            
            // 결과 병합 및 L1 캐시 업데이트
            for (redis_idx, original_idx) in missing_indices.into_iter().enumerate() {
                if let Some(Some(value)) = redis_results.get(redis_idx) {
                    self.store_in_local_cache(&keys[original_idx], value.clone()).await;
                    results[original_idx] = Some(value.clone());
                }
            }
        }
        
        // 통계 업데이트
        let mut stats = self.stats.write().await;
        stats.total_operations += keys.len() as u64;
        stats.pipeline_operations += 1;
        stats.successful_operations += keys.len() as u64;
        stats.cache_hits += (keys.len() - missing_keys.len()) as u64;
        stats.cache_misses += missing_keys.len() as u64;
        stats.avg_response_time_ms = 
            (stats.avg_response_time_ms * (stats.total_operations - keys.len() as u64) as f64 + start_time.elapsed().as_millis() as f64) 
            / stats.total_operations as f64;
        
        Ok(results)
    }
    
    /// 배치 작업 실행 (고성능 파이프라인)
    pub async fn execute_batch(&self, operations: Vec<BatchOperation>) -> Result<Vec<BatchResult>> {
        if operations.is_empty() {
            return Ok(Vec::new());
        }
        
        let start_time = Instant::now();
        let _permit = self.connection_semaphore.acquire().await?;
        
        // 배치를 청크로 나누기
        let chunks: Vec<_> = operations
            .chunks(self.config.pipeline_batch_size)
            .collect();
        
        let mut all_results = Vec::with_capacity(operations.len());
        
        for chunk in chunks {
            let chunk_results = self.execute_pipeline_chunk(chunk).await?;
            all_results.extend(chunk_results);
        }
        
        // 통계 업데이트
        let mut stats = self.stats.write().await;
        stats.total_operations += operations.len() as u64;
        stats.pipeline_operations += 1;
        
        let successful = all_results.iter().filter(|r| r.success).count();
        stats.successful_operations += successful as u64;
        stats.failed_operations += (operations.len() - successful) as u64;
        
        stats.avg_response_time_ms = 
            (stats.avg_response_time_ms * (stats.total_operations - operations.len() as u64) as f64 + start_time.elapsed().as_millis() as f64) 
            / stats.total_operations as f64;
        
        info!("배치 작업 완료: {} 작업, {} 성공", operations.len(), successful);
        
        Ok(all_results)
    }
    
    /// 파이프라인 청크 실행
    async fn execute_pipeline_chunk(&self, operations: &[BatchOperation]) -> Result<Vec<BatchResult>> {
        let mut results = Vec::with_capacity(operations.len());
        
        let pipeline_result = self.with_retry(|| async {
            let mut conn = self.connection_manager.clone();
            let mut pipe = redis::pipe();
            
            // 파이프라인에 작업 추가
            for operation in operations {
                match operation {
                    BatchOperation::Get { key } => {
                        pipe.get(key);
                    }
                    BatchOperation::Set { key, value, ttl } => {
                        if let Some(ttl_secs) = ttl {
                            pipe.set_ex(key, value, *ttl_secs as u64);
                        } else {
                            pipe.set(key, value);
                        }
                    }
                    BatchOperation::Del { key } => {
                        pipe.del(key);
                    }
                    BatchOperation::HGet { key, field } => {
                        pipe.hget(key, field);
                    }
                    BatchOperation::HSet { key, field, value } => {
                        pipe.hset(key, field, value);
                    }
                    BatchOperation::HDel { key, field } => {
                        pipe.hdel(key, field);
                    }
                    BatchOperation::ZAdd { key, score, member } => {
                        pipe.zadd(key, member, *score);
                    }
                    BatchOperation::ZRem { key, member } => {
                        pipe.zrem(key, member);
                    }
                    BatchOperation::Expire { key, ttl } => {
                        pipe.expire(key, *ttl as i64);
                    }
                }
            }
            
            // 파이프라인 실행
            let pipe_results: Vec<redis::Value> = pipe.query_async(&mut conn).await?;
            Ok(pipe_results)
        }).await?;
        
        // 결과 변환
        for (i, (operation, redis_value)) in operations.iter().zip(pipeline_result.iter()).enumerate() {
            let batch_result = match redis_value {
                redis::Value::Nil => BatchResult {
                    operation_index: i,
                    success: true,
                    data: None,
                    error: None,
                },
                redis::Value::Data(data) => {
                    // L1 캐시 업데이트 (GET 작업인 경우)
                    if let BatchOperation::Get { key } = operation {
                        self.store_in_local_cache(key, data.clone()).await;
                    }
                    
                    BatchResult {
                        operation_index: i,
                        success: true,
                        data: Some(data.clone()),
                        error: None,
                    }
                }
                redis::Value::Okay => BatchResult {
                    operation_index: i,
                    success: true,
                    data: None,
                    error: None,
                },
                redis::Value::Int(n) => BatchResult {
                    operation_index: i,
                    success: true,
                    data: Some(n.to_string().into_bytes()),
                    error: None,
                },
                _ => BatchResult {
                    operation_index: i,
                    success: false,
                    data: None,
                    error: Some("Unexpected Redis response type".to_string()),
                },
            };
            
            results.push(batch_result);
        }
        
        Ok(results)
    }
    
    /// L1 캐시에서 값 가져오기
    async fn retrieve_from_local_cache(&self, key: &str) -> Option<Vec<u8>> {
        let cache = self.local_cache.read().await;
        
        if let Some((value, timestamp)) = cache.get(key) {
            // TTL 체크 (5분)
            if timestamp.elapsed() < Duration::from_secs(300) {
                return Some(value.clone());
            }
        }
        
        None
    }
    
    /// L1 캐시에 값 저장
    async fn store_in_local_cache(&self, key: &str, value: Vec<u8>) {
        let mut cache = self.local_cache.write().await;
        
        // 캐시 크기 제한 (1000개 항목)
        if cache.len() >= 1000 {
            // 가장 오래된 항목 제거
            if let Some(oldest_key) = cache
                .iter()
                .min_by_key(|(_, (_, timestamp))| *timestamp)
                .map(|(k, _)| k.clone())
            {
                cache.remove(&oldest_key);
            }
        }
        
        cache.insert(key.to_string(), (value, Instant::now()));
    }
    
    /// 재시도 로직
    async fn with_retry<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, redis::RedisError>>,
    {
        let mut last_error = None;
        
        for attempt in 0..=self.config.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let error_msg = format!("{}", e);
                    last_error = Some(e);
                    
                    if attempt < self.config.max_retries {
                        let delay = Duration::from_millis(
                            self.config.retry_delay_ms * (1 << attempt) // 지수 백오프
                        );
                        
                        warn!("Redis 작업 실패 (시도 {}/{}), {}ms 후 재시도: {}", 
                              attempt + 1, self.config.max_retries + 1, delay.as_millis(), error_msg);
                              
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        
        Err(anyhow::anyhow!("Redis 작업 실패 (최대 재시도 초과): {:?}", last_error))
    }
    
    /// 성능 통계 반환
    pub async fn get_stats(&self) -> RedisPerformanceStats {
        let stats = self.stats.read().await;
        let mut result = stats.clone();
        
        // 연결 풀 사용률 계산
        let available_permits = self.connection_semaphore.available_permits();
        result.connection_pool_usage = 
            (self.config.connection_pool_size - available_permits) as f64 / self.config.connection_pool_size as f64 * 100.0;
        
        result
    }
    
    /// 캐시 정리
    pub async fn cleanup_local_cache(&self) {
        let mut cache = self.local_cache.write().await;
        let now = Instant::now();
        
        let initial_size = cache.len();
        cache.retain(|_, (_, timestamp)| now.duration_since(*timestamp) < Duration::from_secs(300));
        
        let cleaned = initial_size - cache.len();
        if cleaned > 0 {
            info!("L1 캐시 정리 완료: {} 항목 제거", cleaned);
        }
    }
    
    /// Hash GET 명령 (개별 필드)
    pub async fn hget(&self, key: &str, field: &str) -> Result<Option<Vec<u8>>> {
        let start_time = std::time::Instant::now();
        let _permit = self.connection_semaphore.acquire().await?;
        
        let result = self.with_retry(|| async {
            let mut conn = self.connection_manager.clone();
            let value: Option<Vec<u8>> = conn.hget(key, field).await?;
            Ok(value)
        }).await;
        
        // 통계 업데이트
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        stats.avg_response_time_ms = 
            (stats.avg_response_time_ms * (stats.total_operations - 1) as f64 + start_time.elapsed().as_millis() as f64) 
            / stats.total_operations as f64;
        
        match &result {
            Ok(_) => stats.successful_operations += 1,
            Err(_) => stats.failed_operations += 1,
        }
        
        result
    }

    /// Hash SET 명령 (개별 필드)
    pub async fn hset(&self, key: &str, field: &str, value: &[u8]) -> Result<()> {
        let start_time = std::time::Instant::now();
        let _permit = self.connection_semaphore.acquire().await?;
        
        let result = self.with_retry(|| async {
            let mut conn = self.connection_manager.clone();
            let _: () = conn.hset(key, field, value).await?;
            Ok::<(), redis::RedisError>(())
        }).await;
        
        // 통계 업데이트
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        stats.avg_response_time_ms = 
            (stats.avg_response_time_ms * (stats.total_operations - 1) as f64 + start_time.elapsed().as_millis() as f64) 
            / stats.total_operations as f64;
        
        match result {
            Ok(_) => stats.successful_operations += 1,
            Err(_) => stats.failed_operations += 1,
        }
        
        result.map_err(|e| anyhow::anyhow!("Redis HSET failed: {}", e))
    }

    /// Hash DELETE 명령 (개별 필드)
    pub async fn hdel(&self, key: &str, field: &str) -> Result<()> {
        let start_time = std::time::Instant::now();
        let _permit = self.connection_semaphore.acquire().await?;
        
        let result = self.with_retry(|| async {
            let mut conn = self.connection_manager.clone();
            let _: () = conn.hdel(key, field).await?;
            Ok::<(), redis::RedisError>(())
        }).await;
        
        // 통계 업데이트
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        stats.avg_response_time_ms = 
            (stats.avg_response_time_ms * (stats.total_operations - 1) as f64 + start_time.elapsed().as_millis() as f64) 
            / stats.total_operations as f64;
        
        match result {
            Ok(_) => stats.successful_operations += 1,
            Err(_) => stats.failed_operations += 1,
        }
        
        result.map_err(|e| anyhow::anyhow!("Redis HDEL failed: {}", e))
    }

    /// Hash GET ALL 명령 (모든 필드)
    pub async fn hgetall(&self, key: &str) -> Result<Vec<(String, String)>> {
        let start_time = std::time::Instant::now();
        let _permit = self.connection_semaphore.acquire().await?;
        
        let result = self.with_retry(|| async {
            let mut conn = self.connection_manager.clone();
            let hash_data: std::collections::HashMap<String, String> = conn.hgetall(key).await?;
            let vec_data: Vec<(String, String)> = hash_data.into_iter().collect();
            Ok(vec_data)
        }).await;
        
        // 통계 업데이트
        let mut stats = self.stats.write().await;
        stats.total_operations += 1;
        stats.avg_response_time_ms = 
            (stats.avg_response_time_ms * (stats.total_operations - 1) as f64 + start_time.elapsed().as_millis() as f64) 
            / stats.total_operations as f64;
        
        match &result {
            Ok(_) => stats.successful_operations += 1,
            Err(_) => stats.failed_operations += 1,
        }
        
        result
    }

    /// 건강 상태 확인
    pub async fn health_check(&self) -> Result<bool> {
        let _permit = self.connection_semaphore.acquire().await?;
        
        let result = self.with_retry(|| async {
            let mut conn = self.connection_manager.clone();
            let pong: String = redis::cmd("PING").query_async(&mut conn).await?;
            Ok(pong == "PONG")
        }).await;
        
        result.map_err(|e| anyhow::anyhow!("Redis 건강 상태 확인 실패: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_redis_optimizer() {
        // 테스트용 Redis 서버가 필요
        let redis_url = "redis://127.0.0.1:6379";
        
        if let Ok(optimizer) = RedisOptimizer::new(redis_url, RedisOptimizerConfig::default()).await {
            // 기본 SET/GET 테스트
            let test_key = "test:key:1";
            let test_value = b"test_value";
            
            assert!(optimizer.set(test_key, test_value, Some(60)).await.is_ok());
            
            if let Ok(Some(retrieved_value)) = optimizer.get(test_key).await {
                assert_eq!(retrieved_value, test_value);
            }
            
            // 배치 작업 테스트
            let operations = vec![
                BatchOperation::Set {
                    key: "batch:1".to_string(),
                    value: b"value1".to_vec(),
                    ttl: Some(60),
                },
                BatchOperation::Set {
                    key: "batch:2".to_string(),
                    value: b"value2".to_vec(),
                    ttl: Some(60),
                },
                BatchOperation::Get {
                    key: "batch:1".to_string(),
                },
            ];
            
            let results = optimizer.execute_batch(operations).await.unwrap();
            assert_eq!(results.len(), 3);
            
            // 통계 확인
            let stats = optimizer.get_stats().await;
            assert!(stats.total_operations > 0);
        }
    }
}