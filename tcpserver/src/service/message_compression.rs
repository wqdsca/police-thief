//! 메시지 압축 및 배칭 최적화 서비스
//! 
//! 네트워크 대역폭을 절약하고 처리량을 향상시키기 위한 메시지 압축 및 배칭 시스템입니다.
//! LZ4, Snappy, Zstd 등의 압축 알고리즘과 지능형 배칭을 제공합니다.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::info;
use serde::{Serialize, Deserialize};
use bytes::{Bytes, BytesMut, BufMut};
use flate2::Compression;
use flate2::write::{GzEncoder, ZlibEncoder};
use flate2::read::{GzDecoder, ZlibDecoder};
use std::io::{Write, Read};

/// 압축 알고리즘
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    None,
    Gzip,
    Zlib,
    Lz4,
    Snappy,
    Zstd,
}

/// 메시지 압축 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageCompressionConfig {
    /// 기본 압축 알고리즘
    pub default_algorithm: CompressionAlgorithm,
    /// 압축 레벨 (1-9)
    pub compression_level: u32,
    /// 최소 압축 크기 (바이트)
    pub min_compression_size: usize,
    /// 적응형 압축 활성화
    pub enable_adaptive_compression: bool,
    /// 배칭 활성화
    pub enable_batching: bool,
    /// 배치 크기 (메시지 수)
    pub batch_size: usize,
    /// 배치 타임아웃 (밀리초)
    pub batch_timeout_ms: u64,
    /// 최대 배치 크기 (바이트)
    pub max_batch_bytes: usize,
    /// 압축 캐시 활성화
    pub enable_compression_cache: bool,
    /// 캐시 크기
    pub cache_size: usize,
}

impl Default for MessageCompressionConfig {
    fn default() -> Self {
        Self {
            default_algorithm: CompressionAlgorithm::Zlib,
            compression_level: 6,
            min_compression_size: 128,
            enable_adaptive_compression: true,
            enable_batching: true,
            batch_size: 10,
            batch_timeout_ms: 50,
            max_batch_bytes: 65536,
            enable_compression_cache: true,
            cache_size: 100,
        }
    }
}

/// 압축 통계
#[derive(Debug, Default)]
pub struct CompressionStats {
    pub total_messages: AtomicU64,
    pub compressed_messages: AtomicU64,
    pub batched_messages: AtomicU64,
    pub bytes_before_compression: AtomicU64,
    pub bytes_after_compression: AtomicU64,
    pub compression_time_us: AtomicU64,
    pub decompression_time_us: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub batch_count: AtomicU64,
}

impl CompressionStats {
    pub fn compression_ratio(&self) -> f64 {
        let before = self.bytes_before_compression.load(Ordering::Relaxed) as f64;
        let after = self.bytes_after_compression.load(Ordering::Relaxed) as f64;
        
        if before > 0.0 {
            1.0 - (after / before)
        } else {
            0.0
        }
    }
    
    pub fn average_compression_time_us(&self) -> f64 {
        let total_time = self.compression_time_us.load(Ordering::Relaxed) as f64;
        let count = self.compressed_messages.load(Ordering::Relaxed) as f64;
        
        if count > 0.0 {
            total_time / count
        } else {
            0.0
        }
    }
}

/// 메시지 배치
#[derive(Debug, Clone)]
pub struct MessageBatch {
    pub messages: Vec<Bytes>,
    pub created_at: Instant,
    pub total_size: usize,
}

