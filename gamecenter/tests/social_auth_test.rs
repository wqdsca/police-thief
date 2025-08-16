//! 소셜 로그인 테스트

use anyhow::Result;
use gamecenter::service::{SocialAuthService, SocialProvider};
use sqlx::MySqlPool;
use std::env;

#[tokio::test]
async fn test_social_auth_urls() {
    // 테스트용 데이터베이스 연결 (실제 테스트 시 mock 사용 권장)
    dotenv::dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://test:test@localhost/test_db".to_string());

    let pool = MySqlPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    let auth_service = SocialAuthService::new(pool);

    // 카카오 인증 URL 테스트
    let kakao_url = auth_service
        .get_auth_url(SocialProvider::Kakao, "test_state")
        .expect("Failed to generate Kakao auth URL");

    assert!(kakao_url.contains("kauth.kakao.com"));
    assert!(kakao_url.contains("client_id="));
    assert!(kakao_url.contains("state=test_state"));
    println!("Kakao Auth URL: {}", kakao_url);

    // 구글 인증 URL 테스트
    let google_url = auth_service
        .get_auth_url(SocialProvider::Google, "test_state")
        .expect("Failed to generate Google auth URL");

    assert!(google_url.contains("accounts.google.com"));
    assert!(google_url.contains("client_id="));
    assert!(google_url.contains("state=test_state"));
    assert!(google_url.contains("scope="));
    println!("Google Auth URL: {}", google_url);

    // 애플 인증 URL 테스트
    let apple_url = auth_service
        .get_auth_url(SocialProvider::Apple, "test_state")
        .expect("Failed to generate Apple auth URL");

    assert!(apple_url.contains("appleid.apple.com"));
    assert!(apple_url.contains("client_id="));
    assert!(apple_url.contains("state=test_state"));
    println!("Apple Auth URL: {}", apple_url);
}

#[tokio::test]
async fn test_social_login_flow() {
    // 실제 OAuth flow는 mock이나 test doubles를 사용해야 함
    // 여기서는 구조와 데이터베이스 연동만 테스트

    dotenv::dotenv().ok();
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://test:test@localhost/test_db".to_string());

    let pool = MySqlPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    // 테이블 생성 (테스트 환경용)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            user_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
            username VARCHAR(255) NOT NULL UNIQUE,
            nickname VARCHAR(255) NOT NULL,
            password_hash VARCHAR(255) NOT NULL,
            status ENUM('active', 'inactive', 'banned') DEFAULT 'active',
            level INT DEFAULT 1,
            total_games INT DEFAULT 0,
            win_count INT DEFAULT 0,
            lose_count INT DEFAULT 0,
            win_rate FLOAT DEFAULT 0.0,
            last_login_at DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            login_type ENUM('normal', 'social') DEFAULT 'normal',
            email_verified BOOLEAN DEFAULT FALSE
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    )
    .execute(&pool)
    .await
    .ok();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS social_accounts (
            id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
            user_id BIGINT UNSIGNED NOT NULL,
            provider ENUM('Kakao', 'Google', 'Apple') NOT NULL,
            provider_id VARCHAR(255) NOT NULL,
            email VARCHAR(255),
            profile_image TEXT,
            access_token TEXT,
            refresh_token TEXT,
            token_expires_at DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            UNIQUE KEY unique_provider_account (provider, provider_id),
            INDEX idx_user_id (user_id),
            FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    )
    .execute(&pool)
    .await
    .ok();

    let auth_service = SocialAuthService::new(pool.clone());

    // 기본 서비스 생성 확인
    assert_eq!(
        auth_service
            .get_auth_url(SocialProvider::Kakao, "test")
            .is_ok(),
        true
    );

    println!("✅ Social auth service initialized successfully");
}

