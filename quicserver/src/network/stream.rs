//! Stream Multiplexing for QUIC

use crate::game_logic::GameLogicHandler;
use anyhow::Result;
use bytes::BytesMut;
use quinn::{RecvStream, SendStream};
use std::sync::Arc;
use tracing::{debug, trace, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StreamType {
    Control = 0,   // Control messages (login, logout, room management)
    GameState = 1, // Game state synchronization
    Chat = 2,      // Chat messages
    Voice = 3,     // Voice data
    Bulk = 4,      // Large file transfers
}

impl From<u8> for StreamType {
    fn from(value: u8) -> Self {
        match value {
            0 => StreamType::Control,
            1 => StreamType::GameState,
            2 => StreamType::Chat,
            3 => StreamType::Voice,
            4 => StreamType::Bulk,
            _ => StreamType::Control, // Default fallback
        }
    }
}

pub struct StreamMultiplexer {
    // Stream-specific configurations
    control_buffer_size: usize,
    game_buffer_size: usize,
    chat_buffer_size: usize,
    voice_buffer_size: usize,
    bulk_buffer_size: usize,
}

impl Default for StreamMultiplexer {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamMultiplexer {
    pub fn new() -> Self {
        Self {
            control_buffer_size: 4096,
            game_buffer_size: 8192,
            chat_buffer_size: 2048,
            voice_buffer_size: 1024,
            bulk_buffer_size: 65536,
        }
    }

    pub async fn handle_control_stream(
        &self,
        mut send: SendStream,
        mut recv: RecvStream,
        handler: Arc<dyn GameLogicHandler>,
    ) -> Result<()> {
        debug!("Processing control stream");

        loop {
            // Read message length (4 bytes)
            let mut len_buf = [0u8; 4];
            if recv.read_exact(&mut len_buf).await.is_err() {
                break;
            }
            let msg_len = u32::from_be_bytes(len_buf) as usize;

            // Read message
            let mut msg_buf = vec![0u8; msg_len];
            recv.read_exact(&mut msg_buf).await?;

            // Process control message
            let response = handler.process_control_message(&msg_buf).await?;

            // Send response
            let response_len = response.len() as u32;
            send.write_all(&response_len.to_be_bytes()).await?;
            send.write_all(&response).await?;

            trace!("Control message processed: {} bytes", msg_len);
        }

        Ok(())
    }

    pub async fn handle_game_stream(
        &self,
        mut send: SendStream,
        mut recv: RecvStream,
        handler: Arc<dyn GameLogicHandler>,
    ) -> Result<()> {
        debug!("Processing game state stream");

        // Game state uses delta compression and high-frequency updates
        let mut last_state = BytesMut::new();

        loop {
            // Read delta size
            let mut delta_len_buf = [0u8; 2]; // Smaller header for game updates
            if recv.read_exact(&mut delta_len_buf).await.is_err() {
                break;
            }
            let delta_len = u16::from_be_bytes(delta_len_buf) as usize;

            // Read delta
            let mut delta_buf = vec![0u8; delta_len];
            recv.read_exact(&mut delta_buf).await?;

            // Apply delta and process (TODO: implement proper game state handling)
            warn!("Game state processing not yet implemented in QUIC server");
            last_state = BytesMut::from(&delta_buf[..]);

            // Send simple acknowledgment
            let ack = [0x01u8]; // Simple ACK
            send.write_all(&ack).await?;

            trace!("Game delta processed: {} bytes", delta_len);
        }

        Ok(())
    }

    pub async fn handle_chat_stream(
        &self,
        mut send: SendStream,
        mut recv: RecvStream,
        handler: Arc<dyn GameLogicHandler>,
    ) -> Result<()> {
        debug!("Processing chat stream");

        loop {
            // Chat messages are typically small
            let mut msg_len_buf = [0u8; 2];
            if recv.read_exact(&mut msg_len_buf).await.is_err() {
                break;
            }
            let msg_len = u16::from_be_bytes(msg_len_buf) as usize;

            // Read chat message
            let mut msg_buf = vec![0u8; msg_len];
            recv.read_exact(&mut msg_buf).await?;

            // Process chat message (TODO: implement proper chat handling)
            let msg_str = String::from_utf8_lossy(&msg_buf);
            warn!("Chat message received but not yet processed: {}", msg_str);

            // Send broadcast confirmation
            send.write_all(&[0x01]).await?; // Simple ACK

            trace!("Chat message processed: {} bytes", msg_len);
        }

        Ok(())
    }

    pub async fn handle_voice_stream(
        &self,
        send: SendStream,
        mut recv: RecvStream,
        handler: Arc<dyn GameLogicHandler>,
    ) -> Result<()> {
        debug!("Processing voice stream");

        // Voice requires low latency, minimal processing
        loop {
            // Fixed-size voice packets for consistent latency
            let mut voice_packet = vec![0u8; self.voice_buffer_size];
            match recv.read_exact(&mut voice_packet).await {
                Ok(_) => {
                    // Forward voice data with minimal processing (TODO: implement voice handling)
                    warn!("Voice packet received but not yet processed: {} bytes", voice_packet.len());
                }
                Err(_) => break,
            }
        }

        Ok(())
    }

    pub async fn handle_bulk_stream(
        &self,
        mut send: SendStream,
        mut recv: RecvStream,
        handler: Arc<dyn GameLogicHandler>,
    ) -> Result<()> {
        debug!("Processing bulk data stream");

        // Read total size first
        let mut size_buf = [0u8; 8];
        recv.read_exact(&mut size_buf).await?;
        let total_size = u64::from_be_bytes(size_buf);

        // Stream large data in chunks
        let mut received = 0u64;
        let mut data = Vec::with_capacity(total_size.min(self.bulk_buffer_size as u64) as usize);

        while received < total_size {
            let chunk_size = ((total_size - received).min(self.bulk_buffer_size as u64)) as usize;
            let mut chunk = vec![0u8; chunk_size];
            recv.read_exact(&mut chunk).await?;

            data.extend_from_slice(&chunk);
            received += chunk_size as u64;

            // Send progress update
            let progress = (received as f32 / total_size as f32 * 100.0) as u8;
            send.write_all(&[progress]).await?;
        }

        // Process complete bulk data (TODO: implement bulk data handling)
        warn!("Bulk data received but not yet processed: {} bytes", data.len());

        // Send completion
        send.write_all(b"DONE").await?;

        debug!("Bulk transfer complete: {} bytes", total_size);
        Ok(())
    }
}
