//! Service Factory - Dependency Injection Implementation
//!
//! Factory pattern for creating and injecting service dependencies.

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::traits::*;

/// Default Service Factory - Creates mock implementations
pub struct DefaultServiceFactory;

impl Default for DefaultServiceFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultServiceFactory {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ServiceFactory for DefaultServiceFactory {
    async fn create_user_redis(&self) -> Result<Arc<dyn UserRedisService>> {
        Ok(Arc::new(MockUserRedisService))
    }

    async fn create_room_redis(&self) -> Result<Arc<dyn RoomRedisService>> {
        Ok(Arc::new(MockRoomRedisService))
    }

    async fn create_user_db(&self) -> Result<Arc<dyn UserDatabaseService>> {
        Ok(Arc::new(MockUserDatabaseService))
    }

    async fn create_game_state(&self) -> Result<Arc<dyn GameStateService>> {
        Ok(Arc::new(MockGameStateService))
    }

    async fn create_network(&self) -> Result<Arc<dyn NetworkHandler>> {
        Ok(Arc::new(MockNetworkHandler))
    }

    async fn create_social_auth(&self) -> Result<Arc<dyn SocialAuthService>> {
        Ok(Arc::new(MockSocialAuthService))
    }

    async fn create_monitor(&self) -> Result<Arc<dyn PerformanceMonitor>> {
        Ok(Arc::new(MockPerformanceMonitor))
    }
}

// ============================================================================
// Mock Implementations - Log only
// ============================================================================

struct MockUserRedisService;

#[async_trait]
impl UserRedisService for MockUserRedisService {
    async fn get_user(&self, user_id: i64) -> Result<Option<UserData>> {
        tracing::info!("MockUserRedisService::get_user - user_id: {}", user_id);
        Ok(None)
    }

    async fn set_user(&self, user_id: i64, _data: &UserData) -> Result<()> {
        tracing::info!("MockUserRedisService::set_user - user_id: {}", user_id);
        Ok(())
    }

    async fn delete_user(&self, user_id: i64) -> Result<()> {
        tracing::info!("MockUserRedisService::delete_user - user_id: {}", user_id);
        Ok(())
    }

    async fn check_user_exists(&self, user_id: i64) -> Result<bool> {
        tracing::info!(
            "MockUserRedisService::check_user_exists - user_id: {}",
            user_id
        );
        Ok(false)
    }
}

struct MockRoomRedisService;

#[async_trait]
impl RoomRedisService for MockRoomRedisService {
    async fn create_room(&self, _room_data: &RoomData) -> Result<i64> {
        tracing::info!("MockRoomRedisService::create_room");
        Ok(1)
    }

    async fn get_room(&self, room_id: i64) -> Result<Option<RoomData>> {
        tracing::info!("MockRoomRedisService::get_room - room_id: {}", room_id);
        Ok(None)
    }

    async fn update_room(&self, room_id: i64, _room_data: &RoomData) -> Result<()> {
        tracing::info!("MockRoomRedisService::update_room - room_id: {}", room_id);
        Ok(())
    }

    async fn delete_room(&self, room_id: i64) -> Result<()> {
        tracing::info!("MockRoomRedisService::delete_room - room_id: {}", room_id);
        Ok(())
    }

    async fn get_room_list(&self, index: i64) -> Result<Vec<RoomData>> {
        tracing::info!("MockRoomRedisService::get_room_list - index: {}", index);
        Ok(Vec::new())
    }

    async fn join_room(&self, room_id: i64, user_id: i64) -> Result<()> {
        tracing::info!(
            "MockRoomRedisService::join_room - room_id: {}, user_id: {}",
            room_id,
            user_id
        );
        Ok(())
    }

    async fn leave_room(&self, room_id: i64, user_id: i64) -> Result<()> {
        tracing::info!(
            "MockRoomRedisService::leave_room - room_id: {}, user_id: {}",
            room_id,
            user_id
        );
        Ok(())
    }
}

struct MockUserDatabaseService;

#[async_trait]
impl UserDatabaseService for MockUserDatabaseService {
    async fn create_user(&self, _user: &UserData) -> Result<i64> {
        tracing::info!("MockUserDatabaseService::create_user");
        Ok(1)
    }

    async fn get_user_by_id(&self, user_id: i64) -> Result<Option<UserData>> {
        tracing::info!(
            "MockUserDatabaseService::get_user_by_id - user_id: {}",
            user_id
        );
        Ok(None)
    }

    async fn get_user_by_social_id(&self, provider: &str, social_id: &str) -> Result<Option<UserData>> {
        tracing::info!(
            "MockUserDatabaseService::get_user_by_social_id - provider: {}, social_id: {}",
            provider, social_id
        );
        Ok(None)
    }

    async fn update_user(&self, user_id: i64, _user: &UserData) -> Result<()> {
        tracing::info!(
            "MockUserDatabaseService::update_user - user_id: {}",
            user_id
        );
        Ok(())
    }

