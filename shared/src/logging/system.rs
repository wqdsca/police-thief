//! 통합 로깅 시스템
//!
//! 모든 로깅 구성 요소를 통합하여 관리하는 메인 시스템입니다.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::logging::{
    config::{LoggingConfig, ServiceType},
    formatter::{LogFormatter, LogLevel, LogEntry},
    rotation::LogRotationManager,
    writer::{AsyncLogWriter, InMemoryLogWriter},
};

/// 로깅 시스템 상태
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoggingState {
    /// 초기화되지 않음
    Uninitialized,
    /// 초기화됨
    Initialized,
    /// 실행 중
    Running,
    /// 종료됨
    Shutdown,
}

/// 통합 로깅 시스템
pub struct LoggingSystem {
    /// 기본 로그 디렉토리
    base_dir: PathBuf,
    /// 로깅 설정
    config: LoggingConfig,
    /// 로그 순환 관리자
    rotation_manager: Arc<Mutex<LogRotationManager>>,
    /// 로그 포매터
    formatter: Arc<LogFormatter>,
    /// 서비스별 로그 작성기
    writers: Arc<RwLock<HashMap<ServiceType, Arc<AsyncLogWriter>>>>,
    /// 현재 서비스 타입
    current_service: Option<ServiceType>,
    /// 시스템 상태
    state: Arc<RwLock<LoggingState>>,
    /// 백그라운드 태스크 핸들
    background_tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    /// 테스트 모드 (메모리 작성기 사용)
    test_mode: bool,
    /// 테스트용 메모리 작성기
    memory_writer: Option<Arc<InMemoryLogWriter>>,
}

impl LoggingSystem {
    /// 새 로깅 시스템 생성
    pub async fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self> {
        let config = LoggingConfig::from_env();
        config.validate().context("로깅 설정 유효성 검증 실패")?;
        
        let base_dir = base_dir.as_ref().to_path_buf();
        let rotation_manager = Arc::new(Mutex::new(LogRotationManager::new(&base_dir, config.clone())));
        let formatter = Arc::new(LogFormatter::new(config.json_format, !config.json_format));
        
        // 로그 디렉토리 초기화
        {
            let manager = rotation_manager.lock().await;
            manager.initialize_directories().await
                .context("로그 디렉토리 초기화 실패")?;
        }
        
        Ok(Self {
            base_dir,
            config,
            rotation_manager,
            formatter,
            writers: Arc::new(RwLock::new(HashMap::new())),
            current_service: None,
            state: Arc::new(RwLock::new(LoggingState::Uninitialized)),
            background_tasks: Arc::new(Mutex::new(Vec::new())),
            test_mode: false,
            memory_writer: None,
        })
    }
    
    /// 테스트 모드로 로깅 시스템 생성
    pub async fn new_test_mode() -> Result<Self> {
        let mut system = Self::new("./test_logs").await?;
        system.test_mode = true;
        system.memory_writer = Some(Arc::new(InMemoryLogWriter::new(system.formatter.clone())));
        Ok(system)
    }
    
    /// 지정된 서비스로 로깅 시스템 초기화
    pub async fn init(&mut self, service_type: ServiceType) -> Result<()> {
        let mut state = self.state.write().await;
        if *state != LoggingState::Uninitialized {
            return Err(anyhow::anyhow!("로깅 시스템이 이미 초기화됨"));
        }
        
        self.current_service = Some(service_type);
        
        // 서비스별 로그 작성기 생성
        if !self.test_mode {
            self.create_log_writer(service_type).await?;
        }
        
        // 백그라운드 태스크 시작
        self.start_background_tasks().await?;
        
        *state = LoggingState::Initialized;
        
        info!(
            service = service_type.as_str(),
            base_dir = %self.base_dir.display(),
            config = ?self.config,
            "로깅 시스템 초기화 완료"
        );
        
        Ok(())
    }
    
    /// 로그 작성기 생성
    async fn create_log_writer(&self, service_type: ServiceType) -> Result<()> {
        let mut rotation_manager = self.rotation_manager.lock().await;
        let log_file_path = rotation_manager.get_current_log_file(service_type).await?;
        drop(rotation_manager);
        
        let writer = AsyncLogWriter::new(
            log_file_path,
            self.config.clone(),
            self.formatter.clone(),
        ).await?;
        
        let mut writers = self.writers.write().await;
        writers.insert(service_type, Arc::new(writer));
        
        debug!(
            service = service_type.as_str(),
            "로그 작성기 생성됨"
        );
        
        Ok(())
    }
    