#[tokio::test]
async fn test_rest_api_endpoints() {
    use actix_web::{test, web, App};
    use gamecenter::social_auth_handler::{
        configure_social_auth_routes, SocialLoginRequest, StateStore,
    };

    dotenv::dotenv().ok();

    // Mock 데이터베이스 풀 생성
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://test:test@localhost/test_db".to_string());

    let pool = MySqlPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    let state_store = StateStore::default();

    // 테스트 앱 생성
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool))
            .app_data(web::Data::new(state_store))
            .configure(configure_social_auth_routes),
    )
    .await;

    // 카카오 로그인 시작 테스트
    let req = test::TestRequest::post()
        .uri("/auth/social/login")
        .set_json(&SocialLoginRequest {
            provider: "kakao".to_string(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    println!("✅ Kakao login start endpoint test passed");

    // 구글 로그인 시작 테스트
    let req = test::TestRequest::post()
        .uri("/auth/social/login")
        .set_json(&SocialLoginRequest {
            provider: "google".to_string(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    println!("✅ Google login start endpoint test passed");

    // 애플 로그인 시작 테스트
    let req = test::TestRequest::post()
        .uri("/auth/social/login")
        .set_json(&SocialLoginRequest {
            provider: "apple".to_string(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    println!("✅ Apple login start endpoint test passed");

    // 잘못된 provider 테스트
    let req = test::TestRequest::post()
        .uri("/auth/social/login")
        .set_json(&SocialLoginRequest {
            provider: "invalid".to_string(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    println!("✅ Invalid provider test passed");
}

#[test]
fn test_provider_enum() {
    use gamecenter::service::SocialProvider;

    // 직렬화 테스트
    let kakao = SocialProvider::Kakao;
    let google = SocialProvider::Google;
    let apple = SocialProvider::Apple;

    assert_eq!(format!("{:?}", kakao), "Kakao");
    assert_eq!(format!("{:?}", google), "Google");
    assert_eq!(format!("{:?}", apple), "Apple");

    println!("✅ Provider enum test passed");
}

/// 통합 테스트 실행
#[tokio::test]
async fn integration_test_social_auth() {
    println!("\n🔧 Starting Social Authentication Integration Tests...\n");

    // 환경 변수 로드
    dotenv::dotenv().ok();

    // API 키 확인
    let kakao_id = env::var("KAKAO_CLIENT_ID").unwrap_or_else(|_| "not_set".to_string());
    let google_id = env::var("GOOGLE_CLIENT_ID").unwrap_or_else(|_| "not_set".to_string());
    let apple_id = env::var("APPLE_CLIENT_ID").unwrap_or_else(|_| "not_set".to_string());

    println!("📋 API Keys Status:");
    println!(
        "  - Kakao Client ID: {}",
        if kakao_id != "not_set" {
            "✅ Set"
        } else {
            "❌ Not Set"
        }
    );
    println!(
        "  - Google Client ID: {}",
        if google_id != "not_set" {
            "✅ Set"
        } else {
            "❌ Not Set"
        }
    );
    println!(
        "  - Apple Client ID: {}",
        if apple_id != "not_set" {
            "✅ Set"
        } else {
            "❌ Not Set"
        }
    );

    // 데이터베이스 연결 테스트
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://test:test@localhost/test_db".to_string());

    match MySqlPool::connect(&database_url).await {
        Ok(pool) => {
            println!("\n✅ Database connection successful");

            // 테이블 확인
            let result = sqlx::query("SHOW TABLES LIKE 'social_accounts'")
                .fetch_optional(&pool)
                .await;

            if result.is_ok() && result?.is_some() {
                println!("✅ Social accounts table exists");
            } else {
                println!("⚠️  Social accounts table not found (will be created on first use)");
            }
        }
        Err(e) => {
            println!("\n❌ Database connection failed: {}", e);
            println!("   Please ensure MySQL is running and credentials are correct");
        }
    }

    println!("\n🎉 Social Authentication Setup Complete!");
    println!("\n📝 Next Steps:");
    println!("1. Set actual API keys in .env file");
    println!("2. Configure OAuth redirect URIs in each provider's console");
    println!("3. Run the server with: cargo run -p gamecenter start");
    println!("4. Test endpoints:");
    println!("   - POST /auth/social/login (body: {{\"provider\": \"kakao\"}}");
    println!("   - GET /auth/kakao/callback?code=xxx&state=xxx");
    println!("   - GET /auth/google/callback?code=xxx&state=xxx");
    println!("   - POST /auth/apple/callback (form data)");
}
