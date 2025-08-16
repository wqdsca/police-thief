//! Room Handler - Log-only implementation
//!
//! All business logic removed, only logging remains for debugging.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::service::{ConnectionService, MessageService};

/// Room information - minimal structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub room_id: u32,
    pub name: String,
}

/// Room handler - Log only
pub struct RoomHandler {
    connection_service: Arc<ConnectionService>,
    message_service: Arc<MessageService>,
}

impl RoomHandler {
    /// Create new room handler with dependency injection
    pub fn new(
        connection_service: Arc<ConnectionService>,
        message_service: Arc<MessageService>,
    ) -> Self {
        info!("RoomHandler initialized with dependency injection");
        Self {
            connection_service,
            message_service,
        }
    }

    /// Handle room join - Log only
    pub async fn handle_join_room(&self, user_id: u32, room_id: u32) -> Result<()> {
        info!(
            "handle_join_room called - user_id: {}, room_id: {}",
            user_id, room_id
        );
        Ok(())
    }

    /// Handle room leave - Log only
    pub async fn handle_leave_room(&self, user_id: u32, room_id: u32) -> Result<()> {
        info!(
            "handle_leave_room called - user_id: {}, room_id: {}",
            user_id, room_id
        );
        Ok(())
    }

    /// Handle room create - Log only
    pub async fn handle_create_room(&self, user_id: u32, room_name: String) -> Result<u32> {
        info!(
            "handle_create_room called - user_id: {}, room_name: {}",
            user_id, room_name
        );
        Ok(1) // Return dummy room id
    }

    /// Handle room delete - Log only
    pub async fn handle_delete_room(&self, room_id: u32) -> Result<()> {
        info!("handle_delete_room called - room_id: {}", room_id);
        Ok(())
    }

    /// Get room list - Log only
    pub async fn get_room_list(&self) -> Result<Vec<Room>> {
        info!("get_room_list called");
        Ok(Vec::new())
    }

    /// Get room info - Log only
    pub async fn get_room_info(&self, room_id: u32) -> Result<Option<Room>> {
        info!("get_room_info called - room_id: {}", room_id);
        Ok(None)
    }

    /// Update room - Log only
    pub async fn update_room(&self, room_id: u32, room_name: String) -> Result<()> {
        info!(
            "update_room called - room_id: {}, room_name: {}",
            room_id, room_name
        );
        Ok(())
    }

    /// Start game in room - Log only
    pub async fn start_game(&self, room_id: u32) -> Result<()> {
        info!("start_game called - room_id: {}", room_id);
        Ok(())
    }

    /// End game in room - Log only
    pub async fn end_game(&self, room_id: u32) -> Result<()> {
        info!("end_game called - room_id: {}", room_id);
        Ok(())
    }
}
