use crate::interface::{BroadcastMessage, Broadcaster, TranscriptRecord};

pub struct WebhookBroadcaster {
    pub webhook_url: String,
}

#[async_trait::async_trait]
impl Broadcaster for WebhookBroadcaster {
    async fn broadcast(
        &self,
        session_id: i32,
        records: Vec<TranscriptRecord>,
    ) -> anyhow::Result<()> {
        broadcast_to_webhook(self.webhook_url.clone(), session_id, records).await
    }
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

async fn broadcast_to_webhook(
    webhook_url: String,
    session_id: i32,
    records: Vec<TranscriptRecord>,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let mut last_time = 0;

    println!("Starting webhook broadcast to: {}", webhook_url);

    for record in &records {
        // Parse the time field from HH:MM:SS format to total seconds
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

        // Create broadcast message with session_id and body
        let broadcast_message = BroadcastMessage {
            session_id,
            body: record.clone(),
        };

        // Send POST request to webhook
        let response = client
            .post(&webhook_url)
            .json(&broadcast_message)
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    println!(
                        "✓ Sent to webhook at {}s: {} - {}",
                        current_time, record.speaker, record.sentence
                    );
                } else {
                    let status = resp.status();
                    eprintln!(
                        "✗ Webhook returned status {}: {} - {}",
                        status, record.speaker, record.sentence
                    );

                    // Stop session on connection failure errors (4xx and 5xx)
                    if status.is_client_error() || status.is_server_error() {
                        eprintln!(
                            "✗ Stopping webhook broadcast due to error status: {}",
                            status
                        );
                        return Err(anyhow::anyhow!(
                            "Webhook connection failed with status: {}",
                            status
                        ));
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "✗ Failed to send to webhook: {} - {} - {}",
                    e, record.speaker, record.sentence
                );
                // Stop session on connection errors
                eprintln!("✗ Stopping webhook broadcast due to connection error");
                return Err(anyhow::anyhow!("Webhook connection failed: {}", e));
            }
        }

        last_time = current_time;
    }

    // Send completion message
    let completion_message = serde_json::json!({
        "status": "complete",
        "message": "Broadcast completed"
    });

    match client
        .post(&webhook_url)
        .json(&completion_message)
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("✓ Sent completion message to webhook");
            } else {
                eprintln!(
                    "✗ Failed to send completion message, status: {}",
                    resp.status()
                );
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to send completion message: {}", e);
        }
    }

    println!("Webhook broadcast completed");
    Ok(())
}
