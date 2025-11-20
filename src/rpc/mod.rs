mod coordinator;
mod websocket;

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, info};

// Re-export WebSocket client and types
pub use websocket::{HeliusWebSocket, Notification, NotificationResult};

// Re-export coordinator
pub use coordinator::{RpcConfig, RpcCoordinator, TransactionEvent};

/// Helius RPC HTTP client for fetching Solana transactions
pub struct HeliusClient {
    client: Client,
    base_url: String,
}

impl HeliusClient {
    /// Create a new Helius client
    pub fn new(api_key: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        let base_url = format!("https://mainnet.helius-rpc.com/?api-key={}", api_key);

        Ok(Self { client, base_url })
    }

    /// Fetch recent transaction signatures for a program
    pub async fn get_signatures_for_address(&self, address: &str, limit: Option<usize>) -> Result<Vec<SignatureInfo>> {
        info!("Fetching signatures for address: {}", address);

        let params = json!([
            address,
            {
                "limit": limit.unwrap_or(10),
            }
        ]);

        let request_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSignaturesForAddress",
            "params": params,
        });

        debug!("Request body: {}", serde_json::to_string_pretty(&request_body)?);

        let response =
            self.client.post(&self.base_url).json(&request_body).send().await.context("Failed to send request")?;

        let status = response.status();
        let response_text = response.text().await?;

        debug!("Response status: {}", status);
        debug!("Response body: {}", response_text);

        if !status.is_success() {
            anyhow::bail!("Request failed with status {}: {}", status, response_text);
        }

        let rpc_response: RpcResponse<Vec<SignatureInfo>> =
            serde_json::from_str(&response_text).context("Failed to parse response")?;

        if let Some(error) = rpc_response.error {
            anyhow::bail!("RPC error: {:?}", error);
        }

        rpc_response.result.context("No result in response")
    }

    /// Fetch a transaction by signature
    pub async fn get_transaction(&self, signature: &str) -> Result<TransactionResponse> {
        info!("Fetching transaction: {}", signature);

        let params = json!([
            signature,
            {
                "encoding": "jsonParsed",
                "maxSupportedTransactionVersion": 0,
            }
        ]);

        let request_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTransaction",
            "params": params,
        });

        debug!("Request body: {}", serde_json::to_string_pretty(&request_body)?);

        let response =
            self.client.post(&self.base_url).json(&request_body).send().await.context("Failed to send request")?;

        let status = response.status();
        let response_text = response.text().await?;

        debug!("Response status: {}", status);

        if !status.is_success() {
            anyhow::bail!("Request failed with status {}: {}", status, response_text);
        }

        let rpc_response: RpcResponse<TransactionResponse> = serde_json::from_str(&response_text)
            .with_context(|| format!("Failed to parse response: {}", response_text))?;

        if let Some(error) = rpc_response.error {
            anyhow::bail!("RPC error: {:?}", error);
        }

        rpc_response.result.context("No result in response")
    }
}

// ============================================================================
// Response Types (These will be adjusted based on real data we see)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcResponse<T> {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureInfo {
    pub signature: String,
    pub slot: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub err: Option<serde_json::Value>,
    pub memo: Option<String>,
    pub block_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation_status: Option<String>,
}

/// This is a placeholder - we'll adjust this based on what we actually receive
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResponse {
    pub slot: u64,
    pub block_time: Option<i64>,
    pub transaction: serde_json::Value,  // We'll inspect this first
    pub meta: Option<serde_json::Value>, // We'll inspect this first
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Only run with actual API key
    async fn test_fetch_signatures() -> anyhow::Result<()> {
        let api_key = std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY not set");
        let client = HeliusClient::new(api_key)?;

        // Jupiter program ID
        let jupiter_program = "JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB";

        let signatures = client.get_signatures_for_address(jupiter_program, Some(5)).await?;

        println!("Fetched {} signatures", signatures.len());
        for sig in &signatures {
            println!("  - {}", sig.signature);
        }

        assert!(!signatures.is_empty());
        Ok(())
    }

    #[tokio::test]
    #[ignore] // Only run with actual API key
    async fn test_fetch_transaction() -> anyhow::Result<()> {
        let api_key = std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY not set");
        let client = HeliusClient::new(api_key)?;

        // First get a signature
        let jupiter_program = "JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB";
        let signatures = client.get_signatures_for_address(jupiter_program, Some(1)).await?;

        if let Some(sig_info) = signatures.first() {
            let transaction = client.get_transaction(&sig_info.signature).await?;

            println!("Transaction details:");
            println!("{}", serde_json::to_string_pretty(&transaction)?);
        }
        Ok(())
    }
}
