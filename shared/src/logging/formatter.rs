//! 로그 포매터 
//!
//! 로그 항목의 형식화와 구조화를 담당합니다.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

/// 로그 레벨 열거형
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    /// 상세한 추적 정보 (개발환경)
    Trace = 0,
    /// 디버깅 정보 (개발/스테이징) 
    Debug = 1,
    /// 일반 정보 (모든 환경)
    Info = 2,
    /// 경고 상황 (복구 가능한 오류)
    Warn = 3,
    /// 오류 상황 (복구 불가능한 오류)
    Error = 4,
    /// 시스템 중단 수준 오류
    Fatal = 5,
}

impl LogLevel {
    /// 로그 레벨을 문자열로 변환
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG", 
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
        }
    }
    
    /// 문자열에서 로그 레벨 파싱 (레거시 호환성)
    /// 
    /// 추천: `s.parse::<LogLevel>()` 또는 `LogLevel::from_str()` trait 사용
    pub fn from_string(s: &str) -> Option<Self> {
        s.parse().ok()
    }
    
    /// ANSI 색상 코드 반환
    pub fn color_code(&self) -> &'static str {
        match self {
            LogLevel::Trace => "\x1b[90m",   // 회색
            LogLevel::Debug => "\x1b[36m",   // 청록색
            LogLevel::Info => "\x1b[32m",    // 녹색
            LogLevel::Warn => "\x1b[33m",    // 노란색
            LogLevel::Error => "\x1b[31m",   // 빨간색
            LogLevel::Fatal => "\x1b[35m",   // 자홍색
        }
    }
}

impl FromStr for LogLevel {
    type Err = ();
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "TRACE" => Ok(LogLevel::Trace),
            "DEBUG" => Ok(LogLevel::Debug),
            "INFO" => Ok(LogLevel::Info),
            "WARN" => Ok(LogLevel::Warn),
            "ERROR" => Ok(LogLevel::Error),
            "FATAL" => Ok(LogLevel::Fatal),
            _ => Err(()),
        }
    }
}

/// 구조화된 로그 항목
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// 타임스탬프 (ISO 8601 형식)
    pub timestamp: DateTime<Utc>,
    
    /// 로그 레벨
    pub level: LogLevel,
    
    /// 서비스 이름
    pub service: String,
    
    /// 로그 메시지
    pub message: String,
    
    /// 추가 컨텍스트 데이터
    pub context: HashMap<String, serde_json::Value>,
    
    /// 스레드 ID (선택적)
    pub thread_id: Option<String>,
    
    /// 모듈 경로 (선택적)
    pub module_path: Option<String>,
    
    /// 파일 위치 (선택적)
    pub file_location: Option<String>,
}

impl LogEntry {
    /// 새 로그 항목 생성
    pub fn new(
        level: LogLevel,
        service: String,
        message: String,
        context: &[(&str, &str)],
    ) -> Self {
        let mut context_map = HashMap::new();
        for (key, value) in context {
            context_map.insert(key.to_string(), serde_json::Value::String(value.to_string()));
        }
        
        Self {
            timestamp: Utc::now(),
            level,
            service,
            message,
            context: context_map,
            thread_id: Some(format!("{:?}", std::thread::current().id())),
            module_path: None,
            file_location: None,
        }
    }
    
    /// 컨텍스트 데이터 추가
    pub fn add_context<K, V>(&mut self, key: K, value: V) 
    where 
        K: Into<String>,
        V: Into<serde_json::Value>,
    {
        self.context.insert(key.into(), value.into());
    }
    
    /// 모듈 경로 설정
    pub fn with_module_path<S: Into<String>>(mut self, path: S) -> Self {
        self.module_path = Some(path.into());
        self
    }
    
    /// 파일 위치 설정
    pub fn with_file_location<S: Into<String>>(mut self, location: S) -> Self {
        self.file_location = Some(location.into());
        self
    }
}

/// 로그 포매터
pub struct LogFormatter {
    /// JSON 형식 사용 여부
    json_format: bool,
    /// 색상 출력 여부
    colored_output: bool,
}

