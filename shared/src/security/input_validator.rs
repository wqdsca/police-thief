use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::warn;

lazy_static! {
    // 보안 패턴 컴파일
    static ref SQL_INJECTION_PATTERN: Regex = Regex::new(
        r#"(?i)(\b(select|insert|update|delete|drop|union|exec|execute|declare|create|alter|grant|revoke)\b|--|;|'|"|\x00|\n|\r|\x1a)"#
    ).expect("Test assertion failed");

    static ref XSS_PATTERN: Regex = Regex::new(
        r#"(?i)(<script|<iframe|javascript:|on\w+\s*=|<img.*?src|<object|<embed|<applet|<meta|<link|eval\(|expression\()"#
    ).expect("Test assertion failed");

    static ref PATH_TRAVERSAL_PATTERN: Regex = Regex::new(
        r#"(\.\.[/\\]|\.\..%2[fF]|%2e%2e|\.\..\\|\.\../)"#
    ).expect("Test assertion failed");

    static ref COMMAND_INJECTION_PATTERN: Regex = Regex::new(
        r#"([;&|`$]|\$\(|\||&&|\|\||>|<|>>|<<)"#
    ).expect("Test assertion failed");

    static ref EMAIL_PATTERN: Regex = Regex::new(
        r#"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"#
    ).expect("Test assertion failed");

    static ref USERNAME_PATTERN: Regex = Regex::new(
        r#"^[a-zA-Z0-9_-]{3,32}$"#
    ).expect("Test assertion failed");

    static ref SAFE_STRING_PATTERN: Regex = Regex::new(
        r#"^[a-zA-Z0-9\s\-_.,!?@#$%^&*()\[\]{}+=:;'"]+$"#
    ).expect("Test assertion failed");
}

/// 입력 유형 정의
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputType {
    Username,
    Email,
    Password,
    Text,
    Number,
    Json,
    Url,
    FilePath,
    RoomName,
    ChatMessage,
}

/// 검증 규칙
#[derive(Debug, Clone)]
pub struct ValidationRule {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<Regex>,
    pub required: bool,
    pub sanitize: bool,
}

impl Default for ValidationRule {
    fn default() -> Self {
        Self {
            min_length: None,
            max_length: Some(10000),
            pattern: None,
            required: true,
            sanitize: true,
        }
    }
}

/// 입력 검증기
pub struct InputValidator {
    rules: HashMap<InputType, ValidationRule>,
    max_input_size: usize,
    enable_logging: bool,
}

impl InputValidator {
    pub fn new() -> Self {
        let mut rules = HashMap::new();

        // Username 규칙
        rules.insert(
            InputType::Username,
            ValidationRule {
                min_length: Some(3),
                max_length: Some(32),
                pattern: Some(USERNAME_PATTERN.clone()),
                required: true,
                sanitize: true,
            },
        );

        // Email 규칙
        rules.insert(
            InputType::Email,
            ValidationRule {
                min_length: Some(5),
                max_length: Some(254),
                pattern: Some(EMAIL_PATTERN.clone()),
                required: true,
                sanitize: true,
            },
        );

        // Password 규칙
        rules.insert(
            InputType::Password,
            ValidationRule {
                min_length: Some(8),
                max_length: Some(128),
                pattern: None, // 패스워드는 패턴 체크 안함
                required: true,
                sanitize: false, // 패스워드는 sanitize 안함
            },
        );

        // Room Name 규칙
        rules.insert(
            InputType::RoomName,
            ValidationRule {
                min_length: Some(1),
                max_length: Some(50),
                pattern: Some(SAFE_STRING_PATTERN.clone()),
                required: true,
                sanitize: true,
            },
        );

        // Chat Message 규칙
        rules.insert(
            InputType::ChatMessage,
            ValidationRule {
                min_length: Some(1),
                max_length: Some(500),
                pattern: None,
                required: true,
                sanitize: true,
            },
        );

        // Text 규칙
        rules.insert(
            InputType::Text,
            ValidationRule {
                min_length: Some(0),
                max_length: Some(10000),
                pattern: None,
                required: false,
                sanitize: true,
            },
        );

        Self {
            rules,
            max_input_size: 1_048_576, // 1MB
            enable_logging: true,
        }
    }