    /// 백그라운드 태스크 시작
    async fn start_background_tasks(&self) -> Result<()> {
        let mut tasks = self.background_tasks.lock().await;
        
        // 로그 파일 정리 태스크
        {
            let rotation_manager = self.rotation_manager.clone();
            let cleanup_task = tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(3600)); // 1시간마다
                loop {
                    interval.tick().await;
                    let mut manager = rotation_manager.lock().await;
                    if let Err(e) = manager.cleanup_old_logs().await {
                        error!(error = %e, "로그 파일 정리 실패");
                    }
                }
            });
            tasks.push(cleanup_task);
        }
        
        // 로그 파일 순환 태스크
        if let Some(service_type) = self.current_service {
            let rotation_manager = self.rotation_manager.clone();
            let writers = self.writers.clone();
            let config = self.config.clone();
            let formatter = self.formatter.clone();
            
            let rotation_task = tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(300)); // 5분마다
                loop {
                    interval.tick().await;
                    
                    let mut manager = rotation_manager.lock().await;
                    if let Err(e) = manager.rotate_if_needed(service_type).await {
                        error!(error = %e, "로그 파일 순환 실패");
                        continue;
                    }
                    
                    // 새 로그 파일 경로 가져오기
                    let new_log_path = match manager.get_current_log_file(service_type).await {
                        Ok(path) => path,
                        Err(e) => {
                            error!(error = %e, "새 로그 파일 경로 가져오기 실패");
                            continue;
                        }
                    };
                    drop(manager);
                    
                    // 새 작성기로 교체
                    if let Ok(new_writer) = AsyncLogWriter::new(
                        new_log_path, 
                        config.clone(), 
                        formatter.clone()
                    ).await {
                        let mut writers_guard = writers.write().await;
                        if let Some(old_writer) = writers_guard.remove(&service_type) {
                            // 이전 작성기 종료는 백그라운드에서 실행
                            tokio::spawn(async move {
                                if Arc::try_unwrap(old_writer).is_err() {
                                    warn!("이전 작성기가 여전히 사용 중임");
                                }
                                // Arc::try_unwrap 성공 시에만 shutdown 호출 가능
                            });
                        }
                        writers_guard.insert(service_type, Arc::new(new_writer));
                    }
                }
            });
            tasks.push(rotation_task);
        }
        
        debug!("백그라운드 태스크 시작됨");
        Ok(())
    }
    
    /// TRACE 레벨 로그 작성
    pub async fn trace<S: AsRef<str>>(&self, message: S, context: &[(&str, &str)]) {
        self.log(LogLevel::Trace, message, context).await;
    }
    
    /// DEBUG 레벨 로그 작성
    pub async fn debug<S: AsRef<str>>(&self, message: S, context: &[(&str, &str)]) {
        self.log(LogLevel::Debug, message, context).await;
    }
    
    /// INFO 레벨 로그 작성
    pub async fn info<S: AsRef<str>>(&self, message: S, context: &[(&str, &str)]) {
        self.log(LogLevel::Info, message, context).await;
    }
    
    /// WARN 레벨 로그 작성
    pub async fn warn<S: AsRef<str>>(&self, message: S, context: &[(&str, &str)]) {
        self.log(LogLevel::Warn, message, context).await;
    }
    
    /// ERROR 레벨 로그 작성
    pub async fn error<S: AsRef<str>>(&self, message: S, context: &[(&str, &str)]) {
        self.log(LogLevel::Error, message, context).await;
    }
    
    /// FATAL 레벨 로그 작성
    pub async fn fatal<S: AsRef<str>>(&self, message: S, context: &[(&str, &str)]) {
        self.log(LogLevel::Fatal, message, context).await;
    }
    
    /// 일반 로그 작성 메서드
    pub async fn log<S: AsRef<str>>(&self, level: LogLevel, message: S, context: &[(&str, &str)]) {
        let service_name = self.current_service
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        let entry = LogEntry::new(
            level,
            service_name,
            message.as_ref().to_string(),
            context,
        );
        
        if self.test_mode {
            if let Some(memory_writer) = &self.memory_writer {
                if let Err(e) = memory_writer.write_log(entry).await {
                    eprintln!("메모리 로그 작성 실패: {}", e);
                }
            }
        } else if let Some(service_type) = self.current_service {
            let writers = self.writers.read().await;
            if let Some(writer) = writers.get(&service_type) {
                if let Err(e) = writer.write_log(entry) {
                    eprintln!("로그 작성 실패: {}", e);
                }
            }
        }
    }
    
    /// 즉시 플러시
    pub async fn flush(&self) -> Result<()> {
        if self.test_mode {
            return Ok(());
        }
        
        let writers = self.writers.read().await;
        for writer in writers.values() {
            writer.flush()?;
        }
        Ok(())
    }
    
    /// 시스템 상태 반환
    pub async fn get_state(&self) -> LoggingState {
        *self.state.read().await
    }
    
    /// 로깅 시스템 종료
    pub async fn shutdown(self) -> Result<()> {
        let mut state = self.state.write().await;
        if *state == LoggingState::Shutdown {
            return Ok(());
        }
        
        // 백그라운드 태스크 종료
        let mut tasks = self.background_tasks.lock().await;
        for task in tasks.drain(..) {
            task.abort();
            let _ = task.await;
        }
        
        // 모든 작성기 종료
        let mut writers = self.writers.write().await;
        for (service_type, writer) in writers.drain() {
            debug!(service = service_type.as_str(), "로그 작성기 종료 중");
            if let Ok(writer) = Arc::try_unwrap(writer) {
                if let Err(e) = writer.shutdown().await {
                    error!(
                        service = service_type.as_str(),
                        error = %e,
                        "로그 작성기 종료 실패"
                    );
                }
            } else {
                warn!(service = service_type.as_str(), "작성기가 여전히 참조되고 있음");
            }
        }
        
        *state = LoggingState::Shutdown;
        info!("로깅 시스템 종료됨");
        
        Ok(())
    }
    
    // 테스트용 메서드들
    pub async fn get_memory_logs(&self) -> Option<Vec<String>> {
        if let Some(memory_writer) = &self.memory_writer {
            Some(memory_writer.get_logs().await)
        } else {
            None
        }
    }
    
    pub async fn clear_memory_logs(&self) {
        if let Some(memory_writer) = &self.memory_writer {
            memory_writer.clear().await;
        }
    }
}

