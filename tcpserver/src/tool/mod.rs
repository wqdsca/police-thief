//! TCP 서버 공통 유틸리티 모듈
//! 
//! 데이터 전송, 변환, 선형 유틸 등 공통 기능을 제공합니다.

pub mod simple_utils;
pub mod error;
pub mod network_utils;

pub use simple_utils::*;
pub use network_utils::{NetworkUtils, IpInfo, ConnectionQuality};