//! RUDP 서버 설정 관리
//!
//! 환경변수와 명령행 인자를 통한 설정 로드 및 관리
//! - 네트워크 설정 (RUDP 프로토콜)
//! - 게임 설정 (2000명 동시접속)
//! - Redis 설정 (캐싱 및 세션 관리)
//! - 모니터링 설정 (성능 메트릭)
//! - 보안 설정 (패킷 검증, DDoS 방어)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

/// RUDP 서버 메인 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RudpServerConfig {
    /// 네트워크 설정
    pub network: NetworkConfig,
    /// 게임 설정  
    pub game: GameConfig,
    /// Redis 설정
    pub redis: RedisConfig,
    /// 모니터링 설정
    pub monitoring: MonitoringConfig,
    /// 보안 설정
    pub security: SecurityConfig,
}

/// 네트워크 설정 (RUDP 프로토콜)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// 서버 바인딩 주소
    pub host: String,
    /// 서버 포트
    pub port: u16,
    /// 최대 패킷 크기 (바이트)
    pub max_packet_size: usize,
    /// 연결 타임아웃 (초)
    pub connection_timeout_secs: u64,
    /// Keep-alive 간격 (초)
    pub keepalive_interval_secs: u64,
    /// ACK 타임아웃 (밀리초)
    pub ack_timeout_ms: u64,
    /// 최대 재전송 횟수
    pub max_retransmissions: u32,
    /// 송신 버퍼 크기
    pub send_buffer_size: usize,
    /// 수신 버퍼 크기
    pub receive_buffer_size: usize,
    /// 혼잡 제어 활성화
    pub enable_congestion_control: bool,
    /// 순서 보장 윈도우 크기
    pub sequence_window_size: u32,
}

/// 게임 설정 (2000명 동시접속 기준)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    /// 최대 동시 세션 수
    pub max_concurrent_sessions: u32,
    /// 최대 동시 플레이어 수
    pub max_concurrent_players: u32,
    /// 게임 틱 레이트 (TPS - Ticks Per Second)
    pub tick_rate: u32,
    /// 플레이어 업데이트 간격 (틱)
    pub player_update_interval: u32,
    /// 월드 업데이트 간격 (틱)
    pub world_update_interval: u32,
    /// 플레이어 타임아웃 (초)
    pub player_timeout_secs: u64,
    /// 최대 스킬 쿨다운 (초)
    pub max_skill_cooldown_secs: u64,
    /// 최대 상태 효과 지속시간 (초)
    pub max_status_effect_duration_secs: u64,
    /// 전투 거리 제한 (게임 단위)
    pub max_combat_range: f32,
    /// 이동 속도 제한 (초당 게임 단위)
    pub max_movement_speed: f32,
}

/// Redis 설정 (캐싱 및 세션 관리)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis 서버 주소
    pub host: String,
    /// Redis 포트
    pub port: u16,
    /// 연결 풀 크기
    pub pool_size: u32,
    /// 연결 타임아웃 (초)
    pub connection_timeout_secs: u64,
    /// 명령 타임아웃 (초)
    pub command_timeout_secs: u64,
    /// 세션 TTL (초)
    pub session_ttl_secs: u64,
    /// 플레이어 데이터 TTL (초)
    pub player_data_ttl_secs: u64,
    /// 메트릭 TTL (초)
    pub metrics_ttl_secs: u64,
    /// Redis 패스워드 (선택적)
    pub password: Option<String>,
    /// Redis 데이터베이스 번호
    pub database: u8,
}

/// 모니터링 설정 (성능 메트릭)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// 메트릭 수집 간격 (초)
    pub metrics_collection_interval_secs: u64,
    /// 성능 로그 간격 (초)
    pub performance_log_interval_secs: u64,
    /// 메트릭 보관 기간 (시간)
    pub metrics_retention_hours: u32,
    /// CPU 사용률 경고 임계값 (%)
    pub cpu_usage_warning_threshold: f32,
    /// 메모리 사용률 경고 임계값 (%)
    pub memory_usage_warning_threshold: f32,
    /// 네트워크 지연시간 경고 임계값 (밀리초)
    pub network_latency_warning_threshold_ms: u64,
    /// Prometheus 메트릭 내보내기 활성화
    pub enable_prometheus_export: bool,
    /// Prometheus 서버 포트
    pub prometheus_port: u16,
}

