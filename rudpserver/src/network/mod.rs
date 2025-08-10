//! 네트워크 모듈
//!
//! RUDP 프로토콜을 통한 네트워크 통신 및 세션 관리를 담당합니다.
//!
//! # 주요 구성요소
//! - `session`: 세션 관리 및 라이프사이클
//!
//! # 사용 예제
//! ```rust
//! use crate::network::session::SessionManager;
//!
//! // 세션 관리자 생성 및 시작
//! let session_manager = SessionManager::new(config, security, redis, player_manager).await?;
//! session_manager.start().await?;
//! ```

pub mod session;

// 주요 타입들을 re-export
pub use session::{
    SessionEvent, SessionEventListener, SessionId, SessionManager, SessionManagerConfig,
    SessionMetadata, SessionState,
};
