//! MariaDB Database Configuration
//! 
//! Police Thief 게임을 위한 MariaDB 데이터베이스 연결 설정입니다.
//! .env 파일에서 데이터베이스 연결 정보를 읽어와 연결 풀을 관리합니다.

use dotenv::dotenv;
use std::env;
use sqlx::{MySql, Pool, MySqlPool, Error as SqlxError};
use tracing::{info, error, warn};

/// MariaDB 연결 풀 타입 별칭
pub type DbConnection = Pool<MySql>;

/// MariaDB 데이터베이스 설정 구조체
#[derive(Debug, Clone)]
pub struct DbConfig {
    pub pool: DbConnection,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub database: String,
}

impl DbConfig {
    /// 새로운 데이터베이스 연결 풀을 생성합니다.
    /// 
    /// .env 파일에서 데이터베이스 연결 정보를 읽어와 연결 풀을 생성합니다.
    /// 환경 변수가 없으면 기본값을 사용합니다.
    /// 
    /// # Returns
    /// * `Result<Self, SqlxError>` - 데이터베이스 연결 풀 또는 에러
    pub async fn new() -> Result<Self, SqlxError> {
        // .env 파일 로드 - workspace root에서 찾기
        let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let workspace_env = current_dir.join(".env");
        let parent_env = current_dir.parent().map(|p| p.join(".env"));
        
        let mut env_loaded = false;
        
        // 현재 디렉토리의 .env 파일 시도
        if workspace_env.exists() {
            dotenv::from_path(&workspace_env).ok();
            env_loaded = true;
            info!("환경 파일 로드: {:?}", workspace_env);
        }
        // 상위 디렉토리의 .env 파일 시도 (서브패키지에서 실행되는 경우)
        else if let Some(parent_env) = parent_env {
            if parent_env.exists() {
                dotenv::from_path(&parent_env).ok();
                env_loaded = true;
                info!("환경 파일 로드: {:?}", parent_env);
            }
        }
        
        if !env_loaded {
            dotenv().ok(); // 기본 .env 파일 시도
            warn!(".env 파일을 찾을 수 없어서 환경 변수를 직접 사용합니다.");
        }

        // 환경변수에서 데이터베이스 연결 정보 로드
        let host = env::var("db_host").unwrap_or_else(|_| {
            warn!("db_host 환경변수가 없어서 localhost를 사용합니다.");
            "localhost".to_string()
        });
        
        let port_str = env::var("db_port").unwrap_or_else(|_| {
            warn!("db_port 환경변수가 없어서 3306을 사용합니다.");
            "3306".to_string()
        });
        
        let port = port_str.parse::<u16>().expect("db_port는 숫자여야 함");
        
        let user = env::var("db_id").unwrap_or_else(|_| {
            warn!("db_id 환경변수가 없어서 root를 사용합니다.");
            "root".to_string()
        });
        
        let password = env::var("db_password").unwrap_or_else(|_| {
            error!("db_password 환경변수가 필요합니다.");
            "".to_string()
        });
        
        let database = env::var("db_name").unwrap_or_else(|_| {
            warn!("db_name 환경변수가 없어서 police를 사용합니다.");
            "police".to_string()
        });

        // MariaDB 연결 URL 생성
        let database_url = format!(
            "mysql://{}:{}@{}:{}/{}",
            user, password, host, port, database
        );

        info!("데이터베이스 연결 시도: {}:{}@{}/{}", user, "***", host, database);

        // 연결 풀 생성
        let pool = MySqlPool::connect(&database_url).await?;

        info!("MariaDB 연결 풀 생성 완료: {}:{}", host, port);

        Ok(Self {
            pool,
            host,
            port,
            user,
            database,
        })
    }

    /// 연결 풀에서 연결을 가져옵니다.
    /// 
    /// # Returns
    /// * `&DbConnection` - 데이터베이스 연결 풀 참조
    pub fn get_pool(&self) -> &DbConnection {
        &self.pool
    }

    /// 데이터베이스 연결 상태를 확인합니다.
    /// 
    /// # Returns
    /// * `Result<bool, SqlxError>` - 연결 성공 여부
    pub async fn health_check(&self) -> Result<bool, SqlxError> {
        match sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
        {
            Ok(_) => {
                info!("데이터베이스 연결 상태 양호");
                Ok(true)
            }
            Err(e) => {
                error!("데이터베이스 연결 실패: {}", e);
                Err(e)
            }
        }
    }