/// 보안 설정 (패킷 검증, DDoS 방어)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// 패킷 검증 활성화
    pub enable_packet_validation: bool,
    /// Rate limiting 활성화
    pub enable_rate_limiting: bool,
    /// 분당 최대 패킷 수 (per IP)
    pub max_packets_per_minute: u32,
    /// DDoS 방어 활성화
    pub enable_ddos_protection: bool,
    /// IP 블랙리스트 크기
    pub ip_blacklist_size: usize,
    /// 자동 차단 지속시간 (초)
    pub auto_ban_duration_secs: u64,
    /// 패킷 무결성 검사 활성화
    pub enable_packet_integrity_check: bool,
    /// 클라이언트 인증 필수 여부
    pub require_client_authentication: bool,
    /// JWT 토큰 만료시간 (초)
    pub jwt_expiration_secs: u64,
}

impl RudpServerConfig {
    /// 환경변수와 명령행 인자로부터 설정 로드
    pub async fn from_env_and_args() -> Result<Self> {
        // 환경변수 로드
        dotenv::dotenv().ok();

        let config = Self {
            network: NetworkConfig::from_env()?,
            game: GameConfig::from_env()?,
            redis: RedisConfig::from_env()?,
            monitoring: MonitoringConfig::from_env()?,
            security: SecurityConfig::from_env()?,
        };

        // 설정 검증
        config.validate()?;

        Ok(config)
    }

    /// 설정 유효성 검사
    pub fn validate(&self) -> Result<()> {
        // 네트워크 설정 검증
        if self.network.port == 0 {
            return Err(anyhow::anyhow!("Invalid network port: 0"));
        }

        if self.network.max_packet_size > 65507 {
            // UDP 최대 크기
            return Err(anyhow::anyhow!(
                "Max packet size too large: {} (max: 65507)",
                self.network.max_packet_size
            ));
        }

        // 게임 설정 검증
        if self.game.max_concurrent_sessions == 0 {
            return Err(anyhow::anyhow!("Max concurrent sessions must be > 0"));
        }

        if self.game.tick_rate == 0 || self.game.tick_rate > 120 {
            return Err(anyhow::anyhow!(
                "Invalid tick rate: {} (must be 1-120)",
                self.game.tick_rate
            ));
        }

        // Redis 설정 검증
        if self.redis.pool_size == 0 {
            return Err(anyhow::anyhow!("Redis pool size must be > 0"));
        }

        // 보안 설정 검증
        if self.security.max_packets_per_minute == 0 {
            return Err(anyhow::anyhow!("Max packets per minute must be > 0"));
        }

        Ok(())
    }

    /// 개발 환경용 기본 설정
    pub fn development() -> Self {
        Self {
            network: NetworkConfig::development(),
            game: GameConfig::development(),
            redis: RedisConfig::development(),
            monitoring: MonitoringConfig::development(),
            security: SecurityConfig::development(),
        }
    }

    /// 프로덕션 환경용 기본 설정
    pub fn production() -> Self {
        Self {
            network: NetworkConfig::production(),
            game: GameConfig::production(),
            redis: RedisConfig::production(),
            monitoring: MonitoringConfig::production(),
            security: SecurityConfig::production(),
        }
    }
}

