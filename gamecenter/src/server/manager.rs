//! 서버 관리자 모듈
//!
//! 통합 서버의 라이프사이클을 관리합니다.

use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use super::config::UnifiedServerConfig;

/// 서버 관리자
pub struct ServerManager {
    config: UnifiedServerConfig,
    is_running: Arc<AtomicBool>,
    server_handles: Arc<Mutex<Vec<JoinHandle<Result<()>>>>>,
}

impl ServerManager {
    /// 새 서버 관리자 생성
    pub fn new(config: UnifiedServerConfig) -> Self {
        Self {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            server_handles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 모든 서버 시작
    pub async fn start_all(&self) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            warn!("Servers are already running");
            return Ok(());
        }

        info!("Starting unified game servers...");

        let mut handles = self.server_handles.lock().await;
        handles.clear();

        // 관리자 API 서버 시작 (별도 스레드에서 실행)
        if self.config.admin.enabled {
            info!("Starting admin API server on {}", self.config.admin.address);
            let admin_addr = self.config.admin.address;
            super::starters::start_admin_server_thread(admin_addr);
            // Admin 서버는 별도 스레드에서 실행되므로 handle을 저장하지 않음
        }

        // gRPC 서버 시작
        if self.config.grpc.enabled {
            info!("Starting gRPC server on {}", self.config.grpc.address);
            let grpc_addr = self.config.grpc.address;
            let handle = tokio::spawn(async move {
                super::starters::start_grpc_server(grpc_addr)
                    .await
                    .context("Failed to start gRPC server")
            });
            handles.push(handle);
        }

        // Auth gRPC 서버 시작
        if self.config.auth_grpc.enabled {
            info!(
                "Starting Auth gRPC server on {}",
                self.config.auth_grpc.address
            );
            let auth_grpc_addr = self.config.auth_grpc.address;
            let handle = tokio::spawn(async move {
                super::starters::start_auth_grpc_server(auth_grpc_addr)
                    .await
                    .context("Failed to start Auth gRPC server")
            });
            handles.push(handle);
        }

        // TCP 서버 시작
        if self.config.tcp.enabled {
            info!("Starting TCP server on {}", self.config.tcp.address);
            let tcp_addr = self.config.tcp.address;
            let handle = tokio::spawn(async move {
                super::starters::start_tcp_server(tcp_addr)
                    .await
                    .context("Failed to start TCP server")
            });
            handles.push(handle);
        }

        // RUDP 서버 시작
        if self.config.rudp.enabled {
            info!("Starting RUDP server on {}", self.config.rudp.address);
            let rudp_addr = self.config.rudp.address;
            let handle = tokio::spawn(async move {
                super::starters::start_rudp_server(rudp_addr)
                    .await
                    .context("Failed to start RUDP server")
            });
            handles.push(handle);
        }

        // 성능 모니터링 시작
        if self.config.features.monitoring {
            info!("Starting performance monitoring");
            let handle = tokio::spawn(async move {
                super::starters::start_monitoring()
                    .await
                    .context("Failed to start monitoring")
            });
            handles.push(handle);
        }

        self.is_running.store(true, Ordering::SeqCst);
        info!("All servers started successfully!");

        Ok(())
    }

    /// 모든 서버 중지
    pub async fn stop_all(&self) -> Result<()> {
        if !self.is_running.load(Ordering::SeqCst) {
            warn!("Servers are already stopped");
            return Ok(());
        }

        info!("Stopping all servers...");
        self.is_running.store(false, Ordering::SeqCst);

        let mut handles = self.server_handles.lock().await;
        for handle in handles.drain(..) {
            handle.abort();
        }

        info!("All servers stopped successfully");
        Ok(())
    }

    /// 서버 실행 상태 확인
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// 서버 상태 출력
    pub fn print_status(&self) {
        let status = if self.is_running() {
            "Running"
        } else {
            "Stopped"
        };

        info!("Server Status: {}", status);
        info!("Enabled servers: {}", self.config.enabled_server_count());

        if self.config.grpc.enabled {
            info!("  gRPC: {} (enabled)", self.config.grpc.address);
        }
        if self.config.auth_grpc.enabled {
            info!("  Auth gRPC: {} (enabled)", self.config.auth_grpc.address);
        }
        if self.config.tcp.enabled {
            info!("  TCP: {} (enabled)", self.config.tcp.address);
        }
        if self.config.rudp.enabled {
            info!("  RUDP: {} (enabled)", self.config.rudp.address);
        }
        if self.config.admin.enabled {
            info!("  Admin: {} (enabled)", self.config.admin.address);
        }
    }

    /// 종료 시그널 대기
    pub async fn wait_for_shutdown(&self) -> Result<()> {
        tokio::signal::ctrl_c()
            .await
            .context("Failed to listen for ctrl-c")?;

        info!("Shutdown signal received");
        self.stop_all().await
    }
}
