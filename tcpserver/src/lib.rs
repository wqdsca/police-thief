//! TCP 게임 서버 라이브러리
//! 
//! Police Thief 게임을 위한 TCP 서버 구현입니다.
//! 실시간 연결 관리, 하트비트 시스템, 메시지 처리를 담당합니다.
//! 
//! # 주요 기능
//! 
//! - **실시간 연결 관리**: 클라이언트 연결 상태 모니터링
//! - **하트비트 시스템**: 자동 연결 상태 확인 및 타임아웃 처리
//! - **메시지 브로드캐스트**: 효율적인 다중 클라이언트 통신
//! - **프로토콜 처리**: 바이너리 기반 고성능 메시지 직렬화
//! - **에러 처리**: 체계적인 에러 관리 시스템
//! 
//! # 아키텍처
//! 
//! ```
//! TCP Server
//! ├── Service Layer (비즈니스 로직)
//! │   ├── ConnectionService (연결 관리)
//! │   ├── HeartbeatService (하트비트 관리)
//! │   ├── MessageService (메시지 처리)
//! │   └── SimpleTcpService (간단한 서비스)
//! ├── Handler Layer (요청 처리)
//! │   ├── ConnectionHandler (연결 처리)
//! │   ├── MessageHandler (메시지 처리)
//! │   └── GameHandler (게임 로직)
//! ├── Tool Layer (유틸리티)
//! │   ├── SimpleUtils (기본 유틸)
//! │   ├── NetworkUtils (네트워크 유틸)
//! │   ├── LinearUtils (선형 유틸)
//! │   └── Error (에러 처리)
//! └── Protocol (메시지 프로토콜)
//!     └── GameMessage (게임 메시지)
//! ```
//! 
//! # 사용 예시
//! 
//! ```rust
//! use tcpserver::{ConnectionService, HeartbeatService, GameMessage};
//! 
//! let connection_service = Arc::new(ConnectionService::new(1000));
//! let heartbeat_service = HeartbeatService::with_default_config(connection_service.clone());
//! 
//! // 서버 시작
//! heartbeat_service.start().await?;
//! 
//! // 메시지 브로드캐스트
//! connection_service.broadcast_message(&GameMessage::HeartBeat).await?;
//! ```
//! 
//! # 모듈 구조
//! 
//! - **protocol**: 게임 메시지 프로토콜 정의
//! - **service**: 비즈니스 로직 서비스들
//! - **handler**: 요청 처리 핸들러들
//! - **tool**: 공통 유틸리티 도구들
//! - **tests**: 통합 테스트

/// 게임 메시지 프로토콜 정의
/// 
/// 클라이언트와 서버 간 통신을 위한 메시지 타입들을 정의합니다.
pub mod protocol;

/// 비즈니스 로직 서비스 레이어
/// 
/// 연결 관리, 하트비트, 메시지 처리 등의 핵심 서비스들을 포함합니다.
pub mod service;

/// 요청 처리 핸들러 레이어
/// 
/// 클라이언트 요청을 처리하고 응답을 생성하는 핸들러들을 포함합니다.
/// 현재 일부 핸들러는 컴파일 안정화를 위해 비활성화되어 있습니다.
pub mod handler;

/// 공통 유틸리티 도구들
/// 
/// 데이터 변환, 에러 처리, 네트워크 유틸리티 등을 포함합니다.
pub mod tool;

/// 통합 테스트 모듈
/// 
/// 서버의 전체 기능을 테스트하는 통합 테스트들을 포함합니다.
#[cfg(test)]
pub mod tests;

// 주요 타입들 재출장
/// 게임 메시지 타입
/// 
/// 클라이언트와 서버 간 통신에 사용되는 모든 메시지 타입을 정의합니다.
pub use protocol::GameMessage;

/// 서비스 레이어 주요 타입들
/// 
/// 연결 관리, 하트비트, 클라이언트 연결 등의 핵심 서비스 타입들을 제공합니다.
pub use service::{ConnectionService, HeartbeatService, ClientConnection};

/// 간단한 TCP 서비스
/// 
/// 기본적인 TCP 서버 기능을 제공하는 간단한 서비스입니다.
/// 빠른 프로토타이핑이나 테스트에 적합합니다.
pub use service::SimpleTcpService;

/// 기본 유틸리티
/// 
/// 타임스탬프, 16진수 변환 등 기본적인 유틸리티 함수들을 제공합니다.
pub use tool::SimpleUtils;