impl NetworkConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            host: env::var("RUDP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("RUDP_PORT")?
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid RUDP_PORT: {}", e))?,
            max_packet_size: env::var("MAX_PACKET_SIZE")
                .unwrap_or_else(|_| "65536".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid MAX_PACKET_SIZE: {}", e))?,
            connection_timeout_secs: env::var("CONNECTION_TIMEOUT_SECS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid CONNECTION_TIMEOUT_SECS: {}", e))?,
            keepalive_interval_secs: env::var("KEEPALIVE_INTERVAL_SECS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid KEEPALIVE_INTERVAL_SECS: {}", e))?,
            ack_timeout_ms: env::var("ACK_TIMEOUT_MS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid ACK_TIMEOUT_MS: {}", e))?,
            max_retransmissions: env::var("MAX_RETRANSMISSIONS")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid MAX_RETRANSMISSIONS: {}", e))?,
            send_buffer_size: env::var("SEND_BUFFER_SIZE")
                .unwrap_or_else(|_| "1048576".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid SEND_BUFFER_SIZE: {}", e))?,
            receive_buffer_size: env::var("RECEIVE_BUFFER_SIZE")
                .unwrap_or_else(|_| "1048576".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid RECEIVE_BUFFER_SIZE: {}", e))?,
            enable_congestion_control: env::var("ENABLE_CONGESTION_CONTROL")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid ENABLE_CONGESTION_CONTROL: {}", e))?,
            sequence_window_size: env::var("SEQUENCE_WINDOW_SIZE")
                .unwrap_or_else(|_| "256".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid SEQUENCE_WINDOW_SIZE: {}", e))?,
        })
    }

    pub fn development() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 4001,
            max_packet_size: 65536,
            connection_timeout_secs: 30,
            keepalive_interval_secs: 10,
            ack_timeout_ms: 100,
            max_retransmissions: 3,
            send_buffer_size: 1024 * 1024,
            receive_buffer_size: 1024 * 1024,
            enable_congestion_control: true,
            sequence_window_size: 256,
        }
    }

    pub fn production() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 4001,
            max_packet_size: 65536,
            connection_timeout_secs: 60,
            keepalive_interval_secs: 15,
            ack_timeout_ms: 50,
            max_retransmissions: 5,
            send_buffer_size: 4 * 1024 * 1024,
            receive_buffer_size: 4 * 1024 * 1024,
            enable_congestion_control: true,
            sequence_window_size: 512,
        }
    }
}

impl GameConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            max_concurrent_sessions: env::var("MAX_CONCURRENT_SESSIONS")
                .unwrap_or_else(|_| "2000".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid MAX_CONCURRENT_SESSIONS: {}", e))?,
            max_concurrent_players: env::var("MAX_CONCURRENT_PLAYERS")
                .unwrap_or_else(|_| "2000".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid MAX_CONCURRENT_PLAYERS: {}", e))?,
            tick_rate: env::var("GAME_TICK_RATE")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid GAME_TICK_RATE: {}", e))?,
            player_update_interval: env::var("PLAYER_UPDATE_INTERVAL")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid PLAYER_UPDATE_INTERVAL: {}", e))?,
            world_update_interval: env::var("WORLD_UPDATE_INTERVAL")
                .unwrap_or_else(|_| "1".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid WORLD_UPDATE_INTERVAL: {}", e))?,
            player_timeout_secs: env::var("PLAYER_TIMEOUT_SECS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid PLAYER_TIMEOUT_SECS: {}", e))?,
            max_skill_cooldown_secs: env::var("MAX_SKILL_COOLDOWN_SECS")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid MAX_SKILL_COOLDOWN_SECS: {}", e))?,
            max_status_effect_duration_secs: env::var("MAX_STATUS_EFFECT_DURATION_SECS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid MAX_STATUS_EFFECT_DURATION_SECS: {}", e))?,
            max_combat_range: env::var("MAX_COMBAT_RANGE")
                .unwrap_or_else(|_| "10.0".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid MAX_COMBAT_RANGE: {}", e))?,
            max_movement_speed: env::var("MAX_MOVEMENT_SPEED")
                .unwrap_or_else(|_| "50.0".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid MAX_MOVEMENT_SPEED: {}", e))?,
        })
    }

    pub fn development() -> Self {
        Self {
            max_concurrent_sessions: 100,
            max_concurrent_players: 100,
            tick_rate: 60,
            player_update_interval: 3,
            world_update_interval: 1,
            player_timeout_secs: 300,
            max_skill_cooldown_secs: 60,
            max_status_effect_duration_secs: 300,
            max_combat_range: 10.0,
            max_movement_speed: 50.0,
        }
    }

    pub fn production() -> Self {
        Self {
            max_concurrent_sessions: 2000,
            max_concurrent_players: 2000,
            tick_rate: 60,
            player_update_interval: 3,
            world_update_interval: 1,
            player_timeout_secs: 300,
            max_skill_cooldown_secs: 60,
            max_status_effect_duration_secs: 300,
            max_combat_range: 10.0,
            max_movement_speed: 50.0,
        }
    }
}

