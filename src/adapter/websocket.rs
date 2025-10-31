use crate::interface::{Broadcaster, TranscriptRecord};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct RewindSession {
    pub session_id: String,
    pub filename: String,
    pub records: Vec<TranscriptRecord>,
    pub current_index: usize,
}

pub type SessionStore = Arc<Mutex<HashMap<String, RewindSession>>>;

pub struct WebSocketBroadcaster {
    pub session_id: String,
    pub sessions: SessionStore,
}

#[async_trait::async_trait]
impl Broadcaster for WebSocketBroadcaster {
    async fn broadcast(
        &self,
        session_id: String,
        records: Vec<TranscriptRecord>,
    ) -> anyhow::Result<()> {
        let session = RewindSession {
            session_id: session_id.clone(),
            filename: "".to_string(),
            records,
            current_index: 0,
        };

        let mut sessions = self.sessions.lock().await;
        sessions.insert(session_id, session);

        Ok(())
    }
}
