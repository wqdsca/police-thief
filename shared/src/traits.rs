//! Common trait definitions for dependency injection
//!
//! All services are defined as traits to enable:
//! - Dependency injection
//! - Easy testing with mock implementations
//! - Loose coupling between components

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// REDIS SERVICE TRAITS
// ============================================================================

/// Redis service trait for user operations
#[async_trait]
pub trait UserRedisService: Send + Sync {
    async fn get_user(&self, user_id: i64) -> Result<Option<UserData>>;
    async fn set_user(&self, user_id: i64, data: &UserData) -> Result<()>;
    async fn delete_user(&self, user_id: i64) -> Result<()>;
    async fn check_user_exists(&self, user_id: i64) -> Result<bool>;
}

/// Redis service trait for room operations
#[async_trait]
pub trait RoomRedisService: Send + Sync {
    async fn create_room(&self, room_data: &RoomData) -> Result<i64>;
    async fn get_room(&self, room_id: i64) -> Result<Option<RoomData>>;
    async fn update_room(&self, room_id: i64, room_data: &RoomData) -> Result<()>;
    async fn delete_room(&self, room_id: i64) -> Result<()>;
    async fn get_room_list(&self, index: i64) -> Result<Vec<RoomData>>;
    async fn join_room(&self, room_id: i64, user_id: i64) -> Result<()>;
    async fn leave_room(&self, room_id: i64, user_id: i64) -> Result<()>;
}

// ============================================================================
// DATABASE SERVICE TRAITS
// ============================================================================

/// Database service trait for user operations
#[async_trait]
pub trait UserDatabaseService: Send + Sync {
    async fn create_user(&self, user: &UserData) -> Result<i64>;
    async fn get_user_by_id(&self, user_id: i64) -> Result<Option<UserData>>;
    async fn get_user_by_social_id(&self, provider: &str, social_id: &str) -> Result<Option<UserData>>;
    async fn update_user(&self, user_id: i64, user: &UserData) -> Result<()>;
    async fn delete_user(&self, user_id: i64) -> Result<()>;
}



// ============================================================================
// GAME SERVICE TRAITS
// ============================================================================

/// Game state management trait
#[async_trait]
pub trait GameStateService: Send + Sync {
    async fn initialize_game(&self, room_id: i64) -> Result<()>;
    async fn start_game(&self, room_id: i64) -> Result<()>;
    async fn end_game(&self, room_id: i64) -> Result<()>;
    async fn update_game_state(&self, room_id: i64, state: &GameState) -> Result<()>;
    async fn get_game_state(&self, room_id: i64) -> Result<Option<GameState>>;
    async fn process_player_action(&self, player_id: i64, action: &PlayerAction) -> Result<()>;
}

/// Network handler trait
#[async_trait]
pub trait NetworkHandler: Send + Sync {
    async fn send_message(&self, player_id: i64, message: &NetworkMessage) -> Result<()>;
    async fn broadcast_message(&self, room_id: i64, message: &NetworkMessage) -> Result<()>;
    async fn handle_connection(&self, player_id: i64) -> Result<()>;
    async fn handle_disconnection(&self, player_id: i64) -> Result<()>;
}

// ============================================================================
// AUTHENTICATION SERVICE TRAITS
// ============================================================================

/// Social authentication service trait
#[async_trait]
pub trait SocialAuthService: Send + Sync {
    async fn verify_social_token(&self, provider: &str, token: &str) -> Result<SocialUserInfo>;
    async fn create_jwt_token(&self, user_id: i64) -> Result<AuthToken>;
    async fn validate_jwt_token(&self, token: &str) -> Result<TokenValidation>;
    async fn refresh_jwt_token(&self, token: &str) -> Result<AuthToken>;
}

// ============================================================================
// MONITORING SERVICE TRAITS
// ============================================================================

/// Performance monitoring trait
#[async_trait]
pub trait PerformanceMonitor: Send + Sync {
    async fn record_metric(&self, metric: &Metric) -> Result<()>;
    async fn get_metrics(&self) -> Result<MetricsReport>;
    async fn start_monitoring(&self) -> Result<()>;
    async fn stop_monitoring(&self) -> Result<()>;
}

// ============================================================================
// DATA STRUCTURES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserData {
    pub id: i64,
    pub nickname: String,
    pub social_provider: Option<String>, // "google", "apple", "kakao", etc.
    pub social_id: Option<String>, // Social provider's user ID
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomData {
    pub id: i64,
    pub room_name: String,
    pub max_players: i32,
    pub current_players_num: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub room_id: i64,
    pub status: String,
    pub players: Vec<i64>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerAction {
    pub player_id: i64,
    pub action_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMessage {
    pub message_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialLoginCredentials {
    pub provider: String, // "google", "apple", "kakao", etc.
    pub access_token: String, // OAuth access token from provider
    pub provider_user_id: Option<String>, // Optional: user ID from provider
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenValidation {
    pub is_valid: bool,
    pub user_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsReport {
    pub metrics: Vec<Metric>,
}

/// Social user information from OAuth provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialUserInfo {
    pub provider_user_id: String,
    pub nickname: Option<String>,
    pub avatar_url: Option<String>,
    pub provider: String,
}

// ============================================================================
// DEPENDENCY INJECTION CONTAINER
// ============================================================================

/// Service container for dependency injection
pub struct ServiceContainer {
    pub user_redis: Arc<dyn UserRedisService>,
    pub room_redis: Arc<dyn RoomRedisService>,
    pub user_db: Arc<dyn UserDatabaseService>,
    pub game_state: Arc<dyn GameStateService>,
    pub network: Arc<dyn NetworkHandler>,
    pub social_auth: Arc<dyn SocialAuthService>,
    pub monitor: Arc<dyn PerformanceMonitor>,
}

impl ServiceContainer {
    /// Create a new service container with all dependencies
    pub fn new(
        user_redis: Arc<dyn UserRedisService>,
        room_redis: Arc<dyn RoomRedisService>,
        user_db: Arc<dyn UserDatabaseService>,
        game_state: Arc<dyn GameStateService>,
        network: Arc<dyn NetworkHandler>,
        social_auth: Arc<dyn SocialAuthService>,
        monitor: Arc<dyn PerformanceMonitor>,
    ) -> Self {
        Self {
            user_redis,
            room_redis,
            user_db,
            game_state,
            network,
            social_auth,
            monitor,
        }
    }
}

// ============================================================================
// FACTORY TRAIT
// ============================================================================

/// Factory trait for creating service instances
#[async_trait]
pub trait ServiceFactory: Send + Sync {
    async fn create_user_redis(&self) -> Result<Arc<dyn UserRedisService>>;
    async fn create_room_redis(&self) -> Result<Arc<dyn RoomRedisService>>;
    async fn create_user_db(&self) -> Result<Arc<dyn UserDatabaseService>>;
    async fn create_game_state(&self) -> Result<Arc<dyn GameStateService>>;
    async fn create_network(&self) -> Result<Arc<dyn NetworkHandler>>;
    async fn create_social_auth(&self) -> Result<Arc<dyn SocialAuthService>>;
    async fn create_monitor(&self) -> Result<Arc<dyn PerformanceMonitor>>;

    /// Create complete service container
    async fn create_container(&self) -> Result<ServiceContainer> {
        Ok(ServiceContainer::new(
            self.create_user_redis().await?,
            self.create_room_redis().await?,
            self.create_user_db().await?,
            self.create_game_state().await?,
            self.create_network().await?,
            self.create_social_auth().await?,
            self.create_monitor().await?,
        ))
    }
}