impl RedisConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            host: env::var("redis_host").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("redis_port")
                .unwrap_or_else(|_| "6379".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid redis_port: {}", e))?,
            pool_size: env::var("REDIS_POOL_SIZE")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid REDIS_POOL_SIZE: {}", e))?,
            connection_timeout_secs: env::var("REDIS_CONNECTION_TIMEOUT_SECS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid REDIS_CONNECTION_TIMEOUT_SECS: {}", e))?,
            command_timeout_secs: env::var("REDIS_COMMAND_TIMEOUT_SECS")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid REDIS_COMMAND_TIMEOUT_SECS: {}", e))?,
            session_ttl_secs: env::var("SESSION_TTL_SECS")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid SESSION_TTL_SECS: {}", e))?,
            player_data_ttl_secs: env::var("PLAYER_DATA_TTL_SECS")
                .unwrap_or_else(|_| "86400".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid PLAYER_DATA_TTL_SECS: {}", e))?,
            metrics_ttl_secs: env::var("METRICS_TTL_SECS")
                .unwrap_or_else(|_| "604800".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid METRICS_TTL_SECS: {}", e))?,
            password: env::var("REDIS_PASSWORD").ok(),
            database: env::var("REDIS_DATABASE")
                .unwrap_or_else(|_| "0".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid REDIS_DATABASE: {}", e))?,
        })
    }

    pub fn development() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6379,
            pool_size: 5,
            connection_timeout_secs: 5,
            command_timeout_secs: 2,
            session_ttl_secs: 3600,
            player_data_ttl_secs: 86400,
            metrics_ttl_secs: 604800,
            password: None,
            database: 0,
        }
    }

    pub fn production() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6379,
            pool_size: 20,
            connection_timeout_secs: 10,
            command_timeout_secs: 5,
            session_ttl_secs: 7200,
            player_data_ttl_secs: 86400,
            metrics_ttl_secs: 2592000,
            password: None,
            database: 0,
        }
    }
}

impl MonitoringConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            metrics_collection_interval_secs: env::var("METRICS_COLLECTION_INTERVAL_SECS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid METRICS_COLLECTION_INTERVAL_SECS: {}", e))?,
            performance_log_interval_secs: env::var("PERFORMANCE_LOG_INTERVAL_SECS")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid PERFORMANCE_LOG_INTERVAL_SECS: {}", e))?,
            metrics_retention_hours: env::var("METRICS_RETENTION_HOURS")
                .unwrap_or_else(|_| "168".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid METRICS_RETENTION_HOURS: {}", e))?,
            cpu_usage_warning_threshold: env::var("CPU_USAGE_WARNING_THRESHOLD")
                .unwrap_or_else(|_| "70.0".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid CPU_USAGE_WARNING_THRESHOLD: {}", e))?,
            memory_usage_warning_threshold: env::var("MEMORY_USAGE_WARNING_THRESHOLD")
                .unwrap_or_else(|_| "80.0".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid MEMORY_USAGE_WARNING_THRESHOLD: {}", e))?,
            network_latency_warning_threshold_ms: env::var("NETWORK_LATENCY_WARNING_THRESHOLD_MS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .map_err(|e| {
                    anyhow::anyhow!("Invalid NETWORK_LATENCY_WARNING_THRESHOLD_MS: {}", e)
                })?,
            enable_prometheus_export: env::var("ENABLE_PROMETHEUS_EXPORT")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid ENABLE_PROMETHEUS_EXPORT: {}", e))?,
            prometheus_port: env::var("PROMETHEUS_PORT")
                .unwrap_or_else(|_| "9090".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid PROMETHEUS_PORT: {}", e))?,
        })
    }

    pub fn development() -> Self {
        Self {
            metrics_collection_interval_secs: 10,
            performance_log_interval_secs: 30,
            metrics_retention_hours: 24,
            cpu_usage_warning_threshold: 80.0,
            memory_usage_warning_threshold: 85.0,
            network_latency_warning_threshold_ms: 200,
            enable_prometheus_export: false,
            prometheus_port: 9090,
        }
    }

    pub fn production() -> Self {
        Self {
            metrics_collection_interval_secs: 10,
            performance_log_interval_secs: 60,
            metrics_retention_hours: 168,
            cpu_usage_warning_threshold: 70.0,
            memory_usage_warning_threshold: 80.0,
            network_latency_warning_threshold_ms: 100,
            enable_prometheus_export: true,
            prometheus_port: 9090,
        }
    }
}

