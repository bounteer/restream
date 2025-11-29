use poem_openapi::Object;
use serde::{Deserialize, Serialize};

#[async_trait::async_trait]
pub trait Broadcaster {
    async fn broadcast(
        &self,
        session_id: i32,
        records: Vec<TranscriptRecord>,
    ) -> anyhow::Result<()>;
}

#[derive(Serialize, Deserialize, Debug, Object)]
pub struct TranscriptFile {
    /// Filename of the transcript
    pub filename: String,
    /// List of transcript records
    pub records: Vec<TranscriptRecord>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Object)]
pub struct TranscriptRecord {
    /// Time in time
    pub time: String,
    /// Speaker name
    pub speaker: String,
    /// Transcript sentence
    pub sentence: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Object)]
pub struct BroadcastMessage {
    /// Job description enrichment session ID (if applicable)
    pub job_description_enrichment_session: Option<i32>,
    /// Candidate profile enrichment session ID (if applicable)
    pub candidate_profile_enrichment_session: Option<i32>,
    /// Transcript record body
    pub body: TranscriptRecord,
}

#[derive(Serialize, Deserialize, Debug, Clone, Object)]
pub struct WebSocketMessage {
    /// Job description enrichment session ID (if applicable)
    pub job_description_enrichment_session: Option<i32>,
    /// Candidate profile enrichment session ID (if applicable)
    pub candidate_profile_enrichment_session: Option<i32>,
    /// Transcript record body
    pub body: TranscriptRecord,
}
