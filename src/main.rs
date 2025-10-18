use poem::EndpointExt;
use poem::{Result, Route, Server, middleware::Tracing};
use poem_openapi::{OpenApi, ApiResponse, Object, payload::Json, OpenApiService};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path as StdPath;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use tokio_tungstenite::{tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

#[derive(Serialize, Deserialize, Debug, Clone, Object)]
struct TranscriptRecord {
    /// Time in seconds
    seconds: String,
    /// Speaker name
    speaker: String,
    /// Transcript sentence
    sentence: String,
}

#[derive(Serialize, Deserialize, Debug, Object)]
struct TranscriptFile {
    /// Filename of the transcript
    filename: String,
    /// List of transcript records
    records: Vec<TranscriptRecord>,
}

#[derive(Deserialize, Object)]
struct RerunRequest {
    /// Filename of transcript to rerun
    filename: String,
}

#[derive(ApiResponse)]
enum TranscriptResponse {
    /// List of transcript files
    #[oai(status = 200)]
    Ok(Json<Vec<TranscriptFile>>),
}

#[derive(Serialize, Deserialize, Debug, Object)]
struct WebsocketInfo {
    /// WebSocket URL for rerun connection
    websocket_url: String,
    /// Session ID for the rerun
    session_id: String,
    /// Port number for WebSocket connection
    port: u16,
}

#[derive(Debug, Clone)]
struct RerunSession {
    session_id: String,
    filename: String,
    records: Vec<TranscriptRecord>,
    current_index: usize,
}

type SessionStore = Arc<Mutex<HashMap<String, RerunSession>>>;

#[derive(ApiResponse)]
enum RerunResponse {
    /// Rerun initiated successfully with websocket information
    #[oai(status = 200)]
    Ok(Json<WebsocketInfo>),
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
                eprintln!("Error loading transcripts: {}", e);
                TranscriptResponse::Ok(Json(vec![]))
            }
        }
    }

    /// Rerun a transcript by filename
    #[oai(path = "/rerun", method = "post")]
    async fn handle_rerun(&self, request: Json<RerunRequest>) -> RerunResponse {
        println!("Rerunning transcript: {}", request.filename);
        
        // Load transcript from file
        let transcript_path = format!("transcript/{}", request.filename);
        let path = StdPath::new(&transcript_path);
        
        match load_transcript_from_file(path).await {
            Ok(records) => {
                // Create new session
                let session_id = Uuid::new_v4().to_string();
                let session = RerunSession {
                    session_id: session_id.clone(),
                    filename: request.filename.clone(),
                    records,
                    current_index: 0,
                };
                
                // Store session
                let mut sessions = self.sessions.lock().await;
                sessions.insert(session_id.clone(), session);
                
                // Return websocket information
                let websocket_info = WebsocketInfo {
                    websocket_url: format!("ws://127.0.0.1:3031/ws/{}", session_id),
                    session_id,
                    port: 3031,
                };
                
                RerunResponse::Ok(Json(websocket_info))
            }
            Err(e) => {
                eprintln!("Error loading transcript {}: {}", request.filename, e);
                // Return error as websocket info for now (could be improved)
                let websocket_info = WebsocketInfo {
                    websocket_url: "".to_string(),
                    session_id: "".to_string(),
                    port: 0,
                };
                RerunResponse::Ok(Json(websocket_info))
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    println!("Starting Casset OpenAPI Server...");

    let sessions: SessionStore = Arc::new(Mutex::new(HashMap::new()));
    let api = Api {
        sessions: sessions.clone(),
    };

    let api_service = OpenApiService::new(api, "Casset API", "1.0")
        .server("http://127.0.0.1:3030/api");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec_endpoint();

    let app = Route::new()
        .nest("/api", api_service)
        .at("/", ui)
        .at("/spec", spec)
        .with(Tracing);

    // Start WebSocket server
    let ws_sessions = sessions.clone();
    let ws_handle = tokio::spawn(async move {
        start_websocket_server(ws_sessions).await
    });

    // Start server in background
    let server_handle = tokio::spawn(async move {
        Server::new(poem::listener::TcpListener::bind("127.0.0.1:3030"))
            .run(app)
            .await
    });

    // Open browser
    if let Err(e) = open::that("http://127.0.0.1:3030") {
        eprintln!("Failed to open browser: {}", e);
    }

    println!("Server running at http://127.0.0.1:3030");
    println!("OpenAPI UI available at http://127.0.0.1:3030/");
    println!("API endpoints available at http://127.0.0.1:3030/api/");
    println!("WebSocket server running at ws://127.0.0.1:3031");

    // Wait for both servers
    tokio::try_join!(server_handle, ws_handle).unwrap();

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
                        eprintln!("Error loading {}: {}", filename, e);
                    }
                }
            }
        }
    }

    Ok(transcript_files)
}

