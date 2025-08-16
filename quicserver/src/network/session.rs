//! Session Management for QUIC connections

use anyhow::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Option<i32>,
    pub username: Option<String>,
    pub room_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub stream_stats: StreamStats,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamStats {
    pub control_messages: u64,
    pub game_messages: u64,
    pub chat_messages: u64,
    pub voice_packets: u64,
    pub bulk_transfers: u64,
}

pub struct SessionManager {
    sessions: Arc<DashMap<Uuid, Session>>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    pub fn create_session(&self, connection_id: Uuid) -> Session {
        let session = Session {
            id: connection_id,
            user_id: None,
            username: None,
            room_id: None,
            created_at: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
            stream_stats: StreamStats::default(),
        };

        self.sessions.insert(connection_id, session.clone());
        session
    }

    pub fn authenticate_session(
        &self,
        session_id: Uuid,
        user_id: i32,
        username: String,
    ) -> Result<()> {
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            session.user_id = Some(user_id);
            session.username = Some(username);
            session.last_activity = chrono::Utc::now();
            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", session_id)
        }
    }

    pub fn join_room(&self, session_id: Uuid, room_id: String) -> Result<()> {
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            session.room_id = Some(room_id);
            session.last_activity = chrono::Utc::now();
            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", session_id)
        }
    }

    pub fn leave_room(&self, session_id: Uuid) -> Result<()> {
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            session.room_id = None;
            session.last_activity = chrono::Utc::now();
            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", session_id)
        }
    }

    pub fn update_stream_stats<F>(&self, session_id: Uuid, updater: F) -> Result<()>
    where
        F: FnOnce(&mut StreamStats),
    {
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            updater(&mut session.stream_stats);
            session.last_activity = chrono::Utc::now();
            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", session_id)
        }
    }

    pub fn remove_session(&self, session_id: Uuid) {
        self.sessions.remove(&session_id);
    }

    pub fn get_session(&self, session_id: &Uuid) -> Option<Session> {
        self.sessions.get(session_id).map(|s| s.clone())
    }

    pub fn get_room_sessions(&self, room_id: &str) -> Vec<Session> {
        self.sessions
            .iter()
            .filter(|s| s.room_id.as_deref() == Some(room_id))
            .map(|s| s.clone())
            .collect()
    }
}
