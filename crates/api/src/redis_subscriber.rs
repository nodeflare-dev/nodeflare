use crate::ws_manager::WsManager;
use mcp_common::types::WsMessage;
use std::sync::Arc;

/// Start the Redis subscriber that listens for events and broadcasts to WebSocket clients
pub async fn start_redis_subscriber(redis_url: &str, ws_manager: Arc<WsManager>) {
    let redis_url = redis_url.to_string();

    tokio::spawn(async move {
        loop {
            if let Err(e) = run_subscriber(&redis_url, ws_manager.clone()).await {
                tracing::error!("Redis subscriber error: {}. Reconnecting in 5s...", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    });
}

async fn run_subscriber(redis_url: &str, ws_manager: Arc<WsManager>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = redis::Client::open(redis_url)?;
    let mut pubsub = client.get_async_pubsub().await?;

    // Subscribe to all WebSocket event patterns
    pubsub.psubscribe("ws:*").await?;

    tracing::info!("Redis subscriber connected, listening for WebSocket events");

    loop {
        let msg = pubsub.on_message().next().await;

        if let Some(msg) = msg {
            let channel: String = msg.get_channel()?;
            let payload: String = msg.get_payload()?;

            // Parse the WebSocket message
            match serde_json::from_str::<WsMessage>(&payload) {
                Ok(ws_msg) => {
                    // Extract the actual channel name from the pattern
                    // e.g., "ws:deployment:abc123" -> "deployment:abc123"
                    let broadcast_channel = channel.strip_prefix("ws:").unwrap_or(&channel);

                    // Broadcast to WebSocket clients
                    if let Err(e) = ws_manager.broadcast(broadcast_channel, ws_msg).await {
                        tracing::debug!("Failed to broadcast to channel {}: {:?}", broadcast_channel, e);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to parse WebSocket message from Redis: {}", e);
                }
            }
        }
    }
}

// Needed for the pubsub stream
use futures::StreamExt;
