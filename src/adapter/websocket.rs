use crate::interface::{Broadcaster, TranscriptRecord};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct RewindSession {
    pub job_description_enrichment_session: Option<i32>,
    pub candidate_profile_enrichment_session: Option<i32>,
    pub filename: String,
    pub records: Vec<TranscriptRecord>,
    pub current_index: usize,
}

pub type SessionStore = Arc<Mutex<HashMap<String, RewindSession>>>;

pub struct WebSocketBroadcaster {
    pub job_description_enrichment_session: Option<i32>,
    pub candidate_profile_enrichment_session: Option<i32>,
    pub sessions: SessionStore,
}

#[async_trait::async_trait]
impl Broadcaster for WebSocketBroadcaster {
    async fn broadcast(
        &self,
        session_id: i32,
        records: Vec<TranscriptRecord>,
    ) -> anyhow::Result<()> {
        let session = RewindSession {
            job_description_enrichment_session: self.job_description_enrichment_session,
            candidate_profile_enrichment_session: self.candidate_profile_enrichment_session,
            filename: "".to_string(),
            records,
            current_index: 0,
        };

        let mut sessions = self.sessions.lock().await;
        sessions.insert(session_id.to_string(), session);

        Ok(())
    }
}
