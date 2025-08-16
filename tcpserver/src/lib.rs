//! TCP 서버 라이브러리
//!
//! 4가지 핵심 기능을 제공하는 TCP 서버 구현입니다.
//! - 방 입장 (Room Entry)
//! - 채팅 (Chat)
//! - 친구 추가 (Friend Add)
//! - 친구 삭제 (Friend Remove)
//!
//! # 주요 기능
//!
//! - **실시간 연결 관리**: 사용자 연결 상태 모니터링
//! - **하트비트 시스템**: 자동 연결 상태 확인 및 타임아웃 처리
//! - **메시지 브로드캐스트**: 효율적인 다중 사용자 통신
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
//! │   ├── RoomHandler (방 관리)
//! │   └── FriendHandler (친구 관리)
//! ├── Tool Layer (유틸리티)
//! │   ├── SimpleUtils (기본 유틸)
//! │   ├── NetworkUtils (네트워크 유틸)
//! │   ├── LinearUtils (선형 유틸)
//! │   └── Error (에러 처리)
//! └── Protocol (메시지 프로토콜)
//!     └── GameMessage (서버 메시지)
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
//! - **protocol**: 서버 메시지 프로토콜 정의
//! - **service**: 비즈니스 로직 서비스들
//! - **handler**: 요청 처리 핸들러들 (방, 친구, 채팅, 연결 관리)
//! - **tool**: 공통 유틸리티 도구들
//! - **tests**: 통합 테스트

/// 환경 설정 관리
///
/// 서버 실행에 필요한 환경변수 및 설정을 관리합니다.
pub mod config;

/// 서버 메시지 프로토콜 정의
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
pub mod tests;

// 주요 타입들 재출장
/// 서버 메시지 타입
///
/// 클라이언트와 서버 간 통신에 사용되는 모든 메시지 타입을 정의합니다.
pub use protocol::GameMessage;

/// 서비스 레이어 주요 타입들
///
/// 연결 관리, 하트비트, 사용자 연결 등의 핵심 서비스 타입들을 제공합니다.
pub use service::{ConnectionService, HeartbeatService, MessageService};

/// 간단한 TCP 서비스
///
/// 기본적인 TCP 서버 기능을 제공하는 간단한 서비스입니다.
/// 빠른 프로토타이핑이나 테스트에 적합합니다.
pub use service::simple_services::SimpleTcpService;

/// 환경 설정 타입들
///
/// TCP 서버 설정 및 설정 검증 함수를 제공합니다.
pub use config::{validate_config, TcpServerConfig};

/// 기본 유틸리티
///
/// 타임스탬프, 16진수 변환 등 기본적인 유틸리티 함수들을 제공합니다.
pub use tool::SimpleUtils;
