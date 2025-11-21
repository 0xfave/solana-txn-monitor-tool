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

The IDL Guesser is integrated into this repository (forked from [SEC3's original tool](https://github.com/sec3-service/IDLGuesser)) and built automatically:

```bash
# Build everything (main tool + IDL Guesser)
make setup
```

The binary will be located at `idl-guesser/target/release/idl-guesser` and automatically used by the parser.

**Why forked?** I maintain my own copy to ensure long-term availability and compatibility, independent of external repository changes.

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

# Build everything (main tool + IDL Guesser) - takes ~15 minutes first time
make setup

# Or build individually
make build          # Main tool only
make build-idl-guesser  # IDL Guesser only

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

idl-guesser/        # IDL Guesser (forked from SEC3)
├── src/            # Bytecode analysis and IDL recovery
└── Cargo.toml      # Separate build configuration
```

## Acknowledgments

This project incorporates and builds upon excellent work from the Solana community:

### IDL Guesser
I am grateful to the [SEC3 team](https://www.sec3.dev/) for creating [IDL Guesser](https://github.com/sec3-service/IDLGuesser), an innovative tool for recovering IDL information from closed-source Anchor programs through bytecode analysis. Their work enables automatic instruction parsing without requiring manual IDL files, making this tool significantly more powerful.

**Original Repository**: https://github.com/sec3-service/IDLGuesser  
**Blog Post**: [Recovering Instruction Layouts from Closed-Source Solana Programs](https://www.sec3.dev/blog/idl-guesser-recovering-instruction-layouts-from-closed-source-solana-programs)  
**License**: MIT

I maintain a fork of IDL Guesser in the `idl-guesser/` directory to ensure long-term availability and compatibility with this project.

## License

This project is licensed under MIT.
