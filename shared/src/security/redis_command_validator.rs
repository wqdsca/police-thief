//! Redis 명령어 보안 검증기
//! 
//! Redis 명령어에 대한 입력 검증, 화이트리스트 검증, 주입 공격 방지를 제공합니다.
//! OWASP Top 10 A03 (Injection) 대응을 위한 포괄적인 보안 구현.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{error, warn};

/// Redis 명령어 검증 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisCommandValidatorConfig {
    /// 허용된 Redis 명령어 (대소문자 구분 없음)
    pub allowed_commands: HashSet<String>,
    /// 최대 키 길이
    pub max_key_length: usize,
    /// 최대 필드명 길이
    pub max_field_length: usize,
    /// 최대 값 크기 (바이트)
    pub max_value_size: usize,
    /// 엄격 모드 (허용 목록에 없는 명령어 차단)
    pub strict_mode: bool,
    /// 위험 패턴 감지 활성화
    pub enable_dangerous_pattern_detection: bool,
}

impl Default for RedisCommandValidatorConfig {
    fn default() -> Self {
        // 게임 서버에 필요한 안전한 Redis 명령어만 허용
        let mut allowed_commands = HashSet::new();
        
        // 기본 키-값 명령어
        allowed_commands.insert("GET".to_string());
        allowed_commands.insert("SET".to_string());
        allowed_commands.insert("DEL".to_string());
        allowed_commands.insert("EXISTS".to_string());
        allowed_commands.insert("EXPIRE".to_string());
        allowed_commands.insert("TTL".to_string());
        
        // 해시 명령어
        allowed_commands.insert("HGET".to_string());
        allowed_commands.insert("HSET".to_string());
        allowed_commands.insert("HDEL".to_string());
        allowed_commands.insert("HGETALL".to_string());
        allowed_commands.insert("HEXISTS".to_string());
        allowed_commands.insert("HKEYS".to_string());
        allowed_commands.insert("HVALS".to_string());
        allowed_commands.insert("HINCR".to_string());
        allowed_commands.insert("HINCRBY".to_string());
        allowed_commands.insert("HMGET".to_string());
        allowed_commands.insert("HSET_MULTIPLE".to_string());
        
        // 정렬된 집합 (게임 랭킹용)
        allowed_commands.insert("ZADD".to_string());
        allowed_commands.insert("ZREM".to_string());
        allowed_commands.insert("ZRANGE".to_string());
        allowed_commands.insert("ZREVRANGE".to_string());
        allowed_commands.insert("ZRANK".to_string());
        allowed_commands.insert("ZREVRANK".to_string());
        allowed_commands.insert("ZSCORE".to_string());
        allowed_commands.insert("ZCARD".to_string());
        allowed_commands.insert("ZCOUNT".to_string());
        allowed_commands.insert("ZINCRBY".to_string());
        
        // 리스트 (ID 재활용용)
        allowed_commands.insert("LPUSH".to_string());
        allowed_commands.insert("RPUSH".to_string());
        allowed_commands.insert("LPOP".to_string());
        allowed_commands.insert("RPOP".to_string());
        allowed_commands.insert("LLEN".to_string());
        allowed_commands.insert("LRANGE".to_string());
        
        // 관리 명령어
        allowed_commands.insert("PING".to_string());
        allowed_commands.insert("INCR".to_string());
        allowed_commands.insert("DECR".to_string());
        allowed_commands.insert("INCRBY".to_string());
        allowed_commands.insert("DECRBY".to_string());
        
        Self {
            allowed_commands,
            max_key_length: 250,        // Redis 키 최대 길이 제한
            max_field_length: 100,      // 해시 필드명 최대 길이
            max_value_size: 1024 * 1024, // 1MB 값 크기 제한
            strict_mode: true,          // 프로덕션에서는 엄격 모드 활성화
            enable_dangerous_pattern_detection: true,
        }
    }
}

/// Redis 명령어 검증기
#[derive(Debug)]
pub struct RedisCommandValidator {
    config: RedisCommandValidatorConfig,
    dangerous_patterns: Vec<Regex>,
}

