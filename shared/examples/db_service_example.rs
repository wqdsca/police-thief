//! 공통 DB 서비스 사용 예제
//!
//! BaseDbService와 UserDbService의 기본적인 사용법을 보여줍니다.
//!
//! 실행 방법:
//! ```bash
//! cargo run --example db_service_example
//! ```

use shared::config::db::DbConfig;
use shared::service::db::{
    BaseDbService, BaseDbServiceConfig, BaseDbServiceImpl, UserDbService, UserDbServiceConfig,
    UserDbServiceImpl, UserInput, UserSearchCriteria,
};
use shared::tool::error::AppError;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // 환경 설정 초기화
    tracing_subscriber::init();

    println!("🗄️  공통 DB 서비스 사용 예제");
    println!("================================");

    // 1. DB 설정 및 연결
    println!("\n1️⃣  DB 연결 설정");
    let db_config = DbConfig::new()
        .await
        .map_err(|e| AppError::DatabaseConnection(format!("DB 연결 실패: {}", e)))?;

    println!(
        "✅ MariaDB 연결 성공: {}:{}",
        db_config.host, db_config.port
    );

    // 2. 공통 DB 서비스 초기화
    println!("\n2️⃣  공통 DB 서비스 초기화");
    let base_config = BaseDbServiceConfig::new(db_config.clone())
        .with_logging(true)
        .with_timeout(30);

    let base_service = BaseDbServiceImpl::new(base_config);

    // 3. DB 메타정보 조회 예시
    println!("\n3️⃣  DB 메타정보 조회");

    // 데이터베이스 목록
    match base_service.get_databases().await {
        Ok(databases) => {
            println!("📋 데이터베이스 목록:");
            for db in databases {
                println!(
                    "  • {} (charset: {}, collation: {})",
                    db.name, db.charset, db.collation
                );
            }
        }
        Err(e) => println!("❌ 데이터베이스 목록 조회 실패: {}", e),
    }

    // 테이블 목록
    match base_service.get_tables(None).await {
        Ok(tables) => {
            println!("\n📊 테이블 목록:");
            for table in tables.iter().take(5) {
                // 처음 5개만 표시
                println!(
                    "  • {} ({} 행, {}KB)",
                    table.name,
                    table.rows,
                    table.data_length / 1024
                );
            }
            if tables.len() > 5 {
                println!("  ... 및 {} 개 더", tables.len() - 5);
            }
        }
        Err(e) => println!("❌ 테이블 목록 조회 실패: {}", e),
    }

    // 4. 사용자 DB 서비스 예시
    println!("\n4️⃣  사용자 DB 서비스 사용");

    let user_config = UserDbServiceConfig::new(db_config)
        .with_table_name("users".to_string())
        .with_soft_delete(true);

    let user_service = UserDbServiceImpl::new(user_config);

    // 헬스 체크
    match base_service.health_check().await {
        Ok(true) => println!("✅ DB 연결 상태 양호"),
        Ok(false) => println!("⚠️ DB 연결 상태 불안정"),
        Err(e) => println!("❌ DB 헬스 체크 실패: {}", e),
    }

    // 5. 범용 쿼리 실행 예시
    println!("\n5️⃣  범용 쿼리 실행");

    // 현재 시간 조회
    match base_service
        .execute_query("SELECT NOW() as current_time, VERSION() as version", None)
        .await
    {
        Ok(results) => {
            if let Some(row) = results.first() {
                println!("🕐 현재 시간: {:?}", row.get("current_time"));
                println!("🔧 MariaDB 버전: {:?}", row.get("version"));
            }
        }
        Err(e) => println!("❌ 쿼리 실행 실패: {}", e),
    }

    // 6. 사용자 관리 예시 (테이블이 존재하는 경우)
    println!("\n6️⃣  사용자 관리 기능 테스트");

    if base_service.table_exists("users").await.unwrap_or(false) {
        println!("📋 users 테이블 발견 - 사용자 관리 기능 테스트");

        // 사용자 통계
        match user_service.get_user_statistics().await {
            Ok(stats) => {
                println!("📊 사용자 통계:");
                println!("  • 전체 사용자: {} 명", stats.total_users);
                println!("  • 활성 사용자: {} 명", stats.active_users);
                println!("  • 비활성 사용자: {} 명", stats.inactive_users);
                println!("  • 최근 24시간 등록: {} 명", stats.recent_registrations);

                if !stats.users_by_login_type.is_empty() {
                    println!("  • 로그인 타입별:");
                    for (login_type, count) in stats.users_by_login_type {
                        println!("    - {}: {} 명", login_type, count);
                    }
                }
            }
            Err(e) => println!("❌ 사용자 통계 조회 실패: {}", e),
        }

        // 활성 사용자 목록 (최대 3명)
        match user_service.get_active_users(Some(3)).await {
            Ok(users) => {
                println!("\n👥 최근 활성 사용자 (최대 3명):");
                for user in users {
                    println!(
                        "  • {} (ID: {}, 타입: {})",
                        user.nick_name, user.id, user.login_type
                    );
                }
            }
            Err(e) => println!("❌ 활성 사용자 조회 실패: {}", e),
        }

        // 검색 예시
        let search_criteria = UserSearchCriteria {
            is_active: Some(true),
            limit: Some(5),
            ..Default::default()
        };

        match user_service.search_users(search_criteria).await {
            Ok(users) => {
                println!("\n🔍 사용자 검색 결과:");
                println!("  검색된 사용자: {} 명", users.len());
            }
            Err(e) => println!("❌ 사용자 검색 실패: {}", e),
        }
    } else {
        println!("ℹ️  users 테이블이 존재하지 않아 사용자 관리 기능을 건너뜁니다");
        println!("   테이블 생성 SQL:");
        println!("   CREATE TABLE users (");
        println!("     id INT PRIMARY KEY AUTO_INCREMENT,");
        println!("     nick_name VARCHAR(100) NOT NULL UNIQUE,");
        println!("     access_token VARCHAR(500) NOT NULL,");
        println!("     login_type VARCHAR(50) NOT NULL,");
        println!("     is_active BOOLEAN DEFAULT TRUE,");
        println!("     created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,");
        println!("     updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP");
        println!("   );");
    }

    // 7. 고급 기능 예시
    println!("\n7️⃣  고급 기능 예시");

    // 특정 테이블의 컬럼 정보 조회 (information_schema 테이블 자체를 조회)
    match base_service.get_columns("TABLES").await {
        Ok(columns) => {
            println!("🏗️  TABLES 테이블 구조 (처음 5개 컬럼):");
            for col in columns.iter().take(5) {
                println!(
                    "  • {} ({}) - {}",
                    col.name,
                    col.data_type,
                    if col.is_nullable {
                        "NULL 가능"
                    } else {
                        "NOT NULL"
                    }
                );
            }
        }
        Err(e) => println!("❌ 컬럼 정보 조회 실패: {}", e),
    }

    // 트랜잭션 예시 (실제 변경 없이 롤백)
    println!("\n🔄 트랜잭션 테스트");
    let transaction_result = base_service
        .with_transaction(|_tx| {
            Box::pin(async move {
                // 실제로는 여기서 복잡한 작업들을 수행
                println!("  트랜잭션 내부에서 작업 중...");

                // 의도적으로 에러를 발생시켜 롤백 테스트
                Err(AppError::InvalidInput("테스트용 롤백".to_string()))
            })
        })
        .await;

    match transaction_result {
        Ok(_) => println!("✅ 트랜잭션 커밋 완료"),
        Err(e) => println!("🔄 트랜잭션 롤백됨: {}", e),
    }

    println!("\n🎉 모든 테스트 완료!");
    println!("   공통 DB 서비스가 정상적으로 작동합니다.");

    Ok(())
}

