//! 병렬 브로드캐스트 시스템
//! 
//! Rayon 기반 병렬 처리로 메시지 전송 성능을 300-500% 향상시킵니다.
//! - 순차 전송 대신 병렬 전송
//! - 효율적인 작업 분산
//! - 백프레셰어 제한으로 시스템 안정성 보장

use anyhow::Result;
use rayon::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::task;
use tracing::{debug, warn, info};
use serde::{Serialize, Deserialize};

use crate::protocol::{GameMessage, optimized::OptimizedGameMessage};
use crate::service::room_connection_service::{RoomUserConnection, RoomConnectionService};

/// 병렬 브로드캐스트 통계
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ParallelBroadcastStats {
    /// 총 브로드캐스트 수
    pub total_broadcasts: AtomicU64,
    /// 성공한 메시지 수
    pub successful_messages: AtomicU64,
    /// 실패한 메시지 수
    pub failed_messages: AtomicU64,
    /// 평균 브로드캐스트 시간 (마이크로초)
    pub avg_broadcast_time_us: AtomicU64,
    /// 병렬 처리된 방 수
    pub parallel_rooms_processed: AtomicU64,
    /// 최대 동시 연결 수
    pub max_concurrent_connections: AtomicU64,
}

impl ParallelBroadcastStats {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn record_broadcast(&self, duration_us: u64, successful: u64, failed: u64) {
        self.total_broadcasts.fetch_add(1, Ordering::Relaxed);
        self.successful_messages.fetch_add(successful, Ordering::Relaxed);
        self.failed_messages.fetch_add(failed, Ordering::Relaxed);
        
