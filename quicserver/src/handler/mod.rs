mod message;
mod unified_handler;

// 레거시 지원
pub use message::MessageHandler;

// 새로운 구조 (권장)
pub use unified_handler::{StreamHandler, UnifiedMessageHandler};
