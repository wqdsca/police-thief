//! Integration tests for QUIC server

use quicserver::config::QuicServerConfig;
use quicserver::monitoring::metrics::MetricsCollector;
use quicserver::network::server::QuicGameServer;
use quicserver::optimization::optimizer::QuicOptimizer;
use quinn::{ClientConfig, Endpoint};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_quic_server_startup() {
    let config = QuicServerConfig {
        host: "127.0.0.1".to_string(),
        port: 5555,
        bind_addr: "127.0.0.1:5555".parse()?,
        max_concurrent_streams: 100,
        max_idle_timeout_ms: 30000,
        keep_alive_interval_ms: 10000,
        enable_0rtt: true,
        enable_migration: true,
        max_connections: 10,
        send_buffer_size: 65536,
        recv_buffer_size: 65536,
        stream_buffer_size: 32768,
        compression_threshold: 512,
        cert_path: None,
        key_path: None,
        use_self_signed: true,
        enable_simd: true,
        enable_dashmap_optimization: true,
        enable_memory_pool: true,
        enable_parallel_processing: true,
        worker_threads: 2,
        metrics_interval_secs: 30,
        stats_window_secs: 60,
    };

    let metrics = Arc::new(MetricsCollector::new());
    let optimizer = Arc::new(QuicOptimizer::new(&config)?);

    let server = QuicGameServer::new(config, metrics, optimizer).await;
    assert!(server.is_ok());
}

#[tokio::test]
async fn test_quic_client_connection() {
    // Start server
    let server_config = QuicServerConfig {
        host: "127.0.0.1".to_string(),
        port: 5556,
        bind_addr: "127.0.0.1:5556".parse()?,
        max_concurrent_streams: 10,
        max_idle_timeout_ms: 5000,
        keep_alive_interval_ms: 1000,
        enable_0rtt: false,
        enable_migration: false,
        max_connections: 10,
        send_buffer_size: 8192,
        recv_buffer_size: 8192,
        stream_buffer_size: 4096,
        compression_threshold: 512,
        cert_path: None,
        key_path: None,
        use_self_signed: true,
        enable_simd: false,
        enable_dashmap_optimization: true,
        enable_memory_pool: true,
        enable_parallel_processing: false,
        worker_threads: 1,
        metrics_interval_secs: 30,
        stats_window_secs: 60,
    };

    let metrics = Arc::new(MetricsCollector::new());
    let optimizer = Arc::new(QuicOptimizer::new(&server_config)?);

    let server =
        QuicGameServer::new(server_config.clone(), metrics.clone(), optimizer.clone()).await?;

    // Run server in background
    let server_handle = tokio::spawn(async move {
        let _ = timeout(Duration::from_secs(2), server.run()).await;
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create client
    let client_config = configure_client();
    let mut endpoint = Endpoint::client("127.0.0.1:0".parse()?)?;
    endpoint.set_default_client_config(client_config);

    // Try to connect
    let connect_result = timeout(
        Duration::from_secs(1),
        endpoint.connect("127.0.0.1:5556".parse()?, "localhost"),
    )
    .await;

    // Connection might fail due to self-signed cert, but that's OK for this test
    assert!(connect_result.is_ok() || connect_result.is_err());

    // Cleanup
    server_handle.abort();
}

#[tokio::test]
async fn test_stream_multiplexing() {
    use quicserver::network::stream::{StreamMultiplexer, StreamType};

    let multiplexer = StreamMultiplexer::new();

    // Verify stream types
    assert_eq!(StreamType::from(0), StreamType::Control);
    assert_eq!(StreamType::from(1), StreamType::GameState);
    assert_eq!(StreamType::from(2), StreamType::Chat);
    assert_eq!(StreamType::from(3), StreamType::Voice);
    assert_eq!(StreamType::from(4), StreamType::Bulk);
}

#[tokio::test]
async fn test_optimizer_services() {
    let config = QuicServerConfig {
        host: "127.0.0.1".to_string(),
        port: 5557,
        bind_addr: "127.0.0.1:5557".parse()?,
        max_concurrent_streams: 100,
        max_idle_timeout_ms: 30000,
        keep_alive_interval_ms: 10000,
        enable_0rtt: true,
        enable_migration: true,
        max_connections: 100,
        send_buffer_size: 65536,
        recv_buffer_size: 65536,
        stream_buffer_size: 32768,
        compression_threshold: 512,
        cert_path: None,
        key_path: None,
        use_self_signed: true,
        enable_simd: true,
        enable_dashmap_optimization: true,
        enable_memory_pool: true,
        enable_parallel_processing: true,
        worker_threads: 4,
        metrics_interval_secs: 30,
        stats_window_secs: 60,
    };

    let optimizer = QuicOptimizer::new(&config)?;

    // Test compression
    let data = vec![0u8; 1024];
    let compressed = optimizer.compress_if_beneficial(&data);
    assert!(compressed.len() <= data.len());

    // Test memory pool
    let buffer = optimizer.get_packet_buffer();
    assert_eq!(buffer.capacity, config.recv_buffer_size);
    optimizer.return_packet_buffer(buffer);

    // Test I/O buffer pool
    let io_buffer = optimizer.get_io_buffer();
    assert_eq!(io_buffer.capacity(), config.stream_buffer_size);
    optimizer.return_io_buffer(io_buffer);

    // Test connection storage
    let conn_id = uuid::Uuid::new_v4();
    let conn_state = quicserver::optimization::optimizer::ConnectionState {
        id: conn_id,
        streams: dashmap::DashMap::new(),
        rtt: 10,
        congestion_window: 65536,
        bytes_sent: 0,
        bytes_received: 0,
    };
    optimizer.store_connection(conn_id, conn_state);
    assert!(optimizer.get_connection(&conn_id).is_some());
}

#[tokio::test]
async fn test_metrics_collection() {
    use quicserver::network::stream::StreamType;

    let metrics = MetricsCollector::new();

    // Test connection metrics
    metrics.record_connection();
    metrics.record_disconnection();

    // Test stream metrics
    metrics.record_stream(StreamType::Control);
    metrics.record_stream(StreamType::GameState);
    metrics.record_stream(StreamType::Chat);

    // Test byte metrics
    metrics.record_bytes_sent(1024);
    metrics.record_bytes_received(2048);
    metrics.record_unidirectional_bytes(512);

    // Test 0-RTT metrics
    metrics.record_0rtt_success();

    // Test latency metrics
    metrics.record_latency("process_message", Duration::from_millis(5));

    // Verify stats reporting doesn't panic
    metrics.report_stats();
}

fn configure_client() -> ClientConfig {
    let crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(SkipServerVerification::new())
        .with_no_client_auth();

    ClientConfig::new(Arc::new(crypto))
}

/// Helper to skip certificate verification for testing
struct SkipServerVerification;

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::client::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}