    /// 입력 검증
    pub fn validate(&self, input: &str, input_type: InputType) -> Result<String, String> {
        // 크기 체크
        if input.len() > self.max_input_size {
            return Err(format!(
                "Input too large: {} bytes (max: {})",
                input.len(),
                self.max_input_size
            ));
        }

        // 규칙 가져오기
        let rule = self
            .rules
            .get(&input_type)
            .ok_or_else(|| format!("No validation rule for {input_type:?}"))?;

        // 필수 필드 체크
        if rule.required && input.is_empty() {
            return Err(format!("{input_type:?} is required"));
        }

        // 길이 체크
        if let Some(min) = rule.min_length {
            if input.len() < min {
                return Err(format!("{input_type:?} must be at least {min} characters"));
            }
        }

        if let Some(max) = rule.max_length {
            if input.len() > max {
                return Err(format!("{input_type:?} must be at most {max} characters"));
            }
        }

        // 패턴 체크
        if let Some(ref pattern) = rule.pattern {
            if !pattern.is_match(input) {
                return Err(format!("{input_type:?} has invalid format"));
            }
        }

        // 보안 체크
        if self.contains_injection(input) {
            if self.enable_logging {
                warn!(
                    "Potential injection attack detected in {:?}: {}",
                    input_type,
                    &input[..input.len().min(100)]
                );
            }
            return Err("Input contains potentially malicious content".to_string());
        }

        // Sanitize
        let output = if rule.sanitize {
            self.sanitize_input(input)
        } else {
            input.to_string()
        };

        Ok(output)
    }

    /// SQL Injection 체크
    pub fn contains_sql_injection(&self, input: &str) -> bool {
        SQL_INJECTION_PATTERN.is_match(input)
    }

    /// XSS 체크
    pub fn contains_xss(&self, input: &str) -> bool {
        XSS_PATTERN.is_match(input)
    }

    /// Path Traversal 체크
    pub fn contains_path_traversal(&self, input: &str) -> bool {
        PATH_TRAVERSAL_PATTERN.is_match(input)
    }

    /// Command Injection 체크
    pub fn contains_command_injection(&self, input: &str) -> bool {
        COMMAND_INJECTION_PATTERN.is_match(input)
    }

    /// 모든 injection 체크
    pub fn contains_injection(&self, input: &str) -> bool {
        self.contains_sql_injection(input)
            || self.contains_xss(input)
            || self.contains_path_traversal(input)
            || self.contains_command_injection(input)
    }

    /// 입력 sanitize
    pub fn sanitize_input(&self, input: &str) -> String {
        let mut sanitized = input.to_string();

        // HTML 엔티티 인코딩
        sanitized = sanitized
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
            .replace('/', "&#x2F;");

        // Null 바이트 제거
        sanitized = sanitized.replace('\0', "");

        // 제어 문자 제거 (탭과 줄바꿈 제외)
        sanitized = sanitized
            .chars()
            .filter(|c| !c.is_control() || *c == '\t' || *c == '\n' || *c == '\r')
            .collect();

        sanitized
    }

    /// 패스워드 강도 체크
    pub fn check_password_strength(&self, password: &str) -> PasswordStrength {
        let mut score: i32 = 0;

        // 길이
        if password.len() >= 8 {
            score += 1;
        }
        if password.len() >= 12 {
            score += 1;
        }
        if password.len() >= 16 {
            score += 1;
        }

        // 소문자
        if password.chars().any(|c| c.is_lowercase()) {
            score += 1;
        }

        // 대문자
        if password.chars().any(|c| c.is_uppercase()) {
            score += 1;
        }

        // 숫자
        if password.chars().any(|c| c.is_numeric()) {
            score += 1;
        }

        // 특수문자
        if password.chars().any(|c| !c.is_alphanumeric()) {
            score += 1;
        }

        // 일반적인 패스워드 체크
        let common_passwords = [
            "password",
            "123456",
            "12345678",
            "qwerty",
            "abc123",
            "password123",
            "admin",
            "letmein",
            "welcome",
            "monkey",
        ];

        let lower_password = password.to_lowercase();
        if common_passwords.iter().any(|&p| lower_password.contains(p)) {
            score = score.saturating_sub(3);
        }

        match score {
            0..=2 => PasswordStrength::Weak,
            3..=4 => PasswordStrength::Fair,
            5..=6 => PasswordStrength::Good,
            _ => PasswordStrength::Strong,
        }
    }

