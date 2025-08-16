//! 입력 검증 모듈
//!
//! 모든 입력 데이터의 보안 검증을 담당합니다.

use crate::security::{SecurityConfig, SecurityError};
use regex::Regex;
use std::collections::HashMap;

/// 입력 검증기
pub struct InputValidator {
    config: SecurityConfig,
    /// SQL 인젝션 패턴
    sql_injection_patterns: Vec<Regex>,
    /// XSS 패턴
    xss_patterns: Vec<Regex>,
    /// 위험한 문자 패턴
    dangerous_patterns: Vec<Regex>,
}

impl InputValidator {
    /// 새 검증기 생성
    pub fn new(config: SecurityConfig) -> Result<Self, SecurityError> {
        let sql_injection_patterns = vec![
            Regex::new(r"(?i)(union\s+select)").expect("Test assertion failed"),
            Regex::new(r"(?i)(drop\s+table)").expect("Test assertion failed"),
            Regex::new(r"(?i)(insert\s+into)").expect("Test assertion failed"),
            Regex::new(r"(?i)(delete\s+from)").expect("Test assertion failed"),
            Regex::new(r"(?i)(update\s+set)").expect("Test assertion failed"),
            Regex::new(r"(?i)(exec\s*\()").expect("Test assertion failed"),
            Regex::new(r"(?i)(script\s*:)").expect("Test assertion failed"),
            Regex::new(r#"[';"\\]"#).expect("Test assertion failed"),
        ];

        let xss_patterns = vec![
            Regex::new(r"(?i)<script").expect("Test assertion failed"),
            Regex::new(r"(?i)</script>").expect("Test assertion failed"),
            Regex::new(r"(?i)javascript:").expect("Test assertion failed"),
            Regex::new(r"(?i)on\w+\s*=").expect("Test assertion failed"),
            Regex::new(r"(?i)<iframe").expect("Test assertion failed"),
            Regex::new(r"(?i)<object").expect("Test assertion failed"),
            Regex::new(r"(?i)<embed").expect("Test assertion failed"),
        ];

        let dangerous_patterns = vec![
            Regex::new(r"(?i)(\.\./|\.\.\\)").expect("Test assertion failed"), // Path traversal
            Regex::new(r"(?i)(cmd|powershell|bash|sh)\s").expect("Test assertion failed"), // Command injection
            Regex::new(r#"[<>"'&]"#).expect("Test assertion failed"), // HTML/XML 특수문자
        ];

        Ok(Self {
            config,
            sql_injection_patterns,
            xss_patterns,
            dangerous_patterns,
        })
    }

    /// 메시지 크기 검증
    pub fn validate_message_size(&self, data: &[u8]) -> Result<(), SecurityError> {
        if data.len() > self.config.max_message_size {
            return Err(SecurityError::MessageTooLarge {
                current: data.len(),
                max: self.config.max_message_size,
            });
        }
        Ok(())
    }

    /// 공통 패턴 검증 헬퍼 함수
    fn validate_patterns(
        &self,
        input: &str,
        patterns: &[Regex],
        attack_type: &str,
        error_message: &str,
    ) -> Result<(), SecurityError> {
        for pattern in patterns {
            if pattern.is_match(input) {
                tracing::warn!(
                    target: "security",
                    input = %input,
                    pattern = %pattern.as_str(),
                    attack_type = %attack_type,
                    "Security threat detected"
                );
                return Err(SecurityError::InvalidInput(error_message.to_string()));
            }
        }
        Ok(())
    }

    /// SQL 인젝션 검증
    pub fn validate_sql_injection(&self, input: &str) -> Result<(), SecurityError> {
        self.validate_patterns(
            input,
            &self.sql_injection_patterns,
            "SQL injection",
            "Potentially dangerous SQL pattern detected",
        )
    }

    /// XSS 공격 검증
    pub fn validate_xss(&self, input: &str) -> Result<(), SecurityError> {
        self.validate_patterns(
            input,
            &self.xss_patterns,
            "XSS",
            "Potentially dangerous XSS pattern detected",
        )
    }

    /// 일반 위험 패턴 검증
    pub fn validate_dangerous_patterns(&self, input: &str) -> Result<(), SecurityError> {
        self.validate_patterns(
            input,
            &self.dangerous_patterns,
            "Dangerous pattern",
            "Potentially dangerous input pattern detected",
        )
    }

    /// 길이 검증 헬퍼 함수
    fn validate_length(
        &self,
        input: &str,
        field_name: &str,
        min: usize,
        max: usize,
    ) -> Result<(), SecurityError> {
        if input.len() < min || input.len() > max {
            return Err(SecurityError::InvalidInput(format!(
                "{field_name} must be between {min} and {max} characters"
            )));
        }
        Ok(())
    }

    /// 보안 패턴 종합 검증
    fn validate_security_patterns(&self, input: &str) -> Result<(), SecurityError> {
        self.validate_sql_injection(input)?;
        self.validate_xss(input)?;
        Ok(())
    }

    /// 사용자명 검증
    pub fn validate_username(&self, username: &str) -> Result<(), SecurityError> {
        // 길이 검증
        self.validate_length(username, "Username", 3, 32)?;

        // 허용된 문자만 사용 (영숫자, 언더스코어, 하이픈)
        let valid_chars = Regex::new(r"^[a-zA-Z0-9_-]+$").expect("Test assertion failed");
        if !valid_chars.is_match(username) {
            return Err(SecurityError::InvalidInput(
                "Username contains invalid characters".to_string(),
            ));
        }

        // 보안 패턴 검증
        self.validate_security_patterns(username)?;

        Ok(())
    }

    /// 비밀번호 검증
    pub fn validate_password(&self, password: &str) -> Result<(), SecurityError> {
        // 길이 검증
        self.validate_length(password, "Password", 8, 128)?;

        // 복잡성 검증
        let has_lower = password.chars().any(|c| c.is_lowercase());
        let has_upper = password.chars().any(|c| c.is_uppercase());
        let has_digit = password.chars().any(|c| c.is_numeric());
        let has_special = password
            .chars()
            .any(|c| "!@#$%^&*(),.?\":{}|<>".contains(c));

        let complexity_score = [has_lower, has_upper, has_digit, has_special]
            .iter()
            .filter(|&&x| x)
            .count();

        if complexity_score < 3 {
            return Err(SecurityError::InvalidInput(
                "Password must contain at least 3 of: lowercase, uppercase, digit, special character".to_string()
            ));
        }

        Ok(())
    }

    /// 이메일 검증
    pub fn validate_email(&self, email: &str) -> Result<(), SecurityError> {
        let email_pattern = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
            .expect("Test assertion failed");

        if !email_pattern.is_match(email) {
            return Err(SecurityError::InvalidInput(
                "Invalid email format".to_string(),
            ));
        }

        // 보안 패턴 검증
        self.validate_security_patterns(email)?;

        Ok(())
    }

    /// JSON 페이로드 검증
    pub fn validate_json_payload(&self, json: &str) -> Result<(), SecurityError> {
        // 크기 검증
        self.validate_message_size(json.as_bytes())?;

        // JSON 파싱 검증
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| SecurityError::InvalidInput(format!("Invalid JSON: {e}")))?;

        // 보안 패턴 검증
        self.validate_sql_injection(json)?;
        self.validate_xss(json)?;
        self.validate_dangerous_patterns(json)?;

        Ok(())
    }

    /// IP 주소 검증
    pub fn validate_ip_address(&self, ip: &str) -> Result<(), SecurityError> {
        use std::net::IpAddr;

        let parsed_ip: IpAddr = ip
            .parse()
            .map_err(|_| SecurityError::InvalidInput("Invalid IP address format".to_string()))?;

        // 로컬/사설망 체크 (프로덕션에서는 제한할 수 있음)
        match parsed_ip {
            IpAddr::V4(ipv4) => {
                if ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local() {
                    tracing::debug!("Local/private IP detected: {}", ip);
                }
            }
            IpAddr::V6(ipv6) => {
                if ipv6.is_loopback() || ipv6.is_unspecified() {
                    tracing::debug!("Local IPv6 detected: {}", ip);
                }
            }
        }

        Ok(())
    }

    /// 종합 사용자 입력 검증
    pub fn validate_user_input(
        &self,
        input: &HashMap<String, String>,
    ) -> Result<(), SecurityError> {
        for (key, value) in input {
            // 키 검증
            self.validate_sql_injection(key)?;
            self.validate_xss(key)?;

            // 값 검증
            match key.as_str() {
                "username" => self.validate_username(value)?,
                "password" => self.validate_password(value)?,
                "email" => self.validate_email(value)?,
                _ => {
                    // 일반 텍스트 검증
                    self.validate_sql_injection(value)?;
                    self.validate_xss(value)?;
                    self.validate_dangerous_patterns(value)?;

                    // 길이 제한
                    if value.len() > 1000 {
                        return Err(SecurityError::InvalidInput(format!(
                            "Field '{key}' is too long (max 1000 characters)"
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// 문자열 살균 (sanitize)
    pub fn sanitize_string(&self, input: &str) -> String {
        input
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
            .replace('&', "&amp;")
            .trim()
            .to_string()
    }
}

mod tests {

    #[test]
    fn test_sql_injection_detection() {
        let config = SecurityConfig::default();
        let validator = InputValidator::new(config).expect("Test assertion failed");

        // SQL 인젝션 패턴들
        let malicious_inputs = vec![
            "'; DROP TABLE users; --",
            "1' UNION SELECT * FROM passwords",
            "admin'--",
            "1; DELETE FROM users WHERE 1=1",
        ];

        for input in malicious_inputs {
            assert!(validator.validate_sql_injection(input).is_err());
        }
    }

    #[test]
    fn test_xss_detection() {
        let config = SecurityConfig::default();
        let validator = InputValidator::new(config).expect("Test assertion failed");

        let xss_inputs = vec![
            "<script>alert('XSS')</script>",
            "javascript:alert('XSS')",
            "<img onerror=\"alert('XSS')\" src=\"x\">",
            "<iframe src=\"javascript:alert('XSS')\"></iframe>",
        ];

        for input in xss_inputs {
            assert!(validator.validate_xss(input).is_err());
        }
    }

    #[test]
    fn test_username_validation() {
        let config = SecurityConfig::default();
        let validator = InputValidator::new(config).expect("Test assertion failed");

        // 유효한 사용자명
        assert!(validator.validate_username("user123").is_ok());
        assert!(validator.validate_username("test_user").is_ok());
        assert!(validator.validate_username("player-1").is_ok());

        // 무효한 사용자명
        assert!(validator.validate_username("ab").is_err()); // 너무 짧음
        assert!(validator.validate_username("user@domain").is_err()); // 특수문자
        assert!(validator.validate_username("<script>").is_err()); // XSS
    }

    #[test]
    fn test_password_validation() {
        let config = SecurityConfig::default();
        let validator = InputValidator::new(config).expect("Test assertion failed");

        // 유효한 비밀번호
        assert!(validator.validate_password("SecurePass123!").is_ok());
        assert!(validator.validate_password("MyP@ssw0rd").is_ok());

        // 무효한 비밀번호
        assert!(validator.validate_password("123").is_err()); // 너무 짧음
        assert!(validator.validate_password("password").is_err()); // 복잡성 부족
        assert!(validator.validate_password("PASSWORD").is_err()); // 복잡성 부족
    }
}
