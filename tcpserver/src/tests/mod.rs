//\! TCP 서버 테스트 모듈
//\!
//\! 방 기반 연결 관리 시스템의 모든 기능을 테스트합니다.

/// TCP 연결 관리 종합 테스트
///
/// DashMap 기반 방 연결 관리, Redis 백업, 실시간 메시징,
/// 동시성, 성능 등의 모든 기능을 종합적으로 테스트합니다.
pub mod tcp_connect_test;

/// 채팅방 통합 기능 테스트
///
/// 방 입장/퇴장, 채팅 메시지, 빈 방 정리 등의 핵심 기능 테스트
pub mod chat_room_integration_test;

/// 채팅방 성능 테스트
///
/// VCPU 2개, RAM 2GB 환경에서 방 50개, 사용자 300명 성능 테스트
pub mod chat_room_performance_test;

pub use tcp_connect_test::*;
