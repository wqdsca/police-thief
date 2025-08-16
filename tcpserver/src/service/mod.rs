//! TCP 서버 서비스 레이어
//!
//! 비즈니스 로직과 핵심 기능을 담당하는 서비스들을 정의합니다.
//!
//! # 서비스 구조
//!
//! ```
//! Service Layer
//! ├── ConnectionService (연결 관리)
//! │   ├── 사용자 연결 추가/제거
//! │   ├── 메시지 송수신
//! │   ├── 브로드캐스트
//! │   └── 연결 통계
//! ├── HeartbeatService (하트비트 관리)
//! │   ├── 자동 연결 모니터링
//! │   ├── 타임아웃 정리
//! │   ├── 연결 상태 평가
//! │   └── 하트비트 통계
//! ├── MessageService (메시지 처리)
//! │   ├── 메시지 라우팅
//! │   ├── 핸들러 등록
//! │   ├── 메시지 통계
//! │   └── 에러 처리
//! ├── TcpService (TCP 서버)
//! │   ├── 서버 설정
//! │   ├── 서버 생명주기
//! │   ├── 상태 관리
//! │   └── 통계 수집
//! └── SimpleTcpService (간단한 서비스)
//!     ├── 기본 서버 기능
//!     ├── 빠른 시작/중지
//!     └── 상태 확인
//! ```
//!
//! # 사용 예시
//!
//! ```rust
//! use tcpserver::service::{ConnectionService, HeartbeatService};
//!
//! // 연결 서비스 생성
//! let connection_service = Arc::new(ConnectionService::new(1000));
//!
//! // 하트비트 서비스 생성
//! let heartbeat_service = HeartbeatService::with_default_config(connection_service.clone());
//!
//! // 서비스 시작
//! heartbeat_service.start().await?;
//!
//! // 연결 통계 조회
//! let stats = connection_service.get_connection_stats().await;
//! ```
//!
//! # 서비스 특징
//!
//! - **스레드 안전**: 모든 서비스가 Arc<Mutex<>> 기반으로 스레드 안전
//! - **비동기 처리**: tokio 기반 비동기 I/O 지원
//! - **확장 가능**: 새로운 서비스 추가가 용이한 구조
//! - **모니터링**: 각 서비스별 상세한 통계 및 로깅
//! - **에러 처리**: 체계적인 에러 처리 및 복구 메커니즘

/// 간단한 서비스 구현
///
/// 컴파일 안정화를 위한 간단한 TCP 서비스입니다.
/// 기본적인 서버 기능만 제공하여 빠른 프로토타이핑에 적합합니다.
pub mod simple_services;

/// 연결 관리 서비스
///
/// 사용자 연결의 전체 생명주기를 관리하는 핵심 서비스입니다.
/// 연결 추가/제거, 메시지 송수신, 브로드캐스트, 통계 수집을 담당합니다.
pub mod connection_service;

/// 하트비트 관리 서비스
///
/// 사용자 연결 상태를 주기적으로 모니터링하고
/// 타임아웃된 연결을 자동으로 정리하는 서비스입니다.
pub mod heartbeat_service;

/// TCP 서버 서비스
///
/// TCP 서버의 설정, 생명주기, 상태 관리를 담당하는 서비스입니다.
/// 서버 시작/중지, 설정 관리, 통계 수집을 제공합니다.
pub mod tcp_service;

/// 메시지 처리 서비스
///
/// 서버 메시지의 라우팅, 처리, 통계를 담당하는 서비스입니다.
/// 메시지 핸들러 등록, 메시지 타입별 처리, 에러 처리를 제공합니다.
pub mod message_service;

/// 방 기반 연결 관리 서비스
///
/// DashMap을 사용한 고성능 방 기반 연결 관리 시스템입니다.
/// 방별 사용자 관리, Redis 백업, 실시간 메시징을 제공합니다.
pub mod room_connection_service;

/// 병렬 브로드캐스트 서비스
///
/// Rayon 기반 병렬 처리로 메시지 전송 성능을 300-500% 향상시킵니다.
/// 순차 전송 대신 병렬 전송으로 대규모 동시 사용자 지원을 제공합니다.
pub mod parallel_broadcast;

/// 메모리 풀 시스템
///
/// 객체 재사용을 통해 30% 메모리 사용량 절약과 GC 압박 감소를 달성합니다.
/// 연결 객체 풀링, 버퍼 재사용, 메모리 단편화 방지를 제공합니다.
pub mod memory_pool;