impl RedisCommandValidator {
    /// 새 Redis 명령어 검증기 생성
    pub fn new(config: RedisCommandValidatorConfig) -> Result<Self> {
        let dangerous_patterns = Self::compile_dangerous_patterns()?;
        
        Ok(Self {
            config,
            dangerous_patterns,
        })
    }
    
    /// 기본 설정으로 검증기 생성
    pub fn default() -> Result<Self> {
        Self::new(RedisCommandValidatorConfig::default())
    }
    
    /// 위험 패턴 정규식 컴파일
    fn compile_dangerous_patterns() -> Result<Vec<Regex>> {
        let patterns = vec![
            // Lua 스크립트 주입 방지
            r"(?i)(eval|evalsha|script)",
            
            // 시스템 명령어 실행 방지
            r"(?i)(config|debug|save|bgsave|flushall|flushdb|shutdown)",
            
            // 관리자 명령어 방지
            r"(?i)(keys|scan|info|client|monitor|slowlog)",
            
            // SQL 주입 패턴 감지
            r"(?i)(union|select|insert|update|delete|drop|create|alter|exec)",
            
            // 스크립트 주입 패턴
            r"(?i)(<script|javascript:|vbscript:|onload|onerror)",
            
            // 경로 조작 패턴
            r"(\.\./|\.\.\\|/etc/|/proc/|/sys/|\\windows\\|\\system32\\)",
            
            // 명령어 체이닝 방지
            r"(;|\|\||&&|`|\$\()",
            
            // null byte 공격
            r"\x00",
        ];
        
        let mut compiled = Vec::new();
        for pattern in patterns {
            compiled.push(Regex::new(pattern)
                .with_context(|| format!("위험 패턴 정규식 컴파일 실패: {}", pattern))?);
        }
        
        Ok(compiled)
    }
    
