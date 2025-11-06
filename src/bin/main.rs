use futures_util::{SinkExt, StreamExt};
use poem::EndpointExt;
use poem::{Result, Route, Server, middleware::Tracing, web::websocket::{WebSocket, WebSocketStream}, handler, web::Path};
use poem_openapi::{ApiResponse, Object, OpenApi, OpenApiService, payload::Json};
use restream::adapter::{SessionStore, WebSocketBroadcaster, WebhookBroadcaster};
use restream::consts::{WEBHOOK_URL_PROD, WEBHOOK_URL_TEST};
use restream::interface::{Broadcaster, TranscriptFile, TranscriptRecord, WebSocketMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path as StdPath;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::Directive;
use uuid::Uuid;

fn default_filename() -> String {
    "intake_call_test.csv".to_string()
}

fn default_test() -> bool {
    false
}

#[derive(ApiResponse)]
enum TranscriptResponse {
    /// List of transcript files
    #[oai(status = 200)]
    Ok(Json<Vec<TranscriptFile>>),
}

#[derive(Serialize, Deserialize, Debug, Object)]
struct WebsocketInfo {
    /// WebSocket URL for rewind connection
    websocket_url: String,
    /// Session ID for the rewind
    session_id: String,
    /// Port number for WebSocket connection
    port: u16,
}

#[derive(ApiResponse)]
enum RewindResponse {
    /// Rewind initiated successfully with websocket information
    #[oai(status = 200)]
    Ok(Json<WebsocketInfo>),
}

#[derive(ApiResponse)]
enum WebhookBroadcastResponse {
    /// Webhook broadcast initiated successfully
    #[oai(status = 200)]
    Ok(Json<serde_json::Value>),
    /// Error occurred during broadcast
    #[oai(status = 400)]
    BadRequest(Json<serde_json::Value>),
}

struct Api {
    sessions: SessionStore,
}

#[OpenApi]
impl Api {
    /// List all transcripts
    #[oai(path = "/transcripts", method = "get")]
    async fn list_transcripts(&self) -> TranscriptResponse {
        match load_all_transcripts().await {
            Ok(transcripts) => TranscriptResponse::Ok(Json(transcripts)),
            Err(e) => {
                error!("Error loading transcripts: {}", e);
                TranscriptResponse::Ok(Json(vec![]))
            }
        }
    }

    /// Rewind a transcript by filename (defaults to intake_call.csv)
    #[oai(path = "/websocket-broadcast", method = "get")]
    async fn handle_websocket_broadcast(
        &self,
        #[oai(name = "filename", default = "default_filename")]
        filename: poem_openapi::param::Query<String>,
        #[oai(name = "session_id")]
        session_id: poem_openapi::param::Query<i32>,
    ) -> RewindResponse {
        let filename = filename.0;
        let session_id = session_id.0;
        info!("Rewinding transcript: {} with session_id: {}", filename, session_id);

        // Load transcript from file
        let transcript_path = format!("transcript/{}", filename);
        let path = StdPath::new(&transcript_path);

        match load_transcript_from_file(path).await {
            Ok(records) => {
                // Create WebSocket broadcaster
                let session_uuid = Uuid::new_v4().to_string();
                let broadcaster = WebSocketBroadcaster {
                    session_id,
                    sessions: self.sessions.clone(),
                };

                // Use the broadcaster to setup the session
                match broadcaster.broadcast(session_id, records).await {
                    Ok(_) => {
                        // Update the session with the filename
                        let mut sessions = self.sessions.lock().await;
                        if let Some(session) = sessions.get_mut(&session_uuid) {
                            session.filename = filename.clone();
                        }

                        // Return websocket information
                        let websocket_info = WebsocketInfo {
                            websocket_url: format!("ws://0.0.0.0:8080/ws/{}", session_uuid),
                            session_id: session_uuid,
                            port: 8080,
                        };

                        RewindResponse::Ok(Json(websocket_info))
                    }
                    Err(e) => {
                        error!("Error setting up websocket broadcast: {}", e);
                        let websocket_info = WebsocketInfo {
                            websocket_url: "".to_string(),
                            session_id: "".to_string(),
                            port: 0,
                        };
                        RewindResponse::Ok(Json(websocket_info))
                    }
                }
            }
            Err(e) => {
                error!("Error loading transcript {}: {}", filename, e);
                // Return error as websocket info for now (could be improved)
                let websocket_info = WebsocketInfo {
                    websocket_url: "".to_string(),
                    session_id: "".to_string(),
                    port: 0,
                };
                RewindResponse::Ok(Json(websocket_info))
            }
        }
    }

    /// Broadcast a transcript via webhook using POST requests
    #[oai(path = "/webhook-broadcast", method = "get")]
    async fn handle_webhook_broadcast(
        &self,
        #[oai(name = "use_test", default = "default_test")] use_test: poem_openapi::param::Query<
            bool,
        >,
        #[oai(name = "filename", default = "default_filename")]
        filename: poem_openapi::param::Query<String>,
        #[oai(name = "session_id")]
        session_id: poem_openapi::param::Query<i32>,
    ) -> WebhookBroadcastResponse {
        let use_test = use_test.0;
        let filename = filename.0;
        let session_id = session_id.0;

        // Determine the webhook URL to use
        let webhook_url = if use_test {
            WEBHOOK_URL_TEST.to_string()
        } else {
            WEBHOOK_URL_PROD.to_string()
        };

        let environment = if use_test { "test" } else { "production" };
        info!(
            "Starting webhook broadcast to {} environment: {} for file: {} with session_id: {}",
            environment, webhook_url, filename, session_id
        );

        // Load transcript from file
        let transcript_path = format!("transcript/{}", filename);
        let path = StdPath::new(&transcript_path);

        match load_transcript_from_file(path).await {
            Ok(records) => {
                // Create WebHook broadcaster
                let broadcaster = WebhookBroadcaster {
                    webhook_url: webhook_url.clone(),
                };

                // Start broadcasting in background
                tokio::spawn(async move {
                    if let Err(e) = broadcaster.broadcast(session_id, records).await {
                        error!("Webhook broadcast failed: {}", e);
                    }
                });

                WebhookBroadcastResponse::Ok(Json(serde_json::json!({
                    "status": "success",
                    "message": "Webhook broadcast started",
                    "filename": filename,
                    "webhook_url": webhook_url,
                    "environment": environment
                })))
            }
            Err(e) => {
                error!("Error loading transcript {}: {}", filename, e);
                WebhookBroadcastResponse::BadRequest(Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to load transcript: {}", e),
                    "filename": filename
                })))
            }
        }
    }
}

