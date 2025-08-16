//! 데이터베이스 서비스 모듈
//!
//! MariaDB/MySQL 데이터베이스와의 모든 상호작용을 처리하는 서비스들을 제공합니다.
//! Clean Architecture 원칙에 따라 설계된 모듈식 구조입니다.

// 핵심 모듈들 - 관심사의 명확한 분리
pub mod core;          // 핵심 기능 모듈 (설정, 연결, 실행, 메타데이터, 트랜잭션)
pub mod base_service;  // 기본 DB 서비스 구현체

// 향상된 기능 모듈 (ID 관리, 페이지네이션, 검색) - 아직 구현되지 않음
// pub mod enhanced_base_db_service;

// 도메인별 특화 서비스 - 아직 구현되지 않음
// pub mod user_db_service;

// === Clean Architecture 기본 서비스 내보내기 ===
// 기본 DB 서비스 인터페이스와 구현체
pub use base_service::{BaseDbService, BaseDbServiceImpl};

// 핵심 컴포넌트들 내보내기
pub use core::{
    // 설정 관련
    config::{DbServiceConfig, MonitoringConfig, PerformanceConfig, PoolConfig, QueryConfig},
    // 연결 관리
    connection::ConnectionManager,
    // 쿼리 실행
    executor::QueryExecutor,
    // 메타데이터 제공
    metadata::MetadataProvider,
    // 트랜잭션 관리
    transaction::{IsolationLevel, TransactionManager},
    // 데이터 타입들
    types::{
        BatchOptions, ColumnInfo, ConnectionStats, DatabaseInfo, DbError, DbResult, IndexInfo,
        QueryParams, QueryRow, QueryStats, TableInfo,
    },
};

// === 향상된 DB 서비스 내보내기 (고급 기능) - 아직 구현되지 않음 ===
// pub use enhanced_base_db_service::{
//     BulkOperationResult, EnhancedBaseDbService, EnhancedBaseDbServiceImpl, IdInfo, IdStrategy,
//     LockType, PaginationParams, PaginationResult, SearchMatchType, SearchParams,
//     SnowflakeIdGenerator, SortOrder,
// };

// === 도메인 서비스 내보내기 - 아직 구현되지 않음 ===
// pub use user_db_service::{
//     User, UserDbService, UserDbServiceConfig, UserDbServiceImpl, UserInput, UserSearchCriteria,
//     UserStatistics,
// };
