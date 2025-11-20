# Solana Transaction Monitor Tool

A high-performance ETL pipeline for monitoring and flagging Solana transactions in real-time.

## Overview

Monitor specific Solana protocols (Jupiter, Raydium, etc.) and flag high-value or anomalous transactions based on configurable rules. Built with Rust for performance and reliability.

**Current Status**: Phase 2 - RPC Layer Implementation 🚀

## Features

### ✅ Completed
- **HTTP RPC Client**: Fetch transaction signatures and details from Helius
- **WebSocket Client**: Real-time transaction streaming with subscription support
- **Type System**: Comprehensive data structures for transactions, protocols, rules
- **Architecture**: Complete system design with ETL pipeline

### 🚧 In Progress
- Error recovery and rate limiting
- Transaction parser with IDL support
- Rules engine for flagging transactions

### 📋 Planned
- ClickHouse storage integration
- Terminal User Interface (TUI)
- Configuration system
- Complete ETL orchestration

## Quick Start

### Prerequisites
1. Get a free Helius API key: https://www.helius.dev/
2. Install Rust: https://rustup.rs/

### Setup
```bash
# Clone and setup
git clone <repo-url>
cd solana-txn-monitor-tool

# Configure environment
cp .env.example .env
# Edit .env and add your HELIUS_API_KEY

# Build
cargo build

# Run WebSocket monitor (watches Jupiter transactions)
cargo run --example websocket_monitor
```

### Testing RPC Client
```bash
# Fetch recent Jupiter transactions
cargo run

# Run tests (requires API key in .env)
cargo test --lib -- --ignored
```

## Architecture

```
Extract → Transform → Load
   ↓         ↓         ↓
  RPC    → Parser → ClickHouse
         → Rules
         → TUI
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design.

## Examples

### Real-time Transaction Monitoring
```rust
use solana_txn_monitor_tool::rpc::HeliusWebSocket;

let mut ws = HeliusWebSocket::connect(api_key).await?;
let sub_id = ws.subscribe_logs("JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB").await?;

while let Some(notification) = ws.next_notification().await? {
    println!("New transaction: {:?}", notification);
}
```

See `examples/websocket_monitor.rs` for a complete example.

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) - System design and data flow
- [SETUP.md](SETUP.md) - Detailed setup instructions
- [docs/WEBSOCKET_IMPLEMENTATION.md](docs/WEBSOCKET_IMPLEMENTATION.md) - WebSocket client details
- [prd.md](prd.md) - Product requirements

## Project Structure

```
src/
├── config.rs       # Configuration management
├── rpc/            # Helius RPC client (HTTP + WebSocket)
├── parser/         # Transaction parsing and IDL decoding
├── rules/          # Rule engine for flagging
├── storage/        # ClickHouse integration
├── tui/            # Terminal UI
└── types.rs        # Core data structures
```

## License

This project is licensed under MIT.
