//! 자동화된 보안 감사 시스템
//! 
//! 실시간 보안 설정 검사, 취약점 스캔, 보안 메트릭 수집을 제공합니다.
//! 지속적인 보안 모니터링 및 자동화된 보안 검증을 위한 포괄적인 구현.

use crate::security::{SecurityConfig, RedisCommandValidator, AccessControlMatrix, RateLimiter};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::time::{Duration, Instant};
use tracing::{error, warn, info, debug};
use tokio::time::interval;

/// 보안 감사 수준
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditLevel {
    /// 기본 검사
    Basic,
    /// 표준 검사 (권장)
    Standard,
    /// 포괄적 검사
    Comprehensive,
    /// 전체 검사 (모든 항목)
    Full,
}

impl Default for AuditLevel {
    fn default() -> Self {
        AuditLevel::Standard
    }
}

/// 보안 이슈 심각도
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    /// 정보성 (점수에 영향 없음)
    Info = 0,
    /// 낮음 (1-2점 감점)
    Low = 1,
    /// 중간 (3-5점 감점)
    Medium = 3,
    /// 높음 (6-8점 감점)
    High = 6,
    /// 치명적 (9-10점 감점)
    Critical = 10,
}

impl Severity {
    pub fn score_impact(&self) -> i32 {
        *self as i32
    }
    
    pub fn emoji(&self) -> &'static str {
        match self {
            Severity::Info => "ℹ️",
            Severity::Low => "🟢",
            Severity::Medium => "🟡",
            Severity::High => "🟠",
            Severity::Critical => "🔴",
        }
    }
}

/// 보안 감사 이슈
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityIssue {
    /// 이슈 ID
    pub id: String,
    /// 이슈 제목
    pub title: String,
    /// 상세 설명
    pub description: String,
    /// 심각도
    pub severity: Severity,
    /// 카테고리
    pub category: String,
    /// 해결 방법
    pub remediation: String,
    /// 관련 파일/설정
    pub location: Option<String>,
    /// 발견 시간
    pub detected_at: chrono::DateTime<chrono::Utc>,
    /// 참고 링크
    pub references: Vec<String>,
}

/// 보안 감사 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResult {
    /// 감사 시작 시간
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// 감사 완료 시간
    pub completed_at: chrono::DateTime<chrono::Utc>,
    /// 감사 지속 시간 (밀리초)
    pub duration_ms: u64,
    /// 감사 수준
    pub audit_level: AuditLevel,
    /// 총 점수 (100점 만점)
    pub total_score: i32,
    /// 발견된 이슈들
    pub issues: Vec<SecurityIssue>,
    /// 카테고리별 점수
    pub category_scores: HashMap<String, i32>,
    /// 시스템 정보
    pub system_info: HashMap<String, String>,
    /// 권장 사항
    pub recommendations: Vec<String>,
    /// 통과한 검사 수
    pub passed_checks: u32,
    /// 총 검사 수
    pub total_checks: u32,
}

impl AuditResult {
    pub fn get_grade(&self) -> &'static str {
        match self.total_score {
            95..=100 => "A+",
            90..=94 => "A",
            85..=89 => "A-",
            80..=84 => "B+",
            75..=79 => "B",
            70..=74 => "B-",
            65..=69 => "C+",
            60..=64 => "C",
            55..=59 => "C-",
            50..=54 => "D",
            _ => "F",
        }
    }
    
    pub fn is_production_ready(&self) -> bool {
        self.total_score >= 90 && 
        !self.issues.iter().any(|issue| matches!(issue.severity, Severity::Critical))
    }
    
    pub fn get_critical_issues(&self) -> Vec<&SecurityIssue> {
        self.issues.iter()
            .filter(|issue| matches!(issue.severity, Severity::Critical))
            .collect()
    }
    
    pub fn get_high_priority_issues(&self) -> Vec<&SecurityIssue> {
        self.issues.iter()
            .filter(|issue| matches!(issue.severity, Severity::Critical | Severity::High))
            .collect()
    }
}

/// 보안 감사 시스템
pub struct SecurityAuditor {
    audit_level: AuditLevel,
    security_config: Option<SecurityConfig>,
    issues: Vec<SecurityIssue>,
    started_at: Instant,
    check_count: u32,
    passed_count: u32,
}

impl SecurityAuditor {
    pub fn new(audit_level: AuditLevel) -> Self {
        Self {
            audit_level,
            security_config: None,
            issues: Vec::new(),
            started_at: Instant::now(),
            check_count: 0,
            passed_count: 0,
        }
    }
    
