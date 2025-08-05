// src/service/user_service.rs
pub struct UserService;

impl UserService {
    pub fn new() -> Self { Self }

    pub async fn login_user(
        &self,
        _login_type: String,
        _login_token: String,
    ) -> anyhow::Result<(i32, String, String, String, bool)> {
        // TODO: 실제 인증 로직
        Ok((123, "nick".into(), "access_token".into(), "refresh_token".into(), false))
    }

    pub async fn register_user(
        &self,
        _login_type: String,
        _login_token: String,
        _nick_name: String,
    ) -> anyhow::Result<()> {
        // TODO: 실제 회원가입 로직
        Ok(())
    }
}