impl SecurityConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            enable_packet_validation: env::var("ENABLE_PACKET_VALIDATION")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid ENABLE_PACKET_VALIDATION: {}", e))?,
            enable_rate_limiting: env::var("ENABLE_RATE_LIMITING")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid ENABLE_RATE_LIMITING: {}", e))?,
            max_packets_per_minute: env::var("MAX_PACKETS_PER_MINUTE")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid MAX_PACKETS_PER_MINUTE: {}", e))?,
            enable_ddos_protection: env::var("ENABLE_DDOS_PROTECTION")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid ENABLE_DDOS_PROTECTION: {}", e))?,
            ip_blacklist_size: env::var("IP_BLACKLIST_SIZE")
                .unwrap_or_else(|_| "10000".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid IP_BLACKLIST_SIZE: {}", e))?,
            auto_ban_duration_secs: env::var("AUTO_BAN_DURATION_SECS")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid AUTO_BAN_DURATION_SECS: {}", e))?,
            enable_packet_integrity_check: env::var("ENABLE_PACKET_INTEGRITY_CHECK")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid ENABLE_PACKET_INTEGRITY_CHECK: {}", e))?,
            require_client_authentication: env::var("REQUIRE_CLIENT_AUTHENTICATION")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid REQUIRE_CLIENT_AUTHENTICATION: {}", e))?,
            jwt_expiration_secs: env::var("JWT_EXPIRATION_SECS")
                .unwrap_or_else(|_| "7200".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid JWT_EXPIRATION_SECS: {}", e))?,
        })
    }

    pub fn development() -> Self {
        Self {
            enable_packet_validation: true,
            enable_rate_limiting: false,
            max_packets_per_minute: 2000,
            enable_ddos_protection: false,
            ip_blacklist_size: 1000,
            auto_ban_duration_secs: 300,
            enable_packet_integrity_check: false,
            require_client_authentication: false,
            jwt_expiration_secs: 7200,
        }
    }

    pub fn production() -> Self {
        Self {
            enable_packet_validation: true,
            enable_rate_limiting: true,
            max_packets_per_minute: 1000,
            enable_ddos_protection: true,
            ip_blacklist_size: 10000,
            auto_ban_duration_secs: 3600,
            enable_packet_integrity_check: true,
            require_client_authentication: true,
            jwt_expiration_secs: 3600,
        }
    }
}

// Redis 설정을 shared 라이브러리 형식으로 변환 - 비동기 변환 필요
impl RedisConfig {
    pub async fn to_shared_config(
        &self,
    ) -> anyhow::Result<shared::config::redis_config::RedisConfig> {
        // 공유 라이브러리 Redis Config는 자체적으로 연결을 생성함
        shared::config::redis_config::RedisConfig::new()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create Redis config: {}", e))
    }
}

impl RedisConfig {
    /// shared 라이브러리 호환 Redis 연결을 생성
    pub async fn create_shared_config(
        &self,
    ) -> anyhow::Result<shared::config::redis_config::RedisConfig> {
        shared::config::redis_config::RedisConfig::new()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create Redis config: {}", e))
    }
}
