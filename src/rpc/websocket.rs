use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// WebSocket client for real-time Solana transaction streaming
pub struct HeliusWebSocket {
    ws_stream: WsStream,
    next_id: u64,
}

impl HeliusWebSocket {
    /// Connect to Helius WebSocket
    pub async fn connect(api_key: String) -> Result<Self> {
        let url = format!("wss://mainnet.helius-rpc.com/?api-key={}", api_key);
        info!("Connecting to Helius WebSocket");

        let (ws_stream, _) = connect_async(&url).await.context("Failed to connect to WebSocket")?;

        info!("✅ WebSocket connected successfully");

        Ok(Self { ws_stream, next_id: 1 })
    }

    /// Subscribe to transaction logs for a specific program
    pub async fn subscribe_logs(&mut self, program_id: &str) -> Result<u64> {
        let subscription_id = self.next_id;
        self.next_id += 1;

        let request = json!({
            "jsonrpc": "2.0",
            "id": subscription_id,
            "method": "logsSubscribe",
            "params": [
                {
                    "mentions": [program_id]
                },
                {
                    "commitment": "confirmed"
                }
            ]
        });

        info!("Subscribing to logs for program: {}", program_id);
        debug!("Subscription request: {}", serde_json::to_string_pretty(&request)?);

        // Send subscription request
        self.ws_stream.send(Message::Text(request.to_string())).await.context("Failed to send subscription request")?;

        // Wait for subscription confirmation
        if let Some(msg) = self.ws_stream.next().await {
            let msg = msg.context("Failed to receive subscription response")?;
            if let Message::Text(text) = msg {
                debug!("Subscription response: {}", text);

                let response: SubscriptionResponse =
                    serde_json::from_str(&text).context("Failed to parse subscription response")?;

                if let Some(error) = response.error {
                    anyhow::bail!("Subscription failed: {:?}", error);
                }

                if let Some(result) = response.result {
                    info!("✅ Subscribed successfully with ID: {}", result);
                    return Ok(result);
                }
            }
        }

        anyhow::bail!("No response received for subscription")
    }

    /// Subscribe to account changes for a specific program
    pub async fn subscribe_program(&mut self, program_id: &str) -> Result<u64> {
        let subscription_id = self.next_id;
        self.next_id += 1;

        let request = json!({
            "jsonrpc": "2.0",
            "id": subscription_id,
            "method": "programSubscribe",
            "params": [
                program_id,
                {
                    "commitment": "confirmed",
                    "encoding": "jsonParsed"
                }
            ]
        });

        info!("Subscribing to program: {}", program_id);
        debug!("Subscription request: {}", serde_json::to_string_pretty(&request)?);

        // Send subscription request
        self.ws_stream.send(Message::Text(request.to_string())).await.context("Failed to send subscription request")?;

        // Wait for subscription confirmation
        if let Some(msg) = self.ws_stream.next().await {
            let msg = msg.context("Failed to receive subscription response")?;
            if let Message::Text(text) = msg {
                debug!("Subscription response: {}", text);

                let response: SubscriptionResponse =
                    serde_json::from_str(&text).context("Failed to parse subscription response")?;

                if let Some(error) = response.error {
                    anyhow::bail!("Subscription failed: {:?}", error);
                }

                if let Some(result) = response.result {
                    info!("✅ Subscribed successfully with ID: {}", result);
                    return Ok(result);
                }
            }
        }

        anyhow::bail!("No response received for subscription")
    }

    /// Receive next notification from WebSocket
    pub async fn next_notification(&mut self) -> Result<Option<Notification>> {
        loop {
            match self.ws_stream.next().await {
                Some(Ok(Message::Text(text))) => {
                    debug!("Received message: {}", text);

                    // Try to parse as notification
                    if let Ok(notification) = serde_json::from_str::<Notification>(&text) {
                        return Ok(Some(notification));
                    }

                    // Log if it's not a notification (could be subscription response, etc.)
                    debug!("Received non-notification message: {}", text);
                }
                Some(Ok(Message::Ping(data))) => {
                    debug!("Received ping, sending pong");
                    self.ws_stream.send(Message::Pong(data)).await?;
                }
                Some(Ok(Message::Pong(_))) => {
                    debug!("Received pong");
                }
                Some(Ok(Message::Close(_))) => {
                    info!("WebSocket connection closed by server");
                    return Ok(None);
                }
                Some(Err(e)) => {
                    error!("WebSocket error: {}", e);
                    return Err(e.into());
                }
                None => {
                    info!("WebSocket stream ended");
                    return Ok(None);
                }
                _ => {
                    // Binary or other message types we don't handle
                    continue;
                }
            }
        }
    }

    /// Unsubscribe from a subscription
    pub async fn unsubscribe(&mut self, subscription_id: u64) -> Result<()> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_id,
            "method": "logsUnsubscribe",
            "params": [subscription_id]
        });

        self.next_id += 1;

        info!("Unsubscribing from subscription ID: {}", subscription_id);
        self.ws_stream.send(Message::Text(request.to_string())).await.context("Failed to send unsubscribe request")?;

        Ok(())
    }

    /// Close the WebSocket connection gracefully
    pub async fn close(mut self) -> Result<()> {
        info!("Closing WebSocket connection");
        self.ws_stream.close(None).await.context("Failed to close WebSocket")?;
        Ok(())
    }
}

// ============================================================================
// WebSocket Response Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct SubscriptionResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<u64>, // subscription ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    pub params: NotificationParams,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationParams {
    pub subscription: u64,
    pub result: NotificationResult,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NotificationResult {
    /// Logs notification
    Logs(LogsNotification),
    /// Program notification
    Program(ProgramNotification),
    /// Raw JSON for unknown types
    Raw(serde_json::Value),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogsNotification {
    pub signature: String,
    pub logs: Vec<String>,
    pub err: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgramNotification {
    pub pubkey: String,
    pub account: AccountData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountData {
    pub lamports: u64,
    pub data: serde_json::Value,
    pub owner: String,
    pub executable: bool,
    #[serde(rename = "rentEpoch")]
    pub rent_epoch: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Only run with actual API key
    async fn test_websocket_connection() -> anyhow::Result<()> {
        let api_key = std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY not set");
        let mut ws = HeliusWebSocket::connect(api_key).await?;

        // Try to close gracefully
        ws.close().await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore] // Only run with actual API key
    async fn test_subscribe_logs() -> anyhow::Result<()> {
        let api_key = std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY not set");
        let mut ws = HeliusWebSocket::connect(api_key).await?;

        // Subscribe to Jupiter logs
        let jupiter_program = "JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB";
        let sub_id = ws.subscribe_logs(jupiter_program).await?;

        println!("Subscription ID: {}", sub_id);

        // Listen for a few notifications
        for i in 0..3 {
            if let Some(notification) = ws.next_notification().await? {
                println!("Notification {}: {:?}", i + 1, notification);
            }
        }

        // Unsubscribe and close
        ws.unsubscribe(sub_id).await?;
        ws.close().await?;
        Ok(())
    }
}
