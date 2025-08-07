use jsonwebtoken::{encode, decode, Header, Validation, Algorithm, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// JWT에 포함될 클레임 구조체
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    /// 사용자 고유 ID (subject)
    sub: i32,
    /// 토큰 만료 시간 (Unix timestamp, 초 단위)
    exp: usize,
}

/// JWT 토큰 발급 및 검증을 담당하는 서비스
#[derive(Debug, Clone)]
pub struct TokenService {
    /// JWT 비밀키
    secret_key: String,
    /// 사용할 서명 알고리즘 (예: HS256)
    algorithm: Algorithm,
}

impl TokenService {
    /// 새 TokenService 인스턴스를 생성합니다.
    ///
    /// # 인자
    /// - `secret_key`: JWT 서명용 비밀키
    /// - `algorithm`: 사용할 알고리즘 문자열 (예: `"HS256"`)
    ///
    /// # 예외
    /// - `algorithm` 파싱 실패 시 `Algorithm::HS256`로 대체됩니다.
    pub fn new(secret_key: String, algorithm: String) -> Self {
        Self {
            secret_key,
            algorithm: Algorithm::from_str(&algorithm).unwrap_or(Algorithm::HS256),
        }
    }

    /// 사용자 ID를 기반으로 JWT 토큰을 생성합니다.
    ///
    /// # 인자
    /// - `user_id`: 사용자 고유 ID (정수)
    ///
    /// # 반환
    /// - 성공 시 JWT 토큰 문자열
    /// - 실패 시 `anyhow::Error`
    pub fn generate_token(&self, user_id: i32) -> anyhow::Result<String> {
        let expiration = (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize;

        let claims = Claims {
            sub: user_id,
            exp: expiration,
        };

        let mut header = Header::default();
        header.alg = self.algorithm;

        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(self.secret_key.as_bytes()),
        )?;

        Ok(token)
    }

    /// JWT 토큰을 검증하고, 성공 시 사용자 ID를 반환합니다.
    ///
    /// # 인자
    /// - `token`: 클라이언트로부터 받은 JWT 문자열
    ///
    /// # 반환
    /// - 성공 시 사용자 ID (`i32`)
    /// - 실패 시 `anyhow::Error`
    pub fn verify_token(&self, token: &str) -> anyhow::Result<i32> {
        let validation = Validation::new(self.algorithm);

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret_key.as_bytes()),
            &validation,
        )?;

        Ok(token_data.claims.sub)
    }
}