    async fn delete_user(&self, user_id: i64) -> Result<()> {
        tracing::info!(
            "MockUserDatabaseService::delete_user - user_id: {}",
            user_id
        );
        Ok(())
    }
}

struct MockGameStateService;

#[async_trait]
impl GameStateService for MockGameStateService {
    async fn initialize_game(&self, room_id: i64) -> Result<()> {
        tracing::info!(
            "MockGameStateService::initialize_game - room_id: {}",
            room_id
        );
        Ok(())
    }

    async fn start_game(&self, room_id: i64) -> Result<()> {
        tracing::info!("MockGameStateService::start_game - room_id: {}", room_id);
        Ok(())
    }

    async fn end_game(&self, room_id: i64) -> Result<()> {
        tracing::info!("MockGameStateService::end_game - room_id: {}", room_id);
        Ok(())
    }

    async fn update_game_state(&self, room_id: i64, _state: &GameState) -> Result<()> {
        tracing::info!(
            "MockGameStateService::update_game_state - room_id: {}",
            room_id
        );
        Ok(())
    }

    async fn get_game_state(&self, room_id: i64) -> Result<Option<GameState>> {
        tracing::info!(
            "MockGameStateService::get_game_state - room_id: {}",
            room_id
        );
        Ok(None)
    }

    async fn process_player_action(&self, player_id: i64, _action: &PlayerAction) -> Result<()> {
        tracing::info!(
            "MockGameStateService::process_player_action - player_id: {}",
            player_id
        );
        Ok(())
    }
}

struct MockNetworkHandler;

#[async_trait]
impl NetworkHandler for MockNetworkHandler {
    async fn send_message(&self, player_id: i64, _message: &NetworkMessage) -> Result<()> {
        tracing::info!(
            "MockNetworkHandler::send_message - player_id: {}",
            player_id
        );
        Ok(())
    }

    async fn broadcast_message(&self, room_id: i64, _message: &NetworkMessage) -> Result<()> {
        tracing::info!(
            "MockNetworkHandler::broadcast_message - room_id: {}",
            room_id
        );
        Ok(())
    }

    async fn handle_connection(&self, player_id: i64) -> Result<()> {
        tracing::info!(
            "MockNetworkHandler::handle_connection - player_id: {}",
            player_id
        );
        Ok(())
    }

    async fn handle_disconnection(&self, player_id: i64) -> Result<()> {
        tracing::info!(
            "MockNetworkHandler::handle_disconnection - player_id: {}",
            player_id
        );
        Ok(())
    }
}

struct MockSocialAuthService;

#[async_trait]
impl SocialAuthService for MockSocialAuthService {
    async fn verify_social_token(&self, provider: &str, token: &str) -> Result<SocialUserInfo> {
        tracing::info!("MockSocialAuthService::verify_social_token - provider: {}, token: {}", provider, token);
        Ok(SocialUserInfo {
            provider_user_id: "mock_social_123".to_string(),
            nickname: Some("Mock User".to_string()),
            avatar_url: None,
            provider: provider.to_string(),
        })
    }

    async fn create_jwt_token(&self, user_id: i64) -> Result<AuthToken> {
        tracing::info!("MockSocialAuthService::create_jwt_token - user_id: {}", user_id);
        Ok(AuthToken {
            token: "mock_jwt_token".to_string(),
            expires_at: chrono::Utc::now().timestamp() + 3600,
        })
    }

    async fn validate_jwt_token(&self, token: &str) -> Result<TokenValidation> {
        tracing::info!("MockSocialAuthService::validate_jwt_token - token: {}", token);
        Ok(TokenValidation {
            is_valid: true,
            user_id: Some(1),
        })
    }

    async fn refresh_jwt_token(&self, token: &str) -> Result<AuthToken> {
        tracing::info!("MockSocialAuthService::refresh_jwt_token - token: {}", token);
        Ok(AuthToken {
            token: "new_mock_jwt_token".to_string(),
            expires_at: chrono::Utc::now().timestamp() + 3600,
        })
    }
}

struct MockPerformanceMonitor;

#[async_trait]
impl PerformanceMonitor for MockPerformanceMonitor {
    async fn record_metric(&self, metric: &Metric) -> Result<()> {
        tracing::info!(
            "MockPerformanceMonitor::record_metric - {}: {}",
            metric.name,
            metric.value
        );
        Ok(())
    }

    async fn get_metrics(&self) -> Result<MetricsReport> {
        tracing::info!("MockPerformanceMonitor::get_metrics");
        Ok(MetricsReport {
            metrics: Vec::new(),
        })
    }

    async fn start_monitoring(&self) -> Result<()> {
        tracing::info!("MockPerformanceMonitor::start_monitoring");
        Ok(())
    }

    async fn stop_monitoring(&self) -> Result<()> {
        tracing::info!("MockPerformanceMonitor::stop_monitoring");
        Ok(())
    }
}