impl LogFormatter {
    /// 새 포매터 생성
    pub fn new(json_format: bool, colored_output: bool) -> Self {
        Self {
            json_format,
            colored_output,
        }
    }
    
    /// 로그 항목을 문자열로 포매팅
    pub fn format(&self, entry: &LogEntry) -> anyhow::Result<String> {
        if self.json_format {
            self.format_json(entry)
        } else {
            Ok(self.format_text(entry))
        }
    }
    
    /// JSON 형식으로 포매팅
    fn format_json(&self, entry: &LogEntry) -> anyhow::Result<String> {
        let json_str = serde_json::to_string(entry)?;
        Ok(json_str)
    }
    
    /// 텍스트 형식으로 포매팅
    fn format_text(&self, entry: &LogEntry) -> String {
        let timestamp = entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f");
        let level_str = if self.colored_output {
            format!("{}{}[{}]{}", 
                entry.level.color_code(), 
                entry.level.as_str(),
                "\x1b[0m", // 색상 리셋
                " ".repeat(5 - entry.level.as_str().len()) // 정렬용 패딩
            )
        } else {
            format!("[{}]", entry.level.as_str())
        };
        
        let mut formatted = format!(
            "{} {} [{}] {}",
            timestamp,
            level_str,
            entry.service,
            entry.message
        );
        
        // 컨텍스트 데이터 추가
        if !entry.context.is_empty() {
            let context_str = entry.context
                .iter()
                .map(|(k, v)| {
                    let value_str = match v {
                        serde_json::Value::String(s) => s.clone(),
                        _ => v.to_string()
                    };
                    format!("{}={}", k, value_str)
                })
                .collect::<Vec<_>>()
                .join(" ");
            formatted.push_str(&format!(" [{}]", context_str));
        }
        
        // 스레드 ID 추가
        if let Some(thread_id) = &entry.thread_id {
            formatted.push_str(&format!(" thread:{}", thread_id));
        }
        
        formatted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_log_level_as_str() {
        assert_eq!(LogLevel::Trace.as_str(), "TRACE");
        assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
        assert_eq!(LogLevel::Info.as_str(), "INFO");
        assert_eq!(LogLevel::Warn.as_str(), "WARN");
        assert_eq!(LogLevel::Error.as_str(), "ERROR");
        assert_eq!(LogLevel::Fatal.as_str(), "FATAL");
    }
    
    #[test]
    fn test_log_level_from_str() {
        assert_eq!("TRACE".parse(), Ok(LogLevel::Trace));
        assert_eq!("trace".parse(), Ok(LogLevel::Trace));
        assert_eq!("INFO".parse(), Ok(LogLevel::Info));
        assert_eq!("INVALID".parse::<LogLevel>(), Err(()));
    }
    
    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
        assert!(LogLevel::Error < LogLevel::Fatal);
    }
    
    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::new(
            LogLevel::Info,
            "test-service".to_string(),
            "Test message".to_string(),
            &[("user_id", "123"), ("action", "login")],
        );
        
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.service, "test-service");
        assert_eq!(entry.message, "Test message");
        assert_eq!(entry.context.len(), 2);
        assert!(entry.thread_id.is_some());
    }
    
    #[test]
    fn test_formatter_json() {
        let formatter = LogFormatter::new(true, false);
        let entry = LogEntry::new(
            LogLevel::Info,
            "test".to_string(),
            "Test message".to_string(),
            &[],
        );
        
        let formatted = formatter.format(&entry).unwrap();
        assert!(formatted.contains("\"level\":\"Info\""));
        assert!(formatted.contains("\"service\":\"test\""));
        assert!(formatted.contains("\"message\":\"Test message\""));
    }
    
    #[test]
    fn test_formatter_text() {
        let formatter = LogFormatter::new(false, false);
        let entry = LogEntry::new(
            LogLevel::Info,
            "test".to_string(),
            "Test message".to_string(),
            &[("key", "value")],
        );
        
        let formatted = formatter.format(&entry).unwrap();
        assert!(formatted.contains("[INFO]"));
        assert!(formatted.contains("[test]"));
        assert!(formatted.contains("Test message"));
        assert!(formatted.contains("key=value"));
    }
}