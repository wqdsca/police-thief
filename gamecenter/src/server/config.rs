//! 서버 설정 모듈
//!
//! 통합 서버의 설정을 관리합니다.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

// 기본값 상수
const DEFAULT_GRPC_HOST: &str = "127.0.0.1";
const DEFAULT_GRPC_PORT: u16 = 50051;
const DEFAULT_AUTH_GRPC_HOST: &str = "127.0.0.1";
const DEFAULT_AUTH_GRPC_PORT: u16 = 50052;
const DEFAULT_TCP_HOST: &str = "127.0.0.1";
const DEFAULT_TCP_PORT: u16 = 4000;
const DEFAULT_RUDP_HOST: &str = "127.0.0.1";
const DEFAULT_RUDP_PORT: u16 = 5000;
const DEFAULT_ADMIN_HOST: &str = "127.0.0.1";
const DEFAULT_ADMIN_PORT: u16 = 8080;

/// 통합 서버 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedServerConfig {
    pub grpc: ServerEndpoint,
    pub auth_grpc: ServerEndpoint, // Auth 전용 gRPC 서버
    pub tcp: ServerEndpoint,
    pub rudp: ServerEndpoint,
    pub admin: ServerEndpoint,
    pub features: ServerFeatures,
}

/// 서버 엔드포인트 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEndpoint {
    pub address: SocketAddr,
    pub enabled: bool,
}

/// 서버 기능 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerFeatures {
    pub monitoring: bool,
    pub websocket: bool,
    pub redis_lifecycle: bool,
}

impl Default for UnifiedServerConfig {
    fn default() -> Self {
        Self {
            grpc: ServerEndpoint {
                address: format!("{}:{}", DEFAULT_GRPC_HOST, DEFAULT_GRPC_PORT)
                    .parse()
                    .unwrap_or_else(|e| {
                        tracing::error!("Invalid default gRPC address: {}", e);
                        std::process::exit(1);
                    }),
                enabled: true,
            },
            auth_grpc: ServerEndpoint {
                address: format!("{}:{}", DEFAULT_AUTH_GRPC_HOST, DEFAULT_AUTH_GRPC_PORT)
                    .parse()
                    .unwrap_or_else(|e| {
                        tracing::error!("Invalid default Auth gRPC address: {}", e);
                        std::process::exit(1);
                    }),
                enabled: true,
            },
            tcp: ServerEndpoint {
                address: format!("{}:{}", DEFAULT_TCP_HOST, DEFAULT_TCP_PORT)
                    .parse()
                    .unwrap_or_else(|e| {
                        tracing::error!("Invalid default TCP address: {}", e);
                        std::process::exit(1);
                    }),
                enabled: true,
            },
            rudp: ServerEndpoint {
                address: format!("{}:{}", DEFAULT_RUDP_HOST, DEFAULT_RUDP_PORT)
                    .parse()
                    .unwrap_or_else(|e| {
                        tracing::error!("Invalid default RUDP address: {}", e);
                        std::process::exit(1);
                    }),
                enabled: true,
            },
            admin: ServerEndpoint {
                address: format!("{}:{}", DEFAULT_ADMIN_HOST, DEFAULT_ADMIN_PORT)
                    .parse()
                    .unwrap_or_else(|e| {
                        tracing::error!("Invalid default Admin address: {}", e);
                        std::process::exit(1);
                    }),
                enabled: true,
            },
            features: ServerFeatures {
                monitoring: true,
                websocket: true,
                redis_lifecycle: true,
            },
        }
    }
}