fn create_log_filter() -> Result<EnvFilter, tracing_subscriber::filter::ParseError> {
    let filter = EnvFilter::new("info")
        .add_directive(Directive::from_str("aws_config::profile::credentials=off")?)
        .add_directive(Directive::from_str("sqlx::query=off")?)
        .add_directive(Directive::from_str("sqlx::postgres::notice=off")?);
    Ok(filter)
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let filter = create_log_filter().unwrap_or_else(|err| {
        eprintln!("Failed to parse tracing directives {err}. Falling back to 'info'.",);
        EnvFilter::new("info")
    });

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(true)
        .init();

    info!("Starting restream OpenAPI Server...");

    let sessions: SessionStore = Arc::new(Mutex::new(HashMap::new()));
    let api = Api {
        sessions: sessions.clone(),
    };

    let api_service =
        OpenApiService::new(api, "restream API", env!("CARGO_PKG_VERSION")).server("https://restream.bounteer.com/api");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec_endpoint();

    // Add WebSocket route handler
    let ws_sessions = sessions.clone();
    let app = Route::new()
        .nest("/api", api_service)
        .at("/", ui)
        .at("/spec", spec)
        .at("/ws/:session_id", websocket_handler.data(ws_sessions))
        .with(Tracing);

    // Start server
    let server_handle = tokio::spawn(async move {
        Server::new(poem::listener::TcpListener::bind("0.0.0.0:8080"))
            .run(app)
            .await
    });

    // do not open browser

    info!("Server running at http://0.0.0.0:8080");
    info!("OpenAPI UI available at http://0.0.0.0:8080/");
    info!("API endpoints available at http://0.0.0.0:8080/api/");
    info!("WebSocket server running at ws://0.0.0.0:8080/ws/");

    // Wait for server
    let _ = server_handle.await.unwrap();

    Ok(())
}

async fn load_all_transcripts() -> anyhow::Result<Vec<TranscriptFile>> {
    let transcript_dir = "transcript/";

    if !StdPath::new(transcript_dir).exists() {
        return Ok(vec![]);
    }

    let mut transcript_files = Vec::new();
    let entries = fs::read_dir(transcript_dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("csv") {
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                match load_transcript_from_file(&path).await {
                    Ok(records) => {
                        transcript_files.push(TranscriptFile {
                            filename: filename.to_string(),
                            records,
                        });
                    }
                    Err(e) => {
                        error!("Error loading {}: {}", filename, e);
                    }
                }
            }
        }
    }

    Ok(transcript_files)
}

