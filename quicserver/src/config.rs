//! QUIC Server Configuration

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuicServerConfig {
    // Network settings
    pub host: String,
    pub port: u16,
    pub bind_addr: SocketAddr,

    // QUIC protocol settings
    pub max_concurrent_streams: u64,
    pub max_idle_timeout_ms: u64,
    pub keep_alive_interval_ms: u64,
    pub enable_0rtt: bool,
    pub enable_migration: bool,

    // Performance settings
    pub max_connections: usize,
    pub send_buffer_size: usize,
    pub recv_buffer_size: usize,
    pub stream_buffer_size: usize,
    pub compression_threshold: usize,

    // Security settings
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub use_self_signed: bool,

    // Optimization settings
    pub enable_simd: bool,
    pub enable_dashmap_optimization: bool,
    pub enable_memory_pool: bool,
    pub enable_parallel_processing: bool,
    pub worker_threads: usize,

    // Monitoring
    pub metrics_interval_secs: u64,
    pub stats_window_secs: u64,
}

impl QuicServerConfig {
    pub fn from_env() -> Result<Self> {
        let host = env::var("QUIC_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("QUIC_PORT")
            .unwrap_or_else(|_| "5001".to_string())
            .parse::<u16>()?;

        let bind_addr = format!("{}:{}", host, port).parse()?;

        Ok(Self {
            host,
            port,
            bind_addr,

            // QUIC protocol settings
            max_concurrent_streams: env::var("QUIC_MAX_STREAMS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()?,
            max_idle_timeout_ms: 30_000,
            keep_alive_interval_ms: 10_000,
            enable_0rtt: true,
            enable_migration: true,

            // Performance settings (optimized for 15K+ msg/sec)
            max_connections: 1000,
            send_buffer_size: 65536,
            recv_buffer_size: 65536,
            stream_buffer_size: 32768,
            compression_threshold: 512,

            // Security
            cert_path: env::var("QUIC_CERT_PATH").ok(),
            key_path: env::var("QUIC_KEY_PATH").ok(),
            use_self_signed: true,

            // Optimization (reuse TCP server's 8 services)
            enable_simd: true,
            enable_dashmap_optimization: true,
            enable_memory_pool: true,
            enable_parallel_processing: true,
            worker_threads: num_cpus::get(),

            // Monitoring
            metrics_interval_secs: 30,
            stats_window_secs: 60,
        })
    }

    pub fn idle_timeout(&self) -> Duration {
        Duration::from_millis(self.max_idle_timeout_ms)
    }

    pub fn keep_alive_interval(&self) -> Duration {
        Duration::from_millis(self.keep_alive_interval_ms)
    }
}