    /// 연결 풀 통계 정보를 반환합니다.
    /// 
    /// # Returns
    /// * `PoolStats` - 연결 풀 통계
    pub fn get_pool_stats(&self) -> PoolStats {
        PoolStats {
            size: self.pool.size(),
            idle: self.pool.num_idle() as u32, // usize -> u32 변환
        }
    }

    /// 데이터베이스 연결을 닫습니다.
    /// 
    /// 애플리케이션 종료 시 호출하여 리소스를 정리합니다.
    pub async fn close(&self) {
        info!("데이터베이스 연결 풀을 닫는 중...");
        self.pool.close().await;
        info!("데이터베이스 연결 풀 종료 완료");
    }
}

/// 연결 풀 통계 정보
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub size: u32,        // 전체 연결 수
    pub idle: u32,        // 유휴 연결 수
}

/// 데이터베이스 헬퍼 함수들
pub mod helpers {
    use super::*;
    use crate::tool::error::AppError;

    /// SQLx 에러를 AppError로 변환하는 헬퍼 함수
    /// 
    /// # Arguments
    /// * `error` - SQLx 에러
    /// * `context` - 에러 컨텍스트
    /// 
    /// # Returns
    /// * `AppError` - 변환된 애플리케이션 에러
    pub fn map_sqlx_error(error: SqlxError, context: &str) -> AppError {
        match error {
            SqlxError::Database(db_err) => {
                error!("데이터베이스 에러 [{}]: {}", context, db_err);
                AppError::DatabaseQuery(format!("{}: {}", context, db_err))
            }
            SqlxError::Io(io_err) => {
                error!("I/O 에러 [{}]: {}", context, io_err);
                AppError::DatabaseConnection(format!("{}: {}", context, io_err))
            }
            SqlxError::Protocol(proto_err) => {
                error!("프로토콜 에러 [{}]: {}", context, proto_err);
                AppError::DatabaseConnection(format!("{}: {}", context, proto_err))
            }
            SqlxError::RowNotFound => {
                warn!("행을 찾을 수 없음 [{}]", context);
                AppError::DatabaseQuery(format!("{}: 레코드를 찾을 수 없습니다", context))
            }
            SqlxError::TypeNotFound { type_name } => {
                error!("타입을 찾을 수 없음 [{}]: {}", context, type_name);
                AppError::DatabaseQuery(format!("{}: 타입 '{}' 를 찾을 수 없습니다", context, type_name))
            }
            SqlxError::ColumnNotFound(column) => {
                error!("컬럼을 찾을 수 없음 [{}]: {}", context, column);
                AppError::DatabaseQuery(format!("{}: 컬럼 '{}' 를 찾을 수 없습니다", context, column))
            }
            SqlxError::PoolTimedOut => {
                error!("연결 풀 타임아웃 [{}]", context);
                AppError::Timeout(format!("{}: 데이터베이스 연결 풀 타임아웃", context))
            }
            SqlxError::PoolClosed => {
                error!("연결 풀이 닫힘 [{}]", context);
                AppError::ServiceUnavailable(format!("{}: 데이터베이스 연결 풀이 닫혔습니다", context))
            }
            _ => {
                error!("기타 데이터베이스 에러 [{}]: {}", context, error);
                AppError::InternalError(format!("{}: {}", context, error))
            }
        }
    }

    /// 트랜잭션 실행 헬퍼 함수
    /// 
    /// # Arguments
    /// * `pool` - 데이터베이스 연결 풀
    /// * `operation` - 실행할 트랜잭션 클로저
    /// 
    /// # Returns
    /// * `Result<T, AppError>` - 트랜잭션 실행 결과
    pub async fn with_transaction<T, F, Fut>(
        pool: &DbConnection,
        operation: F,
    ) -> Result<T, AppError>
    where
        F: FnOnce(&mut sqlx::Transaction<MySql>) -> Fut,
        Fut: std::future::Future<Output = Result<T, AppError>>,
    {
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| map_sqlx_error(e, "트랜잭션 시작"))?;

        let result = operation(&mut tx).await;

        match result {
            Ok(value) => {
                tx.commit()
                    .await
                    .map_err(|e| map_sqlx_error(e, "트랜잭션 커밋"))?;
                info!("트랜잭션 커밋 성공");
                Ok(value)
            }
            Err(app_error) => {
                tx.rollback()
                    .await
                    .map_err(|e| map_sqlx_error(e, "트랜잭션 롤백"))?;
                warn!("트랜잭션 롤백: {}", app_error);
                Err(app_error)
            }
        }
    }
}