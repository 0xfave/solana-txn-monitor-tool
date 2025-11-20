use anyhow::Result;
use solana_txn_monitor_tool::{
    parser::TransactionParser,
    rpc::{RpcConfig, RpcCoordinator, TransactionEvent},
};
use tokio::{signal, sync::mpsc};

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    tracing::info!("🚀 Solana Transaction Monitor Tool starting...");

    // Get API key from environment
    let api_key = std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY must be set in .env file");

    // Configure RPC coordinator
    let config = RpcConfig {
        api_key,
        max_requests_per_second: 5, // Conservative to avoid rate limits
        max_retries: 3,
        base_retry_delay_ms: 1000,
        auto_fetch_details: false, // We'll fetch details manually when needed
    };

    tracing::info!("📡 Initializing RPC coordinator...");
    let mut coordinator = RpcCoordinator::new(config)?;
    tracing::info!("✅ RPC coordinator ready");

    tracing::info!("🔧 Initializing transaction parser...");
    let mut parser = TransactionParser::new();

    // Add program mapping (user would provide this via IDL/config)
    parser
        .add_program_mapping("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(), "Raydium AMM v4".to_string());

    tracing::info!("✅ Parser ready");

    // Create channel for transaction events
    let (tx, mut rx) = mpsc::channel::<TransactionEvent>(1000);

    // Monitor Raydium AMM (typically very active with swaps)
    let target_program = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string();

    tracing::info!("🎯 Starting real-time monitoring for Raydium AMM");
    tracing::info!("   Program ID: {}", target_program);
    tracing::info!("   Features: WebSocket + HTTP hybrid, rate limiting, auto-recovery");
    tracing::info!("");
    tracing::info!("⏳ Waiting for transactions...");
    tracing::info!("   Note: Raydium is typically very active, should see transactions soon");
    tracing::info!("   Press Ctrl+C to stop gracefully");
    tracing::info!("");

    // Spawn monitoring task
    let monitor_handle = tokio::spawn(async move {
        if let Err(e) = coordinator.monitor_program(target_program, tx).await {
            tracing::error!("Monitor error: {}", e);
        }
    });

    // Statistics
    let mut stats = Statistics::default();

    // Process events until Ctrl+C
    tokio::select! {
        _ = signal::ctrl_c() => {
            tracing::info!("\n🛑 Received shutdown signal...");
        }
        _ = process_transactions(&mut rx, &mut stats, &parser) => {
            tracing::info!("\n📭 Transaction stream ended");
        }
    }

    // Wait for monitor to finish
    monitor_handle.abort();

    // Print final statistics
    tracing::info!("");
    tracing::info!("📊 Final Statistics:");
    tracing::info!("   Total transactions: {}", stats.total_transactions);
    tracing::info!("   Successful: {}", stats.successful);
    tracing::info!("   Failed: {}", stats.failed);
    tracing::info!("   Parsed: {}", stats.parsed);
    tracing::info!("   Swaps detected: {}", stats.swaps);
    tracing::info!("   Reconnections: {}", stats.reconnections);
    if stats.total_transactions > 0 {
        let success_rate = (stats.successful as f64 / stats.total_transactions as f64) * 100.0;
        tracing::info!("   Success rate: {:.2}%", success_rate);
    }

    tracing::info!("");
    tracing::info!("👋 Solana Transaction Monitor Tool stopped");

    Ok(())
}

