//! 통합 인증 모듈
//! 
//! gRPC와 REST에서 공유하는 소셜 로그인 및 JWT 처리

pub mod social_auth;
pub mod token;
pub mod types;

pub use social_auth::SocialAuthService;
pub use token::TokenService;
pub use types::{Provider, TokenPair, UserInfo, OAuthConfig};