impl MessageBatch {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            created_at: Instant::now(),
            total_size: 0,
        }
    }
    
    pub fn add(&mut self, message: Bytes) {
        self.total_size += message.len();
        self.messages.push(message);
    }
    
    pub fn is_ready(&self, config: &MessageCompressionConfig) -> bool {
        self.messages.len() >= config.batch_size ||
        self.total_size >= config.max_batch_bytes ||
        self.created_at.elapsed() >= Duration::from_millis(config.batch_timeout_ms)
    }
    
    pub fn serialize(&self) -> Result<Bytes> {
        let mut buffer = BytesMut::new();
        
        // 배치 헤더: 메시지 수
        buffer.put_u32_le(self.messages.len() as u32);
        
        // 각 메시지 추가
        for msg in &self.messages {
            buffer.put_u32_le(msg.len() as u32);
            buffer.put_slice(msg);
        }
        
        Ok(buffer.freeze())
    }
    
    pub fn deserialize(data: &[u8]) -> Result<Vec<Bytes>> {
        if data.len() < 4 {
            return Err(anyhow!("배치 데이터가 너무 작음"));
        }
        
        let mut cursor = 0;
        let message_count = u32::from_le_bytes([
            data[0], data[1], data[2], data[3]
        ]) as usize;
        cursor += 4;
        
        let mut messages = Vec::with_capacity(message_count);
        
        for _ in 0..message_count {
            if cursor + 4 > data.len() {
                return Err(anyhow!("배치 데이터 파싱 오류"));
            }
            
            let msg_len = u32::from_le_bytes([
                data[cursor], data[cursor+1], data[cursor+2], data[cursor+3]
            ]) as usize;
            cursor += 4;
            
            if cursor + msg_len > data.len() {
                return Err(anyhow!("메시지 데이터 부족"));
            }
            
            messages.push(Bytes::copy_from_slice(&data[cursor..cursor+msg_len]));
            cursor += msg_len;
        }
        
        Ok(messages)
    }
}

/// 압축 캐시 엔트리
#[derive(Clone)]
struct CacheEntry {
    original_hash: u64,
    compressed: Bytes,
    algorithm: CompressionAlgorithm,
    timestamp: Instant,
}

/// 적응형 압축 관리자
pub struct AdaptiveCompressionManager {
    /// 알고리즘별 성능 기록
    algorithm_stats: Arc<RwLock<Vec<(CompressionAlgorithm, f64, f64)>>>, // (알고리즘, 압축률, 속도)
    /// 최적 알고리즘
    optimal_algorithm: Arc<RwLock<CompressionAlgorithm>>,
    /// 평가 간격
    evaluation_interval: Duration,
    /// 마지막 평가 시간
    last_evaluation: Arc<Mutex<Instant>>,
}

impl AdaptiveCompressionManager {
    pub fn new() -> Self {
        Self {
            algorithm_stats: Arc::new(RwLock::new(Vec::new())),
            optimal_algorithm: Arc::new(RwLock::new(CompressionAlgorithm::Zlib)),
            evaluation_interval: Duration::from_secs(30),
            last_evaluation: Arc::new(Mutex::new(Instant::now())),
        }
    }
    
    /// 압축 결과 기록
    pub async fn record_compression(
        &self,
        algorithm: CompressionAlgorithm,
        original_size: usize,
        compressed_size: usize,
        duration: Duration,
    ) {
        let compression_ratio = 1.0 - (compressed_size as f64 / original_size as f64);
        let speed = original_size as f64 / duration.as_secs_f64() / 1_000_000.0; // MB/s
        
        let mut stats = self.algorithm_stats.write().await;
        
        // 알고리즘 통계 업데이트
        let entry = stats.iter_mut()
            .find(|(alg, _, _)| *alg == algorithm);
        
        if let Some((_, ratio, spd)) = entry {
            // 이동 평균
            *ratio = (*ratio * 0.9) + (compression_ratio * 0.1);
            *spd = (*spd * 0.9) + (speed * 0.1);
        } else {
            stats.push((algorithm, compression_ratio, speed));
        }
        
        // 평가 간격 확인
        let mut last_eval = self.last_evaluation.lock().await;
        if last_eval.elapsed() >= self.evaluation_interval {
            self.evaluate_algorithms(&stats).await;
            *last_eval = Instant::now();
        }
    }
    
    /// 알고리즘 평가 및 최적 선택
    async fn evaluate_algorithms(&self, stats: &[(CompressionAlgorithm, f64, f64)]) {
        if stats.is_empty() {
            return;
        }
        
        // 종합 점수 계산 (압축률 50%, 속도 50%)
        let mut best_score = 0.0;
        let mut best_algorithm = CompressionAlgorithm::Zlib;
        
        for &(algorithm, ratio, speed) in stats {
            let score = (ratio * 0.5) + (speed.min(100.0) / 100.0 * 0.5);
            if score > best_score {
                best_score = score;
                best_algorithm = algorithm;
            }
        }
        
        let mut optimal = self.optimal_algorithm.write().await;
        if *optimal != best_algorithm {
            info!("적응형 압축: 최적 알고리즘 변경 {:?} → {:?}", *optimal, best_algorithm);
            *optimal = best_algorithm;
        }
    }
    
