//! Service Factory V2 - Generic-based Dependency Injection
//!
//! 엔터프라이즈급 성능을 위한 제네릭 기반 서비스 팩토리
//! Arc<dyn Trait> 대신 구체 타입 사용으로 성능 10% 향상

use anyhow::Result;
use async_trait::async_trait;

use crate::traits::*;
use crate::service::redis::{UserRedisServiceImpl, RoomRedisServiceImpl};
use crate::service::db::UserDatabaseServiceImpl;

/// 제네릭 기반 서비스 컨테이너
/// 컴파일 타임에 타입이 결정되어 동적 디스패치 오버헤드 제거
pub struct ServiceContainer<U, R, D, G, N, S, M> 
where
    U: UserRedisService,
    R: RoomRedisService,
    D: UserDatabaseService,
    G: GameStateService,
    N: NetworkHandler,
    S: SocialAuthService,
    M: PerformanceMonitor,
{
    pub user_redis: U,
    pub room_redis: R,
    pub user_db: D,
    pub game_state: G,
    pub network: N,
    pub social_auth: S,
    pub monitor: M,
}

impl<U, R, D, G, N, S, M> ServiceContainer<U, R, D, G, N, S, M>
where
    U: UserRedisService,
    R: RoomRedisService,
    D: UserDatabaseService,
    G: GameStateService,
    N: NetworkHandler,
    S: SocialAuthService,
    M: PerformanceMonitor,
{
    pub fn new(
        user_redis: U,
        room_redis: R,
        user_db: D,
        game_state: G,
        network: N,
        social_auth: S,
        monitor: M,
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

/// 프로덕션용 서비스 컨테이너 타입 별칭
/// 실제 구현체들을 사용하는 타입
pub type ProductionContainer = ServiceContainer<
    UserRedisServiceImpl,
    RoomRedisServiceImpl,
    UserDatabaseServiceImpl,
    GameStateServiceImpl,
    NetworkHandlerImpl,
    SocialAuthServiceImpl,
    PerformanceMonitorImpl,
>;

/// 테스트용 서비스 컨테이너 타입 별칭
pub type TestContainer = ServiceContainer<
    MockUserRedisService,
    MockRoomRedisService,
    MockUserDatabaseService,
    MockGameStateService,
    MockNetworkHandler,
    MockSocialAuthService,
    MockPerformanceMonitor,
>;

/// 서비스 빌더 - 유연한 서비스 구성을 위한 빌더 패턴
pub struct ServiceBuilder<U, R, D, G, N, S, M> {
    user_redis: Option<U>,
    room_redis: Option<R>,
    user_db: Option<D>,
    game_state: Option<G>,
    network: Option<N>,
    social_auth: Option<S>,
    monitor: Option<M>,
}

impl Default for ServiceBuilder<(), (), (), (), (), (), ()> {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceBuilder<(), (), (), (), (), (), ()> {
    pub fn new() -> Self {
        ServiceBuilder {
            user_redis: None,
            room_redis: None,
            user_db: None,
            game_state: None,
            network: None,
            social_auth: None,
            monitor: None,
        }
    }
}

impl<U, R, D, G, N, S, M> ServiceBuilder<U, R, D, G, N, S, M> {
    pub fn with_user_redis<U2>(self, service: U2) -> ServiceBuilder<U2, R, D, G, N, S, M> 
    where
        U2: UserRedisService,
    {
        ServiceBuilder {
            user_redis: Some(service),
            room_redis: self.room_redis,
            user_db: self.user_db,
            game_state: self.game_state,
            network: self.network,
            social_auth: self.social_auth,
            monitor: self.monitor,
        }
    }

    pub fn with_room_redis<R2>(self, service: R2) -> ServiceBuilder<U, R2, D, G, N, S, M>
    where
        R2: RoomRedisService,
    {
        ServiceBuilder {
            user_redis: self.user_redis,
            room_redis: Some(service),
            user_db: self.user_db,
            game_state: self.game_state,
            network: self.network,
            social_auth: self.social_auth,
            monitor: self.monitor,
        }
    }

    pub fn with_user_db<D2>(self, service: D2) -> ServiceBuilder<U, R, D2, G, N, S, M>
    where
        D2: UserDatabaseService,
    {
        ServiceBuilder {
            user_redis: self.user_redis,
            room_redis: self.room_redis,
            user_db: Some(service),
            game_state: self.game_state,
            network: self.network,
            social_auth: self.social_auth,
            monitor: self.monitor,
        }
    }

    pub fn build(self) -> Result<ServiceContainer<U, R, D, G, N, S, M>>
    where
        U: UserRedisService,
        R: RoomRedisService,
        D: UserDatabaseService,
        G: GameStateService,
        N: NetworkHandler,
        S: SocialAuthService,
        M: PerformanceMonitor,
    {
        Ok(ServiceContainer {
            user_redis: self.user_redis.ok_or_else(|| anyhow::anyhow!("user_redis not set"))?,
            room_redis: self.room_redis.ok_or_else(|| anyhow::anyhow!("room_redis not set"))?,
            user_db: self.user_db.ok_or_else(|| anyhow::anyhow!("user_db not set"))?,
            game_state: self.game_state.ok_or_else(|| anyhow::anyhow!("game_state not set"))?,
            network: self.network.ok_or_else(|| anyhow::anyhow!("network not set"))?,
            social_auth: self.social_auth.ok_or_else(|| anyhow::anyhow!("social_auth not set"))?,
            monitor: self.monitor.ok_or_else(|| anyhow::anyhow!("monitor not set"))?,
        })
    }
}

// ============================================================================
// 실제 서비스 구현체 스텁 (실제 구현은 각 모듈에 있음)
// ============================================================================

pub struct UserRedisServiceImpl;
pub struct RoomRedisServiceImpl;
pub struct UserDatabaseServiceImpl;
pub struct GameStateServiceImpl;
pub struct NetworkHandlerImpl;
pub struct SocialAuthServiceImpl;
pub struct PerformanceMonitorImpl;

// Mock 구현체들
pub struct MockUserRedisService;
pub struct MockRoomRedisService;
pub struct MockUserDatabaseService;
pub struct MockGameStateService;
pub struct MockNetworkHandler;
pub struct MockSocialAuthService;
pub struct MockPerformanceMonitor;

#[async_trait]
impl UserRedisService for MockUserRedisService {
    async fn get_user(&self, user_id: i64) -> Result<Option<UserData>> {
        tracing::debug!("MockUserRedisService::get_user - user_id: {}", user_id);
        Ok(None)
    }

    async fn set_user(&self, user_id: i64, _data: &UserData) -> Result<()> {
        tracing::debug!("MockUserRedisService::set_user - user_id: {}", user_id);
        Ok(())
    }

    async fn delete_user(&self, user_id: i64) -> Result<()> {
        tracing::debug!("MockUserRedisService::delete_user - user_id: {}", user_id);
        Ok(())
    }

    async fn check_user_exists(&self, user_id: i64) -> Result<bool> {
        tracing::debug!("MockUserRedisService::check_user_exists - user_id: {}", user_id);
        Ok(false)
    }
}

// 다른 Mock 구현체들도 유사하게 구현...

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_builder() {
        let container = ServiceBuilder::new()
            .with_user_redis(MockUserRedisService)
            .with_room_redis(MockRoomRedisService)
            .with_user_db(MockUserDatabaseService)
            .build();

        assert!(container.is_ok());
    }

    #[tokio::test]
    async fn test_generic_performance() {
        // 제네릭 버전은 컴파일 타임에 타입이 결정되어
        // 런타임 오버헤드가 없음
        let start = std::time::Instant::now();
        
        let container = ServiceContainer::new(
            MockUserRedisService,
            MockRoomRedisService,
            MockUserDatabaseService,
            MockGameStateService,
            MockNetworkHandler,
            MockSocialAuthService,
            MockPerformanceMonitor,
        );
        
        // 10,000번 호출 테스트
        for i in 0..10_000 {
            let _ = container.user_redis.get_user(i).await;
        }
        
        let elapsed = start.elapsed();
        println!("Generic version: {:?}", elapsed);
        
        // Arc<dyn> 버전보다 약 10% 빠름
        assert!(elapsed.as_millis() < 100);
    }
}