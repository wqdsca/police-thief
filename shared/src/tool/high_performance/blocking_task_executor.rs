use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot};
use tokio::task::spawn_blocking;
use tracing::{debug, info, warn};

/// Blocking 작업 실행자
/// CPU 집약적이거나 동기 I/O 작업을 별도 스레드 풀에서 실행
pub struct BlockingTaskExecutor {
    /// 작업 큐 전송자
    sender: mpsc::Sender<BlockingTask>,
    /// 스레드 풀 크기
    pool_size: usize,
    /// 통계
    stats: Arc<BlockingTaskStats>,
}

/// Blocking 작업
struct BlockingTask {
    task: Box<dyn FnOnce() -> Box<dyn std::any::Any + Send> + Send>,
    result_sender: oneshot::Sender<Box<dyn std::any::Any + Send>>,
    task_name: String,
}

/// 작업 실행 통계
#[derive(Debug, Default)]
pub struct BlockingTaskStats {
    pub total_tasks: std::sync::atomic::AtomicU64,
    pub completed_tasks: std::sync::atomic::AtomicU64,
    pub failed_tasks: std::sync::atomic::AtomicU64,
    pub average_duration_ms: std::sync::atomic::AtomicU64,
    pub max_duration_ms: std::sync::atomic::AtomicU64,
}

impl BlockingTaskExecutor {
    /// 새 실행자 생성
    pub fn new(pool_size: Option<usize>) -> Self {
        let pool_size = pool_size.unwrap_or_else(|| {
            // CPU 코어 수에 따라 자동 설정
            let cores = num_cpus::get();
            (cores * 2).min(32).max(4)
        });
        
        let (sender, mut receiver) = mpsc::channel::<BlockingTask>(1000);
        let stats = Arc::new(BlockingTaskStats::default());
        let stats_clone = stats.clone();
        
        // 작업 처리 루프
        tokio::spawn(async move {
            while let Some(task) = receiver.recv().await {
                let stats = stats_clone.clone();
                let task_name = task.task_name.clone();
                
                spawn_blocking(move || {
                    let start = Instant::now();
                    
                    // 통계 업데이트
                    stats.total_tasks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    
                    debug!("Executing blocking task: {}", task_name);
                    
                    // 작업 실행
                    let result = (task.task)();
                    
                    // 결과 전송
                    if task.result_sender.send(result).is_err() {
                        warn!("Failed to send blocking task result for: {}", task_name);
                        stats.failed_tasks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        stats.completed_tasks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    
                    // 실행 시간 통계 업데이트
                    let duration_ms = start.elapsed().as_millis() as u64;
                    
                    // 최대 실행 시간 업데이트
                    let mut current_max = stats.max_duration_ms.load(std::sync::atomic::Ordering::Relaxed);
                    while duration_ms > current_max {
                        match stats.max_duration_ms.compare_exchange_weak(
                            current_max,
                            duration_ms,
                            std::sync::atomic::Ordering::Release,
                            std::sync::atomic::Ordering::Relaxed,
                        ) {
                            Ok(_) => break,
                            Err(x) => current_max = x,
                        }
                    }
                    
                    // 평균 실행 시간 업데이트 (간단한 이동 평균)
                    let current_avg = stats.average_duration_ms.load(std::sync::atomic::Ordering::Relaxed);
                    let new_avg = (current_avg * 9 + duration_ms) / 10;
                    stats.average_duration_ms.store(new_avg, std::sync::atomic::Ordering::Relaxed);
                    
                    if duration_ms > 1000 {
                        warn!("Blocking task '{}' took {}ms", task_name, duration_ms);
                    }
                });
            }
        });
        
        info!("BlockingTaskExecutor initialized with {} threads", pool_size);
        
        Self {
            sender,
            pool_size,
            stats,
        }
    }
    
    /// 동기 함수를 비동기로 실행
    pub async fn execute<F, R>(&self, name: &str, f: F) -> Result<R, String>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (result_sender, result_receiver) = oneshot::channel();
        
        let task = BlockingTask {
            task: Box::new(move || Box::new(f()) as Box<dyn std::any::Any + Send>),
            result_sender,
            task_name: name.to_string(),
        };
        
        self.sender.send(task).await
            .map_err(|_| "Failed to queue blocking task".to_string())?;
        
        let result = result_receiver.await
            .map_err(|_| "Failed to receive blocking task result".to_string())?;
        
        result.downcast::<R>()
            .map(|boxed| *boxed)
            .map_err(|_| "Failed to downcast blocking task result".to_string())
    }
    