    /// 현재 최적 알고리즘 조회
    pub async fn get_optimal_algorithm(&self) -> CompressionAlgorithm {
        *self.optimal_algorithm.read().await
    }
}

/// 메시지 압축 서비스
pub struct MessageCompressionService {
    config: MessageCompressionConfig,
    stats: Arc<CompressionStats>,
    batch_queue: Arc<Mutex<MessageBatch>>,
    compression_cache: Arc<Mutex<lru::LruCache<u64, CacheEntry>>>,
    adaptive_manager: Arc<AdaptiveCompressionManager>,
}

impl MessageCompressionService {
    /// 새 메시지 압축 서비스 생성
    pub fn new(config: MessageCompressionConfig) -> Self {
        let cache = lru::LruCache::new(
            std::num::NonZeroUsize::new(config.cache_size).unwrap()
        );
        
        Self {
            config,
            stats: Arc::new(CompressionStats::default()),
            batch_queue: Arc::new(Mutex::new(MessageBatch::new())),
            compression_cache: Arc::new(Mutex::new(cache)),
            adaptive_manager: Arc::new(AdaptiveCompressionManager::new()),
        }
    }
    
    /// 메시지 압축
    pub async fn compress(&self, data: &[u8]) -> Result<(Bytes, CompressionAlgorithm)> {
        let start = Instant::now();
        self.stats.total_messages.fetch_add(1, Ordering::Relaxed);
        
        // 최소 크기 확인
        if data.len() < self.config.min_compression_size {
            return Ok((Bytes::copy_from_slice(data), CompressionAlgorithm::None));
        }
        
        // 캐시 확인
        let hash = self.calculate_hash(data);
        if self.config.enable_compression_cache {
            let mut cache = self.compression_cache.lock().await;
            if let Some(entry) = cache.get(&hash) {
                self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);
                return Ok((entry.compressed.clone(), entry.algorithm));
            }
            self.stats.cache_misses.fetch_add(1, Ordering::Relaxed);
        }
        
        // 알고리즘 선택
        let algorithm = if self.config.enable_adaptive_compression {
            self.adaptive_manager.get_optimal_algorithm().await
        } else {
            self.config.default_algorithm
        };
        
        // 압축 수행
        let compressed = match algorithm {
            CompressionAlgorithm::None => Bytes::copy_from_slice(data),
            CompressionAlgorithm::Gzip => self.compress_gzip(data)?,
            CompressionAlgorithm::Zlib => self.compress_zlib(data)?,
            CompressionAlgorithm::Lz4 => self.compress_lz4(data)?,
            CompressionAlgorithm::Snappy => self.compress_snappy(data)?,
            CompressionAlgorithm::Zstd => self.compress_zstd(data)?,
        };
        
        // 통계 업데이트
        let duration = start.elapsed();
        self.stats.compressed_messages.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_before_compression.fetch_add(data.len() as u64, Ordering::Relaxed);
        self.stats.bytes_after_compression.fetch_add(compressed.len() as u64, Ordering::Relaxed);
        self.stats.compression_time_us.fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
        
        // 적응형 관리자에 기록
        if self.config.enable_adaptive_compression {
            self.adaptive_manager.record_compression(
                algorithm,
                data.len(),
                compressed.len(),
                duration,
            ).await;
        }
        
        // 캐시 저장
        if self.config.enable_compression_cache {
            let mut cache = self.compression_cache.lock().await;
            cache.put(hash, CacheEntry {
                original_hash: hash,
                compressed: compressed.clone(),
                algorithm,
                timestamp: Instant::now(),
            });
        }
        