/// 전역 로깅 시스템 인스턴스 (싱글톤)
static GLOBAL_LOGGER: tokio::sync::OnceCell<Arc<Mutex<LoggingSystem>>> = tokio::sync::OnceCell::const_new();

/// 전역 로깅 시스템 초기화
pub async fn init_global_logging(
    service_type: ServiceType,
    base_dir: Option<&Path>,
) -> Result<()> {
    let base_dir = base_dir.unwrap_or_else(|| Path::new("./logs"));
    let mut logging_system = LoggingSystem::new(base_dir).await?;
    logging_system.init(service_type).await?;
    
    GLOBAL_LOGGER.set(Arc::new(Mutex::new(logging_system)))
        .map_err(|_| anyhow::anyhow!("전역 로깅 시스템이 이미 초기화됨"))?;
    
    Ok(())
}

/// 전역 로깅 시스템 가져오기
pub async fn get_global_logger() -> Option<Arc<Mutex<LoggingSystem>>> {
    GLOBAL_LOGGER.get().cloned()
}

/// 편의 매크로들을 위한 로그 함수들
pub async fn log_trace(message: &str, context: &[(&str, &str)]) {
    if let Some(logger) = get_global_logger().await {
        let logger = logger.lock().await;
        logger.trace(message, context).await;
    }
}

pub async fn log_debug(message: &str, context: &[(&str, &str)]) {
    if let Some(logger) = get_global_logger().await {
        let logger = logger.lock().await;
        logger.debug(message, context).await;
    }
}

pub async fn log_info(message: &str, context: &[(&str, &str)]) {
    if let Some(logger) = get_global_logger().await {
        let logger = logger.lock().await;
        logger.info(message, context).await;
    }
}

pub async fn log_warn(message: &str, context: &[(&str, &str)]) {
    if let Some(logger) = get_global_logger().await {
        let logger = logger.lock().await;
        logger.warn(message, context).await;
    }
}

pub async fn log_error(message: &str, context: &[(&str, &str)]) {
    if let Some(logger) = get_global_logger().await {
        let logger = logger.lock().await;
        logger.error(message, context).await;
    }
}

