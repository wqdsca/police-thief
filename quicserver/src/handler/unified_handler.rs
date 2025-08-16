//! Unified Message Handler
//!
//! 통신 최적화와 게임 로직을 연결하는 통합 메시지 핸들러입니다.
//! 통신 최적화는 프레임워크에서, 게임 로직은 사용자 구현에서 처리합니다.

use crate::communication::{MessageProcessor, StreamOptimization};
use crate::game_logic::{GameLogicHandler, GameMessage, GameResponse};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, error, trace, warn};

/// 통합 메시지 핸들러
/// 통신 최적화와 게임 로직을 연결합니다.
pub struct UnifiedMessageHandler {
    message_processor: Arc<MessageProcessor>,
    game_logic: Arc<dyn GameLogicHandler>,
}

impl UnifiedMessageHandler {
    pub fn new(
        message_processor: Arc<MessageProcessor>,
        game_logic: Arc<dyn GameLogicHandler>,
    ) -> Self {
        Self {
            message_processor,
            game_logic,
        }
    }

    /// 제어 스트림 메시지 처리 (로그인, 방 관리 등)
    pub async fn process_control_message(
        &self,
        data: &[u8],
        player_id: Option<&str>,
    ) -> Result<Vec<u8>> {
        // 1. 통신 최적화: 압축 해제 및 시퀀스 검증
        let (sequence, payload) = self.message_processor.process_message(data).await?;

        // 2. JSON 메시지 파싱
        let mut message: GameMessage = serde_json::from_slice(&payload)?;
        message.sequence = sequence;
        message.player_id = player_id.map(|s| s.to_string());

        debug!(
            "Processing control message: {} (seq: {})",
            message.msg_type, sequence
        );

        // 3. 게임 로직 처리
        let mut response = self.route_to_game_logic(&message).await?;
        response.sequence = self.message_processor.next_sequence();

        // 4. 응답 직렬화 및 통신 최적화
        let response_bytes = serde_json::to_vec(&response)?;
        let optimized_response = self
            .message_processor
            .prepare_message(&response_bytes)
            .await?;

        trace!(
            "Control message processed: {} -> {} bytes",
            payload.len(),
            optimized_response.len()
        );
        Ok(optimized_response)
    }

    /// 게임 상태 스트림 처리 (실시간 이동, 공격 등)
    pub async fn process_game_state_message(
        &self,
        data: &[u8],
        player_id: &str,
    ) -> Result<Vec<u8>> {
        // 1. 통신 최적화 처리
        let (sequence, payload) = self.message_processor.process_message(data).await?;

        // 2. 게임 메시지 파싱
        let mut message: GameMessage = serde_json::from_slice(&payload)?;
        message.sequence = sequence;
        message.player_id = Some(player_id.to_string());

        debug!(
            "Processing game state message: {} from player {}",
            message.msg_type, player_id
        );

        // 3. 게임 로직 처리
        let mut response = match message.msg_type.as_str() {
            "player_move" => {
                self.game_logic
                    .handle_player_move(player_id, message.payload)
                    .await?
            }
            "player_attack" => {
                self.game_logic
                    .handle_player_attack(player_id, message.payload)
                    .await?
            }
            _ => self.route_to_game_logic(&message).await?,
        };
        response.sequence = self.message_processor.next_sequence();

        // 4. 응답 최적화
        let response_bytes = serde_json::to_vec(&response)?;
        let optimized_response = self
            .message_processor
            .prepare_message(&response_bytes)
            .await?;

        Ok(optimized_response)
    }

    /// 채팅 메시지 처리
    pub async fn process_chat_message(&self, data: &[u8], player_id: &str) -> Result<Vec<u8>> {
        let (sequence, payload) = self.message_processor.process_message(data).await?;

        let mut message: GameMessage = serde_json::from_slice(&payload)?;
        message.sequence = sequence;
        message.player_id = Some(player_id.to_string());

        debug!("Processing chat message from player {}", player_id);

        let mut response = self
            .game_logic
            .handle_chat(player_id, message.payload)
            .await?;
        response.sequence = self.message_processor.next_sequence();

        let response_bytes = serde_json::to_vec(&response)?;
        let optimized_response = self
            .message_processor
            .prepare_message(&response_bytes)
            .await?;

        Ok(optimized_response)
    }

    /// 음성 패킷 처리
    pub async fn process_voice_packet(&self, data: &[u8]) -> Result<Vec<u8>> {
        // 음성 데이터는 게임 로직 처리 없이 통신 최적화만 적용
        self.message_processor.process_voice_packet(data).await
    }

    /// 벌크 데이터 처리 (맵 데이터, 리소스 등)
    pub async fn process_bulk_data(&self, data: &[u8]) -> Result<()> {
        debug!("Processing bulk data: {} bytes", data.len());

        // 벌크 데이터는 청크로 분할하여 처리
        let chunks = self.message_processor.prepare_bulk_stream(data, 8192);

        // TODO: 각 청크를 적절한 스트림으로 전송
        for chunk in chunks {
            trace!(
                "Processing bulk chunk {}/{}",
                chunk.chunk_index + 1,
                chunk.total_chunks
            );
        }

        Ok(())
    }

    /// 단방향 데이터 처리 (텔레메트리, 로그 등)
    pub async fn process_unidirectional_data(&self, data: &[u8]) -> Result<()> {
        self.message_processor
            .process_unidirectional_data(data)
            .await
    }

    /// 게임 상태 델타 생성
    pub async fn create_game_state_delta(
        &self,
        previous_state: &[u8],
        current_state: &[u8],
    ) -> Result<Vec<u8>> {
        self.message_processor
            .create_state_delta(previous_state, current_state)
            .await
    }