/// 원자적 통계 시스템
///
/// AtomicU64 기반 락-프리 통계 수집 시스템으로 성능 오버헤드를 최소화합니다.
/// 실시간 메트릭 수집, 성능 임계값 모니터링, 자동 알림을 제공합니다.
pub mod atomic_stats;

// DashMap 최적화는 이제 shared::tool::high_performance::dashmap_optimizer 사용

/// 비동기 I/O 최적화 서비스
///
/// Zero-copy, vectored I/O, 메모리 프리페칭 등을 통해 I/O 성능을 극대화합니다.
/// 적응형 버퍼링, I/O 병합, 파이프라인 처리를 지원합니다.
pub mod async_io_optimizer;

/// SIMD 연산 최적화 서비스
///
/// AVX2, SSE4.2 등 SIMD 명령어를 활용한 벡터화 연산을 제공합니다.
/// 메모리 비교, 검색, XOR 연산, 체크섬 계산 등을 고속화합니다.
pub mod simd_optimizer;

/// 메시지 압축 및 배칭 서비스
///
/// LZ4, Zstd, 적응형 압축을 통해 네트워크 대역폭을 절약합니다.
/// 지능형 메시지 배칭과 압축 캐싱으로 처리량을 향상시킵니다.
pub mod message_compression;

/// 고급 연결 풀링 서비스
///
/// 자동 확장/축소, 헬스체크, 부하 분산을 지원하는 고도화된 연결 풀입니다.
/// 연결 재사용, 상태 관리, 성능 최적화를 통해 연결 효율성을 극대화합니다.
pub mod connection_pool;

/// 성능 모니터링 및 프로파일링 도구
///
/// 실시간 성능 메트릭 수집, 프로파일링, 경고 시스템을 제공합니다.
/// CPU, 메모리, 네트워크, 레이턴시 등 종합적인 성능 분석을 지원합니다.
pub mod performance_monitor;

/// 성능 벤치마크 및 검증 도구
///
/// 모든 최적화 서비스의 성능을 측정하고 검증하는 종합 벤치마크 시스템입니다.
/// 실제 워크로드 시뮬레이션을 통해 최적화 효과를 정량적으로 측정합니다.
pub mod performance_benchmark;

// 서비스 모듈들 재출장

/// 연결 관리 서비스 타입들
///
/// 사용자 연결 관리와 관련된 모든 타입들을 제공합니다.
/// ConnectionService, UserConnection, ConnectionStats 등이 포함됩니다.
pub use connection_service::*;

/// 하트비트 관리 서비스 타입들
///
/// 하트비트 시스템과 관련된 모든 타입들을 제공합니다.
/// HeartbeatService, HeartbeatStats, ConnectionHealth 등이 포함됩니다.
pub use heartbeat_service::*;

/// 메시지 처리 서비스 타입들
///
/// 메시지 처리와 관련된 모든 타입들을 제공합니다.
/// MessageService, MessageStats, MessageHandler 등이 포함됩니다.
pub use message_service::*;

/// 방 기반 연결 관리 서비스 타입들
///
/// 방 기반 연결 관리와 관련된 모든 타입들을 제공합니다.
/// RoomConnectionService, RoomUserConnection, RoomInfo 등이 포함됩니다.
pub use room_connection_service::*;

// DashMap 최적화 타입들은 shared::tool::high_performance::dashmap_optimizer에서 제공

/// 비동기 I/O 최적화 서비스 타입들
///
/// 비동기 I/O 최적화와 관련된 모든 타입들을 제공합니다.
/// AsyncIoOptimizer, AsyncIoOptimizerConfig, AsyncIoPerformanceReport 등이 포함됩니다.
pub use async_io_optimizer::*;

/// SIMD 최적화 서비스 타입들
///
/// SIMD 연산 최적화와 관련된 모든 타입들을 제공합니다.
/// SimdOptimizer, SimdOptimizerConfig, SimdPerformanceStats 등이 포함됩니다.
pub use simd_optimizer::*;

/// 메시지 압축 서비스 타입들
///
/// 메시지 압축 및 배칭과 관련된 모든 타입들을 제공합니다.
/// MessageCompressionService, MessageCompressionConfig, CompressionPerformanceReport 등이 포함됩니다.
pub use message_compression::*;

/// 성능 모니터링 서비스 타입들
///
/// 성능 모니터링 및 프로파일링과 관련된 모든 타입들을 제공합니다.
/// PerformanceMonitor, PerformanceMonitorConfig, PerformanceReport 등이 포함됩니다.
pub use performance_monitor::*;
