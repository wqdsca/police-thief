//! 비동기 로그 작성기
//!
//! 비동기 방식으로 로그를 파일에 작성하여 성능 영향을 최소화합니다.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, warn};

use crate::logging::config::LoggingConfig;
use crate::logging::formatter::{LogFormatter, LogEntry};

/// 로그 작성 명령
#[derive(Debug)]
enum WriteCommand {
    /// 로그 항목 작성
    Write(LogEntry),
    /// 플러시 수행
    Flush,
    /// 작성기 종료
    Shutdown,
}

/// 비동기 로그 작성기
pub struct AsyncLogWriter {
    /// 명령 전송 채널
    sender: mpsc::UnboundedSender<WriteCommand>,
    /// 작성기 핸들 (종료 대기용)
    writer_handle: Option<tokio::task::JoinHandle<()>>,
}

impl AsyncLogWriter {
    /// 새 비동기 로그 작성기 생성
    pub async fn new(
        log_file_path: PathBuf,
        config: LoggingConfig,
        formatter: Arc<LogFormatter>,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        // 백그라운드 작성기 태스크 시작
        let writer_handle = tokio::spawn(Self::writer_task(
            log_file_path,
            receiver,
            config,
            formatter,
        ));
        
        Ok(Self {
            sender,
            writer_handle: Some(writer_handle),
        })
    }
    
    /// 로그 항목 작성 (논블로킹)
    pub fn write_log(&self, entry: LogEntry) -> Result<()> {
        self.sender.send(WriteCommand::Write(entry))
            .context("로그 작성 명령 전송 실패")?;
        Ok(())
    }
    
    /// 즉시 플러시 요청 (논블로킹)
    pub fn flush(&self) -> Result<()> {
        self.sender.send(WriteCommand::Flush)
            .context("플러시 명령 전송 실패")?;
        Ok(())
    }
    
    /// 작성기 종료 및 리소스 정리
    pub async fn shutdown(mut self) -> Result<()> {
        // 종료 명령 전송
        let _ = self.sender.send(WriteCommand::Shutdown);
        
        // 작성기 태스크 종료 대기
        if let Some(handle) = self.writer_handle.take() {
            handle.await.context("작성기 태스크 종료 대기 실패")?;
        }
        
        debug!("비동기 로그 작성기 종료됨");
        Ok(())
    }
    
    /// 백그라운드 작성기 태스크
    async fn writer_task(
        log_file_path: PathBuf,
        mut receiver: mpsc::UnboundedReceiver<WriteCommand>,
        config: LoggingConfig,
        formatter: Arc<LogFormatter>,
    ) {
        let mut writer = match Self::create_writer(&log_file_path).await {
            Ok(w) => w,
            Err(e) => {
                error!(
                    path = %log_file_path.display(),
                    error = %e,
                    "로그 파일 작성기 생성 실패"
                );
                return;
            }
        };
        
        let mut flush_interval = interval(config.flush_interval);
        let mut buffer = Vec::with_capacity(1024);
        let mut pending_writes = 0;
        
        debug!(
            path = %log_file_path.display(),
            flush_interval = ?config.flush_interval,
            "로그 작성기 태스크 시작됨"
        );
        
        loop {
            tokio::select! {
                // 명령 수신
                cmd = receiver.recv() => {
                    match cmd {
                        Some(WriteCommand::Write(entry)) => {
                            if let Err(e) = Self::write_entry(&mut buffer, &formatter, &entry).await {
                                error!(error = %e, "로그 항목 포매팅 실패");
                                continue;
                            }
                            pending_writes += 1;
                            
                            // 버퍼가 가득 차면 즉시 플러시
                            if pending_writes >= 100 || buffer.len() >= 64 * 1024 { // 64KB
                                if let Err(e) = Self::flush_buffer(&mut writer, &mut buffer).await {
                                    error!(error = %e, "로그 버퍼 플러시 실패");
                                }
                                pending_writes = 0;
                            }
                        }
                        Some(WriteCommand::Flush) => {
                            if let Err(e) = Self::flush_buffer(&mut writer, &mut buffer).await {
                                error!(error = %e, "로그 플러시 실패");
                            }
                            pending_writes = 0;
                        }
                        Some(WriteCommand::Shutdown) => {
                            // 남은 데이터 플러시 후 종료
                            if let Err(e) = Self::flush_buffer(&mut writer, &mut buffer).await {
                                error!(error = %e, "종료 시 로그 플러시 실패");
                            }
                            debug!("로그 작성기 태스크 종료");
                            return;
                        }
                        None => {
                            warn!("로그 작성기 채널이 닫힘");
                            return;
                        }
                    }
                }
                
                // 주기적 플러시
                _ = flush_interval.tick() => {
                    if pending_writes > 0 {
                        if let Err(e) = Self::flush_buffer(&mut writer, &mut buffer).await {
                            error!(error = %e, "주기적 로그 플러시 실패");
                        }
                        pending_writes = 0;
                    }
                }
            }
        }
    }
    
    /// 파일 작성기 생성
    async fn create_writer(path: &PathBuf) -> Result<BufWriter<tokio::fs::File>> {
        // 디렉토리가 없으면 생성
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await
                .context("로그 디렉토리 생성 실패")?;
        }
        
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
            .context("로그 파일 열기 실패")?;
        
