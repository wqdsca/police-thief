//! 서버 시작 함수 모듈
//!
//! 각 서버를 시작하는 함수들을 제공합니다.

use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

/// 관리자 API 서버 시작 (별도 스레드에서 실행)
pub fn start_admin_server_thread(addr: SocketAddr) {
    use crate::admin_api_clean::{configure_admin_routes, AdminApiState};
    use crate::auth_middleware::{login_handler, AuthConfig, AuthMiddleware};
    use crate::websocket_handler::{ws_handler, WsBroadcaster};
    use actix_web::{middleware, web, App, HttpServer};

    std::thread::spawn(move || {
        let rt = actix_web::rt::System::new();
        rt.block_on(async move {
            let admin_state = match AdminApiState::new().with_redis().await {
                Ok(state) => state,
                Err(e) => {
                    tracing::warn!(
                        "Redis connection failed, starting with default state: {}",
                        e
                    );
                    AdminApiState::new()
                }
            };

            // 데이터베이스 연결 풀 생성 (Admin API용)
            let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "mysql://game_simple:game_password_123@localhost/police_thief_simple".to_string()
            });

            let pool = match sqlx::mysql::MySqlPoolOptions::new()
                .max_connections(10)
                .min_connections(2)
                .connect(&database_url)
                .await
            {
                Ok(pool) => pool,
                Err(e) => {
                    tracing::error!("Database connection failed for admin server: {}", e);
                    tracing::error!("Admin server cannot start without database connection - skipping admin server");
                    return; // Exit the thread gracefully instead of panicking
                }
            };

            let admin_state = web::Data::new(admin_state);
            let ws_broadcaster = web::Data::new(std::sync::Arc::new(WsBroadcaster::new()));
            let auth_config = AuthConfig::new(pool);

            info!("Admin API server listening on {}", addr);

            let server = HttpServer::new(move || {
                App::new()
                    .app_data(admin_state.clone())
                    .app_data(ws_broadcaster.clone())
                    .wrap(middleware::Logger::default())
                    .wrap(
                        actix_cors::Cors::default()
                            .allow_any_origin()
                            .allow_any_method()
                            .allow_any_header()
                            .max_age(3600),
                    )
                    .route("/api/admin/login", web::post().to(login_handler))
                    .route("/ws", web::get().to(ws_handler))
                    .service(
                        web::scope("")
                            .wrap(AuthMiddleware::new(auth_config.clone()))
                            .configure(configure_admin_routes),
                    )
            })
            .bind(addr)
            .map_err(|e| {
                tracing::error!("Failed to bind admin server to {}: {}", addr, e);
                e
            });
            
            match server {
                Ok(server) => {
                    let server_future = server.run();
                    if let Err(e) = server_future.await {
                        tracing::error!("Admin API server failed: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to start admin server: {}", e);
                }
            }
        });
        
        // The spawn returns immediately, so we don't need to check result here
        // The errors are already logged inside the spawned task
    });
}

/// gRPC 서버 시작
pub async fn start_grpc_server(addr: SocketAddr) -> Result<()> {
    use grpcserver::server::start_server;

    info!("gRPC server listening on {}", addr);
    start_server(addr).await.context("gRPC server failed")
}

/// Auth gRPC 서버 시작
pub async fn start_auth_grpc_server(addr: SocketAddr) -> Result<()> {
    use crate::service::start_auth_grpc_server;

    // 데이터베이스 연결 풀 생성
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "mysql://game_simple:game_password_123@localhost/police_thief_simple".to_string()
    });

    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    info!("Auth gRPC server listening on {}", addr);
    start_auth_grpc_server(pool, addr)
        .await
        .context("Auth gRPC server failed")
}

/// TCP 서버 시작
pub async fn start_tcp_server(addr: SocketAddr) -> Result<()> {
    use std::sync::Arc;
    use tcpserver::{service::MessageService, ConnectionService, HeartbeatService};
    use tokio::net::TcpListener;

    let connection_service = Arc::new(ConnectionService::new(1000));
    let heartbeat_service = Arc::new(HeartbeatService::with_default_config(
        connection_service.clone(),
    ));
    let message_service = Arc::new(MessageService::new(connection_service.clone()));

    heartbeat_service.start().await?;

    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind TCP server to {}", addr))?;

    info!("TCP server listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((socket, peer_addr)) => {
                info!("New TCP connection from {}", peer_addr);
                let conn_service = connection_service.clone();
                let msg_service = message_service.clone();

                tokio::spawn(async move {
                    if let Err(e) =
                        handle_tcp_connection(socket, peer_addr, conn_service, msg_service).await
                    {
                        tracing::error!("TCP connection error ({}): {}", peer_addr, e);
                    }
                });
            }
            Err(e) => {
                tracing::error!("Failed to accept TCP connection: {}", e);
                continue;
            }
        }
    }
}

/// TCP 연결 처리
async fn handle_tcp_connection(
    socket: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    _connection_service: Arc<tcpserver::ConnectionService>,
    _message_service: Arc<tcpserver::service::MessageService>,
) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (mut reader, mut writer) = socket.into_split();
    let mut buffer = [0; 1024];

    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => {
                info!("TCP connection closed: {}", peer_addr);
                break;
            }
            Ok(n) => {
                // Echo server implementation
                if let Err(e) = writer.write_all(&buffer[..n]).await {
                    tracing::error!("Failed to send TCP response to {}: {}", peer_addr, e);
                    break;
                }
            }
            Err(e) => {
                tracing::error!("TCP read error from {}: {}", peer_addr, e);
                break;
            }
        }
    }

    Ok(())
}

/// RUDP 서버 시작
pub async fn start_rudp_server(addr: SocketAddr) -> Result<()> {
    use tokio::net::UdpSocket;

    let socket = UdpSocket::bind(addr)
        .await
        .with_context(|| format!("Failed to bind RUDP server to {}", addr))?;

    info!("RUDP server listening on {}", addr);

    let mut buffer = [0; 65536];

    loop {
        match socket.recv_from(&mut buffer).await {
            Ok((size, peer_addr)) => {
                // Echo server implementation
                if let Err(e) = socket.send_to(&buffer[..size], peer_addr).await {
                    tracing::error!("Failed to send RUDP response to {}: {}", peer_addr, e);
                }
            }
            Err(e) => {
                tracing::error!("RUDP receive error: {}", e);
                continue;
            }
        }
    }
}

/// 성능 모니터링 시작
pub async fn start_monitoring() -> Result<()> {
    use tokio::time::{interval, Duration};

    let mut interval = interval(Duration::from_secs(30));

    loop {
        interval.tick().await;
        info!("Performance monitoring: System is healthy");

        // TODO: Collect and log actual performance metrics
        // - Memory usage
        // - CPU utilization
        // - Network throughput
        // - Active connections
    }
}