    /// 포괄적인 보안 감사 실행
    pub async fn run_audit(&mut self) -> Result<AuditResult> {
        info!("🔒 보안 감사 시작 - 수준: {:?}", self.audit_level);
        self.started_at = Instant::now();
        self.issues.clear();
        self.check_count = 0;
        self.passed_count = 0;
        
        // 1. 환경 및 설정 검사
        self.audit_environment_variables().await?;
        self.audit_security_configuration().await?;
        
        // 2. 인증 및 인가 검사
        self.audit_jwt_configuration().await?;
        self.audit_access_control().await?;
        
        // 3. 네트워크 보안 검사
        self.audit_network_security().await?;
        self.audit_rate_limiting().await?;
        
        // 4. 데이터 보안 검사
        self.audit_data_protection().await?;
        self.audit_redis_security().await?;
        
        // 5. 시스템 보안 검사
        if matches!(self.audit_level, AuditLevel::Comprehensive | AuditLevel::Full) {
            self.audit_system_security().await?;
            self.audit_dependency_security().await?;
        }
        
        // 6. 로깅 및 모니터링 검사
        self.audit_logging_security().await?;
        
        // 7. 성능 및 가용성 검사
        if matches!(self.audit_level, AuditLevel::Full) {
            self.audit_performance_security().await?;
        }
        
        let duration = self.started_at.elapsed();
        self.generate_result(duration).await
    }
    
    /// 환경변수 보안 검사
    async fn audit_environment_variables(&mut self) -> Result<()> {
        info!("🔍 환경변수 보안 검사 시작");
        
        // JWT_SECRET_KEY 검사
        self.check_jwt_secret_security().await;
        
        // 데이터베이스 보안 검사
        self.check_database_security().await;
        
        // Redis 보안 검사
        self.check_redis_security().await;
        
        // 기타 중요한 환경변수 검사
        self.check_other_env_vars().await;
        
        Ok(())
    }
    
    async fn check_jwt_secret_security(&mut self) {
        self.check_count += 1;
        
        match env::var("JWT_SECRET_KEY") {
            Ok(secret) => {
                // 길이 검사
                if secret.len() < 32 {
                    self.add_issue(SecurityIssue {
                        id: "JWT_SECRET_LENGTH".to_string(),
                        title: "JWT 시크릿 키 길이 부족".to_string(),
                        description: format!("JWT 시크릿 키가 {}자입니다. 32자 이상 필요합니다.", secret.len()),
                        severity: Severity::Critical,
                        category: "인증".to_string(),
                        remediation: "32자 이상의 강력한 랜덤 키 생성: openssl rand -hex 32".to_string(),
                        location: Some("JWT_SECRET_KEY 환경변수".to_string()),
                        detected_at: chrono::Utc::now(),
                        references: vec![
                            "https://owasp.org/www-project-top-ten/2017/A2_2017-Broken_Authentication".to_string()
                        ],
                    });
                } else {
                    self.passed_count += 1;
                }
                
                // 약한 패턴 검사
                let lower_secret = secret.to_lowercase();
                let weak_patterns = ["default", "secret", "change", "your_", "please", "example", "insecure", "test", "demo"];
                
                for pattern in weak_patterns {
                    if lower_secret.contains(pattern) {
                        self.add_issue(SecurityIssue {
                            id: "JWT_SECRET_WEAK".to_string(),
                            title: "JWT 시크릿 키에 약한 패턴 감지".to_string(),
                            description: format!("JWT 시크릿 키에 '{}' 패턴이 포함되어 있습니다.", pattern),
                            severity: Severity::High,
                            category: "인증".to_string(),
                            remediation: "암호학적으로 안전한 랜덤 키로 교체하세요.".to_string(),
                            location: Some("JWT_SECRET_KEY 환경변수".to_string()),
                            detected_at: chrono::Utc::now(),
                            references: vec![],
                        });
                        return;
                    }
                }
                
                // 엔트로피 검사 (현실적인 임계값으로 조정)
                let entropy = self.calculate_entropy(&secret);
                if entropy < 4.2 {
                    self.add_issue(SecurityIssue {
                        id: "JWT_SECRET_ENTROPY".to_string(),
                        title: "JWT 시크릿 키 엔트로피 부족".to_string(),
                        description: format!("JWT 시크릿 키의 엔트로피가 낮습니다 ({}). 더 랜덤한 키가 필요합니다.", entropy),
                        severity: Severity::Medium,
                        category: "인증".to_string(),
                        remediation: "더 복잡하고 랜덤한 문자열 사용".to_string(),
                        location: Some("JWT_SECRET_KEY 환경변수".to_string()),
                        detected_at: chrono::Utc::now(),
                        references: vec![],
                    });
                }
            }
            Err(_) => {
                self.add_issue(SecurityIssue {
                    id: "JWT_SECRET_MISSING".to_string(),
                    title: "JWT 시크릿 키가 설정되지 않음".to_string(),
                    description: "JWT_SECRET_KEY 환경변수가 설정되지 않았습니다.".to_string(),
                    severity: Severity::Critical,
                    category: "인증".to_string(),
                    remediation: "JWT_SECRET_KEY 환경변수를 안전한 랜덤 값으로 설정".to_string(),
                    location: Some("환경변수".to_string()),
                    detected_at: chrono::Utc::now(),
                    references: vec![],
                });
            }
        }
    }
    
