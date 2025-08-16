//! Connection Management for QUIC

use crate::monitoring::metrics::MetricsCollector;
use anyhow::{bail, Result};
use dashmap::DashMap;
use quinn::Connection;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

pub struct ConnectionManager {
    connections: Arc<DashMap<Uuid, ManagedConnection>>,
    max_connections: usize,
    metrics: Arc<MetricsCollector>,
}

pub struct ManagedConnection {
    pub id: Uuid,
    pub connection: Connection,
    pub remote_addr: std::net::SocketAddr,
    pub connected_at: std::time::Instant,
    pub last_activity: std::time::Instant,
    pub stream_count: usize,
}

impl ConnectionManager {
    pub fn new(max_connections: usize, metrics: Arc<MetricsCollector>) -> Self {
        Self {
            connections: Arc::new(DashMap::with_capacity(max_connections)),
            max_connections,
            metrics,
        }
    }

    pub async fn register_connection(&self, connection: Connection) -> Result<Uuid> {
        if self.connections.len() >= self.max_connections {
            bail!("Maximum connections reached: {}", self.max_connections);
        }

        let id = Uuid::new_v4();
        let remote_addr = connection.remote_address();

        let managed = ManagedConnection {
            id,
            connection: connection.clone(),
            remote_addr,
            connected_at: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
            stream_count: 0,
        };

        self.connections.insert(id, managed);

        info!("Connection registered: {} from {}", id, remote_addr);
        self.metrics.record_connection();

        Ok(id)
    }

    pub async fn unregister_connection(&self, id: Uuid) {
        if let Some((_, conn)) = self.connections.remove(&id) {
            let duration = conn.connected_at.elapsed();
            info!("Connection unregistered: {} (duration: {:?})", id, duration);
            self.metrics.record_disconnection();
        }
    }

    pub fn get_connection(&self, id: &Uuid) -> Option<Connection> {
        self.connections.get(id).map(|c| c.connection.clone())
    }

    pub fn update_activity(&self, id: &Uuid) {
        if let Some(mut conn) = self.connections.get_mut(id) {
            conn.last_activity = std::time::Instant::now();
        }
    }

    pub fn increment_stream_count(&self, id: &Uuid) {
        if let Some(mut conn) = self.connections.get_mut(id) {
            conn.stream_count += 1;
        }
    }

    pub fn decrement_stream_count(&self, id: &Uuid) {
        if let Some(mut conn) = self.connections.get_mut(id) {
            if conn.stream_count > 0 {
                conn.stream_count -= 1;
            }
        }
    }

    pub async fn prune_idle_connections(&self, idle_timeout: std::time::Duration) {
        let now = std::time::Instant::now();
        let mut to_remove = Vec::new();

        for entry in self.connections.iter() {
            if now.duration_since(entry.last_activity) > idle_timeout {
                to_remove.push(entry.id);
            }
        }

        for id in to_remove {
            debug!("Pruning idle connection: {}", id);
            self.unregister_connection(id).await;
        }
    }

    pub fn get_stats(&self) -> ConnectionStats {
        let mut stats = ConnectionStats::default();
        stats.total_connections = self.connections.len();

        for conn in self.connections.iter() {
            stats.total_streams += conn.stream_count;
            let duration = conn.connected_at.elapsed().as_secs();
            if duration > stats.longest_connection_secs {
                stats.longest_connection_secs = duration;
            }
        }

        stats
    }
}

#[derive(Debug, Default)]
pub struct ConnectionStats {
    pub total_connections: usize,
    pub total_streams: usize,
    pub longest_connection_secs: u64,
}
