//! User Service Business Logic
//! 
//! 사용자 인증 및 회원가입 기능을 담당하는 비즈니스 로직입니다.
//! 실제 데이터베이스 연동 및 사용자 관련 비즈니스 규칙을 처리합니다.

use tracing::info;
use shared::tool::error::AppError;
use shared::model::UserInfo;
use shared::service::redis::user_redis_service::{UserRedisService, UserRedisServiceConfig};
use shared::config::connection_pool::ConnectionPool;
use shared::service::redis::core::redis_get_key::KeyType;

/// User Service 비즈니스 로직
/// 
/// 사용자 인증 및 회원가입 기능을 처리하는 서비스입니다.
/// 현재는 더미 데이터를 반환하지만, 향후 실제 데이터베이스 연동이 추가될 예정입니다.
#[derive(Default)]
pub struct UserService;

impl UserService {
    /// 새로운 UserService 인스턴스를 생성합니다.
    /// 
    /// # Returns
    /// * `Self` - 초기화된 UserService 인스턴스
    pub fn new() -> Self { 
        Self 
    }

    /// 사용자 로그인을 처리합니다.
    /// 
    /// 사용자가 로그인할 때 호출되는 메서드입니다.
    /// 현재는 더미 데이터를 반환하지만, 향후 실제 인증 로직이 추가될 예정입니다.
    /// 
    /// # Arguments
    /// * `login_type` - 로그인 타입 (예: "google", "apple", "guest")
    /// * `login_token` - 로그인 토큰 또는 인증 정보
    /// 
    /// # Returns
    /// * `Result<(i32, String, String, String, bool), AppError>` - (user_id, nick_name, access_token, refresh_token, is_register)
    pub async fn login_user(
        &self,
        login_type: String,
        login_token: String,
    ) -> Result<(i32, String, String, String, bool), AppError> {
        let mut user_id = 1;
        info!("로그인 서비스 호출: login_type={}", login_type);
        let nick_name = "test".to_string();
        let access_token = "access_token".to_string();
        let refresh_token = "refresh_token".to_string();
        let is_register = true;
        // TODO: 실제 인증 로직 구현 필요
        // - 토큰 검증
        // - 사용자 정보 조회
        // - 세션 생성
        // - 액세스 토큰 발급
        
        info!("로그인 완료: nick={}", nick_name);
        let success_login : bool = self.social_login(login_type, login_token)?;
        if success_login {
            user_id = user_id + 1;
        }
        let user_info = UserInfo {
            user_id,
            nick_name: nick_name.clone(),
            tcp_ip: "".to_string(),
            tcp_port: 0,
            udp_ip: "".to_string(),
            udp_port: 0,
            access_token: access_token.clone(),
        };
        let redis_config = ConnectionPool::get_config().await
            .map_err(|e| AppError::RedisConnection(e.to_string()))?;
        let user_redis_service = UserRedisService::new(UserRedisServiceConfig {
            redis_config,
            key_type: KeyType::User,
        });
        user_redis_service.login_success_redis_service(user_id, &user_info).await?;
        Ok((user_id, nick_name, access_token, refresh_token, is_register))
    }

    /// 사용자 회원가입을 처리합니다.
    /// 
    /// 사용자가 회원가입할 때 호출되는 메서드입니다.
    /// 현재는 더미 데이터를 반환하지만, 향후 실제 회원가입 로직이 추가될 예정입니다.
    /// 
    /// # Arguments
    /// * `login_type` - 로그인 타입 (예: "google", "apple", "guest")
    /// * `login_token` - 로그인 토큰 또는 인증 정보
    /// * `nick_name` - 사용자가 설정한 닉네임
    /// 
    /// # Returns
    /// * `Result<(), AppError>` - 회원가입 성공 여부
    pub async fn register_user(
        &self,
        login_type: String,
        _login_token: String,
        nick_name: String,
    ) -> Result<(), AppError> {
        info!("회원가입 서비스 호출: login_type={}, nick={}", login_type, nick_name);
        
        // TODO: 실제 회원가입 로직 구현 필요
        // - 사용자 정보 검증
        // - 닉네임 중복 확인
        // - 사용자 정보 데이터베이스 저장
        // - 초기 설정 적용
        
        // 시뮬레이션: 닉네임 중복 (테스트용)
        if nick_name == "duplicate" {
            return Err(AppError::NicknameExists("테스트용 닉네임 중복: 'duplicate'".to_string()));
        }
        
        // 시뮬레이션: 데이터베이스 오류 (테스트용)
        if nick_name.contains("db_error") {
            return Err(AppError::DatabaseQuery("테스트용 데이터베이스 쿼리 실패".to_string()));
        }
        
        info!("회원가입 완료");
        Ok(())
    }

    // 실제 로그인 로직 구현
    // 1. 회원가입 유무 확인
    // 2. 회원가입 여부에 따라 로그인 처리 bool 반환
    // 3. 회원가입이 되어있으면 로그인 처리 후 user_id 반환
    fn social_login(&self, login_type: String, _login_token:String) -> Result<bool, AppError> {
        match login_type.as_str() {
            "google" => {
                // 구글 로그인 처리
                Ok(true)
            }
            "apple" => {
                // 애플 로그인 처리
                Ok(true)
            }
            "test" => {
                // 테스트 아이디 일때 바로 반환하기 
                Ok(true)
            }
            _ => {
                Err(AppError::InvalidLoginType(login_type))
            }
        }
    }
        
    //     // 1. 토큰 검증
    //     // 2. 사용자 정보 조회
    //     // 3. 세션 생성
    //     // 4. 액세스 토큰 발급
        
    // }
}
