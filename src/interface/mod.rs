use poem_openapi::Object;
use serde::{Deserialize, Serialize};

#[async_trait::async_trait]
pub trait Broadcaster {
    async fn broadcast(&self, session_id: String, records: Vec<TranscriptRecord>) -> anyhow::Result<()>;
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
    /// Session ID for the broadcast
    pub session_id: String,
    /// Transcript record body
    pub body: TranscriptRecord,
}