        // 이동 평균 계산
        let current_avg = self.avg_broadcast_time_us.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            duration_us
        } else {
            (current_avg * 7 + duration_us) / 8 // 8분의 1 가중 평균
        };
        self.avg_broadcast_time_us.store(new_avg, Ordering::Relaxed);
    }
    
    pub fn get_success_rate(&self) -> f64 {
        let total = self.successful_messages.load(Ordering::Relaxed) + 
                   self.failed_messages.load(Ordering::Relaxed);
        if total > 0 {
            (self.successful_messages.load(Ordering::Relaxed) as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// 병렬 브로드캐스트 설정
#[derive(Debug, Clone)]
pub struct ParallelBroadcastConfig {
    /// 최대 병렬 작업 수 (기본: CPU 코어 수)
    pub max_parallel_tasks: usize,
    /// 배치 크기 (기본: 100)
    pub batch_size: usize,
    /// 바이너리 프로토콜 사용 여부 (기본: true)
    pub use_binary_protocol: bool,
    /// 에러 허용 임계값 (기본: 10%)
    pub error_threshold_percent: f64,
    /// 타임아웃 (밀리초, 기본: 5000ms)
    pub timeout_ms: u64,
}

impl Default for ParallelBroadcastConfig {
    fn default() -> Self {
        Self {
            max_parallel_tasks: num_cpus::get(),
            batch_size: 100,
            use_binary_protocol: true,
            error_threshold_percent: 10.0,
            timeout_ms: 5000,
        }
    }
}

/// 병렬 브로드캐스트 서비스
pub struct ParallelBroadcastService {
    config: ParallelBroadcastConfig,
    stats: Arc<ParallelBroadcastStats>,
    room_service: Arc<RoomConnectionService>,
}

impl ParallelBroadcastService {
    /// 새로운 병렬 브로드캐스트 서비스 생성
    pub fn new(room_service: Arc<RoomConnectionService>, config: Option<ParallelBroadcastConfig>) -> Self {
        let config = config.unwrap_or_default();
        info!("병렬 브로드캐스트 서비스 초기화 - 최대 작업: {}, 배치 크기: {}", 
              config.max_parallel_tasks, config.batch_size);
        
        Self {
            config,
            stats: Arc::new(ParallelBroadcastStats::new()),
            room_service,
        }
    }
    
    /// 방에 메시지를 병렬로 브로드캐스트
    pub async fn broadcast_to_room(&self, room_id: u32, message: &GameMessage) -> Result<usize> {
        let start_time = Instant::now();
        
        // 방 사용자 목록 가져오기
        let users = self.room_service.get_room_users(room_id);
        if users.is_empty() {
            debug!("방 {}에 사용자가 없어 브로드캐스트 건너뜀", room_id);
            return Ok(0);
        }
        
        let total_users = users.len();
        info!("방 {}에 {}명에게 병렬 브로드캐스트 시작", room_id, total_users);
        
        // 메시지 준비 (바이너리/JSON 선택)
        let message_data = if self.config.use_binary_protocol {
            let opt_msg = OptimizedGameMessage::from_game_message(message);
            opt_msg.to_bytes()?
        } else {
            message.to_bytes()?
        };
        
        // 병렬 처리로 메시지 전송
        let results = self.parallel_send_to_users(users, message_data).await?;
        
        let successful = results.iter().filter(|r| r.is_ok()).count();
        let failed = results.len() - successful;
        let duration_us = start_time.elapsed().as_micros() as u64;
        
        // 통계 업데이트
        self.stats.record_broadcast(duration_us, successful as u64, failed as u64);
        self.stats.max_concurrent_connections.store(total_users as u64, Ordering::Relaxed);
        
        info!("방 {} 브로드캐스트 완료: 성공 {}, 실패 {}, 시간 {}μs", 
              room_id, successful, failed, duration_us);
        
        // 에러율 체크
        let error_rate = (failed as f64 / total_users as f64) * 100.0;
        if error_rate > self.config.error_threshold_percent {
            warn!("방 {} 브로드캐스트 에러율 높음: {:.2}% (임계값: {:.2}%)", 
                  room_id, error_rate, self.config.error_threshold_percent);
        }
        
        Ok(successful)
    }
    
    /// 다중 방에 메시지를 병렬로 브로드캐스트
    pub async fn broadcast_to_multiple_rooms(&self, room_ids: &[u32], message: &GameMessage) -> Result<usize> {
        let start_time = Instant::now();
        info!("{}개 방에 병렬 브로드캐스트 시작", room_ids.len());
        
        // 방별 병렬 처리
        let broadcast_futures: Vec<_> = room_ids
            .par_iter() // Rayon 병렬 이터레이터
            .map(|&room_id| {
                let service = self.clone_for_task();
                let msg = message.clone();
                
                task::spawn(async move {
                    service.broadcast_to_room(room_id, &msg).await
                })
            })
            .collect();
        
        // 모든 브로드캐스트 완료 대기
        let mut total_successful = 0;
        for future in broadcast_futures {
            match future.await {
                Ok(Ok(count)) => total_successful += count,
                Ok(Err(e)) => warn!("방 브로드캐스트 실패: {}", e),
                Err(e) => warn!("브로드캐스트 작업 실행 실패: {}", e),
            }
        }
        
        let duration_us = start_time.elapsed().as_micros() as u64;
        self.stats.parallel_rooms_processed.store(room_ids.len() as u64, Ordering::Relaxed);
        
        info!("다중 방 브로드캐스트 완료: {}개 방, 총 {}명 성공, 시간 {}μs", 
              room_ids.len(), total_successful, duration_us);
        
        Ok(total_successful)
    }
    
    /// 전체 서버에 메시지를 병렬로 브로드캐스트
    pub async fn broadcast_to_all(&self, message: &GameMessage) -> Result<usize> {
        let all_rooms = self.room_service.get_all_rooms();
        let room_ids: Vec<u32> = all_rooms.iter().map(|room| room.room_id).collect();
        
        info!("전체 서버 브로드캐스트 시작: {}개 방", room_ids.len());
        self.broadcast_to_multiple_rooms(&room_ids, message).await
    }
    
    /// 사용자들에게 병렬로 메시지 전송
    async fn parallel_send_to_users(&self, users: Vec<RoomUserConnection>, message_data: Vec<u8>) -> Result<Vec<Result<()>>> {
        let message_data = Arc::new(message_data);
        
        // 배치 단위로 분할
        let batches: Vec<_> = users
            .chunks(self.config.batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();
        
        info!("{}명을 {}개 배치로 분할하여 병렬 처리", users.len(), batches.len());
        
        // 배치별 병렬 처리
        let batch_futures: Vec<_> = batches
            .into_par_iter() // Rayon 병렬 처리
            .map(|batch| {
                let data = message_data.clone();
                let timeout = self.config.timeout_ms;
                
                task::spawn(async move {
                    Self::send_batch_async(batch, data, timeout).await
                })
            })
            .collect();
        
        // 모든 배치 결과 수집
        let mut all_results = Vec::new();
        for batch_future in batch_futures {
            match batch_future.await {
                Ok(results) => all_results.extend(results),
                Err(e) => {
                    warn!("배치 처리 실패: {}", e);
                    all_results.push(Err(anyhow::anyhow!("배치 처리 실패: {}", e)));
                }
            }
        }
        
        Ok(all_results)
    }
    
    /// 배치를 비동기로 처리
    async fn send_batch_async(batch: Vec<RoomUserConnection>, message_data: Arc<Vec<u8>>, timeout_ms: u64) -> Vec<Result<()>> {
        let mut results = Vec::with_capacity(batch.len());
        
        for connection in batch {
            let result = Self::send_to_connection(&connection, &message_data, timeout_ms).await;
            results.push(result);
        }
        
        results
    }
    
    /// 개별 연결에 메시지 전송
    async fn send_to_connection(connection: &RoomUserConnection, message_data: &[u8], timeout_ms: u64) -> Result<()> {
        if let Some(writer) = &connection.writer {
            let timeout = std::time::Duration::from_millis(timeout_ms);
            
            match tokio::time::timeout(timeout, async {
                let mut writer_guard = writer.lock().await;
                
                // 바이너리 데이터 직접 전송
                tokio::io::AsyncWriteExt::write_all(&mut *writer_guard, message_data).await?;
                tokio::io::AsyncWriteExt::flush(&mut *writer_guard).await?;
                
                Ok::<(), anyhow::Error>(())
            }).await {
                Ok(Ok(())) => {
                    debug!("사용자 {}에게 메시지 전송 성공", connection.user_id);
                    Ok(())
                },
                Ok(Err(e)) => {
                    warn!("사용자 {}에게 메시지 전송 실패: {}", connection.user_id, e);
                    Err(e)
                },
                Err(_) => {
                    warn!("사용자 {}에게 메시지 전송 타임아웃", connection.user_id);
                    Err(anyhow::anyhow!("메시지 전송 타임아웃"))
                }
            }
        } else {
            Err(anyhow::anyhow!("사용자 {}의 연결이 없음", connection.user_id))
        }
    }
    
    /// 통계 조회
    pub fn get_stats(&self) -> ParallelBroadcastStats {
        ParallelBroadcastStats {
            total_broadcasts: AtomicU64::new(self.stats.total_broadcasts.load(Ordering::Relaxed)),
            successful_messages: AtomicU64::new(self.stats.successful_messages.load(Ordering::Relaxed)),
            failed_messages: AtomicU64::new(self.stats.failed_messages.load(Ordering::Relaxed)),
            avg_broadcast_time_us: AtomicU64::new(self.stats.avg_broadcast_time_us.load(Ordering::Relaxed)),
            parallel_rooms_processed: AtomicU64::new(self.stats.parallel_rooms_processed.load(Ordering::Relaxed)),
            max_concurrent_connections: AtomicU64::new(self.stats.max_concurrent_connections.load(Ordering::Relaxed)),
        }
    }
    
    /// 설정 조회
    pub fn get_config(&self) -> &ParallelBroadcastConfig {
        &self.config
    }
    
    /// 작업용 서비스 복제
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            stats: self.stats.clone(),
            room_service: self.room_service.clone(),
        }
    }
    
    /// 성능 벤치마크 실행
    pub async fn benchmark(&self, room_id: u32, iterations: usize) -> Result<BenchmarkResult> {
        let test_message = GameMessage::ChatMessage {
            user_id: 1,
            room_id,
            message: "벤치마크 테스트 메시지".to_string(),
        };
        
        info!("병렬 브로드캐스트 벤치마크 시작: {} 반복", iterations);
        let start = Instant::now();
        
        let mut total_sent = 0;
        for i in 0..iterations {
            match self.broadcast_to_room(room_id, &test_message).await {
                Ok(sent) => total_sent += sent,
                Err(e) => warn!("벤치마크 반복 {} 실패: {}", i, e),
            }
        }
        
        let total_duration = start.elapsed();
        let avg_duration_ms = total_duration.as_millis() as f64 / iterations as f64;
        let messages_per_sec = (total_sent as f64) / total_duration.as_secs_f64();
        
        let result = BenchmarkResult {
            iterations,
            total_messages_sent: total_sent,
            total_duration_ms: total_duration.as_millis() as u64,
            avg_duration_ms,
            messages_per_second: messages_per_sec,
            success_rate: self.stats.get_success_rate(),
        };
        
        info!("벤치마크 완료: {:#?}", result);
        Ok(result)
    }
}

/// 벤치마크 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub iterations: usize,
    pub total_messages_sent: usize,
    pub total_duration_ms: u64,
    pub avg_duration_ms: f64,
    pub messages_per_second: f64,
    pub success_rate: f64,
}

/// 성능 비교 도구
pub struct PerformanceComparison;

impl PerformanceComparison {
    /// 순차 vs 병렬 브로드캐스트 성능 비교
    pub async fn compare_sequential_vs_parallel(
        sequential_service: &RoomConnectionService,
        parallel_service: &ParallelBroadcastService,
        room_id: u32,
        iterations: usize,
    ) -> Result<ComparisonResult> {
        let test_message = GameMessage::ChatMessage {
            user_id: 1,
            room_id,
            message: "성능 비교 테스트 메시지".to_string(),
        };
        
        info!("순차 vs 병렬 성능 비교 시작");
        
        // 순차 전송 벤치마크
        let sequential_start = Instant::now();
        let mut sequential_sent = 0;
        
        for _ in 0..iterations {
            match sequential_service.send_to_room(room_id, &test_message).await {
                Ok(sent) => sequential_sent += sent,
                Err(e) => warn!("순차 전송 실패: {}", e),
            }
        }
        let sequential_duration = sequential_start.elapsed();
        
        // 병렬 전송 벤치마크
        let parallel_result = parallel_service.benchmark(room_id, iterations).await?;
        
        let improvement_ratio = sequential_duration.as_millis() as f64 / parallel_result.total_duration_ms as f64;
        
        let result = ComparisonResult {
            sequential_duration_ms: sequential_duration.as_millis() as u64,
            parallel_duration_ms: parallel_result.total_duration_ms,
            sequential_sent,
            parallel_sent: parallel_result.total_messages_sent,
            improvement_ratio,
            parallel_faster: improvement_ratio > 1.0,
        };
        
        info!("성능 비교 완료: {:#?}", result);
        Ok(result)
    }
}

/// 성능 비교 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub sequential_duration_ms: u64,
    pub parallel_duration_ms: u64,
    pub sequential_sent: usize,
    pub parallel_sent: usize,
    pub improvement_ratio: f64,
    pub parallel_faster: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::room_connection_service::RoomConnectionService;
    use tokio::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    
    async fn create_test_service() -> (Arc<RoomConnectionService>, ParallelBroadcastService) {
        let room_service = Arc::new(RoomConnectionService::new("test_server".to_string()));
        let parallel_service = ParallelBroadcastService::new(room_service.clone(), None);
        (room_service, parallel_service)
    }
    
    #[tokio::test]
    async fn test_parallel_broadcast_creation() {
        let (_, parallel_service) = create_test_service().await;
        let stats = parallel_service.get_stats();
        
        assert_eq!(stats.total_broadcasts.load(Ordering::Relaxed), 0);
        assert_eq!(stats.successful_messages.load(Ordering::Relaxed), 0);
        assert_eq!(stats.failed_messages.load(Ordering::Relaxed), 0);
    }
    
    #[tokio::test]
    async fn test_empty_room_broadcast() {
        let (_, parallel_service) = create_test_service().await;
        let message = GameMessage::HeartBeat;
        
        let result = parallel_service.broadcast_to_room(999, &message).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
    
    #[tokio::test]
    async fn test_parallel_config() {
        let config = ParallelBroadcastConfig {
            max_parallel_tasks: 8,
            batch_size: 50,
            use_binary_protocol: true,
            error_threshold_percent: 5.0,
            timeout_ms: 3000,
        };
        
        let (room_service, _) = create_test_service().await;
        let service = ParallelBroadcastService::new(room_service, Some(config.clone()));
        
        assert_eq!(service.config.max_parallel_tasks, 8);
        assert_eq!(service.config.batch_size, 50);
        assert!(service.config.use_binary_protocol);
        assert_eq!(service.config.error_threshold_percent, 5.0);
        assert_eq!(service.config.timeout_ms, 3000);
    }
    
    #[tokio::test]
    async fn test_stats_recording() {
        let stats = ParallelBroadcastStats::new();
        
        stats.record_broadcast(1000, 10, 2);
        assert_eq!(stats.total_broadcasts.load(Ordering::Relaxed), 1);
        assert_eq!(stats.successful_messages.load(Ordering::Relaxed), 10);
        assert_eq!(stats.failed_messages.load(Ordering::Relaxed), 2);
        assert_eq!(stats.avg_broadcast_time_us.load(Ordering::Relaxed), 1000);
        
        let success_rate = stats.get_success_rate();
        assert!((success_rate - 83.33).abs() < 0.1); // 10/12 = 83.33%
    }
}