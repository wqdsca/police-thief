//! TCP 서버 서비스 레이어
//! 
//! 비즈니스 로직과 핵심 기능을 담당하는 서비스들을 정의합니다.
//! 
//! # 서비스 구조
//! 
//! ```
//! Service Layer
//! ├── ConnectionService (연결 관리)
//! │   ├── 클라이언트 연결 추가/제거
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
/// 클라이언트 연결의 전체 생명주기를 관리하는 핵심 서비스입니다.
/// 연결 추가/제거, 메시지 송수신, 브로드캐스트, 통계 수집을 담당합니다.
pub mod connection_service;

/// 하트비트 관리 서비스
/// 
/// 클라이언트 연결 상태를 주기적으로 모니터링하고
/// 타임아웃된 연결을 자동으로 정리하는 서비스입니다.
pub mod heartbeat_service;

/// TCP 서버 서비스
/// 
/// TCP 서버의 설정, 생명주기, 상태 관리를 담당하는 서비스입니다.
/// 서버 시작/중지, 설정 관리, 통계 수집을 제공합니다.
pub mod tcp_service;

/// 메시지 처리 서비스
/// 
/// 게임 메시지의 라우팅, 처리, 통계를 담당하는 서비스입니다.
/// 메시지 핸들러 등록, 메시지 타입별 처리, 에러 처리를 제공합니다.
pub mod message_service;

// 서비스 모듈들 재출장
/// 간단한 TCP 서비스 타입들
/// 
/// 기본적인 TCP 서버 기능을 제공하는 간단한 서비스 타입들을 제공합니다.
pub use simple_services::*;

/// 연결 관리 서비스 타입들
/// 
/// 클라이언트 연결 관리와 관련된 모든 타입들을 제공합니다.
/// ConnectionService, ClientConnection, ConnectionStats 등이 포함됩니다.
pub use connection_service::*;

/// 하트비트 관리 서비스 타입들
/// 
/// 하트비트 시스템과 관련된 모든 타입들을 제공합니다.
/// HeartbeatService, HeartbeatStats, ConnectionHealth 등이 포함됩니다.
pub use heartbeat_service::*;

/// TCP 서버 서비스 타입들
/// 
/// TCP 서버 설정과 관련된 모든 타입들을 제공합니다.
/// TcpGameService, TcpServerConfig, ServerStats 등이 포함됩니다.
pub use tcp_service::*;

/// 메시지 처리 서비스 타입들
/// 
/// 메시지 처리와 관련된 모든 타입들을 제공합니다.
/// MessageService, MessageStats, MessageHandler 등이 포함됩니다.
pub use message_service::*;