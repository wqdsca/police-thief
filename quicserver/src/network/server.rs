//! QUIC Game Server Implementation

use crate::config::QuicServerConfig;
use crate::game_logic::GameLogicHandler;
use crate::handler::UnifiedMessageHandler;
use crate::monitoring::metrics::MetricsCollector;
use crate::network::connection::ConnectionManager;
use crate::network::stream::{StreamMultiplexer, StreamType};
use crate::optimization::optimizer::QuicOptimizer;
use anyhow::{Context, Result};
use quinn::{Endpoint, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tracing::{debug, error, info, warn};

pub struct QuicGameServer {
    endpoint: Endpoint,
    config: QuicServerConfig,
    connection_manager: Arc<ConnectionManager>,
    message_handler: Arc<UnifiedMessageHandler>,
    stream_multiplexer: Arc<StreamMultiplexer>,
    metrics: Arc<MetricsCollector>,
    optimizer: Arc<QuicOptimizer>,
}

impl QuicGameServer {
    pub async fn new(
        config: QuicServerConfig,
        metrics: Arc<MetricsCollector>,
        optimizer: Arc<QuicOptimizer>,
    ) -> Result<Self> {
        // Generate or load TLS certificates
        let (cert_chain, private_key) = if config.use_self_signed {
            Self::generate_self_signed_cert()?
        } else {
            Self::load_certificates(&config)?
        };

        // Configure QUIC server
        let server_config = Self::configure_server(&config, cert_chain, private_key)?;

        // Create endpoint
        let endpoint = Endpoint::server(server_config, config.bind_addr)?;

        info!("✅ QUIC endpoint created on {}", config.bind_addr);

        // Initialize components
        let connection_manager = Arc::new(ConnectionManager::new(
            config.max_connections,
            metrics.clone(),
        ));

        // Create default message handler with DefaultGameLogicHandler
        let game_logic = Arc::new(crate::game_logic::DefaultGameLogicHandler::new());
        let message_processor = Arc::new(crate::communication::MessageProcessor::new(
            optimizer.clone(),
        ));
        let message_handler = Arc::new(UnifiedMessageHandler::new(message_processor, game_logic));
        let stream_multiplexer = Arc::new(StreamMultiplexer::new());

        Ok(Self {
            endpoint,
            config,
            connection_manager,
            message_handler,
            stream_multiplexer,
            metrics,
            optimizer,
        })
    }

    /// 사용자 정의 게임 로직과 함께 서버 생성 (권장 방법)
    pub async fn new_with_game_logic(
        config: QuicServerConfig,
        game_logic: Arc<dyn GameLogicHandler>,
    ) -> Result<Self> {
        // Generate or load TLS certificates
        let (cert_chain, private_key) = if config.use_self_signed {
            Self::generate_self_signed_cert()?
        } else {
            Self::load_certificates(&config)?
        };

        // Configure QUIC server
        let server_config = Self::configure_server(&config, cert_chain, private_key)?;

        // Create endpoint
        let endpoint = Endpoint::server(server_config, config.bind_addr)?;

        info!("✅ QUIC endpoint created on {}", config.bind_addr);

        // Initialize components
        let metrics = Arc::new(MetricsCollector::new());
        let optimizer = Arc::new(QuicOptimizer::new(&config)?);

        let connection_manager = Arc::new(ConnectionManager::new(
            config.max_connections,
            metrics.clone(),
        ));

        // Create message handler with user's game logic
        let message_processor = Arc::new(crate::communication::MessageProcessor::new(
            optimizer.clone(),
        ));
        let message_handler = Arc::new(UnifiedMessageHandler::new(message_processor, game_logic));
        let stream_multiplexer = Arc::new(StreamMultiplexer::new());

        Ok(Self {
            endpoint,
            config,
            connection_manager,
            message_handler,
            stream_multiplexer,
            metrics,
            optimizer,
        })
    }

    pub async fn run(&self) -> Result<()> {
        info!("🎮 QUIC server running - accepting connections");

        while let Some(incoming) = self.endpoint.accept().await {
            let connection_manager = self.connection_manager.clone();
            let message_handler = self.message_handler.clone();
            let stream_multiplexer = self.stream_multiplexer.clone();
            let metrics = self.metrics.clone();
            let optimizer = self.optimizer.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(
                    incoming,
                    connection_manager,
                    message_handler,
                    stream_multiplexer,
                    metrics,
                    optimizer,
                )
                .await
                {
                    error!("Connection handling error: {}", e);
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(
        incoming: quinn::Incoming,
        connection_manager: Arc<ConnectionManager>,
        message_handler: Arc<UnifiedMessageHandler>,
        stream_multiplexer: Arc<StreamMultiplexer>,
        metrics: Arc<MetricsCollector>,
        optimizer: Arc<QuicOptimizer>,
    ) -> Result<()> {
        let remote_addr = incoming.remote_address();
        debug!("New connection from {}", remote_addr);

        // Accept connection with 0-RTT if available
        let connection = incoming.accept()?.await?;

        // Check for 0-RTT acceptance
        // Note: Quinn 0.11+ no longer has zero_rtt_accepted() method
        // 0-RTT is handled automatically by the connection establishment
        info!("✅ QUIC connection established from {}", remote_addr);

        // Register connection
        let conn_id = connection_manager
            .register_connection(connection.clone())
            .await?;
        metrics.record_connection();

        // Handle streams with multiplexing
        loop {
            tokio::select! {
                // Accept bidirectional streams
                result = connection.accept_bi() => {
                    match result {
                        Ok((send, recv)) => {
                            let handler = message_handler.clone();
                            let multiplexer = stream_multiplexer.clone();
                            let metrics = metrics.clone();

                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_bidirectional_stream(
                                    send, recv, handler, multiplexer, metrics
                                ).await {
                                    warn!("Stream handling error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            warn!("Failed to accept bi stream: {}", e);
                            break;
                        }
                    }
                }

                // Accept unidirectional streams (for bulk data)
                result = connection.accept_uni() => {
                    match result {
                        Ok(recv) => {
                            let handler = message_handler.clone();
                            let metrics = metrics.clone();

                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_unidirectional_stream(
                                    recv, handler, metrics
                                ).await {
                                    warn!("Uni stream handling error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            warn!("Failed to accept uni stream: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        // Cleanup on disconnect
        connection_manager.unregister_connection(conn_id).await;
        metrics.record_disconnection();

        Ok(())
    }

    async fn handle_bidirectional_stream(
        send: quinn::SendStream,
        mut recv: quinn::RecvStream,
        handler: Arc<UnifiedMessageHandler>,
        multiplexer: Arc<StreamMultiplexer>,
        metrics: Arc<MetricsCollector>,
    ) -> Result<()> {
        // Read stream type identifier
        let stream_type_byte = recv.read_u8().await?;
        let stream_type = StreamType::from(stream_type_byte);

        debug!("Handling {:?} stream", stream_type);
        metrics.record_stream(stream_type);

        // Route to appropriate handler based on stream type
        match stream_type {
            StreamType::Control => {
                multiplexer
                    .handle_control_stream(send, recv, handler)
                    .await?
            }
            StreamType::GameState => multiplexer.handle_game_stream(send, recv, handler).await?,
            StreamType::Chat => multiplexer.handle_chat_stream(send, recv, handler).await?,
            StreamType::Voice => multiplexer.handle_voice_stream(send, recv, handler).await?,
            StreamType::Bulk => multiplexer.handle_bulk_stream(send, recv, handler).await?,
        }

        Ok(())
    }

    async fn handle_unidirectional_stream(
        mut recv: quinn::RecvStream,
        handler: Arc<UnifiedMessageHandler>,
        metrics: Arc<MetricsCollector>,
    ) -> Result<()> {
        // Handle one-way data streams (e.g., telemetry, logs)
        let data = recv.read_to_end(1_048_576).await?; // 1MB max
                                                       // TODO: Add process_unidirectional_data to UnifiedMessageHandler
                                                       // handler.process_unidirectional_data(&data).await?;
        metrics.record_unidirectional_bytes(data.len());
        info!("📊 Processed {} bytes of unidirectional data", data.len());
        Ok(())
    }

    fn configure_server(
        config: &QuicServerConfig,
        cert_chain: Vec<CertificateDer<'static>>,
        private_key: PrivateKeyDer<'static>,
    ) -> Result<ServerConfig> {
        let mut crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key)?;

        // Enable 0-RTT
        crypto.max_early_data_size = u32::MAX;
        crypto.send_half_rtt_data = true;

        let mut server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(crypto)?,
        ));

        // Configure transport parameters
        let transport = Arc::get_mut(&mut server_config.transport).ok_or_else(|| {
            anyhow::anyhow!("Failed to get mutable reference to transport config")
        })?;
        transport.max_concurrent_bidi_streams(quinn::VarInt::from_u32(
            config.max_concurrent_streams as u32,
        ));
        transport.max_concurrent_uni_streams(quinn::VarInt::from_u32(
            config.max_concurrent_streams as u32,
        ));
        transport.max_idle_timeout(Some(config.idle_timeout().try_into()?));
        transport.keep_alive_interval(Some(config.keep_alive_interval()));

        // Enable connection migration (if available in this version)
        // Note: Connection migration settings may vary by Quinn version

        Ok(server_config)
    }

    fn generate_self_signed_cert() -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)>
    {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
        let cert_der = cert.cert.der().clone();
        let key_der = PrivateKeyDer::try_from(cert.key_pair.serialize_der())
            .map_err(|_| anyhow::anyhow!("Failed to serialize private key"))?;

        Ok((vec![cert_der], key_der))
    }

    fn load_certificates(
        config: &QuicServerConfig,
    ) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
        // Load from files if paths are provided
        if let (Some(cert_path), Some(key_path)) = (&config.cert_path, &config.key_path) {
            let cert_file = std::fs::read(cert_path).context("Failed to read certificate")?;
            let key_file = std::fs::read(key_path).context("Failed to read private key")?;

            let certs =
                rustls_pemfile::certs(&mut &cert_file[..]).collect::<Result<Vec<_>, _>>()?;
            let key = rustls_pemfile::private_key(&mut &key_file[..])?
                .ok_or_else(|| anyhow::anyhow!("No private key found"))?;

            Ok((certs, key))
        } else {
            Self::generate_self_signed_cert()
        }
    }
}
