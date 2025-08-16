//! High-Performance QUIC Game Server Library
//!
//! 이 라이브러리는 게임 서버를 위한 고성능 QUIC 통신 프레임워크를 제공합니다.
//! 통신 최적화는 프레임워크에서 처리하고, 사용자는 게임 로직에만 집중할 수 있습니다.
//!
//! ## 주요 특징
//!
//! - 🚀 **고성능**: 15,000-20,000 msg/sec, <0.5ms p99 레이턴시
//! - 🔧 **통신 최적화**: 압축, 델타 압축, 스트림 멀티플렉싱
//! - 🎮 **게임 로직 분리**: 통신과 게임 로직의 완전한 분리
//! - 📊 **모니터링**: 실시간 성능 메트릭 수집
//! - 🛡️ **안정성**: 연결 복구, 마이그레이션 지원
//!
//! ## 빠른 시작
//!
//! ```rust,no_run
//! use quicserver::{
//!     game_logic::{GameLogicHandler, GameMessage, GameResponse, DefaultGameLogicHandler},
//!     communication::MessageProcessor,
//!     handler::UnifiedMessageHandler,
//!     config::QuicServerConfig,
//!     network::server::QuicGameServer,
//! };
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // 1. 설정 로드
//!     let config = QuicServerConfig::from_env()?;
//!     
//!     // 2. 게임 로직 구현 (또는 사용자 정의 구현체 사용)
//!     let game_logic: Arc<dyn GameLogicHandler> = Arc::new(DefaultGameLogicHandler::new());
//!     
//!     // 3. 서버 시작
//!     let server = QuicGameServer::new_with_game_logic(config, game_logic).await?;
//!     server.run().await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## 게임 로직 구현
//!
//! 핵심 게임 로직을 구현하려면 `GameLogicHandler` trait를 구현하세요:
//!
//! ```rust,no_run
//! use quicserver::game_logic::{GameLogicHandler, GameMessage, GameResponse};
//! use async_trait::async_trait;
//! use anyhow::Result;
//!
//! pub struct MyGameLogic {
//!     // 게임 상태, 데이터베이스 연결 등
//! }
//!
//! #[async_trait]
//! impl GameLogicHandler for MyGameLogic {
//!     async fn handle_player_move(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> {
//!         // 🎮 여기에 이동 로직 구현
//!         // - 위치 검증, 충돌 검사, 브로드캐스트 등
//!         todo!("이동 로직 구현")
//!     }
//!     
//!     async fn handle_player_attack(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> {
//!         // ⚔️ 여기에 공격 로직 구현  
//!         // - 범위 검증, 데미지 계산, 결과 브로드캐스트 등
//!         todo!("공격 로직 구현")
//!     }
//!     
//!     // 기타 필요한 메소드들 구현...
//! #   async fn handle_login(&self, payload: serde_json::Value) -> Result<GameResponse> { todo!() }
//! #   async fn handle_logout(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> { todo!() }
//! #   async fn handle_create_room(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> { todo!() }
//! #   async fn handle_join_room(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> { todo!() }
//! #   async fn handle_leave_room(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> { todo!() }
//! #   async fn handle_chat(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> { todo!() }
//! #   async fn handle_custom(&self, msg_type: &str, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> { todo!() }
//! #   async fn on_game_state_changed(&self, room_id: &str, players: &[quicserver::game_logic::PlayerInfo]) -> Result<Vec<GameMessage>> { todo!() }
//! #   async fn on_player_disconnected(&self, player_id: &str) -> Result<()> { todo!() }
//! }
//! ```

// 핵심 모듈들
pub mod communication;
pub mod config;
pub mod game_logic;
pub mod handler;
pub mod monitoring;
pub mod network;
pub mod optimization;
pub mod protocol;

// 편의를 위한 재출력
pub use communication::{CommunicationMetrics, MessageProcessor, StreamOptimization};
pub use config::QuicServerConfig;
pub use game_logic::{
    DefaultGameLogicHandler, GameLogicHandler, GameMessage, GameResponse, PlayerInfo, Position,
};
pub use handler::{StreamHandler, UnifiedMessageHandler};
pub use network::server::QuicGameServer;
pub use optimization::optimizer::QuicOptimizer;

/// QUIC 서버 버전 정보
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 라이브러리 초기화 (선택적)
pub fn init() {
    // 필요한 경우 초기화 로직 추가
    tracing::info!("QUIC Game Server Library v{} initialized", VERSION);
}

/// 빠른 서버 생성을 위한 헬퍼 함수
pub async fn create_server_with_default_logic(
    config: QuicServerConfig,
) -> anyhow::Result<QuicGameServer> {
    let game_logic = std::sync::Arc::new(DefaultGameLogicHandler::new());
    QuicGameServer::new_with_game_logic(config, game_logic).await
}

/// 에러 타입들
pub mod error {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum QuicServerError {
        #[error("Configuration error: {0}")]
        Config(String),

        #[error("Network error: {0}")]
        Network(String),

        #[error("Game logic error: {0}")]
        GameLogic(String),

        #[error("Communication error: {0}")]
        Communication(String),

        #[error("Optimization error: {0}")]
        Optimization(String),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_set() {
        assert!(!VERSION.is_empty());
    }

    #[tokio::test]
    async fn default_game_logic_works() {
        let logic = DefaultGameLogicHandler::new();
        let response = logic
            .handle_login(serde_json::json!({
                "nickname": "TestPlayer"
            }))
            .await;

        assert!(response.is_ok());
        let response = response.expect("Login response should be successful");
        assert_eq!(response.msg_type, "login_response");
        assert!(response.success);
    }
}