    /// Redis 명령어 검증
    pub fn validate_command(&self, command: &str) -> Result<()> {
        let command_upper = command.to_uppercase();
        
        // 1. 명령어 화이트리스트 검증
        if self.config.strict_mode && !self.config.allowed_commands.contains(&command_upper) {
            error!(
                target: "security::redis_validation",
                command = %command,
                "차단된 Redis 명령어 감지"
            );
            return Err(anyhow::anyhow!(
                "허용되지 않은 Redis 명령어: {}. 허용된 명령어: {:?}", 
                command, 
                self.config.allowed_commands
            ));
        }
        
        // 2. 위험 패턴 감지
        if self.config.enable_dangerous_pattern_detection {
            for (i, pattern) in self.dangerous_patterns.iter().enumerate() {
                if pattern.is_match(command) {
                    error!(
                        target: "security::redis_validation",
                        command = %command,
                        pattern_id = i,
                        "위험 패턴이 포함된 Redis 명령어 차단"
                    );
                    return Err(anyhow::anyhow!(
                        "위험 패턴이 감지된 Redis 명령어: {}",
                        command
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    /// Redis 키 이름 검증
    pub fn validate_key(&self, key: &str) -> Result<()> {
        // 1. 키 길이 검증
        if key.len() > self.config.max_key_length {
            warn!(
                target: "security::redis_validation",
                key_length = key.len(),
                max_length = self.config.max_key_length,
                "Redis 키 길이 초과"
            );
            return Err(anyhow::anyhow!(
                "Redis 키 길이가 너무 깁니다: {} > {}",
                key.len(),
                self.config.max_key_length
            ));
        }
        
        // 2. 빈 키 방지
        if key.is_empty() {
            return Err(anyhow::anyhow!("Redis 키가 비어있습니다"));
        }
        
        // 3. 위험 문자 검증
        if self.config.enable_dangerous_pattern_detection {
            for pattern in &self.dangerous_patterns {
                if pattern.is_match(key) {
                    error!(
                        target: "security::redis_validation",
                        key = %key,
                        "위험 패턴이 포함된 Redis 키 차단"
                    );
                    return Err(anyhow::anyhow!(
                        "위험 패턴이 감지된 Redis 키: {}",
                        key
                    ));
                }
            }
        }
        
        // 4. 제어 문자 방지
        for ch in key.chars() {
            if ch.is_control() {
                return Err(anyhow::anyhow!(
                    "Redis 키에 제어 문자가 포함되어 있습니다: {:?}",
                    key
                ));
            }
        }
        
        Ok(())
    }
    
    /// Redis 필드명 검증
    pub fn validate_field(&self, field: &str) -> Result<()> {
        // 1. 필드명 길이 검증
        if field.len() > self.config.max_field_length {
            return Err(anyhow::anyhow!(
                "Redis 필드명 길이가 너무 깁니다: {} > {}",
                field.len(),
                self.config.max_field_length
            ));
        }
        
        // 2. 빈 필드명 방지
        if field.is_empty() {
            return Err(anyhow::anyhow!("Redis 필드명이 비어있습니다"));
        }
        
        // 3. 위험 패턴 검증
        if self.config.enable_dangerous_pattern_detection {
            for pattern in &self.dangerous_patterns {
                if pattern.is_match(field) {
                    error!(
                        target: "security::redis_validation",
                        field = %field,
                        "위험 패턴이 포함된 Redis 필드명 차단"
                    );
                    return Err(anyhow::anyhow!(
                        "위험 패턴이 감지된 Redis 필드명: {}",
                        field
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    /// Redis 값 검증
    pub fn validate_value(&self, value: &[u8]) -> Result<()> {
        // 1. 값 크기 검증
        if value.len() > self.config.max_value_size {
            return Err(anyhow::anyhow!(
                "Redis 값 크기가 너무 큽니다: {} > {}",
                value.len(),
                self.config.max_value_size
            ));
        }
        
        // 2. 문자열 값인 경우 위험 패턴 검증
        if let Ok(value_str) = std::str::from_utf8(value) {
            if self.config.enable_dangerous_pattern_detection {
                for pattern in &self.dangerous_patterns {
                    if pattern.is_match(value_str) {
                        warn!(
                            target: "security::redis_validation",
                            value_preview = %&value_str[..std::cmp::min(100, value_str.len())],
                            "위험 패턴이 포함된 Redis 값 경고"
                        );
                        // 값의 경우 완전히 차단하지 않고 경고만 (JSON 데이터일 수 있음)
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 포괄적인 Redis 연산 검증
    pub fn validate_operation(
        &self,
        command: &str,
        key: &str,
        field: Option<&str>,
        value: Option<&[u8]>,
    ) -> Result<()> {
        // 명령어 검증
        self.validate_command(command)
            .context("Redis 명령어 검증 실패")?;
        
        // 키 검증
        self.validate_key(key)
            .context("Redis 키 검증 실패")?;
        
        // 필드 검증 (해시 명령어의 경우)
        if let Some(f) = field {
            self.validate_field(f)
                .context("Redis 필드 검증 실패")?;
        }
        
        // 값 검증
        if let Some(v) = value {
            self.validate_value(v)
                .context("Redis 값 검증 실패")?;
        }
        
        tracing::debug!(
            target: "security::redis_validation",
            command = %command,
            key = %key,
            field = ?field,
            value_size = ?value.map(|v| v.len()),
            "✅ Redis 연산 보안 검증 통과"
        );
        
        Ok(())
    }
    
    /// 보안 통계 조회
    pub fn get_config(&self) -> &RedisCommandValidatorConfig {
        &self.config
    }
    
    /// 허용된 명령어 목록 조회
    pub fn get_allowed_commands(&self) -> Vec<String> {
        let mut commands: Vec<String> = self.config.allowed_commands.iter().cloned().collect();
        commands.sort();
        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_allowed_commands() {
        let validator = RedisCommandValidator::default().unwrap();
        
        // 허용된 명령어
        assert!(validator.validate_command("GET").is_ok());
        assert!(validator.validate_command("set").is_ok()); // 대소문자 무관
        assert!(validator.validate_command("HGET").is_ok());
        
        // 차단된 명령어
        assert!(validator.validate_command("EVAL").is_err());
        assert!(validator.validate_command("config").is_err());
        assert!(validator.validate_command("FLUSHALL").is_err());
    }
    
    #[test]
    fn test_key_validation() {
        let validator = RedisCommandValidator::default().unwrap();
        
        // 정상적인 키
        assert!(validator.validate_key("user:123").is_ok());
        assert!(validator.validate_key("room:info:456").is_ok());
        
        // 비정상적인 키
        assert!(validator.validate_key("").is_err()); // 빈 키
        assert!(validator.validate_key(&"a".repeat(300)).is_err()); // 너무 긴 키
        assert!(validator.validate_key("key\x00with\x00null").is_err()); // 제어 문자
    }
    
    #[test]
    fn test_dangerous_patterns() {
        let validator = RedisCommandValidator::default().unwrap();
        
        // SQL 주입 패턴
        assert!(validator.validate_key("user'; DROP TABLE users; --").is_err());
        assert!(validator.validate_field("field UNION SELECT").is_err());
        
        // 스크립트 주입 패턴
        assert!(validator.validate_key("<script>alert('xss')</script>").is_err());
        assert!(validator.validate_field("javascript:void(0)").is_err());
        
        // 경로 조작 패턴
        assert!(validator.validate_key("../../../etc/passwd").is_err());
        assert!(validator.validate_key("\\windows\\system32\\config").is_err());
    }
    
    #[test]
    fn test_field_validation() {
        let validator = RedisCommandValidator::default().unwrap();
        
        // 정상적인 필드
        assert!(validator.validate_field("username").is_ok());
        assert!(validator.validate_field("score_2024").is_ok());
        
        // 비정상적인 필드
        assert!(validator.validate_field("").is_err()); // 빈 필드
        assert!(validator.validate_field(&"a".repeat(200)).is_err()); // 너무 긴 필드
    }
    
    #[test]
    fn test_value_validation() {
        let validator = RedisCommandValidator::default().unwrap();
        
        // 정상적인 값
        assert!(validator.validate_value(b"normal_value").is_ok());
        assert!(validator.validate_value(b"{\"user_id\":123,\"score\":456}").is_ok());
        
        // 비정상적인 값
        let huge_value = vec![0u8; 2 * 1024 * 1024]; // 2MB
        assert!(validator.validate_value(&huge_value).is_err());
    }
    
    #[test]
    fn test_comprehensive_operation() {
        let validator = RedisCommandValidator::default().unwrap();
        
        // 정상적인 HSET 연산
        assert!(validator.validate_operation(
            "HSET",
            "user:123",
            Some("username"),
            Some(b"john_doe")
        ).is_ok());
        
        // 비정상적인 연산 - 위험한 명령어
        assert!(validator.validate_operation(
            "EVAL",
            "user:123",
            None,
            Some(b"redis.call('FLUSHALL')")
        ).is_err());
        
        // 비정상적인 연산 - 위험한 키
        assert!(validator.validate_operation(
            "GET",
            "../../../etc/passwd",
            None,
            None
        ).is_err());
    }
    
    #[test]
    fn test_allowed_commands_list() {
        let validator = RedisCommandValidator::default().unwrap();
        let commands = validator.get_allowed_commands();
        
        // 기본 명령어들이 포함되어 있는지 확인
        assert!(commands.contains(&"GET".to_string()));
        assert!(commands.contains(&"SET".to_string()));
        assert!(commands.contains(&"HGET".to_string()));
        assert!(commands.contains(&"ZADD".to_string()));
        
        // 위험한 명령어는 포함되지 않았는지 확인
        assert!(!commands.contains(&"EVAL".to_string()));
        assert!(!commands.contains(&"CONFIG".to_string()));
        assert!(!commands.contains(&"FLUSHALL".to_string()));
        
        println!("허용된 Redis 명령어 ({} 개): {:?}", commands.len(), commands);
    }
}