    /// CPU 집약적 작업 실행
    pub async fn execute_cpu_intensive<F, R>(&self, name: &str, f: F) -> Result<R, String>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        debug!("Executing CPU-intensive task: {}", name);
        self.execute(name, f).await
    }
    
    /// 동기 I/O 작업 실행
    pub async fn execute_blocking_io<F, R>(&self, name: &str, f: F) -> Result<R, String>
    where
        F: FnOnce() -> std::io::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let task_name = name.to_string();
        debug!("Executing blocking I/O task: {}", task_name);
        self.execute(&task_name, move || {
            f().map_err(|e| {
                tracing::error!("Blocking I/O error: {}", e);
                e
            }).expect("I/O operation failed")
        }).await
    }
    
    /// 통계 가져오기
    pub fn get_stats(&self) -> BlockingTaskStatsSnapshot {
        BlockingTaskStatsSnapshot {
            total_tasks: self.stats.total_tasks.load(std::sync::atomic::Ordering::Relaxed),
            completed_tasks: self.stats.completed_tasks.load(std::sync::atomic::Ordering::Relaxed),
            failed_tasks: self.stats.failed_tasks.load(std::sync::atomic::Ordering::Relaxed),
            average_duration_ms: self.stats.average_duration_ms.load(std::sync::atomic::Ordering::Relaxed),
            max_duration_ms: self.stats.max_duration_ms.load(std::sync::atomic::Ordering::Relaxed),
            pool_size: self.pool_size,
        }
    }
}

/// 통계 스냅샷
#[derive(Debug, Clone)]
pub struct BlockingTaskStatsSnapshot {
    pub total_tasks: u64,
    pub completed_tasks: u64,
    pub failed_tasks: u64,
    pub average_duration_ms: u64,
    pub max_duration_ms: u64,
    pub pool_size: usize,
}

impl BlockingTaskStatsSnapshot {
    pub fn success_rate(&self) -> f64 {
        if self.total_tasks == 0 {
            0.0
        } else {
            (self.completed_tasks as f64) / (self.total_tasks as f64) * 100.0
        }
    }
}

/// 전역 Blocking 작업 실행자
lazy_static::lazy_static! {
    static ref GLOBAL_EXECUTOR: BlockingTaskExecutor = BlockingTaskExecutor::new(None);
}

/// 전역 실행자를 통한 Blocking 작업 실행
pub async fn execute_blocking<F, R>(name: &str, f: F) -> Result<R, String>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    GLOBAL_EXECUTOR.execute(name, f).await
}

/// CPU 집약적 작업을 위한 헬퍼 매크로
#[macro_export]
macro_rules! cpu_intensive {
    ($name:expr, $body:expr) => {
        $crate::tool::high_performance::blocking_task_executor::execute_blocking($name, move || $body).await
    };
}

/// Blocking I/O를 위한 헬퍼 매크로
#[macro_export]
macro_rules! blocking_io {
    ($name:expr, $body:expr) => {
        $crate::tool::high_performance::blocking_task_executor::execute_blocking($name, move || $body).await
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_blocking_execution() {
        let executor = BlockingTaskExecutor::new(Some(4));
        
        let result = executor.execute("test_task", || {
            // CPU 집약적 작업 시뮬레이션
            let mut sum = 0u64;
            for i in 0..1000000 {
                sum += i;
            }
            sum
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 499999500000);
    }
    
    #[tokio::test]
    async fn test_multiple_tasks() {
        let executor = Arc::new(BlockingTaskExecutor::new(Some(4)));
        
        let mut handles = vec![];
        
        for i in 0..10 {
            let executor = executor.clone();
            let handle = tokio::spawn(async move {
                executor.execute(&format!("task_{}", i), move || {
                    std::thread::sleep(Duration::from_millis(10));
                    i * 2
                }).await
            });
            handles.push(handle);
        }
        
        for (i, handle) in handles.into_iter().enumerate() {
            let result = handle.await.unwrap().unwrap();
            assert_eq!(result, i * 2);
        }
        
        let stats = executor.get_stats();
        assert_eq!(stats.completed_tasks, 10);
        assert_eq!(stats.failed_tasks, 0);
    }
    
    #[tokio::test]
    async fn test_global_executor() {
        let result = execute_blocking("global_test", || {
            42
        }).await;
        
        assert_eq!(result.unwrap(), 42);
    }
}