/// 사용자 생성 예시 (실제 테이블이 있는 경우에만 실행)
#[allow(dead_code)]
async fn example_user_operations(user_service: &UserDbServiceImpl) -> Result<(), AppError> {
    println!("\n🧪 사용자 생성/수정/삭제 예시");

    // 1. 사용자 생성
    let new_user = UserInput {
        nick_name: "테스트사용자".to_string(),
        access_token: "test_token_12345".to_string(),
        login_type: "test".to_string(),
    };

    match user_service.create_user(new_user).await {
        Ok(user_id) => {
            println!("✅ 사용자 생성 성공: ID = {}", user_id);

            // 2. 사용자 조회
            if let Ok(Some(user)) = user_service.get_user_by_id(user_id).await {
                println!("👤 생성된 사용자: {}", user.nick_name);
            }

            // 3. 사용자 업데이트
            let updated_user = UserInput {
                nick_name: "수정된사용자".to_string(),
                access_token: "updated_token_67890".to_string(),
                login_type: "updated".to_string(),
            };

            if user_service.update_user(user_id, updated_user).await? {
                println!("✅ 사용자 업데이트 성공");
            }

            // 4. 사용자 삭제 (소프트 삭제)
            if user_service.delete_user(user_id).await? {
                println!("✅ 사용자 삭제 성공 (소프트 삭제)");
            }
        }
        Err(e) => {
            println!("❌ 사용자 생성 실패: {}", e);
        }
    }

    Ok(())
}