    /// JSON 검증
    pub fn validate_json(&self, input: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str(input).map_err(|e| format!("Invalid JSON: {e}"))
    }

    /// URL 검증
    pub fn validate_url(&self, input: &str) -> Result<String, String> {
        // 기본 URL 검증
        if !input.starts_with("http://") && !input.starts_with("https://") {
            return Err("URL must start with http:// or https://".to_string());
        }

        // URL 파싱 시도
        url::Url::parse(input)
            .map(|_| input.to_string())
            .map_err(|e| format!("Invalid URL: {e}"))
    }

    /// 파일 경로 검증
    pub fn validate_file_path(&self, input: &str) -> Result<String, String> {
        if self.contains_path_traversal(input) {
            return Err("Path traversal detected".to_string());
        }

        // 허용된 문자만 포함하는지 체크
        let safe_path_pattern =
            Regex::new(r#"^[a-zA-Z0-9\-_./ \\]+$"#).expect("Test assertion failed");
        if !safe_path_pattern.is_match(input) {
            return Err("Invalid characters in file path".to_string());
        }

        Ok(input.to_string())
    }

    /// 배치 검증
    pub fn validate_batch(&self, inputs: Vec<(&str, InputType)>) -> Vec<Result<String, String>> {
        inputs
            .into_iter()
            .map(|(input, input_type)| self.validate(input, input_type))
            .collect()
    }
}

/// 패스워드 강도
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordStrength {
    Weak,
    Fair,
    Good,
    Strong,
}

impl Default for InputValidator {
    fn default() -> Self {
        Self::new()
    }
}

mod tests {
    

    #[test]
    fn test_sql_injection_detection() {
        let validator = InputValidator::new();

        assert!(validator.contains_sql_injection("SELECT * FROM users"));
        assert!(validator.contains_sql_injection("'; DROP TABLE users; --"));
        assert!(validator.contains_sql_injection("1' OR '1'='1"));
        assert!(!validator.contains_sql_injection("normal text"));
    }

    #[test]
    fn test_xss_detection() {
        let validator = InputValidator::new();

        assert!(validator.contains_xss("<script>alert('XSS')</script>"));
        assert!(validator.contains_xss("javascript:alert(1)"));
        assert!(validator.contains_xss("<img src=x onerror=alert(1)>"));
        assert!(!validator.contains_xss("normal text"));
    }

    #[test]
    fn test_username_validation() {
        let validator = InputValidator::new();

        assert!(validator
            .validate("validuser123", InputType::Username)
            .is_ok());
        assert!(validator.validate("ab", InputType::Username).is_err()); // Too short
        assert!(validator
            .validate("user@name", InputType::Username)
            .is_err()); // Invalid char
    }

    #[test]
    fn test_email_validation() {
        let validator = InputValidator::new();

        assert!(validator
            .validate("user@example.com", InputType::Email)
            .is_ok());
        assert!(validator
            .validate("invalid.email", InputType::Email)
            .is_err());
        assert!(validator
            .validate("@example.com", InputType::Email)
            .is_err());
    }

    #[test]
    fn test_password_strength() {
        let validator = InputValidator::new();

        assert_eq!(
            validator.check_password_strength("weak"),
            PasswordStrength::Weak
        );
        assert_eq!(
            validator.check_password_strength("Password1"),
            PasswordStrength::Fair
        );
        assert_eq!(
            validator.check_password_strength("P@ssw0rd123"),
            PasswordStrength::Good
        );
        assert_eq!(
            validator.check_password_strength("MyV3ry$tr0ngP@ssw0rd!"),
            PasswordStrength::Strong
        );
    }

    #[test]
    fn test_sanitization() {
        let validator = InputValidator::new();

        let input = "<script>alert('xss')</script>";
        let sanitized = validator.sanitize_input(input);
        assert_eq!(
            sanitized,
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;&#x2F;script&gt;"
        );
    }
}
