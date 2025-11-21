pub mod idl_decoder;
pub mod idl_guesser;
pub mod idl_loader;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tracing::debug;

/// Parsed transaction with extracted metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTransaction {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<i64>,
    pub signer: String,
    pub fee: u64,
    pub success: bool,
    pub instructions: Vec<ParsedInstruction>,
    pub program_ids: Vec<String>,
    pub sol_transfers: Vec<SolTransfer>,
    pub token_transfers: Vec<TokenTransfer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedInstruction {
    pub program_id: String,
    pub program_name: Option<String>,
    pub instruction_type: Option<String>,
    pub accounts: Vec<String>,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolTransfer {
    pub from: String,
    pub to: String,
    pub amount_lamports: u64,
    pub amount_sol: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTransfer {
    pub from: String,
    pub to: String,
    pub mint: String,
    pub amount: String,
    pub decimals: Option<u8>,
}

/// Transaction parser that extracts metadata from raw Solana transactions
pub struct TransactionParser {
    /// User-provided program mappings (program_id -> name from IDL)
    program_mappings: std::collections::HashMap<String, String>,
    /// Cached IDL schemas (program_id -> IDL)
    idl_cache: std::collections::HashMap<String, crate::types::IDLSchema>,
    /// RPC URL for fetching program data when guessing IDLs
    rpc_url: Option<String>,
}

impl TransactionParser {
    /// Create a new transaction parser
    pub fn new() -> Self {
        Self {
            program_mappings: std::collections::HashMap::new(),
            idl_cache: std::collections::HashMap::new(),
            rpc_url: None,
        }
    }

    /// Create a new transaction parser with RPC URL for IDL guessing
    pub fn with_rpc_url(rpc_url: String) -> Self {
        Self {
            program_mappings: std::collections::HashMap::new(),
            idl_cache: std::collections::HashMap::new(),
            rpc_url: Some(rpc_url),
        }
    }

    /// Add a program mapping from IDL or user config
    pub fn add_program_mapping(&mut self, program_id: String, name: String) {
        self.program_mappings.insert(program_id, name);
    }

    /// Add multiple program mappings at once
    pub fn add_program_mappings(&mut self, mappings: std::collections::HashMap<String, String>) {
        self.program_mappings.extend(mappings);
    }

    /// Load IDL for a program using 3-tier strategy:
    /// 1. User-provided IDL file (if path given)
    /// 2. IDL Guesser (automatic from bytecode)
    /// 3. Basic parsing (fallback, no IDL)
    ///
    /// Returns the tier used: "user-provided", "guessed", or "basic"
    pub fn load_idl_for_program(
        &mut self,
        program_id: &str,
        user_idl_path: Option<std::path::PathBuf>,
    ) -> Result<String> {
        // Tier 1: User-provided IDL
        if let Some(idl_path) = user_idl_path {
            debug!("Loading user-provided IDL from: {:?}", idl_path);
            let idl = idl_loader::load_idl_from_file(&idl_path).context("Failed to load user-provided IDL")?;

            self.program_mappings.insert(program_id.to_string(), idl.name.clone());
            self.idl_cache.insert(program_id.to_string(), idl);
            return Ok("user-provided".to_string());
        }

        // Tier 2: IDL Guesser (if RPC URL available)
        if let Some(ref rpc_url) = self.rpc_url {
            debug!("Attempting to guess IDL for program: {}", program_id);
            match idl_guesser::guess_idl_from_program(program_id, rpc_url) {
                Ok(idl) => {
                    debug!("Successfully guessed IDL with {} instructions", idl.instructions.len());
                    self.program_mappings.insert(program_id.to_string(), idl.name.clone());
                    self.idl_cache.insert(program_id.to_string(), idl);
                    return Ok("guessed".to_string());
                }
                Err(e) => {
                    debug!("IDL Guesser failed: {}, falling back to basic parsing", e);
                    // Fall through to Tier 3
                }
            }
        }

        // Tier 3: Basic parsing (no IDL, use existing logic)
        debug!("Using basic parsing for program: {}", program_id);
        Ok("basic".to_string())
    }

    /// Get cached IDL for a program
    pub fn get_idl(&self, program_id: &str) -> Option<&crate::types::IDLSchema> {
        self.idl_cache.get(program_id)
    }

    /// Parse a raw transaction JSON from Helius RPC
    pub fn parse_transaction(&self, signature: &str, raw_json: &serde_json::Value) -> Result<ParsedTransaction> {
        debug!("Parsing transaction: {}", signature);

        // Extract basic metadata
        let slot = raw_json["slot"].as_u64().context("Missing slot")?;
        let block_time = raw_json["blockTime"].as_i64();

        // Extract transaction data
        let tx = &raw_json["transaction"];
        let message = &tx["message"];
        let meta = &raw_json["meta"];

        // Extract signer (first account key that is a signer)
        let signer = self.extract_signer(message)?;

        // Extract fee
        let fee = meta["fee"].as_u64().unwrap_or(0);

        // Check if transaction succeeded
        let success = meta["err"].is_null();

        // Parse instructions
        let instructions = self.parse_instructions(message)?;

        // Extract unique program IDs
        let program_ids = self.extract_program_ids(&instructions);

        // Parse SOL transfers
        let sol_transfers = self.parse_sol_transfers(message)?;

        // Parse token transfers
        let token_transfers = self.parse_token_transfers(meta)?;

        Ok(ParsedTransaction {
            signature: signature.to_string(),
            slot,
            block_time,
            signer,
            fee,
            success,
            instructions,
            program_ids,
            sol_transfers,
            token_transfers,
        })
    }

    /// Extract the transaction signer
    fn extract_signer(&self, message: &serde_json::Value) -> Result<String> {
        let account_keys = message["accountKeys"].as_array().context("Missing accountKeys")?;

        for account in account_keys {
            if account["signer"].as_bool().unwrap_or(false) {
                return Ok(account["pubkey"].as_str().context("Missing pubkey")?.to_string());
            }
        }

        anyhow::bail!("No signer found in transaction")
    }

    /// Parse all instructions in the transaction
    fn parse_instructions(&self, message: &serde_json::Value) -> Result<Vec<ParsedInstruction>> {
        let mut instructions = Vec::new();

        // Parse top-level instructions
        if let Some(instrs) = message["instructions"].as_array() {
            for instr in instrs {
                if let Ok(parsed) = self.parse_single_instruction(instr, message) {
                    instructions.push(parsed);
                }
            }
        }

        Ok(instructions)
    }

    /// Parse a single instruction
    fn parse_single_instruction(
        &self,
        instr: &serde_json::Value,
        message: &serde_json::Value,
    ) -> Result<ParsedInstruction> {
        // Get program ID
        let program_id = if let Some(program_id_str) = instr["programId"].as_str() {
            program_id_str.to_string()
        } else if let Some(program_id_index) = instr["programIdIndex"].as_u64() {
            // Resolve index to actual pubkey
            self.resolve_account_index(message, program_id_index as usize)?
        } else {
            anyhow::bail!("No program ID found")
        };

        let program_name = self.program_mappings.get(&program_id).cloned();

        // Extract instruction type (if parsed)
        let instruction_type = instr["parsed"]["type"].as_str().map(|s| s.to_string());

        // Extract accounts
        let accounts = self.extract_instruction_accounts(instr, message)?;

        // Extract raw data (if not parsed)
        let data = instr["data"].as_str().map(|s| s.to_string());

        Ok(ParsedInstruction { program_id, program_name, instruction_type, accounts, data })
    }

    /// Extract accounts from an instruction
    fn extract_instruction_accounts(
        &self,
        instr: &serde_json::Value,
        message: &serde_json::Value,
    ) -> Result<Vec<String>> {
        let mut accounts = Vec::new();

        // Try parsed accounts first
        if let Some(parsed_accounts) = instr["parsed"]["info"].as_object() {
            for (_key, value) in parsed_accounts {
                if let Some(account_str) = value.as_str() {
                    // Check if it looks like a pubkey
                    if Pubkey::from_str(account_str).is_ok() {
                        accounts.push(account_str.to_string());
                    }
                }
            }
        }

        // Try account indices
        if let Some(account_indices) = instr["accounts"].as_array() {
            for index in account_indices {
                if let Some(idx) = index.as_u64() {
                    if let Ok(pubkey) = self.resolve_account_index(message, idx as usize) {
                        accounts.push(pubkey);
                    }
                }
            }
        }

        Ok(accounts)
    }

    /// Resolve an account index to its pubkey
    fn resolve_account_index(&self, message: &serde_json::Value, index: usize) -> Result<String> {
        let account_keys = message["accountKeys"].as_array().context("Missing accountKeys")?;

        account_keys
            .get(index)
            .and_then(|a| a["pubkey"].as_str())
            .map(|s| s.to_string())
            .context("Invalid account index")
    }

    /// Extract unique program IDs
    fn extract_program_ids(&self, instructions: &[ParsedInstruction]) -> Vec<String> {
        let mut program_ids: Vec<String> = instructions
            .iter()
            .map(|i| i.program_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        program_ids.sort();
        program_ids
    }

    /// Parse SOL transfers from system program instructions
    fn parse_sol_transfers(&self, message: &serde_json::Value) -> Result<Vec<SolTransfer>> {
        let mut transfers = Vec::new();

        if let Some(instructions) = message["instructions"].as_array() {
            for instr in instructions {
                // Check if it's a system program transfer
                if let Some(parsed) = instr["parsed"].as_object() {
                    if parsed["type"].as_str() == Some("transfer") {
                        if let Some(info) = parsed["info"].as_object() {
                            let from = info["source"].as_str().unwrap_or_default().to_string();
                            let to = info["destination"].as_str().unwrap_or_default().to_string();
                            let amount_lamports = info["lamports"].as_u64().unwrap_or(0);
                            let amount_sol = amount_lamports as f64 / 1_000_000_000.0;

                            transfers.push(SolTransfer { from, to, amount_lamports, amount_sol });
                        }
                    }
                }
            }
        }

        Ok(transfers)
    }

    /// Parse token transfers from meta.postTokenBalances
    fn parse_token_transfers(&self, meta: &serde_json::Value) -> Result<Vec<TokenTransfer>> {
        let mut transfers = Vec::new();

        let pre_balances = meta["preTokenBalances"].as_array();
        let post_balances = meta["postTokenBalances"].as_array();

        if let (Some(pre), Some(post)) = (pre_balances, post_balances) {
            // Match pre and post balances by account index
            for post_balance in post {
                let account_index = post_balance["accountIndex"].as_u64();
                let mint = post_balance["mint"].as_str().unwrap_or_default().to_string();
                let owner = post_balance["owner"].as_str().unwrap_or_default().to_string();
                let decimals = post_balance["uiTokenAmount"]["decimals"].as_u64().map(|d| d as u8);
                let post_amount = post_balance["uiTokenAmount"]["amount"].as_str().unwrap_or("0").to_string();

                // Find matching pre-balance
                if let Some(account_idx) = account_index {
                    if let Some(pre_balance) = pre.iter().find(|b| b["accountIndex"].as_u64() == Some(account_idx)) {
                        let pre_amount = pre_balance["uiTokenAmount"]["amount"].as_str().unwrap_or("0");

                        // If amounts differ, there was a transfer
                        if pre_amount != post_amount.as_str() {
                            transfers.push(TokenTransfer {
                                from: owner.clone(),
                                to: owner,
                                mint,
                                amount: post_amount,
                                decimals,
                            });
                        }
                    }
                }
            }
        }

        Ok(transfers)
    }

    /// Get program name if mapped
    pub fn get_program_name(&self, program_id: &str) -> Option<&str> {
        self.program_mappings.get(program_id).map(|s| s.as_str())
    }
}

impl Default for TransactionParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let mut parser = TransactionParser::new();

        // Initially no mappings
        assert!(parser.get_program_name("JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB").is_none());

        // Add mapping
        parser.add_program_mapping(
            "JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB".to_string(),
            "Jupiter Aggregator".to_string(),
        );

        assert_eq!(parser.get_program_name("JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB"), Some("Jupiter Aggregator"));
    }

    #[test]
    fn test_parse_real_transaction() -> anyhow::Result<()> {
        let parser = TransactionParser::new();

        // Load the sample transaction
        let json_str =
            std::fs::read_to_string("transaction_sample.json").expect("transaction_sample.json should exist");
        let raw_json: serde_json::Value = serde_json::from_str(&json_str)?;

        let signature = "test_signature";
        let parsed = parser.parse_transaction(signature, &raw_json)?;

        assert_eq!(parsed.signature, signature);
        assert_eq!(parsed.slot, 376620296);
        assert!(parsed.success);
        assert!(!parsed.instructions.is_empty());
        assert!(!parsed.program_ids.is_empty());

        println!("Parsed transaction:");
        println!("  Signature: {}", parsed.signature);
        println!("  Slot: {}", parsed.slot);
        println!("  Signer: {}", parsed.signer);
        println!("  Fee: {} lamports", parsed.fee);
        println!("  Success: {}", parsed.success);
        println!("  Instructions: {}", parsed.instructions.len());
        println!("  Program IDs: {:?}", parsed.program_ids);
        println!("  SOL transfers: {}", parsed.sol_transfers.len());
        println!("  Token transfers: {}", parsed.token_transfers.len());
        Ok(())
    }

    #[test]
    fn test_load_idl_basic_fallback() -> anyhow::Result<()> {
        // Parser without RPC URL, no user IDL
        let mut parser = TransactionParser::new();

        let tier = parser.load_idl_for_program("JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB", None)?;

        assert_eq!(tier, "basic");
        assert!(parser.get_idl("JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB").is_none());

        Ok(())
    }

    #[test]
    #[ignore] // Requires RPC connection and IDL Guesser binary
    fn test_load_idl_with_guesser() -> anyhow::Result<()> {
        // Initialize tracing for test
        let _ = tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).try_init();

        // Parser with RPC URL
        let mut parser = TransactionParser::with_rpc_url("https://api.mainnet-beta.solana.com".to_string());

        // Test with pump_amm program (known to work)
        let program_id = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";

        println!("\n=== Testing IDL Guesser integration ===");
        println!("Program ID: {}", program_id);
        println!("RPC URL: https://api.mainnet-beta.solana.com");

        let tier = parser.load_idl_for_program(program_id, None);

        match &tier {
            Ok(t) => println!("✓ Tier used: {}", t),
            Err(e) => println!("✗ Error loading IDL: {:?}", e),
        }

        let tier = tier?;
        assert_eq!(tier, "guessed", "Expected guessed tier but got: {}", tier);

        // Verify IDL was cached
        let idl = parser.get_idl(program_id).expect("IDL should be cached");
        assert!(!idl.instructions.is_empty());
        assert!(idl.instructions.len() >= 10); // Should have at least the core 10 instructions

        // Verify program mapping was added
        assert!(parser.get_program_name(program_id).is_some());

        println!("✓ Successfully guessed IDL with {} instructions", idl.instructions.len());
        println!("✓ Program name: {}", idl.name);

        Ok(())
    }

    #[test]
    #[ignore] // Requires sample IDL file
    fn test_load_idl_user_provided() -> anyhow::Result<()> {
        let mut parser = TransactionParser::with_rpc_url("https://api.mainnet-beta.solana.com".to_string());

        // Test with user-provided IDL
        let idl_path = std::path::PathBuf::from("test_data/jupiter_v6.json");
        let tier = parser.load_idl_for_program("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4", Some(idl_path))?;

        assert_eq!(tier, "user-provided");

        // Verify IDL was cached
        let idl = parser.get_idl("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4").expect("IDL should be cached");
        assert!(!idl.instructions.is_empty());

        // User-provided should take precedence even with RPC URL
        println!("Loaded user-provided IDL with {} instructions", idl.instructions.len());

        Ok(())
    }

    #[test]
    fn test_parser_with_rpc_url() {
        let parser = TransactionParser::with_rpc_url("https://api.mainnet-beta.solana.com".to_string());
        assert!(parser.rpc_url.is_some());

        let parser_no_rpc = TransactionParser::new();
        assert!(parser_no_rpc.rpc_url.is_none());
    }
}
