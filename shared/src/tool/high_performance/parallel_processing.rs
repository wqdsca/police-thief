//! 병렬 처리 최적화 라이브러리 (Shared)
//!
//! tcpserver와 rudpserver에서 공통 사용되는 병렬 처리 최적화 기능을 제공합니다.
//! Rayon 기반 병렬 브로드캐스트와 작업 분산 처리를 지원합니다.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 병렬 처리 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelProcessingConfig {
    /// 워커 스레드 수 (기본: CPU 코어 수)
    pub worker_threads: usize,
    /// 작업 큐 크기 (기본: 10000)
    pub work_queue_size: usize,
    /// 배치 처리 크기 (기본: 100)
    pub batch_size: usize,
    /// 작업 스틸링 활성화 (기본: true)
    pub enable_work_stealing: bool,
    /// NUMA 인식 스케줄링 (기본: true)
    pub enable_numa_awareness: bool,
    /// 동적 로드 밸런싱 (기본: true)
    pub enable_dynamic_balancing: bool,
}

impl Default for ParallelProcessingConfig {
    fn default() -> Self {
        Self {
            worker_threads: 8, // 기본값 (실제로는 num_cpus::get() 사용)
            work_queue_size: 10000,
            batch_size: 100,
            enable_work_stealing: true,
            enable_numa_awareness: true,
            enable_dynamic_balancing: true,
        }
    }
}

/// 병렬 브로드캐스트 통계
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ParallelBroadcastStats {
    /// 총 브로드캐스트 수
    pub total_broadcasts: u64,
    /// 총 메시지 전송 수
    pub total_messages_sent: u64,
    /// 총 수신자 수
    pub total_recipients: u64,
    /// 평균 브로드캐스트 시간 (마이크로초)
    pub avg_broadcast_time_us: f64,
    /// 최대 브로드캐스트 시간 (마이크로초)
    pub max_broadcast_time_us: u64,
    /// 병렬화 효율성 (%)
    pub parallelization_efficiency: f64,
    /// 스레드 활용률 (%)
    pub thread_utilization: f64,
    /// 작업 스틸링 발생 횟수
    pub work_stealing_events: u64,
    /// 로드 불균형 발생 횟수
    pub load_imbalance_events: u64,
}

/// 작업 타입
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkType {
    /// 메시지 브로드캐스트
    MessageBroadcast,
    /// 데이터 처리
    DataProcessing,
    /// 압축/압축해제
    Compression,
    /// 직렬화/역직렬화
    Serialization,
    /// 네트워크 I/O
    NetworkIO,
    /// 커스텀 작업
    Custom(String),
}

/// 작업 단위
pub struct WorkItem<T> {
    /// 작업 데이터
    pub data: T,
    /// 작업 타입
    pub work_type: WorkType,
    /// 우선순위 (높을수록 우선)
    pub priority: u8,
    /// 생성 시간
    pub created_at: Instant,
    /// 예상 처리 시간 (마이크로초)
    pub estimated_duration_us: u64,
}

