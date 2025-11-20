.PHONY: help setup build build-idl-guesser test clean run

help:
	@echo "Solana Transaction Monitor Tool - Build Commands"
	@echo ""
	@echo "Usage:"
	@echo "  make setup              Build everything (main tool + IDL Guesser)"
	@echo "  make build              Build main tool only"
	@echo "  make build-idl-guesser  Build IDL Guesser only"
	@echo "  make test               Run all tests"
	@echo "  make clean              Clean build artifacts"
	@echo "  make run                Run the main tool"
	@echo ""

setup: build build-idl-guesser
	@echo ""
	@echo "✅ Setup complete!"
	@echo "   Main tool: target/release/solana-txn-monitor-tool"
	@echo "   IDL Guesser: idl-guesser/target/release/idl-guesser"
	@echo ""
	@echo "Next steps:"
	@echo "  1. Copy .env.example to .env and add your HELIUS_API_KEY"
	@echo "  2. Run: make run"

build:
	@echo "📦 Building main tool..."
	cargo build --release

build-idl-guesser:
	@echo "🔍 Building IDL Guesser (may take 10-15 minutes on first run)..."
	cd idl-guesser && cargo build --release

test:
	@echo "🧪 Running tests..."
	cargo test --lib

test-all:
	@echo "🧪 Running all tests (including ignored)..."
	cargo test --lib -- --ignored

clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	cd idl-guesser && cargo clean

run:
	@echo "🚀 Running Solana Transaction Monitor Tool..."
	cargo run --release
