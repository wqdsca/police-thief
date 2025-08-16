//! gRPC Interceptor Module
//!
//! gRPC 요청/응답을 가로채서 처리하는 인터셉터들을 정의합니다.
//! JWT 토큰 검증, 로깅, 에러 처리 등을 담당합니다.

use shared::service::TokenService;
use tonic::{metadata::MetadataValue, service::Interceptor, Request, Status};
use tracing::{error, info};

/// JWT 토큰 검증 인터셉터
///
/// gRPC 요청에서 JWT 토큰을 검증하고, 사용자 정보를 요청에 추가합니다.
/// 인증이 필요한 서비스에만 적용됩니다.
#[allow(dead_code)]
pub fn jwt_interceptor(token_service: TokenService) -> impl Interceptor {
    move |mut req: Request<()>| {
        // Authorization 헤더에서 토큰 추출
        let token = extract_token_from_headers(req.metadata())?;

        // 토큰 검증
        match token_service.verify_token(&token) {
            Ok(user_id) => {
                info!("✅ JWT 토큰 검증 성공: user_id={}", user_id);

                // 사용자 ID를 요청 메타데이터에 추가
                req.metadata_mut().insert(
                    "user_id",
                    user_id
                        .to_string()
                        .parse()
                        .unwrap_or_else(|_| MetadataValue::from_static("0")),
                );

                Ok(req)
            }
            Err(e) => {
                error!("❌ JWT 토큰 검증 실패: error={}", e);
                Err(Status::unauthenticated("Invalid or expired token"))
            }
        }
    }
}

/// 요청 헤더에서 JWT 토큰을 추출합니다.
///
/// # Arguments
/// * `metadata` - gRPC 요청 메타데이터
///
/// # Returns
/// * `Result<String, Status>` - 추출된 토큰 또는 에러
#[allow(dead_code)]
fn extract_token_from_headers(metadata: &tonic::metadata::MetadataMap) -> Result<String, Status> {
    // Authorization 헤더에서 Bearer 토큰 추출
    let auth_header = metadata
        .get("authorization")
        .ok_or_else(|| Status::unauthenticated("Missing authorization header"))?;

    let auth_value = auth_header
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid authorization header"))?;

    if !auth_value.starts_with("Bearer ") {
        return Err(Status::invalid_argument(
            "Invalid authorization format. Expected 'Bearer <token>'",
        ));
    }

    let token = auth_value[7..].to_string(); // "Bearer " 제거

    if token.is_empty() {
        return Err(Status::invalid_argument("Empty token"));
    }

    Ok(token)
}

/// 로깅 인터셉터
///
/// 모든 gRPC 요청을 로깅합니다.
#[allow(dead_code)]
pub fn logging_interceptor() -> impl Interceptor {
    |req: Request<()>| {
        info!("📨 gRPC 요청 처리 중...");

        // 요청 메타데이터 로깅 (민감한 정보 제외)
        if let Some(user_agent) = req.metadata().get("user-agent") {
            info!(
                "👤 User-Agent: {}",
                user_agent.to_str().unwrap_or("unknown")
            );
        }

        Ok(req)
    }
}
