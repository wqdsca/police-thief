//! Metrics collection for QUIC server

use crate::network::stream::StreamType;
use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec, CounterVec, GaugeVec,
    HistogramVec,
};
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::info;

lazy_static! {
    static ref CONNECTION_COUNTER: CounterVec = register_counter_vec!(
        "quic_connections_total",
        "Total number of QUIC connections",
        &["type"]
    )
    .expect("Failed to create CONNECTION_COUNTER metric");
    static ref STREAM_COUNTER: CounterVec = register_counter_vec!(
        "quic_streams_total",
        "Total number of QUIC streams by type",
        &["stream_type"]
    )
    .expect("Failed to create STREAM_COUNTER metric");
    static ref MESSAGE_COUNTER: CounterVec = register_counter_vec!(
        "quic_messages_total",
        "Total number of messages processed",
        &["msg_type"]
    )
    .expect("Failed to create MESSAGE_COUNTER metric");
    static ref BYTES_COUNTER: CounterVec = register_counter_vec!(
        "quic_bytes_total",
        "Total bytes transferred",
        &["direction"]
    )
    .expect("Failed to create BYTES_COUNTER metric");
    static ref ACTIVE_CONNECTIONS: GaugeVec = register_gauge_vec!(
        "quic_active_connections",
        "Number of active connections",
        &["state"]
    )
    .expect("Failed to create ACTIVE_CONNECTIONS metric");
    static ref LATENCY_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "quic_message_latency_seconds",
        "Message processing latency",
        &["operation"],
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
    )
    .expect("Failed to create LATENCY_HISTOGRAM metric");
}

pub struct MetricsCollector {
    // Atomic counters for fast access
    total_connections: AtomicU64,
    active_connections: AtomicU64,
    total_messages: AtomicU64,
    messages_per_second: AtomicU64,
    total_bytes_sent: AtomicU64,
    total_bytes_received: AtomicU64,
    zero_rtt_connections: AtomicU64,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            total_connections: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
            total_messages: AtomicU64::new(0),
            messages_per_second: AtomicU64::new(0),
            total_bytes_sent: AtomicU64::new(0),
            total_bytes_received: AtomicU64::new(0),
            zero_rtt_connections: AtomicU64::new(0),
        }
    }

    pub fn record_connection(&self) {
        self.total_connections.fetch_add(1, Ordering::Relaxed);
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        CONNECTION_COUNTER.with_label_values(&["new"]).inc();
        ACTIVE_CONNECTIONS.with_label_values(&["connected"]).inc();
    }

    pub fn record_disconnection(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
        CONNECTION_COUNTER.with_label_values(&["closed"]).inc();
        ACTIVE_CONNECTIONS.with_label_values(&["connected"]).dec();
    }

    pub fn record_0rtt_success(&self) {
        self.zero_rtt_connections.fetch_add(1, Ordering::Relaxed);
        CONNECTION_COUNTER.with_label_values(&["0rtt"]).inc();
    }

    pub fn record_stream(&self, stream_type: StreamType) {
        let label = match stream_type {
            StreamType::Control => "control",
            StreamType::GameState => "game",
            StreamType::Chat => "chat",
            StreamType::Voice => "voice",
            StreamType::Bulk => "bulk",
        };
        STREAM_COUNTER.with_label_values(&[label]).inc();
    }

    pub fn record_message(&self, msg_type: &str) {
        self.total_messages.fetch_add(1, Ordering::Relaxed);
        MESSAGE_COUNTER.with_label_values(&[msg_type]).inc();
    }

    pub fn record_bytes_sent(&self, bytes: usize) {
        self.total_bytes_sent
            .fetch_add(bytes as u64, Ordering::Relaxed);
        BYTES_COUNTER
            .with_label_values(&["sent"])
            .inc_by(bytes as f64);
    }

    pub fn record_bytes_received(&self, bytes: usize) {
        self.total_bytes_received
            .fetch_add(bytes as u64, Ordering::Relaxed);
        BYTES_COUNTER
            .with_label_values(&["received"])
            .inc_by(bytes as f64);
    }

    pub fn record_unidirectional_bytes(&self, bytes: usize) {
        self.record_bytes_received(bytes);
        BYTES_COUNTER
            .with_label_values(&["unidirectional"])
            .inc_by(bytes as f64);
    }

    pub fn record_latency(&self, operation: &str, duration: std::time::Duration) {
        LATENCY_HISTOGRAM
            .with_label_values(&[operation])
            .observe(duration.as_secs_f64());
    }

    pub fn update_messages_per_second(&self, rate: u64) {
        self.messages_per_second.store(rate, Ordering::Relaxed);
    }

    pub fn report_stats(&self) {
        let stats = QuicServerStats {
            total_connections: self.total_connections.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            total_messages: self.total_messages.load(Ordering::Relaxed),
            messages_per_second: self.messages_per_second.load(Ordering::Relaxed),
            total_bytes_sent: self.total_bytes_sent.load(Ordering::Relaxed),
            total_bytes_received: self.total_bytes_received.load(Ordering::Relaxed),
            zero_rtt_connections: self.zero_rtt_connections.load(Ordering::Relaxed),
        };

        info!(
            "📊 QUIC Stats: {} msg/sec | {} active conns | {:.1}% 0-RTT | {:.2} GB sent | {:.2} GB recv",
            stats.messages_per_second,
            stats.active_connections,
            (stats.zero_rtt_connections as f64 / stats.total_connections.max(1) as f64) * 100.0,
            stats.total_bytes_sent as f64 / 1_073_741_824.0,
            stats.total_bytes_received as f64 / 1_073_741_824.0,
        );
    }
}

#[derive(Debug)]
pub struct QuicServerStats {
    pub total_connections: u64,
    pub active_connections: u64,
    pub total_messages: u64,
    pub messages_per_second: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub zero_rtt_connections: u64,
}