fn parse_time_to_seconds(time_str: &str) -> i32 {
    let parts: Vec<&str> = time_str.split(':').collect();
    
    match parts.len() {
        3 => {
            // HH:MM:SS format
            let hours = parts[0].parse::<i32>().unwrap_or(0);
            let minutes = parts[1].parse::<i32>().unwrap_or(0);
            let seconds = parts[2].parse::<i32>().unwrap_or(0);
            hours * 3600 + minutes * 60 + seconds
        }
        2 => {
            // MM:SS format
            let minutes = parts[0].parse::<i32>().unwrap_or(0);
            let seconds = parts[1].parse::<i32>().unwrap_or(0);
            minutes * 60 + seconds
        }
        1 => {
            // Just seconds
            parts[0].parse::<i32>().unwrap_or(0)
        }
        _ => 0
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

async fn start_websocket_server(sessions: SessionStore) -> anyhow::Result<()> {
    let addr = "127.0.0.1:3031";
    let listener = TcpListener::bind(&addr).await?;
    println!("WebSocket server listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let sessions = sessions.clone();
        tokio::spawn(handle_websocket_connection(stream, sessions));
    }

    Ok(())
}

async fn handle_websocket_connection(stream: TcpStream, sessions: SessionStore) {
    let addr = stream.peer_addr().unwrap();
    println!("New WebSocket connection from: {}", addr);

    let session_id = Arc::new(Mutex::new(String::new()));
    let session_id_clone = session_id.clone();
    
    let callback = move |req: &Request, response: Response| {
        let path = req.uri().path();
        println!("WebSocket upgrade request path: {}", path);
        
        // Extract session ID from path like "/ws/{session_id}"
        if let Some(extracted_id) = path.strip_prefix("/ws/") {
            if !extracted_id.is_empty() {
                if let Ok(mut id) = session_id_clone.try_lock() {
                    *id = extracted_id.to_string();
                    println!("Extracted session ID: {}", *id);
                }
            }
        }
        
        Ok(response)
    };

    let ws_stream = match tokio_tungstenite::accept_hdr_async(stream, callback).await {
        Ok(ws_stream) => ws_stream,
        Err(e) => {
            eprintln!("WebSocket handshake failed: {}", e);
            return;
        }
    };

    let session_id_str = {
        let id_guard = session_id.lock().await;
        id_guard.clone()
    };

    if session_id_str.is_empty() {
        eprintln!("No session ID found in WebSocket request path");
        return;
    }

    // Check if the session exists
    {
        let sessions_guard = sessions.lock().await;
        if !sessions_guard.contains_key(&session_id_str) {
            eprintln!("Session not found: {}", session_id_str);
            return;
        }
    }

    let (mut ws_sender, _ws_receiver) = ws_stream.split();

    // Start broadcasting for this session
    if let Err(e) = broadcast_session_messages(&session_id_str, &mut ws_sender, sessions).await {
        eprintln!("Error broadcasting messages: {}", e);
    }
}

async fn broadcast_session_messages(
    session_id: &str,
    ws_sender: &mut futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<TcpStream>, Message>,
    sessions: SessionStore,
) -> anyhow::Result<()> {
    let session = {
        let sessions_guard = sessions.lock().await;
        sessions_guard.get(session_id).cloned()
    };

    if let Some(session) = session {
        let mut last_seconds = 0;
        
        // Broadcast all messages from the session
        for record in &session.records {
            // Parse the seconds field from HH:MM:SS format to total seconds
            let current_seconds = parse_time_to_seconds(&record.seconds);
            
            // Calculate how long we should wait before sending this message
            let wait_duration = if current_seconds > last_seconds {
                current_seconds - last_seconds
            } else {
                0
            };
            
            // Wait for the calculated duration
            if wait_duration > 0 {
                tokio::time::sleep(tokio::time::Duration::from_secs(wait_duration as u64)).await;
            }
            
            let message = serde_json::to_string(&record)?;
            ws_sender.send(Message::Text(message)).await?;
            
            last_seconds = current_seconds;
            println!("Sent message at {}s: {} - {}", current_seconds, record.speaker, record.sentence);
        }

        // Send completion message
        ws_sender.send(Message::Text("SESSION_COMPLETE".to_string())).await?;
        
        // Clean up session after broadcasting is complete
        let mut sessions_guard = sessions.lock().await;
        sessions_guard.remove(session_id);
        println!("Session {} completed and cleaned up", session_id);
    } else {
        ws_sender.send(Message::Text("SESSION_NOT_FOUND".to_string())).await?;
    }

    Ok(())
}