impl UnifiedServerConfig {
    /// 환경변수에서 설정 로드
    pub fn from_env() -> Result<Self> {
        let mut config = Self::default();

        // gRPC 설정
        if let Ok(host) = std::env::var("GRPC_HOST") {
            if let Ok(port_str) = std::env::var("GRPC_PORT") {
                if let Ok(port) = port_str.parse::<u16>() {
                    config.grpc.address = format!("{}:{}", host, port).parse()?;
                }
            }
        }

        // TCP 설정
        if let Ok(host) = std::env::var("TCP_HOST") {
            if let Ok(port_str) = std::env::var("TCP_PORT") {
                if let Ok(port) = port_str.parse::<u16>() {
                    config.tcp.address = format!("{}:{}", host, port).parse()?;
                }
            }
        }

        // RUDP 설정
        if let Ok(host) = std::env::var("UDP_HOST") {
            if let Ok(port_str) = std::env::var("UDP_PORT") {
                if let Ok(port) = port_str.parse::<u16>() {
                    config.rudp.address = format!("{}:{}", host, port).parse()?;
                }
            }
        }

        // 기능 활성화 설정
        if let Ok(val) = std::env::var("ENABLE_GRPC") {
            config.grpc.enabled = val.parse().unwrap_or(true);
        }
        if let Ok(val) = std::env::var("ENABLE_TCP") {
            config.tcp.enabled = val.parse().unwrap_or(true);
        }
        if let Ok(val) = std::env::var("ENABLE_RUDP") {
            config.rudp.enabled = val.parse().unwrap_or(true);
        }
        if let Ok(val) = std::env::var("ENABLE_MONITORING") {
            config.features.monitoring = val.parse().unwrap_or(true);
        }

        config.validate()?;
        Ok(config)
    }

    /// 설정 검증
    pub fn validate(&self) -> Result<()> {
        if !self.grpc.enabled
            && !self.auth_grpc.enabled
            && !self.tcp.enabled
            && !self.rudp.enabled
            && !self.admin.enabled
        {
            return Err(anyhow::anyhow!("At least one server must be enabled"));
        }

        // 포트 충돌 검사
        let mut ports = Vec::new();
        if self.grpc.enabled {
            ports.push(self.grpc.address.port());
        }
        if self.auth_grpc.enabled {
            ports.push(self.auth_grpc.address.port());
        }
        if self.tcp.enabled {
            ports.push(self.tcp.address.port());
        }
        if self.rudp.enabled {
            ports.push(self.rudp.address.port());
        }
        if self.admin.enabled {
            ports.push(self.admin.address.port());
        }

        ports.sort_unstable();
        for window in ports.windows(2) {
            if window[0] == window[1] {
                return Err(anyhow::anyhow!("Port conflict detected: {}", window[0]));
            }
        }

        Ok(())
    }

    /// 활성화된 서버 수
    pub fn enabled_server_count(&self) -> usize {
        let mut count = 0;
        if self.grpc.enabled {
            count += 1;
        }
        if self.auth_grpc.enabled {
            count += 1;
        }
        if self.tcp.enabled {
            count += 1;
        }
        if self.rudp.enabled {
            count += 1;
        }
        if self.admin.enabled {
            count += 1;
        }
        count
    }
}

/// 서버 설정 빌더
pub struct UnifiedServerConfigBuilder {
    config: UnifiedServerConfig,
}

impl UnifiedServerConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: UnifiedServerConfig::default(),
        }
    }

    pub fn with_grpc(mut self, address: SocketAddr, enabled: bool) -> Self {
        self.config.grpc = ServerEndpoint { address, enabled };
        self
    }

    pub fn with_tcp(mut self, address: SocketAddr, enabled: bool) -> Self {
        self.config.tcp = ServerEndpoint { address, enabled };
        self
    }

    pub fn with_rudp(mut self, address: SocketAddr, enabled: bool) -> Self {
        self.config.rudp = ServerEndpoint { address, enabled };
        self
    }

    pub fn with_auth_grpc(mut self, address: SocketAddr, enabled: bool) -> Self {
        self.config.auth_grpc = ServerEndpoint { address, enabled };
        self
    }

    pub fn with_admin(mut self, address: SocketAddr, enabled: bool) -> Self {
        self.config.admin = ServerEndpoint { address, enabled };
        self
    }

    pub fn with_features(mut self, features: ServerFeatures) -> Self {
        self.config.features = features;
        self
    }

    pub fn build(self) -> Result<UnifiedServerConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for UnifiedServerConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}
