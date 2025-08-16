//! User Service - Log-only implementation
//!
//! All business logic removed, only logging remains for debugging.

use async_trait::async_trait;
use shared::tool::error::AppError;
use shared::traits::{
    SocialAuthService, AuthToken, SocialLoginCredentials, TokenValidation, UserData, UserDatabaseService,
    UserRedisService as UserRedisServiceTrait, SocialUserInfo,
};
use std::sync::Arc;
use tracing::info;

/// User Service - Social login only implementation with concrete types
pub struct UserService<S, R, D> 
where
    S: SocialAuthService + Send + Sync,
    R: UserRedisServiceTrait + Send + Sync,
    D: UserDatabaseService + Send + Sync,
{
    #[allow(dead_code)]
    social_auth_service: Arc<S>,
    #[allow(dead_code)]
    user_redis: Arc<R>,
    #[allow(dead_code)]
    user_db: Arc<D>,
}

impl<S, R, D> UserService<S, R, D>
where
    S: SocialAuthService + Send + Sync,
    R: UserRedisServiceTrait + Send + Sync,
    D: UserDatabaseService + Send + Sync,
{
    /// Create new UserService with dependency injection
    pub fn new(
        social_auth_service: Arc<S>,
        user_redis: Arc<R>,
        user_db: Arc<D>,
    ) -> Self {
        info!("UserService initialized with social authentication only");
        Self {
            social_auth_service,
            user_redis,
            user_db,
        }
    }

    /// Handle social login only
    pub async fn login_user(
        &self,
        login_type: String,
        login_token: String,
    ) -> Result<(i32, String, String, bool), AppError> {
        info!(
            "social_login called - provider: {}, token: {}",
            login_type, login_token
        );
        
        // 1. Verify social token with provider
        let social_user_info = self.social_auth_service
            .verify_social_token(&login_type, &login_token)
            .await
            .map_err(|e| AppError::InvalidInput(format!("Social token verification failed: {}", e)))?;
        
        // 2. Check if user exists by social ID
        let existing_user = self.user_db
            .get_user_by_social_id(&login_type, &social_user_info.provider_user_id)
            .await
            .map_err(|e| AppError::DatabaseQuery(e.to_string()))?;
        
        let (user_id, nickname, is_new_user) = if let Some(user) = existing_user {
            // Existing user
            (user.id, user.nickname, false)
        } else {
            // Create new user
            let new_user = UserData {
                id: 0, // Will be set by database
                nickname: social_user_info.nickname.unwrap_or_else(|| format!("User_{}", social_user_info.provider_user_id)),
                social_provider: Some(login_type.clone()),
                social_id: Some(social_user_info.provider_user_id.clone()),
                created_at: chrono::Utc::now().timestamp(),
            };
            
            let user_id = self.user_db
                .create_user(&new_user)
                .await
                .map_err(|e| AppError::DatabaseQuery(e.to_string()))?;
                
            (user_id, new_user.nickname, true)
        };
        
        // 3. Generate JWT token for the user
        let auth_token = self.social_auth_service
            .create_jwt_token(user_id)
            .await
            .map_err(|e| AppError::InternalError(format!("JWT creation failed: {}", e)))?;
        
        Ok((user_id as i32, nickname, auth_token.token, is_new_user))
    }

    /// Handle social login - Log only
    pub async fn social_login(
        &self,
        login_type: String,
        login_token: String,
    ) -> Result<bool, AppError> {
        info!(
            "social_login called - type: {}, token: {}",
            login_type, login_token
        );
        match login_type.as_str() {
            "Google" => {
                // Google OAuth 토큰 검증
                info!("Google login requested - validating OAuth token");
                // 실제 구현은 GameCenter의 social_auth_service를 통해 처리됨
                // 여기서는 토큰 존재 여부만 확인
                Ok(!login_token.is_empty())
            }
            "Kakao" => {
                // Kakao OAuth 토큰 검증
                info!("Kakao login requested - validating OAuth token");
                // 실제 구현은 GameCenter의 social_auth_service를 통해 처리됨
                Ok(!login_token.is_empty())
            }
            "Apple" => {
                // Apple OAuth 토큰 검증
                info!("Apple login requested - validating OAuth token");
                // 실제 구현은 GameCenter의 social_auth_service를 통해 처리됨
                Ok(!login_token.is_empty())
            }
            _ => {
                info!("Unknown login type: {}", login_type);
                Ok(false)
            }
        }
    }

    /// Handle user registration - Log only
    pub async fn register_user(&self, nick_name: String) -> Result<i32, AppError> {
        info!("register_user called - nickname: {}", nick_name);
        Ok(1)
    }

    /// Get user info - Social login only
    pub async fn get_user_info(&self, user_id: i32) -> Result<UserData, AppError> {
        info!("get_user_info called - user_id: {}", user_id);

        Ok(UserData {
            id: user_id as i64,
            nickname: "test_user".to_string(),
            social_provider: Some("google".to_string()),
            social_id: Some("123456789".to_string()),
            created_at: chrono::Utc::now().timestamp(),
        })
    }

    /// Update user - Log only
    pub async fn update_user(&self, user_id: i32, data: UserData) -> Result<(), AppError> {
        info!(
            "update_user called - user_id: {}, data: {:?}",
            user_id, data
        );
        Ok(())
    }

    /// Delete user - Log only
    pub async fn delete_user(&self, user_id: i32) -> Result<(), AppError> {
        info!("delete_user called - user_id: {}", user_id);
        Ok(())
    }

    /// Logout - Log only
    pub async fn logout(&self, token: String) -> Result<(), AppError> {
        info!("logout called - token: {}", token);
        Ok(())
    }

    /// Validate token - Log only
    pub async fn validate_token(&self, token: String) -> Result<bool, AppError> {
        info!("validate_token called - token: {}", token);
        Ok(true)
    }
}

