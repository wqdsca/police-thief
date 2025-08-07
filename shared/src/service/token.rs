use jsonwebtoken::{encode, decode, Header, Validation, Algorithm, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tonic::{Request, Status};
use tracing::{info, error};
use chrono::{Utc, Duration};

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

    /// 공통 인증 함수 - 모든 컨트롤러에서 재사용 가능
    /// 
    /// gRPC 요청에서 JWT 토큰을 검증하고, 성공 시 콜백 함수를 실행합니다.
    /// 
    /// # Arguments
    /// * `req` - gRPC 요청
    /// * `callback` - 토큰 검증 성공 시 실행할 콜백 함수
    /// 
    /// # Returns
    /// * `Result<T, Status>` - 콜백 함수의 결과 또는 에러
    pub fn with_auth<T, F>(&self, req: &Request<()>, callback: F) -> Result<T, Status>
    where
        F: FnOnce(i32) -> Result<T, Status>,
    {
        // Authorization 헤더에서 토큰 추출
        let auth_header = req.metadata()
            .get("authorization")
            .ok_or_else(|| Status::unauthenticated("Missing authorization header"))?;
        
        let auth_value = auth_header
            .to_str()
            .map_err(|_| Status::invalid_argument("Invalid authorization header"))?;
        
        if !auth_value.starts_with("Bearer ") {
            return Err(Status::invalid_argument("Invalid authorization format. Expected 'Bearer <token>'"));
        }
        
        let token = auth_value[7..].to_string(); // "Bearer " 제거
        
        if token.is_empty() {
            return Err(Status::invalid_argument("Empty token"));
        }
        
        // 토큰 검증
        match self.verify_token(&token) {
            Ok(user_id) => {
                info!("✅ JWT 토큰 검증 성공: user_id={}", user_id);
                callback(user_id)
            }
            Err(e) => {
                error!("❌ JWT 토큰 검증 실패: error={}", e);
                Err(Status::unauthenticated("Invalid or expired token"))
            }
        }
    }

    /// 선택적 인증 함수 - 토큰이 있으면 검증, 없으면 통과
    /// 
    /// # Arguments
    /// * `req` - gRPC 요청
    /// * `callback` - 실행할 콜백 함수 (user_id는 Option<i32>)
    /// 
    /// # Returns
    /// * `Result<T, Status>` - 콜백 함수의 결과 또는 에러
    pub fn with_optional_auth<T, F>(&self, req: &Request<()>, callback: F) -> Result<T, Status>
    where
        F: FnOnce(Option<i32>) -> Result<T, Status>,
    {
        // Authorization 헤더가 없으면 None으로 콜백 실행
        let auth_header = match req.metadata().get("authorization") {
            Some(header) => header,
            None => {
                return callback(None);
            }
        };
        
        let auth_value = match auth_header.to_str() {
            Ok(value) => value,
            Err(_) => {
                return callback(None);
            }
        };
        
        if !auth_value.starts_with("Bearer ") {
            return callback(None);
        }
        
        let token = auth_value[7..].to_string();
        
        if token.is_empty() {
            return callback(None);
        }
        
        // 토큰 검증 시도
        match self.verify_token(&token) {
            Ok(user_id) => {
                info!("✅ JWT 토큰 검증 성공: user_id={}", user_id);
                callback(Some(user_id))
            }
            Err(e) => {
                error!("❌ JWT 토큰 검증 실패: error={}", e);
                callback(None)
            }
        }
    }

    /// 공개 엔드포인트인지 확인하는 함수
    /// 
    /// # Arguments
    /// * `path` - gRPC 요청 경로
    /// 
    /// # Returns
    /// * `bool` - 공개 엔드포인트 여부
    pub fn is_public_endpoint(path: &str) -> bool {
        let public_paths = ["/user.UserService/LoginUser",
            "/user.UserService/RegisterUser"];
        
        public_paths.contains(&path)
    }

    /// 조건부 인증 함수 - 공개 엔드포인트는 통과, 보호된 엔드포인트는 인증 필요
    /// 
    /// # Arguments
    /// * `req` - gRPC 요청
    /// * `callback` - 실행할 콜백 함수
    /// 
    /// # Returns
    /// * `Result<T, Status>` - 콜백 함수의 결과 또는 에러
    pub fn with_conditional_auth<T, F>(&self, req: &Request<()>, callback: F) -> Result<T, Status>
    where
        F: FnOnce(Option<i32>) -> Result<T, Status>,
    {
        // 요청 경로 확인 (실제 구현에서는 req.uri().path() 사용)
        let path = "unknown"; // TODO: 실제 경로 추출 로직 필요
        
        if Self::is_public_endpoint(path) {
            // 공개 엔드포인트는 인증 없이 실행
            callback(None)
        } else {
            // 보호된 엔드포인트는 인증 필요
            self.with_auth(req, |user_id| callback(Some(user_id)))
        }
    }
}
