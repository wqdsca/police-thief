//! RUDP Protocol Definitions
//!
//! 게임 메시지 프로토콜 정의 및 직렬화/역직렬화 기능을 제공합니다.

pub mod rudp;

use serde::{Deserialize, Serialize};

/// 게임 메시지 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameMessageType {
    /// 하트비트 메시지
    Heartbeat,

    /// 플레이어 연결
    PlayerConnect { player_id: u32, player_name: String },

    /// 플레이어 연결 해제
    PlayerDisconnect { player_id: u32 },

    /// 채팅 메시지
    Chat { player_id: u32, message: String },

    /// 게임 상태 업데이트 (확장 예정)
    GameState {
        // TODO: 게임 로직에 따른 상태 정의
        data: Vec<u8>,
    },

    /// 확장을 위한 커스텀 메시지
    Custom { message_type: String, data: Vec<u8> },
}

/// 게임 메시지 래퍼
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMessage {
    /// 메시지 ID (중복 검출용)
    pub message_id: u64,

    /// 타임스탬프
    pub timestamp: u64,

    /// 메시지 내용
    pub content: GameMessageType,

    /// 신뢰성 요구사항
    pub reliability: ReliabilityLevel,
}

/// 신뢰성 레벨
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReliabilityLevel {
    /// 신뢰성 불필요 (속도 우선)
    Unreliable,

    /// 순서 보장 필요
    Sequenced,

    /// 신뢰성 보장 필요
    Reliable,

    /// 신뢰성 + 순서 보장
    ReliableSequenced,
}

impl GameMessage {
    /// 새로운 게임 메시지를 생성합니다.
    pub fn new(content: GameMessageType, reliability: ReliabilityLevel) -> Self {
        Self {
            message_id: generate_message_id(),
            timestamp: current_timestamp(),
            content,
            reliability,
        }
    }

    /// 메시지를 바이트 배열로 직렬화합니다.
    pub fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| anyhow::anyhow!("메시지 직렬화 실패: {}", e))
    }

    /// 바이트 배열에서 메시지를 역직렬화합니다.
    pub fn from_bytes(data: &[u8]) -> anyhow::Result<Self> {
        serde_json::from_slice(data).map_err(|e| anyhow::anyhow!("메시지 역직렬화 실패: {}", e))
    }

    /// 메시지 타입 문자열을 반환합니다.
    pub fn message_type_str(&self) -> &'static str {
        match &self.content {
            GameMessageType::Heartbeat => "heartbeat",
            GameMessageType::PlayerConnect { .. } => "player_connect",
            GameMessageType::PlayerDisconnect { .. } => "player_disconnect",
            GameMessageType::Chat { .. } => "chat",
            GameMessageType::GameState { .. } => "game_state",
            GameMessageType::Custom { .. } => "custom",
        }
    }
}

/// 메시지 ID 생성 함수
fn generate_message_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static MESSAGE_COUNTER: AtomicU64 = AtomicU64::new(0);
    MESSAGE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// 현재 타임스탬프 획득 함수
fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
