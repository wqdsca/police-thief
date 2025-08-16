//! Legacy Message Handler - DEPRECATED
//!
//! ⚠️ 이 파일은 더 이상 사용되지 않습니다.
//! 대신 다음 파일들을 사용하세요:
//! - src/game_logic.rs: 게임 로직 인터페이스
//! - src/communication.rs: 통신 최적화 유틸리티
//! - src/handler/unified_handler.rs: 통합 메시지 핸들러
//!
//! 이 파일은 하위 호환성을 위해 임시로 유지됩니다.

use crate::handler::unified_handler::UnifiedMessageHandler;

/// 레거시 MessageHandler - 새로운 UnifiedMessageHandler로 마이그레이션 권장
#[deprecated(since = "0.2.0", note = "Use UnifiedMessageHandler instead")]
pub struct MessageHandler {
    unified_handler: Option<std::sync::Arc<UnifiedMessageHandler>>,
}

impl MessageHandler {
    #[deprecated(since = "0.2.0", note = "Use UnifiedMessageHandler::new instead")]
    pub fn new(_optimizer: std::sync::Arc<crate::optimization::optimizer::QuicOptimizer>) -> Self {
        eprintln!("⚠️  WARNING: MessageHandler is deprecated. Please migrate to:");
        eprintln!("   - GameLogicHandler trait for game logic");
        eprintln!("   - MessageProcessor for communication optimization");
        eprintln!("   - UnifiedMessageHandler for message routing");
        eprintln!("   See examples in src/game_logic.rs");

        Self {
            unified_handler: None,
        }
    }
}

// 마이그레이션을 위한 예제 코드
#[cfg(feature = "migration-example")]
mod migration_example {
    use super::*;
    use crate::communication::MessageProcessor;
    use crate::game_logic::{DefaultGameLogicHandler, GameLogicHandler};
    use crate::handler::unified_handler::UnifiedMessageHandler;
    use std::sync::Arc;

    /// 새로운 구조로 마이그레이션하는 방법 예제
    pub async fn create_new_handler(
        optimizer: Arc<crate::optimization::optimizer::QuicOptimizer>,
    ) -> Arc<UnifiedMessageHandler> {
        // 1. 통신 최적화 프로세서 생성
        let message_processor = Arc::new(MessageProcessor::new(optimizer));

        // 2. 게임 로직 핸들러 생성 (사용자 구현체 사용)
        let game_logic: Arc<dyn GameLogicHandler> = Arc::new(DefaultGameLogicHandler::new());

        // 3. 통합 핸들러 생성
        Arc::new(UnifiedMessageHandler::new(message_processor, game_logic))
    }
}