        Ok(BufWriter::new(file))
    }
    
    /// 로그 항목을 버퍼에 작성
    async fn write_entry(
        buffer: &mut Vec<u8>,
        formatter: &LogFormatter,
        entry: &LogEntry,
    ) -> Result<()> {
        let formatted = formatter.format(entry)?;
        buffer.extend_from_slice(formatted.as_bytes());
        buffer.push(b'\n');
        Ok(())
    }
    
    /// 버퍼를 파일에 플러시
    async fn flush_buffer(
        writer: &mut BufWriter<tokio::fs::File>,
        buffer: &mut Vec<u8>,
    ) -> Result<()> {
        if buffer.is_empty() {
            return Ok(());
        }
        
        writer.write_all(buffer).await
            .context("로그 데이터 작성 실패")?;
        
        writer.flush().await
            .context("로그 파일 플러시 실패")?;
        
        buffer.clear();
        Ok(())
    }
}

impl Drop for AsyncLogWriter {
    fn drop(&mut self) {
        // Drop에서는 async를 사용할 수 없으므로 종료 신호만 전송
        let _ = self.sender.send(WriteCommand::Shutdown);
        
        if let Some(handle) = self.writer_handle.take() {
            // 백그라운드에서 종료 대기 (결과는 무시)
            tokio::spawn(async move {
                let _ = handle.await;
            });
        }
    }
}

/// 메모리 내 로그 작성기 (테스트용)
pub struct InMemoryLogWriter {
    /// 로그 항목들
    pub entries: Arc<tokio::sync::Mutex<Vec<String>>>,
    /// 포매터
    formatter: Arc<LogFormatter>,
}

impl InMemoryLogWriter {
    /// 새 메모리 내 작성기 생성
    pub fn new(formatter: Arc<LogFormatter>) -> Self {
        Self {
            entries: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            formatter,
        }
    }
    
    /// 로그 항목 작성
    pub async fn write_log(&self, entry: LogEntry) -> Result<()> {
        let formatted = self.formatter.format(&entry)?;
        let mut entries = self.entries.lock().await;
        entries.push(formatted);
        Ok(())
    }
    
    /// 모든 로그 항목 반환
    pub async fn get_logs(&self) -> Vec<String> {
        let entries = self.entries.lock().await;
        entries.clone()
    }
    
    /// 로그 개수 반환
    pub async fn len(&self) -> usize {
        let entries = self.entries.lock().await;
        entries.len()
    }
    
    /// 로그가 비어있는지 확인
    pub async fn is_empty(&self) -> bool {
        let entries = self.entries.lock().await;
        entries.is_empty()
    }
    
    /// 로그 지우기
    pub async fn clear(&self) {
        let mut entries = self.entries.lock().await;
        entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::formatter::{LogFormatter, LogLevel, LogEntry};
    use tempfile::TempDir;
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};
    
    fn test_config() -> LoggingConfig {
        LoggingConfig {
            flush_interval: Duration::from_millis(100),
            ..Default::default()
        }
    }
    
    fn test_formatter() -> Arc<LogFormatter> {
        Arc::new(LogFormatter::new(false, false))
    }
    
    #[tokio::test]
    async fn test_async_log_writer() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");
        
        let config = test_config();
        let formatter = test_formatter();
        
        let writer = AsyncLogWriter::new(log_path.clone(), config, formatter).await.unwrap();
        
        // 로그 항목 작성
        let entry = LogEntry::new(
            LogLevel::Info,
            "test-service".to_string(),
            "Test message".to_string(),
            &[("key", "value")],
        );
        
        writer.write_log(entry).unwrap();
        writer.flush().unwrap();
        
        // 잠시 대기 (비동기 작성 완료를 위해)
        sleep(Duration::from_millis(200)).await;
        
        // 파일 내용 확인
        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        assert!(content.contains("Test message"));
        assert!(content.contains("[INFO]"));
        assert!(content.contains("test-service"));
        
        // 정리
        writer.shutdown().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_memory_writer() {
        let formatter = test_formatter();
        let writer = InMemoryLogWriter::new(formatter);
        
        let entry = LogEntry::new(
            LogLevel::Error,
            "test-service".to_string(),
            "Error message".to_string(),
            &[],
        );
        
        writer.write_log(entry).await.unwrap();
        
        let logs = writer.get_logs().await;
        assert_eq!(logs.len(), 1);
        assert!(logs[0].contains("Error message"));
        assert!(logs[0].contains("[ERROR]"));
    }
    
    #[tokio::test]
    async fn test_buffer_flush() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test_flush.log");
        
        let config = test_config();
        let formatter = test_formatter();
        
        let writer = AsyncLogWriter::new(log_path.clone(), config, formatter).await.unwrap();
        
        // 여러 로그 항목 작성
        for i in 0..10 {
            let entry = LogEntry::new(
                LogLevel::Debug,
                "test".to_string(),
                format!("Message {}", i),
                &[],
            );
            writer.write_log(entry).unwrap();
        }
        
        // 명시적 플러시
        writer.flush().unwrap();
        
        // 잠시 대기
        sleep(Duration::from_millis(200)).await;
        
        // 파일 내용 확인
        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        for i in 0..10 {
            assert!(content.contains(&format!("Message {}", i)));
        }
        
        writer.shutdown().await.unwrap();
    }
}