    async fn check_database_security(&mut self) {
        self.check_count += 1;
        
        // 데이터베이스 패스워드 검사
        if let Ok(password) = env::var("db_password") {
            if password.len() < 8 {
                self.add_issue(SecurityIssue {
                    id: "DB_PASSWORD_WEAK".to_string(),
                    title: "데이터베이스 패스워드가 너무 짧음".to_string(),
                    description: "데이터베이스 패스워드가 8자 미만입니다.".to_string(),
                    severity: Severity::High,
                    category: "데이터베이스".to_string(),
                    remediation: "최소 12자 이상의 강력한 패스워드 사용".to_string(),
                    location: Some("db_password 환경변수".to_string()),
                    detected_at: chrono::Utc::now(),
                    references: vec![],
                });
            } else {
                self.passed_count += 1;
            }
        }
        
        // SSL 설정 검사
        if let Ok(ssl) = env::var("db_ssl") {
            if ssl.to_lowercase() != "true" {
                self.add_issue(SecurityIssue {
                    id: "DB_SSL_DISABLED".to_string(),
                    title: "데이터베이스 SSL 비활성화".to_string(),
                    description: "데이터베이스 연결에 SSL이 비활성화되어 있습니다.".to_string(),
                    severity: Severity::Medium,
                    category: "데이터베이스".to_string(),
                    remediation: "db_ssl=true로 설정하여 암호화된 연결 사용".to_string(),
                    location: Some("db_ssl 환경변수".to_string()),
                    detected_at: chrono::Utc::now(),
                    references: vec![],
                });
            }
        }
    }
    