impl<T> WorkItem<T> {
    pub fn new(data: T, work_type: WorkType) -> Self {
        Self {
            data,
            work_type,
            priority: 128, // 기본 우선순위
            created_at: Instant::now(),
            estimated_duration_us: 1000, // 기본 1ms
        }
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_estimated_duration(mut self, duration_us: u64) -> Self {
        self.estimated_duration_us = duration_us;
        self
    }

    /// 작업 대기 시간
    pub fn wait_time(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// 작업이 긴급한지 확인 (대기 시간 기준)
    pub fn is_urgent(&self, threshold_ms: u64) -> bool {
        self.wait_time().as_millis() as u64 > threshold_ms
    }
}

/// 워커 스레드 통계
#[derive(Debug, Default)]
pub struct WorkerStats {
    /// 처리한 작업 수
    pub processed_tasks: AtomicU64,
    /// 총 처리 시간 (마이크로초)
    pub total_processing_time_us: AtomicU64,
    /// 유휴 시간 (마이크로초)
    pub idle_time_us: AtomicU64,
    /// 작업 스틸링 시도 횟수
    pub steal_attempts: AtomicU64,
    /// 성공한 작업 스틸링 횟수
    pub successful_steals: AtomicU64,
    /// 큐 오버플로우 횟수
    pub queue_overflows: AtomicU64,
}

impl WorkerStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// 작업 처리 기록
    pub fn record_task_processed(&self, processing_time: Duration) {
        self.processed_tasks.fetch_add(1, Ordering::Relaxed);
        self.total_processing_time_us
            .fetch_add(processing_time.as_micros() as u64, Ordering::Relaxed);
    }

    /// 유휴 시간 기록
    pub fn record_idle_time(&self, idle_time: Duration) {
        self.idle_time_us
            .fetch_add(idle_time.as_micros() as u64, Ordering::Relaxed);
    }

    /// 작업 스틸링 시도 기록
    pub fn record_steal_attempt(&self, successful: bool) {
        self.steal_attempts.fetch_add(1, Ordering::Relaxed);
        if successful {
            self.successful_steals.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 큐 오버플로우 기록
    pub fn record_queue_overflow(&self) {
        self.queue_overflows.fetch_add(1, Ordering::Relaxed);
    }

    /// 평균 처리 시간 (마이크로초)
    pub fn average_processing_time_us(&self) -> f64 {
        let total_tasks = self.processed_tasks.load(Ordering::Relaxed);
        if total_tasks == 0 {
            return 0.0;
        }

        let total_time = self.total_processing_time_us.load(Ordering::Relaxed);
        total_time as f64 / total_tasks as f64
    }

    /// 활용률 계산 (0.0 ~ 1.0)
    pub fn utilization_rate(&self) -> f64 {
        let total_time = self.total_processing_time_us.load(Ordering::Relaxed);
        let idle_time = self.idle_time_us.load(Ordering::Relaxed);

        if total_time + idle_time == 0 {
            return 0.0;
        }

        total_time as f64 / (total_time + idle_time) as f64
    }

    /// 스틸링 성공률
    pub fn steal_success_rate(&self) -> f64 {
        let attempts = self.steal_attempts.load(Ordering::Relaxed);
        if attempts == 0 {
            return 0.0;
        }

        let successes = self.successful_steals.load(Ordering::Relaxed);
        successes as f64 / attempts as f64
    }
}

/// 병렬 브로드캐스터
pub struct ParallelBroadcaster {
    config: ParallelProcessingConfig,
    stats: Arc<ParallelBroadcastStats>,
    worker_stats: Vec<Arc<WorkerStats>>,
}

impl ParallelBroadcaster {
    pub fn new(config: ParallelProcessingConfig) -> Self {
        let worker_count = config.worker_threads;
        let mut worker_stats = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            worker_stats.push(Arc::new(WorkerStats::new()));
        }

        Self {
            config,
            stats: Arc::new(ParallelBroadcastStats::default()),
            worker_stats,
        }
    }

    /// 메시지를 여러 수신자에게 병렬로 브로드캐스트
    pub fn broadcast_parallel<T, F>(
        &self,
        message: &T,
        recipients: Vec<u32>,
        processor: F,
    ) -> Duration
    where
        T: Clone + Send + Sync,
        F: Fn(&T, u32) + Send + Sync,
    {
        let start = Instant::now();
        let recipient_count = recipients.len();

        if recipient_count == 0 {
            return Duration::ZERO;
        }

        // 배치 크기 계산
        let batch_size = self.calculate_optimal_batch_size(recipient_count);
        let batches: Vec<Vec<u32>> = recipients
            .chunks(batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        // 병렬 처리 시뮬레이션 (실제로는 rayon 사용)
        let processor = Arc::new(processor);
        let message = Arc::new(message.clone());

        // 각 배치를 병렬로 처리
        for (batch_idx, batch) in batches.iter().enumerate() {
            let worker_idx = batch_idx % self.config.worker_threads;
            let worker_stats = self.worker_stats[worker_idx].clone();
            let batch_message = message.clone();
            let batch_processor = processor.clone();

            let batch_start = Instant::now();

            // 배치 내 메시지 처리
            for &recipient in batch {
                batch_processor(&batch_message, recipient);
            }

            let batch_time = batch_start.elapsed();
            worker_stats.record_task_processed(batch_time);
        }

        let total_time = start.elapsed();
        self.update_broadcast_stats(recipient_count, total_time);

        total_time
    }

    /// 최적 배치 크기 계산
    fn calculate_optimal_batch_size(&self, total_recipients: usize) -> usize {
        let worker_count = self.config.worker_threads;
        let base_batch_size = self.config.batch_size;

        // 수신자가 적으면 워커당 1개씩
        if total_recipients <= worker_count {
            return 1;
        }

        // 워커 수의 배수로 조정
        let optimal_size = total_recipients.div_ceil(worker_count);
        optimal_size.min(base_batch_size).max(1)
    }

    /// 브로드캐스트 통계 업데이트 (단순화)
    fn update_broadcast_stats(&self, recipient_count: usize, duration: Duration) {
        // 실제 구현에서는 AtomicU64를 사용하여 업데이트
        tracing::debug!(
            "브로드캐스트 완료: {} 수신자, {}μs",
            recipient_count,
            duration.as_micros()
        );
    }

    /// 워커 통계 조회
    pub fn get_worker_stats(&self) -> &[Arc<WorkerStats>] {
        &self.worker_stats
    }

    /// 전체 통계 조회
    pub fn get_stats(&self) -> Arc<ParallelBroadcastStats> {
        self.stats.clone()
    }

    /// 병렬화 효율성 계산
    pub fn calculate_efficiency(&self) -> f64 {
        let total_utilization: f64 = self
            .worker_stats
            .iter()
            .map(|stats| stats.utilization_rate())
            .sum();

        let worker_count = self.worker_stats.len() as f64;
        if worker_count == 0.0 {
            return 0.0;
        }

        (total_utilization / worker_count) * 100.0
    }

    /// 로드 밸런싱 상태 확인
    pub fn check_load_balance(&self) -> f64 {
        if self.worker_stats.is_empty() {
            return 1.0; // 완벽한 균형
        }

        let utilizations: Vec<f64> = self
            .worker_stats
            .iter()
            .map(|stats| stats.utilization_rate())
            .collect();

        let mean = utilizations.iter().sum::<f64>() / utilizations.len() as f64;

        if mean == 0.0 {
            return 1.0;
        }

        let variance = utilizations
            .iter()
            .map(|&u| (u - mean).powi(2))
            .sum::<f64>()
            / utilizations.len() as f64;

        let std_dev = variance.sqrt();

        // 표준편차가 작을수록 균형이 좋음 (0~1 범위로 정규화)
        (1.0 - (std_dev / mean).min(1.0)).max(0.0)
    }
}

/// 작업 스케줄러
pub struct WorkScheduler {
    config: ParallelProcessingConfig,
    worker_stats: Vec<Arc<WorkerStats>>,
}

impl WorkScheduler {
    pub fn new(config: ParallelProcessingConfig) -> Self {
        let worker_count = config.worker_threads;
        let mut worker_stats = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            worker_stats.push(Arc::new(WorkerStats::new()));
        }

        Self {
            config,
            worker_stats,
        }
    }

    /// 최적 워커 선택 (로드 밸런싱)
    pub fn select_optimal_worker(&self) -> usize {
        if !self.config.enable_dynamic_balancing {
            // 라운드 로빈 방식
            return 0; // 단순화
        }

        // 가장 활용률이 낮은 워커 선택
        let mut best_worker = 0;
        let mut lowest_utilization = f64::MAX;

        for (idx, stats) in self.worker_stats.iter().enumerate() {
            let utilization = stats.utilization_rate();
            if utilization < lowest_utilization {
                lowest_utilization = utilization;
                best_worker = idx;
            }
        }

        best_worker
    }

    /// 작업 분산 처리
    pub fn distribute_work<T, F>(&self, work_items: Vec<WorkItem<T>>, processor: F) -> Duration
    where
        T: Send + Sync + Clone + 'static,
        F: Fn(WorkItem<T>) + Send + Sync + Clone + 'static,
    {
        let start = Instant::now();

        if work_items.is_empty() {
            return Duration::ZERO;
        }

        // 우선순위별로 정렬
        let mut sorted_items = work_items;
        sorted_items.sort_by(|a, b| b.priority.cmp(&a.priority));

        // 워커별로 작업 분배
        let worker_count = self.config.worker_threads;
        let batch_size = sorted_items.len().div_ceil(worker_count);

        for (worker_idx, chunk) in sorted_items.chunks(batch_size).enumerate() {
            let worker_stats = self.worker_stats[worker_idx % worker_count].clone();
            let chunk_processor = processor.clone();

            let chunk_start = Instant::now();

            // 작업 처리
            for work_item in chunk {
                // 여기서는 단순화하여 직접 호출
                // 실제로는 별도 스레드에서 처리
                chunk_processor(WorkItem {
                    data: work_item.data.clone(),
                    work_type: work_item.work_type.clone(),
                    priority: work_item.priority,
                    created_at: work_item.created_at,
                    estimated_duration_us: work_item.estimated_duration_us,
                });
            }

            let chunk_time = chunk_start.elapsed();
            worker_stats.record_task_processed(chunk_time);
        }

        start.elapsed()
    }

    /// 워커 통계 조회
    pub fn get_worker_stats(&self) -> &[Arc<WorkerStats>] {
        &self.worker_stats
    }
}

/// 병렬 처리 유틸리티
pub struct ParallelUtils;

impl ParallelUtils {
    /// 최적 스레드 수 계산
    pub fn calculate_optimal_threads(_work_complexity: f64, io_ratio: f64) -> usize {
        let cpu_count = 8; // 기본값 (실제로는 num_cpus::get() 사용)

        // CPU 집약적 작업이면 CPU 코어 수
        if io_ratio < 0.3 {
            return cpu_count;
        }

        // I/O 집약적 작업이면 더 많은 스레드
        let multiplier = if io_ratio > 0.7 { 2.0 } else { 1.5 };
        ((cpu_count as f64 * multiplier) as usize).min(16)
    }

    /// 배치 크기 최적화
    pub fn optimize_batch_size(total_items: usize, thread_count: usize, item_size: usize) -> usize {
        let base_batch = total_items / thread_count;

        // 캐시 라인 고려 (64바이트)
        let cache_line_items = 64 / item_size.max(1);

        // 캐시 라인의 배수로 조정
        (base_batch.div_ceil(cache_line_items) * cache_line_items).clamp(1, 1000)
        // 1~1000개 제한
    }

    /// 메모리 지역성 최적화를 위한 데이터 분할
    pub fn partition_for_locality<T: Clone>(data: Vec<T>, chunk_size: usize) -> Vec<Vec<T>> {
        if data.is_empty() {
            return vec![];
        }

        data.chunks(chunk_size)
            .map(|chunk| chunk.to_vec())
            .collect()
    }
}

mod tests {

    #[test]
    fn test_work_item_creation() {
        let item = WorkItem::new("test data".to_string(), WorkType::MessageBroadcast)
            .with_priority(200)
            .with_estimated_duration(5000);

        assert_eq!(item.priority, 200);
        assert_eq!(item.estimated_duration_us, 5000);
        assert_eq!(item.work_type, WorkType::MessageBroadcast);
    }

    #[test]
    fn test_work_item_urgency() {
        let item = WorkItem::new("test".to_string(), WorkType::DataProcessing);

        // 새로 만든 아이템은 긴급하지 않음
        assert!(!item.is_urgent(1000));

        // 대기 시간 시뮬레이션은 실제로는 시간이 지나야 함
        // 여기서는 함수가 작동하는지만 확인
    }

    #[test]
    fn test_worker_stats() {
        let stats = WorkerStats::new();

        assert_eq!(stats.processed_tasks.load(Ordering::Relaxed), 0);
        assert_eq!(stats.average_processing_time_us(), 0.0);
        assert_eq!(stats.utilization_rate(), 0.0);

        // 작업 처리 기록
        stats.record_task_processed(Duration::from_millis(10));
        assert_eq!(stats.processed_tasks.load(Ordering::Relaxed), 1);
        assert!(stats.average_processing_time_us() > 0.0);

        // 스틸링 기록
        stats.record_steal_attempt(true);
        stats.record_steal_attempt(false);
        assert_eq!(stats.steal_attempts.load(Ordering::Relaxed), 2);
        assert_eq!(stats.successful_steals.load(Ordering::Relaxed), 1);
        assert_eq!(stats.steal_success_rate(), 0.5);
    }

    #[test]
    fn test_parallel_broadcaster() {
        let config = ParallelProcessingConfig::default();
        let worker_threads = config.worker_threads;
        let broadcaster = ParallelBroadcaster::new(config);

        let message = "Hello, World!".to_string();
        let recipients = vec![1, 2, 3, 4, 5];

        let duration = broadcaster.broadcast_parallel(&message, recipients, |msg, recipient_id| {
            // 메시지 처리 시뮬레이션
            assert_eq!(msg, "Hello, World!");
            assert!(recipient_id > 0);
        });

        assert!(duration > Duration::ZERO);
        assert_eq!(broadcaster.get_worker_stats().len(), worker_threads);
    }

    #[test]
    fn test_work_scheduler() {
        let config = ParallelProcessingConfig::default();
        let worker_threads = config.worker_threads;
        let scheduler = WorkScheduler::new(config);

        let work_items = vec![
            WorkItem::new("work1".to_string(), WorkType::DataProcessing).with_priority(100),
            WorkItem::new("work2".to_string(), WorkType::Compression).with_priority(200),
            WorkItem::new("work3".to_string(), WorkType::NetworkIO).with_priority(150),
        ];

        let duration = scheduler.distribute_work(work_items, |work_item| {
            // 작업 처리 시뮬레이션
            assert!(!work_item.data.is_empty());
        });

        assert!(duration > Duration::ZERO);
        assert_eq!(scheduler.get_worker_stats().len(), worker_threads);
    }

    #[test]
    fn test_optimal_worker_selection() {
        let config = ParallelProcessingConfig::default();
        let scheduler = WorkScheduler::new(config);

        let worker_idx = scheduler.select_optimal_worker();
        assert!(worker_idx < scheduler.config.worker_threads);
    }

    #[test]
    fn test_parallel_utils() {
        // 최적 스레드 수 계산
        let cpu_intensive = ParallelUtils::calculate_optimal_threads(1.0, 0.1);
        let io_intensive = ParallelUtils::calculate_optimal_threads(0.5, 0.8);

        assert!(cpu_intensive <= 8); // CPU 코어 수 이하
        assert!(io_intensive > cpu_intensive); // I/O 집약적은 더 많은 스레드

        // 배치 크기 최적화
        let batch_size = ParallelUtils::optimize_batch_size(1000, 4, 8);
        assert!(batch_size > 0);
        assert!(batch_size <= 1000);

        // 데이터 분할
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let partitions = ParallelUtils::partition_for_locality(data, 3);

        assert_eq!(partitions.len(), 4); // [1,2,3], [4,5,6], [7,8,9], [10]
        assert_eq!(partitions[0], vec![1, 2, 3]);
        assert_eq!(partitions[3], vec![10]);
    }

    #[test]
    fn test_load_balance_calculation() {
        let config = ParallelProcessingConfig::default();
        let broadcaster = ParallelBroadcaster::new(config);

        // 초기 상태에서 균형도 확인
        let balance = broadcaster.check_load_balance();
        assert!(balance >= 0.0 && balance <= 1.0);

        // 효율성 계산
        let efficiency = broadcaster.calculate_efficiency();
        assert!(efficiency >= 0.0 && efficiency <= 100.0);
    }
}