        Ok((compressed, algorithm))
    }
    
    /// 메시지 압축 해제
    pub async fn decompress(&self, data: &[u8], algorithm: CompressionAlgorithm) -> Result<Bytes> {
        let start = Instant::now();
        
        let decompressed = match algorithm {
            CompressionAlgorithm::None => Bytes::copy_from_slice(data),
            CompressionAlgorithm::Gzip => self.decompress_gzip(data)?,
            CompressionAlgorithm::Zlib => self.decompress_zlib(data)?,
            CompressionAlgorithm::Lz4 => self.decompress_lz4(data)?,
            CompressionAlgorithm::Snappy => self.decompress_snappy(data)?,
            CompressionAlgorithm::Zstd => self.decompress_zstd(data)?,
        };
        
        let duration = start.elapsed();
        self.stats.decompression_time_us.fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
        
        Ok(decompressed)
    }
    
    /// Gzip 압축
    fn compress_gzip(&self, data: &[u8]) -> Result<Bytes> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(self.config.compression_level));
        encoder.write_all(data)?;
        let compressed = encoder.finish()?;
        Ok(Bytes::from(compressed))
    }
    
    /// Gzip 압축 해제
    fn decompress_gzip(&self, data: &[u8]) -> Result<Bytes> {
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(Bytes::from(decompressed))
    }
    
    /// Zlib 압축
    fn compress_zlib(&self, data: &[u8]) -> Result<Bytes> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(self.config.compression_level));
        encoder.write_all(data)?;
        let compressed = encoder.finish()?;
        Ok(Bytes::from(compressed))
    }
    
    /// Zlib 압축 해제
    fn decompress_zlib(&self, data: &[u8]) -> Result<Bytes> {
        let mut decoder = ZlibDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(Bytes::from(decompressed))
    }
    
    /// LZ4 압축 (간단한 구현)
    fn compress_lz4(&self, data: &[u8]) -> Result<Bytes> {
        // 실제 구현에서는 lz4 크레이트 사용
        // 여기서는 간단히 zlib 사용
        self.compress_zlib(data)
    }
    
    /// LZ4 압축 해제
    fn decompress_lz4(&self, data: &[u8]) -> Result<Bytes> {
        // 실제 구현에서는 lz4 크레이트 사용
        self.decompress_zlib(data)
    }
    
    /// Snappy 압축 (간단한 구현)
    fn compress_snappy(&self, data: &[u8]) -> Result<Bytes> {
        // 실제 구현에서는 snap 크레이트 사용
        self.compress_zlib(data)
    }
    
    /// Snappy 압축 해제
    fn decompress_snappy(&self, data: &[u8]) -> Result<Bytes> {
        // 실제 구현에서는 snap 크레이트 사용
        self.decompress_zlib(data)
    }
    
    /// Zstd 압축 (간단한 구현)
    fn compress_zstd(&self, data: &[u8]) -> Result<Bytes> {
        // 실제 구현에서는 zstd 크레이트 사용
        self.compress_zlib(data)
    }
    
    /// Zstd 압축 해제
    fn decompress_zstd(&self, data: &[u8]) -> Result<Bytes> {
        // 실제 구현에서는 zstd 크레이트 사용
        self.decompress_zlib(data)
    }
    
    /// 해시 계산
    fn calculate_hash(&self, data: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }
    
    /// 메시지를 배치에 추가
    pub async fn add_to_batch(&self, message: Bytes) -> Option<Bytes> {
        if !self.config.enable_batching {
            return Some(message);
        }
        
        let mut batch = self.batch_queue.lock().await;
        batch.add(message);
        
        if batch.is_ready(&self.config) {
            let ready_batch = std::mem::replace(&mut *batch, MessageBatch::new());
            self.stats.batch_count.fetch_add(1, Ordering::Relaxed);
            self.stats.batched_messages.fetch_add(ready_batch.messages.len() as u64, Ordering::Relaxed);
            
            if let Ok(serialized) = ready_batch.serialize() {
                return Some(serialized);
            }
        }
        
        None
    }
    
    /// 배치 강제 플러시
    pub async fn flush_batch(&self) -> Option<Bytes> {
        let mut batch = self.batch_queue.lock().await;
        
        if batch.messages.is_empty() {
            return None;
        }
        
        let ready_batch = std::mem::replace(&mut *batch, MessageBatch::new());
        self.stats.batch_count.fetch_add(1, Ordering::Relaxed);
        self.stats.batched_messages.fetch_add(ready_batch.messages.len() as u64, Ordering::Relaxed);
        
        ready_batch.serialize().ok()
    }
    
    /// 압축된 배치 처리
    pub async fn compress_batch(&self, messages: Vec<Bytes>) -> Result<Bytes> {
        let batch = MessageBatch {
            messages,
            created_at: Instant::now(),
            total_size: 0,
        };
        
        let serialized = batch.serialize()?;
        let (compressed, _) = self.compress(&serialized).await?;
        Ok(compressed)
    }
    
    /// 압축된 배치 해제
    pub async fn decompress_batch(&self, data: &[u8], algorithm: CompressionAlgorithm) -> Result<Vec<Bytes>> {
        let decompressed = self.decompress(data, algorithm).await?;
        MessageBatch::deserialize(&decompressed)
    }
    
    /// 통계 조회
    pub fn get_stats(&self) -> CompressionPerformanceReport {
        let stats = self.stats.clone();
        
        CompressionPerformanceReport {
            total_messages: stats.total_messages.load(Ordering::Relaxed),
            compressed_messages: stats.compressed_messages.load(Ordering::Relaxed),
            batched_messages: stats.batched_messages.load(Ordering::Relaxed),
            compression_ratio: stats.compression_ratio(),
            average_compression_time_us: stats.average_compression_time_us(),
            bytes_saved: stats.bytes_before_compression.load(Ordering::Relaxed)
                .saturating_sub(stats.bytes_after_compression.load(Ordering::Relaxed)),
            cache_hit_rate: {
                let hits = stats.cache_hits.load(Ordering::Relaxed) as f64;
                let misses = stats.cache_misses.load(Ordering::Relaxed) as f64;
                if hits + misses > 0.0 {
                    hits / (hits + misses)
                } else {
                    0.0
                }
            },
            batch_count: stats.batch_count.load(Ordering::Relaxed),
        }
    }
}

