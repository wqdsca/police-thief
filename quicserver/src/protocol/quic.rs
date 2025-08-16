//! QUIC Protocol Implementation Details

use serde::{Deserialize, Serialize};

/// QUIC Protocol Version
pub const QUIC_VERSION: u32 = 1;

/// Protocol message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtocolMessage {
    // Connection management
    Connect {
        version: u32,
        capabilities: ClientCapabilities,
    },
    ConnectAck {
        session_id: String,
        features: ServerFeatures,
    },
    Disconnect {
        reason: String,
    },

    // Stream control
    OpenStream {
        stream_type: u8,
    },
    CloseStream {
        stream_id: u64,
    },

    // Keep-alive
    Ping {
        timestamp: i64,
    },
    Pong {
        timestamp: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub supports_0rtt: bool,
    pub supports_migration: bool,
    pub max_streams: u32,
    pub preferred_protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerFeatures {
    pub compression_enabled: bool,
    pub encryption_level: String,
    pub max_message_size: usize,
    pub stream_types_supported: Vec<u8>,
}