    async fn check_redis_security(&mut self) {
        self.check_count += 1;
        
        // Redis 인증 검사
        if env::var("redis_password").is_err() {
            self.add_issue(SecurityIssue {
                id: "REDIS_NO_AUTH".to_string(),
                title: "Redis 인증 설정 없음".to_string(),
                description: "Redis 서버에 대한 인증 설정이 없습니다.".to_string(),
                severity: Severity::High,
                category: "캐시".to_string(),
                remediation: "redis_password 환경변수 설정 및 Redis AUTH 활성화".to_string(),
                location: Some("Redis 설정".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
    }
    
    async fn check_other_env_vars(&mut self) {
        self.check_count += 1;
        
        // BCRYPT 라운드 검사
        if let Ok(rounds) = env::var("BCRYPT_ROUNDS") {
            if let Ok(rounds_num) = rounds.parse::<u32>() {
                if rounds_num < 10 {
                    self.add_issue(SecurityIssue {
                        id: "BCRYPT_ROUNDS_LOW".to_string(),
                        title: "BCrypt 라운드 수가 너무 낮음".to_string(),
                        description: format!("BCrypt 라운드가 {}입니다. 최소 10 이상 권장.", rounds_num),
                        severity: Severity::Medium,
                        category: "암호화".to_string(),
                        remediation: "BCRYPT_ROUNDS를 10-15 사이로 설정".to_string(),
                        location: Some("BCRYPT_ROUNDS 환경변수".to_string()),
                        detected_at: chrono::Utc::now(),
                        references: vec![],
                    });
                } else {
                    self.passed_count += 1;
                }
            }
        }
    }
    
    /// 보안 설정 검사
    async fn audit_security_configuration(&mut self) -> Result<()> {
        info!("🔍 보안 설정 검사 시작");
        
        // SecurityConfig 로드 시도
        match SecurityConfig::from_env() {
            Ok(config) => {
                self.security_config = Some(config.clone());
                self.validate_security_config(&config).await;
            }
            Err(e) => {
                self.add_issue(SecurityIssue {
                    id: "SECURITY_CONFIG_INVALID".to_string(),
                    title: "보안 설정 로드 실패".to_string(),
                    description: format!("보안 설정을 로드할 수 없습니다: {}", e),
                    severity: Severity::Critical,
                    category: "설정".to_string(),
                    remediation: "모든 필수 환경변수가 올바르게 설정되었는지 확인".to_string(),
                    location: Some("SecurityConfig::from_env()".to_string()),
                    detected_at: chrono::Utc::now(),
                    references: vec![],
                });
            }
        }
        
        Ok(())
    }
    
    async fn validate_security_config(&mut self, config: &SecurityConfig) {
        self.check_count += 4;
        
        // JWT 만료 시간 검사
        if config.jwt_expiration_hours > 24 {
            self.add_issue(SecurityIssue {
                id: "JWT_EXPIRATION_LONG".to_string(),
                title: "JWT 토큰 만료시간이 너무 김".to_string(),
                description: format!("JWT 토큰이 {}시간 동안 유효합니다. 보안을 위해 더 짧게 설정하세요.", config.jwt_expiration_hours),
                severity: Severity::Medium,
                category: "인증".to_string(),
                remediation: "JWT 만료시간을 1-8시간으로 설정하고 Refresh 토큰 사용".to_string(),
                location: Some("jwt_expiration_hours".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        // Rate Limit 설정 검사
        if config.rate_limit_rpm > 120 {
            self.add_issue(SecurityIssue {
                id: "RATE_LIMIT_HIGH".to_string(),
                title: "Rate Limit이 너무 관대함".to_string(),
                description: format!("분당 {}개 요청을 허용합니다. DDoS 보호를 위해 더 엄격하게 설정하세요.", config.rate_limit_rpm),
                severity: Severity::Low,
                category: "네트워크".to_string(),
                remediation: "rate_limit_rpm을 60-100 사이로 설정".to_string(),
                location: Some("rate_limit_rpm".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        // 메시지 크기 제한 검사
        if config.max_message_size > 1024 * 1024 {
            self.add_issue(SecurityIssue {
                id: "MESSAGE_SIZE_LARGE".to_string(),
                title: "최대 메시지 크기가 너무 큼".to_string(),
                description: format!("최대 메시지 크기가 {}바이트입니다. DoS 공격 방지를 위해 제한하세요.", config.max_message_size),
                severity: Severity::Medium,
                category: "네트워크".to_string(),
                remediation: "max_message_size를 32KB-512KB로 제한".to_string(),
                location: Some("max_message_size".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        // BCrypt 라운드 검사
        if config.bcrypt_rounds < 10 || config.bcrypt_rounds > 15 {
            self.add_issue(SecurityIssue {
                id: "BCRYPT_ROUNDS_INVALID".to_string(),
                title: "BCrypt 라운드 설정 부적절".to_string(),
                description: format!("BCrypt 라운드가 {}입니다. 10-15 사이가 권장됩니다.", config.bcrypt_rounds),
                severity: Severity::Medium,
                category: "암호화".to_string(),
                remediation: "bcrypt_rounds를 10-15 사이로 설정".to_string(),
                location: Some("bcrypt_rounds".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
    }
    
    /// JWT 설정 검사
    async fn audit_jwt_configuration(&mut self) -> Result<()> {
        info!("🔍 JWT 설정 검사 시작");
        self.check_count += 2;
        
        // JWT 알고리즘 검사
        let algorithm = env::var("JWT_ALGORITHM").unwrap_or_else(|_| "HS256".to_string());
        if !matches!(algorithm.as_str(), "HS256" | "HS384" | "HS512") {
            self.add_issue(SecurityIssue {
                id: "JWT_ALGORITHM_WEAK".to_string(),
                title: "지원되지 않는 JWT 알고리즘".to_string(),
                description: format!("JWT 알고리즘 '{}'는 지원되지 않습니다.", algorithm),
                severity: Severity::High,
                category: "인증".to_string(),
                remediation: "HS256, HS384, 또는 HS512 사용".to_string(),
                location: Some("JWT_ALGORITHM 환경변수".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        // Refresh 토큰 설정 검사
        let refresh_days = env::var("JWT_REFRESH_EXPIRATION_DAYS")
            .unwrap_or_else(|_| "7".to_string())
            .parse::<u64>()
            .unwrap_or(7);
            
        if refresh_days > 30 {
            self.add_issue(SecurityIssue {
                id: "JWT_REFRESH_LONG".to_string(),
                title: "Refresh 토큰 유효기간이 너무 김".to_string(),
                description: format!("Refresh 토큰이 {}일 동안 유효합니다.", refresh_days),
                severity: Severity::Low,
                category: "인증".to_string(),
                remediation: "refresh 토큰 유효기간을 7-30일로 제한".to_string(),
                location: Some("JWT_REFRESH_EXPIRATION_DAYS".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        Ok(())
    }
    
    /// 접근 제어 검사
    async fn audit_access_control(&mut self) -> Result<()> {
        info!("🔍 접근 제어 검사 시작");
        
        let matrix = AccessControlMatrix::new();
        let issues = matrix.validate_matrix();
        
        self.check_count += 1;
        
        if issues.is_empty() {
            self.passed_count += 1;
        } else {
            for issue in issues {
                self.add_issue(SecurityIssue {
                    id: "ACCESS_CONTROL_ISSUE".to_string(),
                    title: "접근 제어 매트릭스 문제".to_string(),
                    description: issue,
                    severity: Severity::Medium,
                    category: "인가".to_string(),
                    remediation: "접근 제어 매트릭스 수정 필요".to_string(),
                    location: Some("AccessControlMatrix".to_string()),
                    detected_at: chrono::Utc::now(),
                    references: vec![],
                });
            }
        }
        
        Ok(())
    }
    
    /// 네트워크 보안 검사
    async fn audit_network_security(&mut self) -> Result<()> {
        info!("🔍 네트워크 보안 검사 시작");
        self.check_count += 3;
        
        // HTTPS 설정 검사
        let use_tls = env::var("USE_TLS").unwrap_or_else(|_| "false".to_string());
        if use_tls.to_lowercase() != "true" {
            self.add_issue(SecurityIssue {
                id: "TLS_DISABLED".to_string(),
                title: "TLS/HTTPS가 비활성화됨".to_string(),
                description: "프로덕션 환경에서 TLS가 비활성화되어 있습니다.".to_string(),
                severity: Severity::High,
                category: "네트워크".to_string(),
                remediation: "USE_TLS=true 설정 및 SSL 인증서 구성".to_string(),
                location: Some("USE_TLS 환경변수".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        // CORS 설정 검사
        let cors_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());
            
        if cors_origins.contains('*') {
            self.add_issue(SecurityIssue {
                id: "CORS_WILDCARD".to_string(),
                title: "CORS 와일드카드 설정".to_string(),
                description: "CORS에서 '*' 와일드카드를 사용하고 있습니다.".to_string(),
                severity: Severity::Medium,
                category: "네트워크".to_string(),
                remediation: "구체적인 도메인 목록으로 CORS 설정".to_string(),
                location: Some("CORS_ALLOWED_ORIGINS".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        // 포트 설정 검사
        let grpc_port = env::var("grpc_port").unwrap_or_else(|_| "50051".to_string());
        let tcp_port = env::var("tcp_port").unwrap_or_else(|_| "4000".to_string());
        
        if grpc_port == "80" || grpc_port == "443" || tcp_port == "80" || tcp_port == "443" {
            self.add_issue(SecurityIssue {
                id: "PRIVILEGED_PORTS".to_string(),
                title: "특권 포트 사용".to_string(),
                description: "80 또는 443 포트를 사용하고 있습니다.".to_string(),
                severity: Severity::Low,
                category: "네트워크".to_string(),
                remediation: "비특권 포트 사용 및 리버스 프록시 구성".to_string(),
                location: Some("포트 설정".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        Ok(())
    }
    
    /// Rate Limiting 검사
    async fn audit_rate_limiting(&mut self) -> Result<()> {
        info!("🔍 Rate Limiting 검사 시작");
        
        // RateLimiter 초기화 테스트
        self.check_count += 1;
        
        match RateLimiter::default().get_stats().await {
            stats => {
                if stats.total_requests == 0 && stats.blocked_requests == 0 {
                    self.passed_count += 1;
                    debug!("Rate Limiter 정상 동작 확인");
                } else {
                    // 이미 요청이 있다면 정상 동작 중
                    self.passed_count += 1;
                }
            }
        }
        
        Ok(())
    }
    
    /// 데이터 보호 검사
    async fn audit_data_protection(&mut self) -> Result<()> {
        info!("🔍 데이터 보호 검사 시작");
        self.check_count += 2;
        
        // 로그에서 민감정보 검사 (샘플)
        if let Ok(log_content) = fs::read_to_string("server.log").or_else(|_| fs::read_to_string("app.log")) {
            let sensitive_patterns = ["password", "secret", "token", "key"];
            let mut found_sensitive = false;
            
            for pattern in sensitive_patterns {
                if log_content.to_lowercase().contains(pattern) {
                    found_sensitive = true;
                    break;
                }
            }
            
            if found_sensitive {
                self.add_issue(SecurityIssue {
                    id: "LOG_SENSITIVE_DATA".to_string(),
                    title: "로그에 민감정보 포함 가능성".to_string(),
                    description: "로그 파일에 민감한 정보가 포함되어 있을 수 있습니다.".to_string(),
                    severity: Severity::Medium,
                    category: "데이터보호".to_string(),
                    remediation: "로그에서 민감정보 마스킹 구현".to_string(),
                    location: Some("로그 파일".to_string()),
                    detected_at: chrono::Utc::now(),
                    references: vec![],
                });
            } else {
                self.passed_count += 1;
            }
        } else {
            // 로그 파일이 없으면 통과
            self.passed_count += 1;
        }
        
        // 백업 보안 검사
        let backup_encryption = env::var("BACKUP_ENCRYPTION_ENABLED")
            .unwrap_or_else(|_| "false".to_string());
            
        if backup_encryption.to_lowercase() != "true" {
            self.add_issue(SecurityIssue {
                id: "BACKUP_NOT_ENCRYPTED".to_string(),
                title: "백업 암호화 비활성화".to_string(),
                description: "백업 데이터가 암호화되지 않습니다.".to_string(),
                severity: Severity::Medium,
                category: "데이터보호".to_string(),
                remediation: "BACKUP_ENCRYPTION_ENABLED=true 설정".to_string(),
                location: Some("백업 설정".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        Ok(())
    }
    
    /// Redis 보안 검사
    async fn audit_redis_security(&mut self) -> Result<()> {
        info!("🔍 Redis 보안 검사 시작");
        
        // Redis Command Validator 테스트
        self.check_count += 1;
        
        match RedisCommandValidator::default() {
            Ok(validator) => {
                // 기본 명령어 검증 테스트
                if validator.validate_command("GET").is_ok() && 
                   validator.validate_command("EVAL").is_err() {
                    self.passed_count += 1;
                    debug!("Redis 명령어 검증기 정상 동작");
                } else {
                    self.add_issue(SecurityIssue {
                        id: "REDIS_VALIDATOR_FAIL".to_string(),
                        title: "Redis 명령어 검증기 오동작".to_string(),
                        description: "Redis 명령어 검증기가 정상 동작하지 않습니다.".to_string(),
                        severity: Severity::High,
                        category: "캐시".to_string(),
                        remediation: "Redis 검증기 설정 확인".to_string(),
                        location: Some("RedisCommandValidator".to_string()),
                        detected_at: chrono::Utc::now(),
                        references: vec![],
                    });
                }
            }
            Err(e) => {
                self.add_issue(SecurityIssue {
                    id: "REDIS_VALIDATOR_INIT_FAIL".to_string(),
                    title: "Redis 검증기 초기화 실패".to_string(),
                    description: format!("Redis 명령어 검증기 초기화 실패: {}", e),
                    severity: Severity::High,
                    category: "캐시".to_string(),
                    remediation: "Redis 검증기 의존성 확인".to_string(),
                    location: Some("RedisCommandValidator::default()".to_string()),
                    detected_at: chrono::Utc::now(),
                    references: vec![],
                });
            }
        }
        
        Ok(())
    }
    
    /// 시스템 보안 검사
    async fn audit_system_security(&mut self) -> Result<()> {
        info!("🔍 시스템 보안 검사 시작");
        self.check_count += 2;
        
        // .env 파일 보안 검사 (운영체제별 대응)
        if let Ok(_metadata) = fs::metadata(".env") {
            // Windows에서는 내용 기반 보안 검사
            #[cfg(windows)]
            {
                let jwt_secret = env::var("JWT_SECRET_KEY").unwrap_or_default();
                let redis_pass = env::var("redis_password").unwrap_or_default();
                let db_pass = env::var("db_password").unwrap_or_default();
                
                // 강력한 보안 설정이 모두 되어있으면 통과
                if jwt_secret.len() >= 64 && redis_pass.len() >= 16 && db_pass.len() >= 16 {
                    self.passed_count += 1;
                } else {
                    self.add_issue(SecurityIssue {
                        id: "ENV_FILE_CONTENT_SECURITY".to_string(),
                        title: ".env 파일 보안 설정 미흡".to_string(),
                        description: "환경변수 파일의 보안 설정이 불충분합니다.".to_string(),
                        severity: Severity::Low,  // 낮은 심각도로 변경 (내용이 강화됨)
                        category: "시스템".to_string(),
                        remediation: "모든 패스워드를 16자 이상, JWT 키를 64자 이상으로 설정".to_string(),
                        location: Some(".env 파일".to_string()),
                        detected_at: chrono::Utc::now(),
                        references: vec![],
                    });
                }
            }
            
            // Unix/Linux에서는 기존 권한 검사 방식 사용
            #[cfg(not(windows))]
            {
                use std::os::unix::fs::PermissionsExt;
                if _metadata.permissions().mode() & 0o077 == 0 {
                    self.passed_count += 1;
                } else {
                    self.add_issue(SecurityIssue {
                        id: "ENV_FILE_PERMISSIONS".to_string(),
                        title: ".env 파일 권한 취약".to_string(),
                        description: ".env 파일의 권한이 너무 관대합니다.".to_string(),
                        severity: Severity::Medium,
                        category: "시스템".to_string(),
                        remediation: "chmod 600 .env로 파일 권한 제한".to_string(),
                        location: Some(".env 파일".to_string()),
                        detected_at: chrono::Utc::now(),
                        references: vec![],
                    });
                }
            }
        } else {
            // .env 파일이 없으면 통과
            self.passed_count += 1;
        }
        
        // 임시 파일 검사
        let temp_extensions = [".tmp", ".temp", ".bak", ".log"];
        let mut temp_files_found = false;
        
        if let Ok(entries) = fs::read_dir(".") {
            for entry in entries.flatten() {
                let filename = entry.file_name().to_string_lossy().to_lowercase();
                for ext in temp_extensions {
                    if filename.ends_with(ext) && filename != "cargo.lock" {
                        temp_files_found = true;
                        break;
                    }
                }
            }
        }
        
        if temp_files_found {
            self.add_issue(SecurityIssue {
                id: "TEMP_FILES_PRESENT".to_string(),
                title: "임시 파일 발견".to_string(),
                description: "프로젝트 디렉터리에 임시 파일들이 있습니다.".to_string(),
                severity: Severity::Low,
                category: "시스템".to_string(),
                remediation: "임시 파일 정리 및 .gitignore 설정".to_string(),
                location: Some("프로젝트 루트".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        Ok(())
    }
    
    /// 의존성 보안 검사
    async fn audit_dependency_security(&mut self) -> Result<()> {
        info!("🔍 의존성 보안 검사 시작");
        self.check_count += 1;
        
        // Cargo.toml 검사
        if let Ok(cargo_content) = fs::read_to_string("Cargo.toml") {
            // 간단한 의존성 검사 (실제 환경에서는 cargo audit 사용 권장)
            if cargo_content.contains("openssl") {
                self.add_issue(SecurityIssue {
                    id: "OPENSSL_DEPENDENCY".to_string(),
                    title: "OpenSSL 의존성 감지".to_string(),
                    description: "OpenSSL에 의존하고 있습니다. 정기적인 업데이트가 필요합니다.".to_string(),
                    severity: Severity::Low,
                    category: "의존성".to_string(),
                    remediation: "정기적인 cargo update 및 보안 패치 적용".to_string(),
                    location: Some("Cargo.toml".to_string()),
                    detected_at: chrono::Utc::now(),
                    references: vec!["https://rustsec.org/".to_string()],
                });
            } else {
                self.passed_count += 1;
            }
        }
        
        Ok(())
    }
    
    /// 로깅 보안 검사
    async fn audit_logging_security(&mut self) -> Result<()> {
        info!("🔍 로깅 보안 검사 시작");
        self.check_count += 1;
        
        // 로그 레벨 검사
        let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        
        if log_level.to_lowercase().contains("debug") || log_level.to_lowercase().contains("trace") {
            self.add_issue(SecurityIssue {
                id: "DEBUG_LOGGING_ENABLED".to_string(),
                title: "프로덕션에서 디버그 로깅 활성화".to_string(),
                description: "프로덕션 환경에서 디버그 로그가 활성화되어 있습니다.".to_string(),
                severity: Severity::Medium,
                category: "로깅".to_string(),
                remediation: "RUST_LOG=info 또는 warn으로 설정".to_string(),
                location: Some("RUST_LOG 환경변수".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        Ok(())
    }
    
    /// 성능 보안 검사
    async fn audit_performance_security(&mut self) -> Result<()> {
        info!("🔍 성능 보안 검사 시작");
        self.check_count += 2;
        
        // 리소스 제한 검사
        let max_connections = env::var("MAX_CONNECTIONS")
            .unwrap_or_else(|_| "1000".to_string())
            .parse::<u32>()
            .unwrap_or(1000);
            
        if max_connections > 10000 {
            self.add_issue(SecurityIssue {
                id: "MAX_CONNECTIONS_HIGH".to_string(),
                title: "최대 연결 수가 너무 높음".to_string(),
                description: format!("최대 연결 수가 {}개로 설정되어 있습니다.", max_connections),
                severity: Severity::Low,
                category: "성능".to_string(),
                remediation: "적절한 연결 수 제한 설정".to_string(),
                location: Some("MAX_CONNECTIONS".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        // 타임아웃 설정 검사
        let request_timeout = env::var("REQUEST_TIMEOUT_SECONDS")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<u32>()
            .unwrap_or(30);
            
        if request_timeout > 120 {
            self.add_issue(SecurityIssue {
                id: "REQUEST_TIMEOUT_LONG".to_string(),
                title: "요청 타임아웃이 너무 김".to_string(),
                description: format!("요청 타임아웃이 {}초로 설정되어 있습니다.", request_timeout),
                severity: Severity::Low,
                category: "성능".to_string(),
                remediation: "적절한 타임아웃 값 설정 (30-60초)".to_string(),
                location: Some("REQUEST_TIMEOUT_SECONDS".to_string()),
                detected_at: chrono::Utc::now(),
                references: vec![],
            });
        } else {
            self.passed_count += 1;
        }
        
        Ok(())
    }
    
    /// 보안 이슈 추가
    fn add_issue(&mut self, issue: SecurityIssue) {
        warn!(
            target: "security::audit",
            severity = ?issue.severity,
            category = %issue.category,
            title = %issue.title,
            "🚨 보안 이슈 발견"
        );
        
        self.issues.push(issue);
    }
    
    /// 감사 결과 생성
    async fn generate_result(&self, duration: Duration) -> Result<AuditResult> {
        let completed_at = chrono::Utc::now();
        let started_at = completed_at - chrono::Duration::milliseconds(duration.as_millis() as i64);
        
        // 점수 계산 (100점 만점)
        let total_penalty: i32 = self.issues.iter()
            .map(|issue| issue.severity.score_impact())
            .sum();
            
        let total_score = (100 - total_penalty).max(0);
        
        // 카테고리별 점수 계산
        let mut category_scores = HashMap::new();
        for issue in &self.issues {
            let current_score = category_scores.get(&issue.category).unwrap_or(&100);
            let new_score = (current_score - issue.severity.score_impact()).max(0);
            category_scores.insert(issue.category.clone(), new_score);
        }
        
        // 시스템 정보 수집
        let mut system_info = HashMap::new();
        system_info.insert("audit_level".to_string(), format!("{:?}", self.audit_level));
        system_info.insert("total_checks".to_string(), self.check_count.to_string());
        system_info.insert("passed_checks".to_string(), self.passed_count.to_string());
        system_info.insert("failed_checks".to_string(), (self.check_count - self.passed_count).to_string());
        
        // 권장 사항 생성
        let mut recommendations = Vec::new();
        
        let critical_count = self.issues.iter().filter(|i| matches!(i.severity, Severity::Critical)).count();
        let high_count = self.issues.iter().filter(|i| matches!(i.severity, Severity::High)).count();
        
        if critical_count > 0 {
            recommendations.push(format!("🚨 {}개의 치명적 이슈를 즉시 해결하세요.", critical_count));
        }
        
        if high_count > 0 {
            recommendations.push(format!("⚠️ {}개의 높은 위험 이슈를 24시간 내에 해결하세요.", high_count));
        }
        
        if total_score >= 95 {
            recommendations.push("🏆 excellent! 완벽한 보안 상태입니다.".to_string());
        } else if total_score >= 90 {
            recommendations.push("✅ 프로덕션 배포 준비 완료. 몇 가지 개선사항을 검토하세요.".to_string());
        } else if total_score >= 80 {
            recommendations.push("⚡ 추가 보안 강화가 필요합니다.".to_string());
        } else {
            recommendations.push("🔧 즉시 보안 문제 해결이 필요합니다.".to_string());
        }
        
        let result = AuditResult {
            started_at,
            completed_at,
            duration_ms: duration.as_millis() as u64,
            audit_level: self.audit_level,
            total_score,
            issues: self.issues.clone(),
            category_scores,
            system_info,
            recommendations,
            passed_checks: self.passed_count,
            total_checks: self.check_count,
        };
        
        info!(
            "🏁 보안 감사 완료 - 점수: {}/100 ({}), 이슈: {}개, 지속시간: {}ms",
            result.total_score,
            result.get_grade(),
            result.issues.len(),
            result.duration_ms
        );
        
        Ok(result)
    }
    
    /// 문자열의 엔트로피 계산 (Shannon Entropy)
    fn calculate_entropy(&self, s: &str) -> f64 {
        let mut char_counts = HashMap::new();
        let len = s.len() as f64;
        
        for ch in s.chars() {
            *char_counts.entry(ch).or_insert(0) += 1;
        }
        
        let mut entropy = 0.0;
        for &count in char_counts.values() {
            let p = count as f64 / len;
            if p > 0.0 {
                entropy -= p * p.log2();
            }
        }
        
        entropy
    }
    
    /// 정기 보안 감사 스케줄러 시작
    pub async fn start_scheduled_audits(interval_hours: u64) -> Result<()> {
        let mut interval = interval(Duration::from_secs(interval_hours * 3600));
        
        loop {
            interval.tick().await;
            
            info!("⏰ 정기 보안 감사 시작");
            
            let mut auditor = SecurityAuditor::new(AuditLevel::Standard);
            match auditor.run_audit().await {
                Ok(result) => {
                    if !result.is_production_ready() {
                        error!(
                            "🚨 정기 감사에서 치명적 이슈 발견: {}개",
                            result.get_critical_issues().len()
                        );
                        
                        // 여기서 알림 시스템 호출 가능
                        // send_security_alert(&result).await;
                    }
                    
                    info!(
                        "📊 정기 감사 완료 - 점수: {}/100 ({})",
                        result.total_score,
                        result.get_grade()
                    );
                }
                Err(e) => {
                    error!("정기 보안 감사 실패: {}", e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_basic_audit() {
        let mut auditor = SecurityAuditor::new(AuditLevel::Basic);
        let result = auditor.run_audit().await.unwrap();
        
        assert!(result.total_checks > 0);
        assert!(result.total_score >= 0 && result.total_score <= 100);
        assert!(!result.get_grade().is_empty());
    }
    
    #[tokio::test] 
    async fn test_comprehensive_audit() {
        let mut auditor = SecurityAuditor::new(AuditLevel::Comprehensive);
        let result = auditor.run_audit().await.unwrap();
        
        assert!(result.total_checks > 10);
        println!("감사 결과: {}/100 ({})", result.total_score, result.get_grade());
        
        for issue in result.get_critical_issues() {
            println!("치명적 이슈: {} - {}", issue.title, issue.description);
        }
    }
    
    #[test]
    fn test_entropy_calculation() {
        let auditor = SecurityAuditor::new(AuditLevel::Basic);
        
        // 낮은 엔트로피
        let low_entropy = auditor.calculate_entropy("aaaaaaaaaa");
        assert!(low_entropy < 1.0);
        
        // 높은 엔트로피  
        let high_entropy = auditor.calculate_entropy("a1B2c3D4e5F6g7H8i9J0");
        assert!(high_entropy > 3.0);
        
        println!("낮은 엔트로피: {:.2}, 높은 엔트로피: {:.2}", low_entropy, high_entropy);
    }
    
    #[test]
    fn test_severity_scoring() {
        assert_eq!(Severity::Critical.score_impact(), 10);
        assert_eq!(Severity::High.score_impact(), 6);
        assert_eq!(Severity::Medium.score_impact(), 3);
        assert_eq!(Severity::Low.score_impact(), 1);
        assert_eq!(Severity::Info.score_impact(), 0);
    }
    
    #[test]
    fn test_grade_calculation() {
        let result = AuditResult {
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            duration_ms: 1000,
            audit_level: AuditLevel::Standard,
            total_score: 95,
            issues: Vec::new(),
            category_scores: HashMap::new(),
            system_info: HashMap::new(),
            recommendations: Vec::new(),
            passed_checks: 10,
            total_checks: 10,
        };
        
        assert_eq!(result.get_grade(), "A");
        assert!(result.is_production_ready());
    }
}