    /// 게임 상태 델타 적용
    pub async fn apply_game_state_delta(
        &self,
        previous_state: &[u8],
        delta: &[u8],
    ) -> Result<Vec<u8>> {
        self.message_processor
            .apply_state_delta(previous_state, delta)
            .await
    }

    /// 플레이어 연결 해제 처리
    pub async fn handle_player_disconnected(&self, player_id: &str) -> Result<()> {
        self.game_logic.on_player_disconnected(player_id).await
    }

    /// 스트림 최적화 설정 가져오기
    pub fn get_stream_optimization(&self, stream_type: &str) -> StreamOptimization {
        match stream_type {
            "control" => StreamOptimization::control_stream(),
            "game_state" => StreamOptimization::game_state_stream(),
            "chat" => StreamOptimization::chat_stream(),
            "voice" => StreamOptimization::voice_stream(),
            "bulk" => StreamOptimization::bulk_stream(),
            _ => StreamOptimization::control_stream(), // 기본값
        }
    }

    // 내부 메소드: 메시지를 적절한 게임 로직 핸들러로 라우팅
    async fn route_to_game_logic(&self, message: &GameMessage) -> Result<GameResponse> {
        let player_id = message.player_id.as_deref().unwrap_or("unknown");

        match message.msg_type.as_str() {
            // 기본 세션 관리
            "login" => self.game_logic.handle_login(message.payload.clone()).await,
            "logout" => {
                self.game_logic
                    .handle_logout(player_id, message.payload.clone())
                    .await
            }

            // 방 관리
            "create_room" => {
                self.game_logic
                    .handle_create_room(player_id, message.payload.clone())
                    .await
            }
            "join_room" => {
                self.game_logic
                    .handle_join_room(player_id, message.payload.clone())
                    .await
            }
            "leave_room" => {
                self.game_logic
                    .handle_leave_room(player_id, message.payload.clone())
                    .await
            }

            // 핵심 게임 로직
            "player_move" => {
                self.game_logic
                    .handle_player_move(player_id, message.payload.clone())
                    .await
            }
            "player_attack" => {
                self.game_logic
                    .handle_player_attack(player_id, message.payload.clone())
                    .await
            }

            // 채팅
            "chat" => {
                self.game_logic
                    .handle_chat(player_id, message.payload.clone())
                    .await
            }

            // 커스텀 메시지
            _ => {
                warn!(
                    "Unknown message type: {}, routing to custom handler",
                    message.msg_type
                );
                self.game_logic
                    .handle_custom(&message.msg_type, player_id, message.payload.clone())
                    .await
            }
        }
    }
}

/// 스트림별 메시지 핸들링을 위한 헬퍼 구조체
pub struct StreamHandler {
    handler: Arc<UnifiedMessageHandler>,
    stream_type: String,
    optimization: StreamOptimization,
}

impl StreamHandler {
    pub fn new(handler: Arc<UnifiedMessageHandler>, stream_type: String) -> Self {
        let optimization = handler.get_stream_optimization(&stream_type);

        Self {
            handler,
            stream_type,
            optimization,
        }
    }

    pub async fn handle_message(&self, data: &[u8], player_id: Option<&str>) -> Result<Vec<u8>> {
        match self.stream_type.as_str() {
            "control" => self.handler.process_control_message(data, player_id).await,
            "game_state" => {
                let player_id = player_id
                    .ok_or_else(|| anyhow::anyhow!("Player ID required for game state messages"))?;
                self.handler
                    .process_game_state_message(data, player_id)
                    .await
            }
            "chat" => {
                let player_id = player_id
                    .ok_or_else(|| anyhow::anyhow!("Player ID required for chat messages"))?;
                self.handler.process_chat_message(data, player_id).await
            }
            "voice" => self.handler.process_voice_packet(data).await,
            _ => {
                error!("Unknown stream type: {}", self.stream_type);
                Err(anyhow::anyhow!(
                    "Unsupported stream type: {}",
                    self.stream_type
                ))
            }
        }
    }

    pub fn optimization(&self) -> &StreamOptimization {
        &self.optimization
    }
}

// UnifiedMessageHandler가 GameLogicHandler trait를 구현
#[async_trait]
impl GameLogicHandler for UnifiedMessageHandler {
    async fn handle_login(&self, payload: serde_json::Value) -> Result<GameResponse> {
        self.game_logic.handle_login(payload).await
    }

    async fn handle_logout(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> {
        self.game_logic.handle_logout(player_id, payload).await
    }

    async fn handle_create_room(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> {
        self.game_logic.handle_create_room(player_id, payload).await
    }

    async fn handle_join_room(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> {
        self.game_logic.handle_join_room(player_id, payload).await
    }

    async fn handle_leave_room(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> {
        self.game_logic.handle_leave_room(player_id, payload).await
    }

    async fn handle_player_move(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> {
        self.game_logic.handle_player_move(player_id, payload).await
    }

    async fn handle_player_attack(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> {
        self.game_logic.handle_player_attack(player_id, payload).await
    }

    async fn handle_chat(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> {
        self.game_logic.handle_chat(player_id, payload).await
    }

    async fn handle_custom(&self, msg_type: &str, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> {
        self.game_logic.handle_custom(msg_type, player_id, payload).await
    }

    async fn on_game_state_changed(
        &self,
        room_id: &str,
        players: &[crate::game_logic::PlayerInfo],
    ) -> Result<Vec<GameMessage>> {
        self.game_logic.on_game_state_changed(room_id, players).await
    }

    async fn on_player_disconnected(&self, player_id: &str) -> Result<()> {
        self.game_logic.on_player_disconnected(player_id).await
    }

    async fn process_control_message(&self, message: &[u8]) -> Result<Vec<u8>> {
        self.process_control_message(message, None).await
    }
}
