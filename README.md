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
- Instruction decoding from IDL schemas
- Rules engine for flagging transactions

### 📋 Planned
- ClickHouse storage integration
- Terminal User Interface (TUI)
- Configuration system
- Complete ETL orchestration

## Transaction Parser

The tool supports **3-tier IDL loading** for parsing Solana program instructions:

### Tier 1: User-Provided IDL
Manually provide IDL files for programs you want to monitor:

```rust
use solana_txn_monitor_tool::parser::TransactionParser;

let mut parser = TransactionParser::with_rpc_url("https://api.mainnet-beta.solana.com".to_string());
let tier = parser.load_idl_for_program(
    "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4",
    Some(PathBuf::from("idls/jupiter_v6.json"))
)?;
// tier == "user-provided"
```

### Tier 2: Automatic IDL Recovery (IDL Guesser)
Automatically recover IDL from closed-source Anchor programs using bytecode analysis:

```rust
let mut parser = TransactionParser::with_rpc_url("https://api.mainnet-beta.solana.com".to_string());
let tier = parser.load_idl_for_program(
    "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA", // pump_amm program
    None // No IDL file provided
)?;
// tier == "guessed" (21 instructions discovered automatically!)

let idl = parser.get_idl("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA").unwrap();
println!("Recovered {} instructions", idl.instructions.len());
```

The IDL Guesser uses bytecode analysis to extract:
- ✅ Instruction names and discriminators
- ✅ Account structures and constraints
- ✅ Parameter types (primitives, structs, enums)
- ✅ Admin functions often missing from public IDLs

### Tier 3: Basic Parsing (Fallback)
When no IDL is available, falls back to basic transaction parsing:

```rust
let mut parser = TransactionParser::new(); // No RPC URL
let tier = parser.load_idl_for_program("SomeProgram111111111111111111111111111111", None)?;
// tier == "basic"
```

### IDL Guesser Setup

The IDL Guesser is a separate tool by [SEC3](https://github.com/sec3-service/IDLGuesser) that must be built first:

```bash
# Build the IDL Guesser (one-time, takes ~11 minutes)
cd idl-guesser
cargo build --release
cd ..

# Add to .gitignore
echo "idl-guesser/target/" >> .gitignore
```

The binary will be located at `idl-guesser/target/release/idl-guesser` and automatically used by the parser.

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

# Build IDL Guesser (for automatic IDL recovery)
cd idl-guesser
cargo build --release
cd ..

# Build main project
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
