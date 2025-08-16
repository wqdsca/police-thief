//! High-Performance QUIC Game Server
//!
//! Primary protocol for Police Thief game with stream multiplexing,
//! 0-RTT resumption, and connection migration support.
//! Target: 15,000-20,000 msg/sec with <0.5ms p99 latency

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

mod communication;
mod config;
mod game_logic;
mod handler;
mod monitoring;
mod network;
mod optimization;
mod protocol;

use config::QuicServerConfig;
use game_logic::DefaultGameLogicHandler;
use monitoring::metrics::MetricsCollector;
use network::server::QuicGameServer;
use optimization::optimizer::QuicOptimizer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize environment and logging
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("🚀 QUIC Game Server Starting - Target: 15,000+ msg/sec");

    // Load configuration
    let config = QuicServerConfig::from_env()?;
    info!("📋 Configuration loaded: {:?}", config);

    // Initialize metrics collector
    let metrics = Arc::new(MetricsCollector::new());

    // Initialize optimizer with 8 services from TCP server
    let optimizer = Arc::new(QuicOptimizer::new(&config)?);
    info!("⚡ Performance optimizer initialized with 8 services");

    // Create game logic handler (사용자는 여기서 자신만의 구현체를 사용)
    let game_logic = Arc::new(DefaultGameLogicHandler::new());
    info!("🎮 Default game logic handler initialized");

    // Create and start QUIC server with game logic
    let server = QuicGameServer::new_with_game_logic(config.clone(), game_logic).await?;

    info!(
        "🎮 QUIC server listening on {}:{}",
        config.host, config.port
    );

    // Start server with graceful shutdown
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.run().await {
            error!("Server error: {}", e);
        }
    });

    // Start metrics reporting
    let metrics_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            metrics.report_stats();
        }
    });

    // Wait for shutdown signal
    shutdown_signal().await;
    info!("🛑 Shutdown signal received");

    // Graceful shutdown
    server_handle.abort();
    metrics_handle.abort();

    info!("✅ QUIC server shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
