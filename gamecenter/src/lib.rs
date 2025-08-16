//! GameCenter Library Module
//!
//! Exposes necessary modules for testing and external usage

pub mod service;
pub mod social_auth_handler;

// Re-export commonly used types
pub use service::social_auth_service::SocialUserInfo;
pub use service::{SocialAuthService, SocialProvider};
pub use social_auth_handler::{configure_social_auth_routes, SocialLoginRequest, StateStore};