/// Process transaction events and update statistics
async fn process_transactions(
    rx: &mut mpsc::Receiver<TransactionEvent>,
    stats: &mut Statistics,
    parser: &TransactionParser,
) {
    while let Some(event) = rx.recv().await {
        match event {
            TransactionEvent::NewTransaction { signature, logs, error } => {
                stats.total_transactions += 1;

                if error.is_some() {
                    stats.failed += 1;
                    tracing::warn!("❌ Transaction failed: {}", signature);
                } else {
                    stats.successful += 1;
                }

                // Log every 10th transaction to avoid spam
                if stats.total_transactions % 10 == 0 {
                    tracing::info!(
                        "📦 Processed {} transactions (Success: {}, Failed: {})",
                        stats.total_transactions,
                        stats.successful,
                        stats.failed
                    );
                }

                // Example: Check logs for interesting patterns
                let has_swap = logs.iter().any(|log| log.to_lowercase().contains("swap"));
                if has_swap {
                    tracing::info!("💱 Swap detected in transaction: {}", signature);
                    stats.swaps += 1;

                    // TODO: Fetch full details for swap transactions
                    // let http_client = coordinator.http_client();
                    // let full_tx = http_client.get_transaction(&signature).await?;
                }
            }
            TransactionEvent::FullTransaction { signature, data } => {
                stats.parsed += 1;

                // Parse the full transaction
                match parser.parse_transaction(&signature, &data) {
                    Ok(parsed) => {
                        tracing::info!("═══════════════════════════════════════════════════════════");
                        tracing::info!("🔍 Parsed Transaction #{}", stats.parsed);
                        tracing::info!("═══════════════════════════════════════════════════════════");
                        tracing::info!("Signature: {}", parsed.signature);
                        tracing::info!("Slot: {}", parsed.slot);
                        tracing::info!(
                            "Signer: {}...{}",
                            &parsed.signer[..8],
                            &parsed.signer[parsed.signer.len() - 8..]
                        );
                        tracing::info!("Fee: {} SOL", parsed.fee as f64 / 1_000_000_000.0);
                        tracing::info!("Success: {}", parsed.success);

                        // Show programs
                        tracing::info!("\n📋 Programs ({}):", parsed.program_ids.len());
                        for prog in &parsed.program_ids {
                            if let Some(name) = parser.get_program_name(prog) {
                                tracing::info!("  • {} ({})", name, prog);
                            } else {
                                tracing::info!("  • {}", prog);
                            }
                        }

                        // Show instructions
                        tracing::info!("\n🔧 Instructions: {}", parsed.instructions.len());

                        // Show SOL transfers
                        if !parsed.sol_transfers.is_empty() {
                            tracing::info!("\n💰 SOL Transfers:");
                            for transfer in &parsed.sol_transfers {
                                tracing::info!(
                                    "  {} SOL: {}...{} → {}...{}",
                                    transfer.amount_sol,
                                    &transfer.from[..8],
                                    &transfer.from[transfer.from.len() - 8..],
                                    &transfer.to[..8],
                                    &transfer.to[transfer.to.len() - 8..]
                                );
                            }
                        }

                        // Show token transfers
                        if !parsed.token_transfers.is_empty() {
                            tracing::info!("\n🪙 Token Transfers: {}", parsed.token_transfers.len());
                            for (i, transfer) in parsed.token_transfers.iter().take(5).enumerate() {
                                tracing::info!(
                                    "  #{}: {} (mint: {}...{})",
                                    i + 1,
                                    transfer.amount,
                                    &transfer.mint[..8],
                                    &transfer.mint[transfer.mint.len() - 8..]
                                );
                            }
                            if parsed.token_transfers.len() > 5 {
                                tracing::info!("  ... and {} more", parsed.token_transfers.len() - 5);
                            }
                        }

                        tracing::info!("═══════════════════════════════════════════════════════════\n");

                        // TODO: Apply rules engine
                        // TODO: Store in ClickHouse if flagged
                    }
                    Err(e) => {
                        tracing::error!("❌ Failed to parse transaction {}: {}", signature, e);
                    }
                }
            }
            TransactionEvent::Reconnecting => {
                stats.reconnections += 1;
                tracing::warn!("⚠️  WebSocket disconnected. Reconnecting... (attempt #{})", stats.reconnections);
            }
            TransactionEvent::Reconnected => {
                tracing::info!("✅ WebSocket reconnected! Backfilling missed transactions...");
            }
            TransactionEvent::FatalError(msg) => {
                tracing::error!("💀 Fatal error: {}", msg);
                break;
            }
        }
    }
}

#[derive(Default)]
struct Statistics {
    total_transactions: usize,
    successful: usize,
    failed: usize,
    parsed: usize,
    swaps: usize,
    reconnections: usize,
}
