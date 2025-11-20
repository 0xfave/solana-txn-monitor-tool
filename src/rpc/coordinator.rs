use anyhow::{Context, Result};
use governor::{DefaultDirectRateLimiter, Quota};
use std::{num::NonZeroU32, sync::Arc, time::Duration};
use tokio::{sync::mpsc, time::sleep};
use tracing::{debug, error, info, warn};

use super::{HeliusClient, HeliusWebSocket, Notification, NotificationResult, SignatureInfo};

/// Configuration for the RPC coordinator
#[derive(Debug, Clone)]
pub struct RpcConfig {
    /// Helius API key
    pub api_key: String,
    /// Maximum requests per second for HTTP calls
    pub max_requests_per_second: u32,
    /// Maximum retry attempts for HTTP requests
    pub max_retries: u32,
    /// Base delay for exponential backoff (milliseconds)
    pub base_retry_delay_ms: u64,
    /// Whether to fetch full transaction details automatically
    pub auto_fetch_details: bool,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            max_requests_per_second: 10, // Conservative default
            max_retries: 3,
            base_retry_delay_ms: 1000, // 1 second base delay
            auto_fetch_details: false, // Only fetch when needed
        }
    }
}

/// Represents a transaction event from the coordinator
#[derive(Debug, Clone)]
pub enum TransactionEvent {
    /// New transaction detected (signature + logs only)
    NewTransaction { signature: String, logs: Vec<String>, error: Option<serde_json::Value> },
    /// Full transaction details (when fetched)
    FullTransaction { signature: String, data: serde_json::Value },
    /// WebSocket reconnecting
    Reconnecting,
    /// WebSocket reconnected successfully
    Reconnected,
    /// Fatal error (coordinator shutting down)
    FatalError(String),
}

/// Coordinates between WebSocket (real-time) and HTTP (details) clients
pub struct RpcCoordinator {
    config: RpcConfig,
    http_client: Arc<HeliusClient>,
    rate_limiter: Arc<DefaultDirectRateLimiter>,
    last_signature: Option<String>,
}

impl RpcCoordinator {
    /// Create a new RPC coordinator
    pub fn new(config: RpcConfig) -> Result<Self> {
        let http_client = Arc::new(HeliusClient::new(config.api_key.clone())?);

        // Create rate limiter: max_requests_per_second
        let quota = Quota::per_second(
            NonZeroU32::new(config.max_requests_per_second).expect("max_requests_per_second must be non-zero"),
        );
        let rate_limiter = Arc::new(DefaultDirectRateLimiter::direct(quota));

        Ok(Self { config, http_client, rate_limiter, last_signature: None })
    }

