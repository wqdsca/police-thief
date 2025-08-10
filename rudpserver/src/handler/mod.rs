//! RUDP Message Handlers
//!
//! 게임 메시지 처리를 위한 핸들러 시스템입니다.
//! 확장 가능한 구조로 새로운 게임 기능을 쉽게 추가할 수 있습니다.

/// 게임 메시지 핸들러 인터페이스
///
/// 새로운 게임 기능을 추가할 때 이 트레이트를 구현하세요.
#[async_trait::async_trait]
pub trait GameMessageHandler: Send + Sync {
    /// 메시지를 처리할 수 있는지 확인합니다.
    fn can_handle(&self, message_type: &str) -> bool;

    /// 메시지를 처리합니다.
    async fn handle(&self, message: &[u8]) -> anyhow::Result<Vec<u8>>;

    /// 핸들러 이름을 반환합니다.
    fn handler_name(&self) -> &'static str;
}

/// 기본 게임 메시지 핸들러
///
/// 게임 로직이 구현되기 전까지 사용되는 기본 핸들러입니다.
pub struct DefaultGameHandler;

#[async_trait::async_trait]
impl GameMessageHandler for DefaultGameHandler {
    fn can_handle(&self, _message_type: &str) -> bool {
        true // 모든 메시지를 처리
    }

    async fn handle(&self, message: &[u8]) -> anyhow::Result<Vec<u8>> {
        tracing::info!("🎮 기본 게임 핸들러: {} bytes 메시지 처리", message.len());

        // TODO: 실제 게임 로직 구현
        // 현재는 에코 응답
        Ok(message.to_vec())
    }

    fn handler_name(&self) -> &'static str {
        "DefaultGameHandler"
    }
}

/// 게임 메시지 라우터
///
/// 메시지 타입에 따라 적절한 핸들러로 라우팅합니다.
pub struct GameMessageRouter {
    handlers: Vec<Box<dyn GameMessageHandler + Send + Sync>>,
}

impl GameMessageRouter {
    /// 새로운 메시지 라우터를 생성합니다.
    pub fn new() -> Self {
        let mut router = Self {
            handlers: Vec::new(),
        };

        // 기본 핸들러 등록
        router.add_handler(Box::new(DefaultGameHandler));

        router
    }

    /// 새로운 핸들러를 추가합니다.
    pub fn add_handler(&mut self, handler: Box<dyn GameMessageHandler + Send + Sync>) {
        tracing::info!("📝 게임 핸들러 등록: {}", handler.handler_name());
        self.handlers.push(handler);
    }

    /// 메시지를 적절한 핸들러로 라우팅합니다.
    pub async fn route_message(
        &self,
        message_type: &str,
        message: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        for handler in &self.handlers {
            if handler.can_handle(message_type) {
                return handler.handle(message).await;
            }
        }

        Err(anyhow::anyhow!(
            "메시지 타입 '{}' 에 대한 핸들러를 찾을 수 없습니다",
            message_type
        ))
    }
}