#[handler]
async fn websocket_handler(Path(session_id): Path<String>, websocket: WebSocket, sessions: poem::web::Data<&SessionStore>) -> impl poem::IntoResponse {
    let sessions = sessions.0.clone();
    
    websocket.on_upgrade(move |socket| handle_websocket(socket, sessions, session_id))
}

fn parse_time_to_time(time_str: &str) -> i32 {
    let parts: Vec<&str> = time_str.split(':').collect();

    match parts.len() {
        3 => {
            // HH:MM:SS format
            let hours = parts[0].parse::<i32>().unwrap_or(0);
            let minutes = parts[1].parse::<i32>().unwrap_or(0);
            let time = parts[2].parse::<i32>().unwrap_or(0);
            hours * 3600 + minutes * 60 + time
        }
        2 => {
            // MM:SS format
            let minutes = parts[0].parse::<i32>().unwrap_or(0);
            let time = parts[1].parse::<i32>().unwrap_or(0);
            minutes * 60 + time
        }
        1 => {
            // Just time
            parts[0].parse::<i32>().unwrap_or(0)
        }
        _ => 0,
    }
}

async fn load_transcript_from_file(path: &StdPath) -> anyhow::Result<Vec<TranscriptRecord>> {
    let contents = fs::read_to_string(path)?;
    let mut reader = csv::Reader::from_reader(contents.as_bytes());
    let mut transcripts = Vec::new();

    for result in reader.deserialize() {
        let record: TranscriptRecord = result?;
        transcripts.push(record);
    }

    Ok(transcripts)
}

async fn handle_websocket(socket: WebSocketStream, sessions: SessionStore, session_id: String) {
    info!("New WebSocket connection for session: {}", session_id);

    // Check if the session exists
    {
        let sessions_guard = sessions.lock().await;
        if !sessions_guard.contains_key(&session_id) {
            error!("Session not found: {}", session_id);
            return;
        }
    }

    let (mut sender, _receiver) = socket.split();

    // Start broadcasting for this session
    if let Err(e) = broadcast_session_messages_poem(&session_id, &mut sender, sessions).await {
        error!("Error broadcasting messages: {}", e);
    }
}

async fn broadcast_session_messages_poem(
    session_id: &str,
    ws_sender: &mut futures_util::stream::SplitSink<WebSocketStream, poem::web::websocket::Message>,
    sessions: SessionStore,
) -> anyhow::Result<()> {
    let session = {
        let sessions_guard = sessions.lock().await;
        sessions_guard.get(session_id).cloned()
    };

    if let Some(session) = session {
        let mut last_time = 0;

        // Broadcast all messages from the session
        for record in &session.records {
            // Parse the time field from HH:MM:SS format to total time
            let current_time = parse_time_to_time(&record.time);

            // Calculate how long we should wait before sending this message
            let wait_duration = if current_time > last_time {
                current_time - last_time
            } else {
                0
            };

            // Wait for the calculated duration
            if wait_duration > 0 {
                tokio::time::sleep(tokio::time::Duration::from_secs(wait_duration as u64)).await;
            }

            let ws_message = WebSocketMessage {
                session_id: session.session_id,
                body: record.clone(),
            };
            let message = serde_json::to_string(&ws_message)?;
            ws_sender.send(poem::web::websocket::Message::Text(message)).await.map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;

            last_time = current_time;
            debug!(
                "Sent message at {}s: {} - {}",
                current_time, record.speaker, record.sentence
            );
        }

        // Send completion message
        ws_sender
            .send(poem::web::websocket::Message::Text("SESSION_COMPLETE".to_string()))
            .await.map_err(|e| anyhow::anyhow!("Failed to send completion message: {}", e))?;

        // Clean up session after broadcasting is complete
        let mut sessions_guard = sessions.lock().await;
        sessions_guard.remove(session_id);
        info!("Session {} completed and cleaned up", session_id);
    } else {
        ws_sender
            .send(poem::web::websocket::Message::Text("SESSION_NOT_FOUND".to_string()))
            .await.map_err(|e| anyhow::anyhow!("Failed to send not found message: {}", e))?;
    }

    Ok(())
}