// ============================================================================
// Type aliases for convenience
// ============================================================================

/// Default UserService with Mock implementations
pub type MockUserService = UserService<MockSocialAuthService, MockUserRedisService, MockUserDatabaseService>;

impl MockUserService {
    /// Create a default UserService with mock implementations
    pub fn with_mocks() -> Self {
        Self::new(
            Arc::new(MockSocialAuthService),
            Arc::new(MockUserRedisService),
            Arc::new(MockUserDatabaseService),
        )
    }
}

// ============================================================================
// Mock implementations for testing
// ============================================================================

/// Mock Social Auth Service - Log only
pub struct MockSocialAuthService;

#[async_trait]
impl SocialAuthService for MockSocialAuthService {
    async fn verify_social_token(&self, provider: &str, token: &str) -> anyhow::Result<SocialUserInfo> {
        info!("MockSocialAuthService::verify_social_token - provider: {}, token: {}", provider, token);
        Ok(SocialUserInfo {
            provider_user_id: "mock_social_123".to_string(),
            nickname: Some("Mock User".to_string()),
            avatar_url: None,
            provider: provider.to_string(),
        })
    }

    async fn create_jwt_token(&self, user_id: i64) -> anyhow::Result<AuthToken> {
        info!("MockSocialAuthService::create_jwt_token - user_id: {}", user_id);
        Ok(AuthToken {
            token: "mock_jwt_token".to_string(),
            expires_at: chrono::Utc::now().timestamp() + 3600,
        })
    }

    async fn validate_jwt_token(&self, token: &str) -> anyhow::Result<TokenValidation> {
        info!("MockSocialAuthService::validate_jwt_token - token: {}", token);
        Ok(TokenValidation {
            is_valid: true,
            user_id: Some(1),
        })
    }

    async fn refresh_jwt_token(&self, token: &str) -> anyhow::Result<AuthToken> {
        info!("MockSocialAuthService::refresh_jwt_token - token: {}", token);
        Ok(AuthToken {
            token: "new_mock_jwt_token".to_string(),
            expires_at: chrono::Utc::now().timestamp() + 3600,
        })
    }
}

/// Mock User Redis Service - Log only
pub struct MockUserRedisService;

#[async_trait]
impl UserRedisServiceTrait for MockUserRedisService {
    async fn get_user(&self, user_id: i64) -> anyhow::Result<Option<UserData>> {
        info!("MockUserRedisService::get_user - user_id: {}", user_id);
        Ok(None)
    }

    async fn set_user(&self, user_id: i64, data: &UserData) -> anyhow::Result<()> {
        info!(
            "MockUserRedisService::set_user - user_id: {}, data: {:?}",
            user_id, data
        );
        Ok(())
    }

    async fn delete_user(&self, user_id: i64) -> anyhow::Result<()> {
        info!("MockUserRedisService::delete_user - user_id: {}", user_id);
        Ok(())
    }

    async fn check_user_exists(&self, user_id: i64) -> anyhow::Result<bool> {
        info!(
            "MockUserRedisService::check_user_exists - user_id: {}",
            user_id
        );
        Ok(false)
    }
}

/// Mock User Database Service - Log only
pub struct MockUserDatabaseService;

#[async_trait]
impl UserDatabaseService for MockUserDatabaseService {
    async fn create_user(&self, user: &UserData) -> anyhow::Result<i64> {
        info!("MockUserDatabaseService::create_user - user: {:?}", user);
        Ok(1)
    }

    async fn get_user_by_id(&self, user_id: i64) -> anyhow::Result<Option<UserData>> {
        info!(
            "MockUserDatabaseService::get_user_by_id - user_id: {}",
            user_id
        );
        Ok(None)
    }

    async fn get_user_by_social_id(&self, provider: &str, social_id: &str) -> anyhow::Result<Option<UserData>> {
        info!(
            "MockUserDatabaseService::get_user_by_social_id - provider: {}, social_id: {}",
            provider, social_id
        );
        Ok(None)
    }

    async fn update_user(&self, user_id: i64, user: &UserData) -> anyhow::Result<()> {
        info!(
            "MockUserDatabaseService::update_user - user_id: {}, user: {:?}",
            user_id, user
        );
        Ok(())
    }

    async fn delete_user(&self, user_id: i64) -> anyhow::Result<()> {
        info!(
            "MockUserDatabaseService::delete_user - user_id: {}",
            user_id
        );
        Ok(())
    }
}
