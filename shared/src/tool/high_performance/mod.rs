//! High-Performance Optimization Library
//!
//! tcpserver와 rudpserver에서 공통으로 사용되는 고성능 최적화 라이브러리입니다.
//! 16개 최적화 서비스의 핵심 구성 요소들을 제공합니다.

pub mod async_task_scheduler;
pub mod atomic_stats;
pub mod blocking_task_executor;
pub mod compression;
pub mod dashmap_optimizer;
pub mod enhanced_memory_pool;
pub mod lock_free_primitives;
pub mod memory_pool;
pub mod metrics_collector;
pub mod network_optimization;
pub mod parallel_processing;
pub mod redis_optimizer;
pub mod safe_primitives;
pub mod simd_optimizer;

pub use async_task_scheduler::{AsyncTaskScheduler, TaskPriority};
pub use atomic_stats::*;
pub use blocking_task_executor::*;
pub use compression::*;
pub use dashmap_optimizer::*;
pub use enhanced_memory_pool::*;
pub use lock_free_primitives::*;
// 주요 타입들 재출력 (중복 타입 제외)
pub use memory_pool::*;
pub use metrics_collector::*;
pub use network_optimization::*;
pub use parallel_processing::ParallelProcessingConfig;
pub use redis_optimizer::*;
pub use safe_primitives::*;
pub use simd_optimizer::*;