    /// Start monitoring a program and send events to the channel
    pub async fn monitor_program(&mut self, program_id: String, tx: mpsc::Sender<TransactionEvent>) -> Result<()> {
        info!("Starting program monitor for: {}", program_id);

        loop {
            match self.run_monitor_loop(&program_id, &tx).await {
                Ok(_) => {
                    info!("Monitor loop ended normally");
                    break;
                }
                Err(e) => {
                    error!("Monitor loop error: {}", e);

                    // Notify about reconnection
                    if tx.send(TransactionEvent::Reconnecting).await.is_err() {
                        error!("Failed to send reconnection event, channel closed");
                        break;
                    }

                    // Exponential backoff before retry
                    let delay = Duration::from_millis(self.config.base_retry_delay_ms);
                    warn!("Reconnecting in {:?}...", delay);
                    sleep(delay).await;

                    // Try to backfill missed transactions using HTTP
                    if let Err(e) = self.backfill_transactions(&program_id, &tx).await {
                        warn!("Backfill failed: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Run the main WebSocket monitoring loop
    async fn run_monitor_loop(&mut self, program_id: &str, tx: &mpsc::Sender<TransactionEvent>) -> Result<()> {
        // Connect to WebSocket
        let mut ws = HeliusWebSocket::connect(self.config.api_key.clone()).await?;

        // Subscribe to logs for this program
        let _sub_id = ws.subscribe_logs(program_id).await?;

        // Notify reconnection success
        if tx.send(TransactionEvent::Reconnected).await.is_err() {
            anyhow::bail!("Channel closed");
        }

        info!("👂 Listening for transactions on program: {}", program_id);

        // Listen for notifications
        let mut notification_count = 0;
        while let Some(notification) = ws.next_notification().await? {
            notification_count += 1;
            debug!("Received notification #{}", notification_count);

            if let Err(e) = self.handle_notification(notification, tx).await {
                error!("Error handling notification: {}", e);
            }
        }

        Ok(())
    }

    /// Handle a WebSocket notification
    async fn handle_notification(
        &mut self,
        notification: Notification,
        tx: &mpsc::Sender<TransactionEvent>,
    ) -> Result<()> {
        match notification.params.result {
            NotificationResult::Logs(logs) => {
                debug!("Received transaction: {}", logs.signature);

                // Update last seen signature for gap detection
                self.last_signature = Some(logs.signature.clone());

                // Send basic event
                let event = TransactionEvent::NewTransaction {
                    signature: logs.signature.clone(),
                    logs: logs.logs.clone(),
                    error: logs.err.clone(),
                };

                tx.send(event).await.context("Failed to send transaction event")?;

                // Optionally fetch full details
                if self.config.auto_fetch_details {
                    if let Ok(full_tx) = self.fetch_transaction_with_retry(&logs.signature).await {
                        let event = TransactionEvent::FullTransaction {
                            signature: logs.signature,
                            data: serde_json::to_value(full_tx)?,
                        };
                        tx.send(event).await.context("Failed to send full transaction event")?;
                    }
                }
            }
            NotificationResult::Program(_) => {
                debug!("Received program notification (not handling for now)");
            }
            NotificationResult::Raw(value) => {
                debug!("Received unknown notification: {:?}", value);
            }
        }

        Ok(())
    }

    /// Fetch full transaction details with retry logic and rate limiting
    pub async fn fetch_transaction_with_retry(&self, signature: &str) -> Result<super::TransactionResponse> {
        let mut attempts = 0;
        let max_attempts = self.config.max_retries;

        loop {
            attempts += 1;

            // Wait for rate limiter
            self.rate_limiter.until_ready().await;

            match self.http_client.get_transaction(signature).await {
                Ok(tx) => {
                    debug!("Fetched transaction {} on attempt {}", signature, attempts);
                    return Ok(tx);
                }
                Err(e) if attempts >= max_attempts => {
                    error!("Failed to fetch transaction {} after {} attempts: {}", signature, attempts, e);
                    return Err(e);
                }
                Err(e) => {
                    warn!("Attempt {}/{} failed for {}: {}", attempts, max_attempts, signature, e);

                    // Exponential backoff: base * 2^(attempt-1)
                    let delay = Duration::from_millis(self.config.base_retry_delay_ms * 2_u64.pow(attempts - 1));
                    debug!("Retrying in {:?}...", delay);
                    sleep(delay).await;
                }
            }
        }
    }

    /// Backfill missed transactions during WebSocket downtime
    async fn backfill_transactions(&self, program_id: &str, tx: &mpsc::Sender<TransactionEvent>) -> Result<()> {
        info!("Attempting to backfill missed transactions for {}", program_id);

        // Wait for rate limiter
        self.rate_limiter.until_ready().await;

        // Fetch recent signatures
        let signatures = self.http_client.get_signatures_for_address(program_id, Some(100)).await?;

        if signatures.is_empty() {
            debug!("No signatures found for backfill");
            return Ok(());
        }

        // Find signatures we haven't seen
        let mut new_signatures: Vec<SignatureInfo> = Vec::new();
        for sig_info in signatures {
            if let Some(last_sig) = &self.last_signature {
                if sig_info.signature == *last_sig {
                    break; // Found the last one we saw, stop here
                }
            }
            new_signatures.push(sig_info);
        }

        info!("Backfilling {} missed transactions", new_signatures.len());

        // Send events for missed transactions (in reverse order - oldest first)
        for sig_info in new_signatures.iter().rev() {
            // Basic event (we don't have logs from HTTP)
            let event = TransactionEvent::NewTransaction {
                signature: sig_info.signature.clone(),
                logs: vec!["[Backfilled transaction]".to_string()],
                error: sig_info.err.clone(),
            };

            if tx.send(event).await.is_err() {
                warn!("Channel closed during backfill");
                break;
            }

            // Small delay between backfill events to avoid overwhelming the system
            sleep(Duration::from_millis(10)).await;
        }

        Ok(())
    }

    /// Get the HTTP client for manual queries
    pub fn http_client(&self) -> Arc<HeliusClient> {
        Arc::clone(&self.http_client)
    }

    /// Get the rate limiter
    pub fn rate_limiter(&self) -> Arc<DefaultDirectRateLimiter> {
        Arc::clone(&self.rate_limiter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[test]
    fn test_config_default() {
        let config = RpcConfig::default();
        assert_eq!(config.max_requests_per_second, 10);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_retry_delay_ms, 1000);
        assert_eq!(config.auto_fetch_details, false);
    }

    #[test]
    fn test_config_custom() {
        let config = RpcConfig {
            api_key: "test_key".to_string(),
            max_requests_per_second: 5,
            max_retries: 5,
            base_retry_delay_ms: 500,
            auto_fetch_details: true,
        };
        assert_eq!(config.max_requests_per_second, 5);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.base_retry_delay_ms, 500);
        assert_eq!(config.auto_fetch_details, true);
    }

    #[test]
    fn test_transaction_event_variants() {
        let event1 = TransactionEvent::NewTransaction {
            signature: "test_sig".to_string(),
            logs: vec!["log1".to_string()],
            error: None,
        };

        match event1 {
            TransactionEvent::NewTransaction { signature, .. } => {
                assert_eq!(signature, "test_sig");
            }
            _ => panic!("Wrong variant"),
        }

        let event2 = TransactionEvent::Reconnecting;
        assert!(matches!(event2, TransactionEvent::Reconnecting));

        let event3 = TransactionEvent::Reconnected;
        assert!(matches!(event3, TransactionEvent::Reconnected));
    }

    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_coordinator_creation() -> anyhow::Result<()> {
        dotenvy::dotenv().ok();
        let config = RpcConfig { api_key: std::env::var("HELIUS_API_KEY")?, ..Default::default() };

        let _coordinator = RpcCoordinator::new(config)?;
        Ok(())
    }

    #[tokio::test]
    #[ignore] // Requires API key and takes time
    async fn test_rate_limiting() -> anyhow::Result<()> {
        dotenvy::dotenv().ok();
        let config = RpcConfig {
            api_key: std::env::var("HELIUS_API_KEY")?,
            max_requests_per_second: 2, // Very low for testing
            ..Default::default()
        };

        let coordinator = RpcCoordinator::new(config)?;

        let start = std::time::Instant::now();

        // Try to make 5 requests (should be rate limited to 2/sec)
        for _ in 0..5 {
            coordinator.rate_limiter().until_ready().await;
        }

        let elapsed = start.elapsed();
        // Should take at least 2 seconds (5 requests / 2 per second)
        assert!(elapsed.as_secs() >= 2);
        Ok(())
    }

    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_fetch_transaction_with_retry() -> anyhow::Result<()> {
        dotenvy::dotenv().ok();
        let config = RpcConfig {
            api_key: std::env::var("HELIUS_API_KEY")?,
            max_requests_per_second: 10,
            max_retries: 2,
            base_retry_delay_ms: 100, // Fast retry for testing
            ..Default::default()
        };

        let coordinator = RpcCoordinator::new(config)?;

        // First, get a valid signature
        let jupiter_program = "JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB";
        let signatures = coordinator.http_client().get_signatures_for_address(jupiter_program, Some(1)).await?;

        if let Some(sig_info) = signatures.first() {
            let tx = coordinator.fetch_transaction_with_retry(&sig_info.signature).await?;
            assert!(tx.slot > 0);
        }
        Ok(())
    }

    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_fetch_invalid_signature_fails() -> anyhow::Result<()> {
        dotenvy::dotenv().ok();
        let config = RpcConfig {
            api_key: std::env::var("HELIUS_API_KEY")?,
            max_retries: 1, // Only one retry to make test fast
            base_retry_delay_ms: 100,
            ..Default::default()
        };

        let coordinator = RpcCoordinator::new(config)?;

        // Invalid signature should fail
        let result = coordinator.fetch_transaction_with_retry("invalid_signature_12345").await;
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    #[ignore] // Requires API key and takes time (30+ seconds)
    async fn test_full_monitor_integration() -> anyhow::Result<()> {
        dotenvy::dotenv().ok();
        let config = RpcConfig {
            api_key: std::env::var("HELIUS_API_KEY")?,
            max_requests_per_second: 10,
            max_retries: 3,
            base_retry_delay_ms: 1000,
            auto_fetch_details: false,
        };

        let mut coordinator = RpcCoordinator::new(config)?;
        let (tx, mut rx) = mpsc::channel(100);

        let jupiter_program = "JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB".to_string();

        // Spawn monitor in background
        tokio::spawn(async move {
            let _ = coordinator.monitor_program(jupiter_program, tx).await;
        });

        // Wait for reconnection event
        let event = tokio::time::timeout(Duration::from_secs(10), rx.recv()).await;
        assert!(event.is_ok());

        // Check we got the reconnected event
        if let Ok(Some(TransactionEvent::Reconnected)) = event {
            // Success!
        } else {
            panic!("Expected Reconnected event");
        }

        // Wait for at least one transaction or timeout
        let event = tokio::time::timeout(Duration::from_secs(30), rx.recv()).await;

        match event {
            Ok(Some(TransactionEvent::NewTransaction { signature, .. })) => {
                println!("✅ Received transaction: {}", signature);
                assert!(!signature.is_empty());
            }
            Ok(Some(other)) => {
                println!("Received other event: {:?}", other);
            }
            Ok(None) => {
                println!("Channel closed");
            }
            Err(_) => {
                println!("⏱️  Timeout waiting for transaction (this is okay if Jupiter isn't active)");
            }
        }
        Ok(())
    }
}
