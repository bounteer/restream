use anyhow::Result;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirefliesConfig {
    pub api_token: String,
    pub transcript_id: String,
    pub webhook_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub timestamp: i64,
    pub speaker: String,
    pub text: String,
    pub confidence: Option<f64>,
    pub is_final: Option<bool>,
    #[serde(rename = "transcriptId", skip_serializing_if = "Option::is_none")]
    pub transcript_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthPayload {
    pub token: String,
    #[serde(rename = "transcriptId")]
    pub transcript_id: String,
}

/// this is a bridge that connects to Fireflies WebSocket API and forwards events to a webhook
pub struct FirefliesBridge {
    config: FirefliesConfig,
    webhook_sender: mpsc::UnboundedSender<TranscriptionEvent>,
}

impl FirefliesBridge {
    pub fn new(
        config: FirefliesConfig,
    ) -> Result<(Self, mpsc::UnboundedReceiver<TranscriptionEvent>)> {
        let (tx, rx) = mpsc::unbounded_channel();

        let bridge = Self {
            config,
            webhook_sender: tx,
        };

        Ok((bridge, rx))
    }

    pub async fn start(&self) -> Result<()> {
        let url = Url::parse("wss://api.fireflies.ai")?;

        // Create auth payload
        let auth = AuthPayload {
            token: format!("Bearer {}", self.config.api_token),
            transcript_id: self.config.transcript_id.clone(),
        };

        let webhook_sender = self.webhook_sender.clone();
        let config = self.config.clone();

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        println!("Fireflies WebSocket connection established");

        // Send authentication message
        let auth_json = serde_json::to_string(&auth)?;
        let auth_message = Message::Text(format!("{{\"type\":\"authenticate\",\"data\":{}}}", auth_json));
        write.send(auth_message).await?;

        println!(
            "Fireflies WebSocket bridge started for transcript: {}",
            config.transcript_id
        );

        // Handle incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Parse the message to determine its type
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                        match parsed.get("type").and_then(|t| t.as_str()) {
                            Some("auth.success") => {
                                println!("Fireflies authentication successful");
                                if let Some(data) = parsed.get("data") {
                                    println!("Auth success data: {}", data);
                                }
                            }
                            Some("auth.failed") => {
                                eprintln!("Fireflies authentication failed");
                                if let Some(data) = parsed.get("data") {
                                    eprintln!("Auth failed data: {}", data);
                                }
                            }
                            Some("connection.established") => {
                                println!("Fireflies connection established");
                            }
                            Some("connection.error") => {
                                eprintln!("Fireflies connection error");
                                if let Some(data) = parsed.get("data") {
                                    eprintln!("Connection error: {}", data);
                                }
                            }
                            Some("transcription.broadcast") => {
                                if let Some(data) = parsed.get("data") {
                                    match serde_json::from_value::<TranscriptionEvent>(data.clone()) {
                                        Ok(event) => {
                                            println!("Received transcription event: {:?}", event);
                                            if let Err(e) = webhook_sender.send(event) {
                                                eprintln!("Failed to send event to webhook handler: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to parse transcription event: {}", e);
                                            eprintln!("Raw data: {}", data);
                                        }
                                    }
                                }
                            }
                            _ => {
                                // Handle unknown message types or try to parse as TranscriptionEvent directly
                                match serde_json::from_str::<TranscriptionEvent>(&text) {
                                    Ok(event) => {
                                        println!("Received transcription event: {:?}", event);
                                        if let Err(e) = webhook_sender.send(event) {
                                            eprintln!("Failed to send event to webhook handler: {}", e);
                                        }
                                    }
                                    Err(_) => {
                                        println!("Received unknown message: {}", text);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    println!("Fireflies WebSocket connection closed");
                    break;
                }
                Err(e) => {
                    eprintln!("Error reading from WebSocket: {}", e);
                    break;
                }
                _ => {
                    // Handle other message types (Binary, Ping, Pong)
                }
            }
        }

        Ok(())
    }
}

pub struct FirefliesWebhookForwarder {
    webhook_url: String,
    client: reqwest::Client,
}

impl FirefliesWebhookForwarder {
    pub fn new(webhook_url: String) -> Self {
        Self {
            webhook_url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn start_forwarding(
        &self,
        mut receiver: mpsc::UnboundedReceiver<TranscriptionEvent>,
    ) -> Result<()> {
        println!("Starting webhook forwarder to: {}", self.webhook_url);

        while let Some(event) = receiver.recv().await {
            if let Err(e) = self.forward_event(event).await {
                eprintln!("Failed to forward event to webhook: {}", e);
            }
        }

        Ok(())
    }

    async fn forward_event(&self, event: TranscriptionEvent) -> Result<()> {
        let payload = serde_json::json!({
            "source": "fireflies",
            "event": event,
            "timestamp": chrono::Utc::now().timestamp()
        });

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .timeout(Duration::from_secs(30))
            .send()
            .await?;

        if response.status().is_success() {
            println!(
                "Successfully forwarded event to webhook: {} - {}",
                event.speaker, event.text
            );
        } else {
            eprintln!("Webhook responded with status: {}", response.status());
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "No response body".to_string());
            eprintln!("Response body: {}", body);
        }

        Ok(())
    }
}

#[async_trait]
pub trait FirefliesBridgeManager {
    async fn start_bridge(&self, config: FirefliesConfig) -> Result<String>;
    async fn stop_bridge(&self, bridge_id: &str) -> Result<()>;
    async fn list_bridges(&self) -> Result<Vec<String>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fireflies_config_creation() {
        let config = FirefliesConfig {
            api_token: "test_token".to_string(),
            transcript_id: "test_transcript".to_string(),
            webhook_url: "https://example.com/webhook".to_string(),
        };

        assert_eq!(config.api_token, "test_token");
        assert_eq!(config.transcript_id, "test_transcript");
        assert_eq!(config.webhook_url, "https://example.com/webhook");
    }

    #[test]
    fn test_transcription_event_serialization() {
        let event = TranscriptionEvent {
            event_type: "transcription".to_string(),
            timestamp: 1640995200,
            speaker: "John Doe".to_string(),
            text: "Hello world".to_string(),
            confidence: Some(0.95),
            is_final: Some(true),
            transcript_id: None,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("transcription"));
        assert!(json.contains("John Doe"));
        assert!(json.contains("Hello world"));

        let deserialized: TranscriptionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type, "transcription");
        assert_eq!(deserialized.speaker, "John Doe");
        assert_eq!(deserialized.text, "Hello world");
        assert_eq!(deserialized.confidence, Some(0.95));
        assert_eq!(deserialized.is_final, Some(true));
    }

    #[test]
    fn test_auth_payload_serialization() {
        let auth = AuthPayload {
            token: "Bearer test_token".to_string(),
            transcript_id: "transcript_123".to_string(),
        };

        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("Bearer test_token"));
        assert!(json.contains("transcriptId"));
        assert!(json.contains("transcript_123"));

        let deserialized: AuthPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.token, "Bearer test_token");
        assert_eq!(deserialized.transcript_id, "transcript_123");
    }

    #[test]
    fn test_fireflies_bridge_creation() {
        let config = FirefliesConfig {
            api_token: "test_token".to_string(),
            transcript_id: "test_transcript".to_string(),
            webhook_url: "https://example.com/webhook".to_string(),
        };

        let result = FirefliesBridge::new(config);
        assert!(result.is_ok());

        let (bridge, _receiver) = result.unwrap();
        assert_eq!(bridge.config.api_token, "test_token");
        assert_eq!(bridge.config.transcript_id, "test_transcript");
    }

    #[test]
    fn test_webhook_forwarder_creation() {
        let webhook_url = "https://example.com/webhook".to_string();
        let forwarder = FirefliesWebhookForwarder::new(webhook_url.clone());

        assert_eq!(forwarder.webhook_url, webhook_url);
    }

    #[tokio::test]
    async fn test_transcription_event_channel() {
        let config = FirefliesConfig {
            api_token: "test_token".to_string(),
            transcript_id: "test_transcript".to_string(),
            webhook_url: "https://example.com/webhook".to_string(),
        };

        let (bridge, mut receiver) = FirefliesBridge::new(config).unwrap();

        let test_event = TranscriptionEvent {
            event_type: "transcription".to_string(),
            timestamp: 1640995200,
            speaker: "Test Speaker".to_string(),
            text: "Test message".to_string(),
            confidence: Some(0.9),
            is_final: Some(false),
            transcript_id: None,
        };

        bridge.webhook_sender.send(test_event.clone()).unwrap();

        let received_event = receiver.recv().await.unwrap();
        assert_eq!(received_event.event_type, test_event.event_type);
        assert_eq!(received_event.speaker, test_event.speaker);
        assert_eq!(received_event.text, test_event.text);
        assert_eq!(received_event.confidence, test_event.confidence);
        assert_eq!(received_event.is_final, test_event.is_final);
    }

    #[test]
    fn test_transcription_event_json_with_renamed_fields() {
        let json_str = r#"{"type":"transcription","timestamp":1640995200,"speaker":"John","text":"Hello","confidence":0.85,"is_final":true}"#;

        let event: TranscriptionEvent = serde_json::from_str(json_str).unwrap();
        assert_eq!(event.event_type, "transcription");
        assert_eq!(event.speaker, "John");
        assert_eq!(event.text, "Hello");
        assert_eq!(event.confidence, Some(0.85));
        assert_eq!(event.is_final, Some(true));
    }

    #[test]
    fn test_auth_payload_json_with_renamed_fields() {
        let json_str = r#"{"token":"Bearer abc123","transcriptId":"transcript_456"}"#;

        let auth: AuthPayload = serde_json::from_str(json_str).unwrap();
        assert_eq!(auth.token, "Bearer abc123");
        assert_eq!(auth.transcript_id, "transcript_456");
    }

    #[test]
    fn test_transcription_event_optional_fields() {
        let event = TranscriptionEvent {
            event_type: "transcription".to_string(),
            timestamp: 1640995200,
            speaker: "Jane Doe".to_string(),
            text: "Test without optional fields".to_string(),
            confidence: None,
            is_final: None,
            transcript_id: None,
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: TranscriptionEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.confidence, None);
        assert_eq!(deserialized.is_final, None);
        assert_eq!(deserialized.text, "Test without optional fields");
    }

    #[tokio::test]
    async fn test_multiple_events_through_channel() {
        let config = FirefliesConfig {
            api_token: "test_token".to_string(),
            transcript_id: "test_transcript".to_string(),
            webhook_url: "https://example.com/webhook".to_string(),
        };

        let (bridge, mut receiver) = FirefliesBridge::new(config).unwrap();

        let events = vec![
            TranscriptionEvent {
                event_type: "transcription".to_string(),
                timestamp: 1640995200,
                speaker: "Speaker 1".to_string(),
                text: "First message".to_string(),
                confidence: Some(0.9),
                is_final: Some(false),
                transcript_id: None,
            },
            TranscriptionEvent {
                event_type: "transcription".to_string(),
                timestamp: 1640995201,
                speaker: "Speaker 2".to_string(),
                text: "Second message".to_string(),
                confidence: Some(0.95),
                is_final: Some(true),
                transcript_id: None,
            },
        ];

        for event in &events {
            bridge.webhook_sender.send(event.clone()).unwrap();
        }

        for expected_event in events {
            let received_event = receiver.recv().await.unwrap();
            assert_eq!(received_event.speaker, expected_event.speaker);
            assert_eq!(received_event.text, expected_event.text);
        }
    }

    #[tokio::test]
    async fn test_realtime_transcript_with_transcript_id() {
        use tokio::time::{Duration, timeout};

        let transcript_id = "test_transcript_123".to_string();
        let api_token = "test_api_token".to_string();

        let config = FirefliesConfig {
            api_token: api_token.clone(),
            transcript_id: transcript_id.clone(),
            webhook_url: "https://example.com/webhook".to_string(),
        };

        let (bridge, mut receiver) = FirefliesBridge::new(config).unwrap();

        let test_events = vec![
            TranscriptionEvent {
                event_type: "transcription".to_string(),
                timestamp: 1640995200,
                speaker: "John Doe".to_string(),
                text: "Hello, this is a test transcript".to_string(),
                confidence: Some(0.95),
                is_final: Some(false),
                transcript_id: None,
            },
            TranscriptionEvent {
                event_type: "transcription".to_string(),
                timestamp: 1640995205,
                speaker: "Jane Smith".to_string(),
                text: "Yes, I can hear you clearly".to_string(),
                confidence: Some(0.98),
                is_final: Some(true),
                transcript_id: None,
            },
        ];

        for event in &test_events {
            bridge.webhook_sender.send(event.clone()).unwrap();
        }

        let mut received_events = Vec::new();
        for _ in 0..test_events.len() {
            if let Ok(Some(event)) = timeout(Duration::from_millis(100), receiver.recv()).await {
                received_events.push(event);
            }
        }

        assert_eq!(received_events.len(), test_events.len());

        for (expected, received) in test_events.iter().zip(received_events.iter()) {
            assert_eq!(received.event_type, expected.event_type);
            assert_eq!(received.transcript_id, expected.transcript_id);
            assert_eq!(received.speaker, expected.speaker);
            assert_eq!(received.text, expected.text);
            assert_eq!(received.confidence, expected.confidence);
            assert_eq!(received.is_final, expected.is_final);
        }

        assert_eq!(bridge.config.transcript_id, transcript_id);
        assert_eq!(bridge.config.api_token, api_token);
    }

    #[tokio::test]
    async fn test_realtime_transcript_websocket_connection() {

        let transcript_id = "meeting_456".to_string();
        let _session_id = uuid::Uuid::new_v4().to_string();

        let config = FirefliesConfig {
            api_token: "test_token".to_string(),
            transcript_id: transcript_id.clone(),
            webhook_url: "https://example.com/webhook".to_string(),
        };

        let (bridge, mut receiver) = FirefliesBridge::new(config).unwrap();

        let test_transcript_events = vec![
            TranscriptionEvent {
                event_type: "transcription".to_string(),
                timestamp: 1640995200,
                speaker: "Presenter".to_string(),
                text: "Welcome to today's meeting".to_string(),
                confidence: Some(0.99),
                is_final: Some(true),
                transcript_id: None,
            },
            TranscriptionEvent {
                event_type: "transcription".to_string(),
                timestamp: 1640995210,
                speaker: "Attendee".to_string(),
                text: "Thank you for the introduction".to_string(),
                confidence: Some(0.96),
                is_final: Some(false),
                transcript_id: None,
            },
        ];

        for event in &test_transcript_events {
            bridge.webhook_sender.send(event.clone()).unwrap();
        }

        let mut collected_events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            collected_events.push(event);
        }

        assert_eq!(collected_events.len(), test_transcript_events.len());
        assert_eq!(collected_events[0].speaker, "Presenter");
        assert_eq!(collected_events[0].text, "Welcome to today's meeting");
        assert_eq!(collected_events[1].speaker, "Attendee");
        assert_eq!(collected_events[1].text, "Thank you for the introduction");

        assert_eq!(bridge.config.transcript_id, transcript_id);
    }
}