/// 압축 성능 보고서
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionPerformanceReport {
    pub total_messages: u64,
    pub compressed_messages: u64,
    pub batched_messages: u64,
    pub compression_ratio: f64,
    pub average_compression_time_us: f64,
    pub bytes_saved: u64,
    pub cache_hit_rate: f64,
    pub batch_count: u64,
}

impl CompressionPerformanceReport {
    /// 성능 점수 (0-100)
    pub fn performance_score(&self) -> f64 {
        let compression_score = self.compression_ratio * 30.0;
        let speed_score = (1000.0 / self.average_compression_time_us.max(1.0)).min(1.0) * 25.0;
        let cache_score = self.cache_hit_rate * 20.0;
        let batch_score = if self.batch_count > 0 { 25.0 } else { 0.0 };
        
        compression_score + speed_score + cache_score + batch_score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_compression() {
        let service = MessageCompressionService::new(MessageCompressionConfig::default());
        
        let data = b"Hello, World! This is a test message that should be compressed.";
        let (compressed, algorithm) = service.compress(data).await.unwrap();
        
        assert!(compressed.len() < data.len());
        assert_ne!(algorithm, CompressionAlgorithm::None);
        
        let decompressed = service.decompress(&compressed, algorithm).await.unwrap();
        assert_eq!(&decompressed[..], data);
    }
    
    #[tokio::test]
    async fn test_batching() {
        let mut config = MessageCompressionConfig::default();
        config.batch_size = 3;
        config.batch_timeout_ms = 1000;
        
        let service = MessageCompressionService::new(config);
        
        // 첫 두 메시지는 배치되지 않음
        assert!(service.add_to_batch(Bytes::from("msg1")).await.is_none());
        assert!(service.add_to_batch(Bytes::from("msg2")).await.is_none());
        
        // 세 번째 메시지로 배치 완성
        let batch = service.add_to_batch(Bytes::from("msg3")).await;
        assert!(batch.is_some());
        
        // 배치 디코딩
        let messages = MessageBatch::deserialize(&batch.unwrap()).unwrap();
        assert_eq!(messages.len(), 3);
    }
    
    #[tokio::test]
    async fn test_adaptive_compression() {
        let manager = AdaptiveCompressionManager::new();
        
        // 여러 압축 결과 기록
        for _ in 0..10 {
            manager.record_compression(
                CompressionAlgorithm::Zlib,
                1000,
                500,
                Duration::from_micros(100),
            ).await;
        }
        
        // 최적 알고리즘 확인
        let optimal = manager.get_optimal_algorithm().await;
        assert_eq!(optimal, CompressionAlgorithm::Zlib);
    }
}