pub async fn log_fatal(message: &str, context: &[(&str, &str)]) {
    if let Some(logger) = get_global_logger().await {
        let logger = logger.lock().await;
        logger.fatal(message, context).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_logging_system_creation() {
        let temp_dir = TempDir::new().unwrap();
        let system = LoggingSystem::new(temp_dir.path()).await.unwrap();
        
        assert_eq!(system.get_state().await, LoggingState::Uninitialized);
        assert!(system.current_service.is_none());
    }
    
    #[tokio::test]
    async fn test_logging_system_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let mut system = LoggingSystem::new(temp_dir.path()).await.unwrap();
        
        system.init(ServiceType::GrpcServer).await.unwrap();
        
        assert_eq!(system.get_state().await, LoggingState::Initialized);
        assert_eq!(system.current_service, Some(ServiceType::GrpcServer));
    }
    
    #[tokio::test]
    async fn test_test_mode_logging() {
        let mut system = LoggingSystem::new_test_mode().await.unwrap();
        system.init(ServiceType::GrpcServer).await.unwrap();
        
        system.info("Test message", &[("key", "value")]).await;
        system.error("Error message", &[("error_code", "E001")]).await;
        
        let logs = system.get_memory_logs().await.unwrap();
        assert_eq!(logs.len(), 2);
        assert!(logs[0].contains("Test message"));
        assert!(logs[1].contains("Error message"));
    }
    
    #[tokio::test]
    async fn test_log_levels() {
        let mut system = LoggingSystem::new_test_mode().await.unwrap();
        system.init(ServiceType::TcpServer).await.unwrap();
        
        system.trace("Trace message", &[]).await;
        system.debug("Debug message", &[]).await;
        system.info("Info message", &[]).await;
        system.warn("Warn message", &[]).await;
        system.error("Error message", &[]).await;
        system.fatal("Fatal message", &[]).await;
        
        let logs = system.get_memory_logs().await.unwrap();
        assert_eq!(logs.len(), 6);
        
        // 각 레벨이 올바르게 포함되는지 확인 (JSON 형식에서는 "Trace", "Debug" 등으로 표시)
        assert!(logs.iter().any(|log| log.contains("\"level\":\"Trace\"") && log.contains("Trace message")));
        assert!(logs.iter().any(|log| log.contains("\"level\":\"Debug\"") && log.contains("Debug message")));
        assert!(logs.iter().any(|log| log.contains("\"level\":\"Info\"") && log.contains("Info message")));
        assert!(logs.iter().any(|log| log.contains("\"level\":\"Warn\"") && log.contains("Warn message")));
        assert!(logs.iter().any(|log| log.contains("\"level\":\"Error\"") && log.contains("Error message")));
        assert!(logs.iter().any(|log| log.contains("\"level\":\"Fatal\"") && log.contains("Fatal message")));
    }
    
    #[tokio::test]
    async fn test_context_data() {
        let mut system = LoggingSystem::new_test_mode().await.unwrap();
        system.init(ServiceType::RudpServer).await.unwrap();
        
        system.info("User login", &[
            ("user_id", "12345"),
            ("ip_address", "192.168.1.100"),
            ("user_agent", "TestClient/1.0"),
        ]).await;
        
        let logs = system.get_memory_logs().await.unwrap();
        let log_entry = &logs[0];
        
        assert!(log_entry.contains("User login"));
        // JSON 형식에서는 context 오브젝트 안에 키-값 쌍이 있음
        assert!(log_entry.contains("\"user_id\":\"12345\""));
        assert!(log_entry.contains("\"ip_address\":\"192.168.1.100\""));
        assert!(log_entry.contains("\"user_agent\":\"TestClient/1.0\""));
    }
    
    #[tokio::test]
    async fn test_global_logging() {
        init_global_logging(ServiceType::GameCenter, None).await.unwrap();
        
        log_info("Global test message", &[("test", "global")]).await;
        
        let logger = get_global_logger().await.unwrap();
        let logger = logger.lock().await;
        
        if let Some(_logs) = logger.get_memory_logs().await {
            // 전역 로거는 테스트 모드가 아니므로 메모리 로그가 없을 수 있음
            // 실제 파일 시스템을 사용하므로 여기서는 에러가 없는지만 확인
        }
    }
}