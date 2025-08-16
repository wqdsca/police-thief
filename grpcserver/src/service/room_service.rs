//! Room Service - Log-only implementation
//!
//! All business logic removed, only logging remains for debugging.

use async_trait::async_trait;
use shared::tool::error::AppError;
use shared::traits::{
    GameState, GameStateService, RoomData, RoomRedisService as RoomRedisServiceTrait,
};
use std::sync::Arc;
use tracing::info;

/// Room Service - Log-only implementation with concrete types
pub struct RoomService<R, G> 
where
    R: RoomRedisServiceTrait + Send + Sync,
    G: GameStateService + Send + Sync,
{
    #[allow(dead_code)]
    room_redis: Arc<R>,
    #[allow(dead_code)]
    game_state: Arc<G>,
}

impl<R, G> RoomService<R, G>
where
    R: RoomRedisServiceTrait + Send + Sync,
    G: GameStateService + Send + Sync,
{
    /// Create new RoomService with dependency injection
    pub fn new(
        room_redis: Arc<R>,
        game_state: Arc<G>,
    ) -> Self {
        info!("RoomService initialized with dependency injection");
        Self {
            room_redis,
            game_state,
        }
    }

    /// Create room - Log only
    pub async fn create_room(
        &self,
        user_id: i32,
        room_name: String,
        max_players: i32,
    ) -> Result<i64, AppError> {
        info!(
            "create_room called - user_id: {}, room_name: {}, max_players: {}",
            user_id, room_name, max_players
        );
        Ok(1)
    }

    /// Join room - Log only
    pub async fn join_room(&self, room_id: i64, user_id: i32) -> Result<(), AppError> {
        info!(
            "join_room called - room_id: {}, user_id: {}",
            room_id, user_id
        );
        Ok(())
    }

    /// Leave room - Log only
    pub async fn leave_room(&self, room_id: i64, user_id: i32) -> Result<(), AppError> {
        info!(
            "leave_room called - room_id: {}, user_id: {}",
            room_id, user_id
        );
        Ok(())
    }

    /// Get room info - Log only
    pub async fn get_room_info(&self, room_id: i64) -> Result<RoomData, AppError> {
        info!("get_room_info called - room_id: {}", room_id);

        Ok(RoomData {
            id: room_id,
            room_name: "test_room".to_string(),
            max_players: 10,
            current_players_num: 0,
            created_at: 0,
        })
    }

    /// Get room list - Log only
    pub async fn get_room_list(&self, index: i64) -> Result<Vec<RoomData>, AppError> {
        info!("get_room_list called - index: {}", index);
        Ok(Vec::new())
    }

    /// Start game - Log only
    pub async fn start_game(&self, room_id: i64) -> Result<(), AppError> {
        info!("start_game called - room_id: {}", room_id);
        Ok(())
    }

    /// End game - Log only
    pub async fn end_game(&self, room_id: i64) -> Result<(), AppError> {
        info!("end_game called - room_id: {}", room_id);
        Ok(())
    }

    /// Update room - Log only
    pub async fn update_room(&self, room_id: i64, room_data: RoomData) -> Result<(), AppError> {
        info!(
            "update_room called - room_id: {}, room_data: {:?}",
            room_id, room_data
        );
        Ok(())
    }

    /// Delete room - Log only
    pub async fn delete_room(&self, room_id: i64) -> Result<(), AppError> {
        info!("delete_room called - room_id: {}", room_id);
        Ok(())
    }
}

// ============================================================================
// Mock implementations for testing
// ============================================================================

/// Mock Room Redis Service - Log only
pub struct MockRoomRedisService;

#[async_trait]
impl RoomRedisServiceTrait for MockRoomRedisService {
    async fn create_room(&self, room_data: &RoomData) -> anyhow::Result<i64> {
        info!(
            "MockRoomRedisService::create_room - room_data: {:?}",
            room_data
        );
        Ok(1)
    }

    async fn get_room(&self, room_id: i64) -> anyhow::Result<Option<RoomData>> {
        info!("MockRoomRedisService::get_room - room_id: {}", room_id);
        Ok(None)
    }

    async fn update_room(&self, room_id: i64, room_data: &RoomData) -> anyhow::Result<()> {
        info!(
            "MockRoomRedisService::update_room - room_id: {}, room_data: {:?}",
            room_id, room_data
        );
        Ok(())
    }

    async fn delete_room(&self, room_id: i64) -> anyhow::Result<()> {
        info!("MockRoomRedisService::delete_room - room_id: {}", room_id);
        Ok(())
    }

    async fn get_room_list(&self, index: i64) -> anyhow::Result<Vec<RoomData>> {
        info!("MockRoomRedisService::get_room_list - index: {}", index);
        Ok(Vec::new())
    }

    async fn join_room(&self, room_id: i64, user_id: i64) -> anyhow::Result<()> {
        info!(
            "MockRoomRedisService::join_room - room_id: {}, user_id: {}",
            room_id, user_id
        );
        Ok(())
    }

    async fn leave_room(&self, room_id: i64, user_id: i64) -> anyhow::Result<()> {
        info!(
            "MockRoomRedisService::leave_room - room_id: {}, user_id: {}",
            room_id, user_id
        );
        Ok(())
    }
}

/// Mock Game State Service - Log only
pub struct MockGameStateService;

#[async_trait]
impl GameStateService for MockGameStateService {
    async fn initialize_game(&self, room_id: i64) -> anyhow::Result<()> {
        info!(
            "MockGameStateService::initialize_game - room_id: {}",
            room_id
        );
        Ok(())
    }

    async fn start_game(&self, room_id: i64) -> anyhow::Result<()> {
        info!("MockGameStateService::start_game - room_id: {}", room_id);
        Ok(())
    }

    async fn end_game(&self, room_id: i64) -> anyhow::Result<()> {
        info!("MockGameStateService::end_game - room_id: {}", room_id);
        Ok(())
    }

    async fn update_game_state(&self, room_id: i64, state: &GameState) -> anyhow::Result<()> {
        info!(
            "MockGameStateService::update_game_state - room_id: {}, state: {:?}",
            room_id, state
        );
        Ok(())
    }

    async fn get_game_state(&self, room_id: i64) -> anyhow::Result<Option<GameState>> {
        info!(
            "MockGameStateService::get_game_state - room_id: {}",
            room_id
        );
        Ok(None)
    }

    async fn process_player_action(
        &self,
        player_id: i64,
        action: &shared::traits::PlayerAction,
    ) -> anyhow::Result<()> {
        info!(
            "MockGameStateService::process_player_action - player_id: {}, action: {:?}",
            player_id, action
        );
        Ok(())
    }
}
