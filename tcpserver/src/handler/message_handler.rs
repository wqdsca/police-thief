//! Message Handler - Log-only implementation
//!
//! All business logic removed, only logging remains for debugging.

use anyhow::Result;
use std::sync::Arc;
use tracing::info;

use crate::protocol::GameMessage;
use crate::service::{ConnectionService, HeartbeatService, MessageService};

/// Server message handler - Log only
pub struct ServerMessageHandler {
    connection_service: Arc<ConnectionService>,
    heartbeat_service: Arc<HeartbeatService>,
    message_service: Arc<MessageService>,
}

impl ServerMessageHandler {
    /// Create new message handler with dependency injection
    pub fn new(
        connection_service: Arc<ConnectionService>,
        heartbeat_service: Arc<HeartbeatService>,
        message_service: Arc<MessageService>,
    ) -> Self {
        info!("ServerMessageHandler initialized with dependency injection");
        Self {
            connection_service,
            heartbeat_service,
            message_service,
        }
    }

    /// Handle incoming message - Log only
    pub async fn handle_message(&self, player_id: i64, message: GameMessage) -> Result<()> {
        info!(
            "handle_message called - player_id: {}, message: {:?}",
            player_id, message
        );
        Ok(())
    }

    /// Register all message handlers - Log only
    pub async fn register_all_handlers(&self) -> Result<()> {
        info!("register_all_handlers called");
        Ok(())
    }

    /// Handle heartbeat - Log only
    pub async fn handle_heartbeat(&self, player_id: i64) -> Result<()> {
        info!("handle_heartbeat called - player_id: {}", player_id);
        Ok(())
    }

    /// Handle echo - Log only
    pub async fn handle_echo(&self, player_id: i64, data: String) -> Result<()> {
        info!(
            "handle_echo called - player_id: {}, data: {}",
            player_id, data
        );
        Ok(())
    }

    /// Handle room message - Log only
    pub async fn handle_room_message(
        &self,
        player_id: i64,
        room_id: i64,
        message: String,
    ) -> Result<()> {
        info!(
            "handle_room_message called - player_id: {}, room_id: {}, message: {}",
            player_id, room_id, message
        );
        Ok(())
    }

    /// Handle game action - Log only
    pub async fn handle_game_action(&self, player_id: i64, action: String) -> Result<()> {
        info!(
            "handle_game_action called - player_id: {}, action: {}",
            player_id, action
        );
        Ok(())
    }

    /// Broadcast message - Log only
    pub async fn broadcast_message(&self, room_id: i64, message: String) -> Result<()> {
        info!(
            "broadcast_message called - room_id: {}, message: {}",
            room_id, message
        );
        Ok(())
    }

    /// Send to player - Log only
    pub async fn send_to_player(&self, player_id: i64, message: String) -> Result<()> {
        info!(
            "send_to_player called - player_id: {}, message: {}",
            player_id, message
        );
        Ok(())
    }
}
