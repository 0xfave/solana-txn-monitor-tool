use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// Transaction Types
// ============================================================================

/// Raw transaction received from blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTransaction {
    /// Transaction signature (unique identifier)
    pub signature: String,
    /// Slot number when transaction was processed
    pub slot: u64,
    /// Block time (Unix timestamp)
    pub block_time: Option<i64>,
    /// Raw transaction data (serialized bytes)
    pub raw_data: Vec<u8>,
    /// Transaction metadata
    pub meta: TransactionMeta,
}

/// Transaction metadata from blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMeta {
    /// Transaction fee in lamports
    pub fee: u64,
    /// Whether transaction succeeded
    pub success: bool,
    /// Error message if transaction failed
    pub err: Option<String>,
    /// Log messages from transaction execution
    pub log_messages: Vec<String>,
}

/// Parsed transaction after decoding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTransaction {
    /// Transaction signature
    pub signature: String,
    /// When transaction was processed
    pub timestamp: DateTime<Utc>,
    /// Slot number
    pub slot: u64,
    /// Program ID that was invoked
    pub program_id: String,
    /// Account keys involved in transaction
    pub accounts: Vec<String>,
    /// Decoded instruction data (if IDL available)
    pub instruction: Option<DecodedInstruction>,
    /// Raw instruction data (hex encoded)
    pub raw_instruction_data: String,
    /// Transaction fee
    pub fee: u64,
    /// Success status
    pub success: bool,
    /// SOL amount transferred (if applicable)
    pub sol_amount: Option<f64>,
    /// Token transfers (if applicable)
    pub token_transfers: Vec<TokenTransfer>,
}

impl Default for ParsedTransaction {
    fn default() -> Self {
        Self {
            signature: String::new(),
            timestamp: Utc::now(),
            slot: 0,
            program_id: String::new(),
            accounts: Vec::new(),
            instruction: None,
            raw_instruction_data: String::new(),
            fee: 0,
            success: true,
            sol_amount: None,
            token_transfers: Vec::new(),
        }
    }
}

/// Decoded instruction using IDL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedInstruction {
    /// Instruction name (e.g., "swap", "transfer")
    pub name: String,
    /// Decoded instruction arguments
    pub args: HashMap<String, serde_json::Value>,
    /// Account information
    pub accounts: Vec<AccountInfo>,
}

/// Account information in instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    /// Account public key
    pub pubkey: String,
    /// Account role/name from IDL
    pub name: String,
    /// Whether account is signer
    pub is_signer: bool,
    /// Whether account is writable
    pub is_writable: bool,
}

/// Token transfer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTransfer {
    /// Source account
    pub from: String,
    /// Destination account
    pub to: String,
    /// Token mint address
    pub mint: String,
    /// Token symbol (if known)
    pub symbol: Option<String>,
    /// Transfer amount
    pub amount: f64,
    /// Number of decimals
    pub decimals: u8,
}

/// Flagged transaction produced by rule engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlaggedTransaction {
    /// Unique ID for this flag event
    pub flag_id: Uuid,
    /// Transaction signature
    pub signature: String,
    /// When transaction occurred
    pub timestamp: DateTime<Utc>,
    /// Program ID
    pub program_id: String,
    /// Protocol name
    pub protocol_name: String,
    /// Rules that flagged this transaction
    pub flag_reasons: Vec<String>,
    /// Severity score (0-100)
    pub severity_score: u8,
    /// Transaction amount (if applicable)
    pub amount: Option<f64>,
    /// Token symbol
    pub token_symbol: Option<String>,
    /// Accounts involved
    pub accounts: Vec<String>,
    /// Instruction name
    pub instruction_name: Option<String>,
    /// Full parsed transaction data
    pub parsed_data: ParsedTransaction,
}

// ============================================================================
// Protocol Configuration Types
// ============================================================================

/// Protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Protocol {
    /// Protocol name (e.g., "Jupiter", "Raydium")
    pub name: String,
    /// Program ID to monitor
    pub program_id: String,
    /// Optional IDL file path
    pub idl_path: Option<String>,
    /// Parsed IDL schema (loaded at runtime)
    #[serde(skip)]
    pub idl: Option<IDLSchema>,
    /// Rules to apply for this protocol
    pub rules: Vec<String>, // Rule names/IDs
    /// Whether this protocol is active
    pub enabled: bool,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl Protocol {
    /// Create a new protocol configuration
    pub fn new(name: String, program_id: String) -> Self {
        Self { name, program_id, idl_path: None, idl: None, rules: Vec::new(), enabled: true, metadata: HashMap::new() }
    }

    /// Get program ID as Pubkey
    pub fn program_pubkey(&self) -> Result<Pubkey, Box<dyn std::error::Error>> {
        self.program_id.parse().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

/// IDL Schema (Anchor format) - Root structure
/// Supports both old format (version at top level) and new format (metadata object)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLSchema {
    /// IDL version (old format) (e.g., "0.1.0")
    #[serde(default)]
    pub version: String,
    /// Program name (old format)
    #[serde(default)]
    pub name: String,
    /// Program address (new format)
    #[serde(default)]
    pub address: Option<String>,
    /// Program ID (optional in some IDLs, old format)
    #[serde(rename = "programId", default)]
    pub program_id: Option<String>,
    /// Metadata (new Anchor 0.30+ format)
    #[serde(default)]
    pub metadata: Option<IDLMetadata>,
    /// Instructions defined in IDL
    pub instructions: Vec<IDLInstruction>,
    /// Account types (state structs)
    #[serde(default)]
    pub accounts: Vec<IDLAccountType>,
    /// Custom types (structs, enums)
    #[serde(default)]
    pub types: Vec<IDLTypeDef>,
    /// Error codes
    #[serde(default)]
    pub errors: Vec<IDLError>,
    /// Events (for event logs)
    #[serde(default)]
    pub events: Vec<IDLEvent>,
    /// Constants
    #[serde(default)]
    pub constants: Vec<IDLConstant>,
}

/// IDL Metadata (new Anchor 0.30+ format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLMetadata {
    /// Program name
    pub name: String,
    /// IDL version
    pub version: String,
    /// IDL spec version
    #[serde(default)]
    pub spec: Option<String>,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
}

/// IDL Instruction definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLInstruction {
    /// Instruction name (e.g., "swap", "initialize")
    pub name: String,
    /// Instruction arguments/parameters
    #[serde(default)]
    pub args: Vec<IDLField>,
    /// Accounts required by instruction
    pub accounts: Vec<IDLAccountItem>,
    /// Returns type (optional)
    #[serde(default)]
    pub returns: Option<IDLTypeReference>,
    /// Discriminator bytes (8 bytes for Anchor instructions)
    #[serde(default)]
    pub discriminator: Vec<u8>,
    /// Documentation
    #[serde(default)]
    pub docs: Vec<String>,
}

impl IDLInstruction {
    /// Compute 8-byte discriminator from instruction name
    /// Anchor standard: SHA256("global:{name}")[..8]
    pub fn compute_discriminator(&self) -> [u8; 8] {
        use sha2::{Digest, Sha256};
        let preimage = format!("global:{}", self.name);
        let hash = Sha256::digest(preimage.as_bytes());
        let mut discriminator = [0u8; 8];
        discriminator.copy_from_slice(&hash[..8]);
        discriminator
    }
}

/// IDL Account type definition (state accounts)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLAccountType {
    /// Account name
    pub name: String,
    /// Account type structure (optional in new format)
    #[serde(rename = "type", default)]
    pub type_def: Option<IDLAccountTypeKind>,
    /// Discriminator (new format)
    #[serde(default)]
    pub discriminator: Vec<u8>,
}

/// Kind of account type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLAccountTypeKind {
    /// "struct" for account data structures
    pub kind: String,
    /// Fields in the account
    pub fields: Vec<IDLField>,
}

/// IDL Account item in instruction (supports both old and new Anchor formats)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLAccountItem {
    /// Account name
    pub name: String,
    /// Whether account is mutable (old format: isMut, new format: writable)
    #[serde(rename = "isMut", alias = "writable", default)]
    pub is_mut: bool,
    /// Whether account is signer
    #[serde(rename = "isSigner", alias = "signer", default)]
    pub is_signer: bool,
    /// Optional description
    #[serde(default)]
    pub docs: Vec<String>,
    /// Optional PDA seeds (new format)
    #[serde(default)]
    pub pda: Option<IDLPda>,
    /// Optional address constraint (new format)
    #[serde(default)]
    pub address: Option<String>,
}

/// PDA (Program Derived Address) seeds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLPda {
    /// Seed values
    pub seeds: Vec<IDLSeed>,
}

/// Seed definition for PDA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLSeed {
    /// Seed kind (e.g., "const", "arg", "account")
    pub kind: String,
    /// Seed value (depends on kind)
    #[serde(default)]
    pub value: Option<serde_json::Value>,
}

/// IDL Field (argument or struct field)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLField {
    /// Field name
    pub name: String,
    /// Field type
    #[serde(rename = "type")]
    pub type_ref: IDLTypeReference,
    /// Optional documentation
    #[serde(default)]
    pub docs: Vec<String>,
}

/// Defined type reference (supports both string and object formats)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IDLDefinedType {
    /// String format: "defined": "TypeName"
    String(String),
    /// Object format: "defined": { "name": "TypeName" }
    Object { name: String },
}

/// Type reference - can be primitive or complex
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IDLTypeReference {
    /// Simple primitive type (string)
    Primitive(String),
    /// Defined type (struct/enum) - supports both string and object formats
    Defined { defined: IDLDefinedType },
    /// Option type
    Option { option: Box<IDLTypeReference> },
    /// Vec/Array type
    Vec { vec: Box<IDLTypeReference> },
    /// Array with fixed size
    Array { array: [Box<IDLTypeReference>; 2] },
}

/// Custom type definition (struct or enum)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLTypeDef {
    /// Type name
    pub name: String,
    /// Type structure
    #[serde(rename = "type")]
    pub type_def: IDLTypeDefKind,
}

/// Kind of type definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum IDLTypeDefKind {
    /// Struct type
    Struct {
        /// Struct fields
        fields: Vec<IDLField>,
    },
    /// Enum type
    Enum {
        /// Enum variants
        variants: Vec<IDLEnumVariant>,
    },
}

/// Enum variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLEnumVariant {
    /// Variant name
    pub name: String,
    /// Optional fields for this variant
    #[serde(default)]
    pub fields: Option<Vec<IDLField>>,
}

/// IDL Event definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLEvent {
    /// Event name
    pub name: String,
    /// Event fields (optional in new format)
    #[serde(default)]
    pub fields: Vec<IDLField>,
    /// Discriminator (new format)
    #[serde(default)]
    pub discriminator: Vec<u8>,
}

/// IDL Constant definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLConstant {
    /// Constant name
    pub name: String,
    /// Constant type
    #[serde(rename = "type")]
    pub type_ref: IDLTypeReference,
    /// Constant value
    pub value: String,
}

/// IDL Error definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLError {
    /// Error code
    pub code: u32,
    /// Error name
    pub name: String,
    /// Error message
    #[serde(default)]
    pub msg: String,
}

// ============================================================================
// Rule Types
// ============================================================================

/// Rule for flagging transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Unique rule ID
    pub id: String,
    /// Rule name
    pub name: String,
    /// Rule type
    pub rule_type: RuleType,
    /// Whether rule is enabled
    pub enabled: bool,
    /// Description
    pub description: String,
}

/// Rule type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleType {
    /// Flag if amount exceeds threshold
    AmountThreshold {
        /// Threshold value
        threshold: f64,
        /// Token to check (e.g., "SOL", "USDC")
        token: String,
        /// Optional program ID filter
        program_id: Option<String>,
    },
    /// Flag if frequency exceeds limit
    Frequency {
        /// Time window in seconds
        window_seconds: u64,
        /// Maximum transaction count
        max_count: u32,
        /// What to track (sender, receiver, program)
        track_by: FrequencyTracker,
    },
    /// Flag if account matches pattern
    AccountPattern {
        /// Regex pattern to match
        pattern: String,
        /// Account role (sender, receiver, any)
        role: AccountRole,
        /// Action (flag, alert)
        action: PatternAction,
    },
    /// Custom rule (future: Lua/WASM)
    Custom {
        /// Script content
        script: String,
        /// Script language
        language: String,
    },
}

/// What to track for frequency rules
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrequencyTracker {
    /// Track by sender address
    Sender,
    /// Track by receiver address
    Receiver,
    /// Track by program ID
    Program,
    /// Track by any account
    AnyAccount,
}

/// Account role for pattern matching
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountRole {
    /// Sender/source account
    Sender,
    /// Receiver/destination account
    Receiver,
    /// Any account in transaction
    Any,
}

/// Action to take when pattern matches
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternAction {
    /// Flag the transaction
    Flag,
    /// Alert operator
    Alert,
    /// Flag and alert
    FlagAndAlert,
}

/// Rule evaluation result
#[derive(Debug, Clone)]
pub struct RuleEvaluationResult {
    /// Rule that was evaluated
    pub rule_id: String,
    /// Whether rule matched
    pub matched: bool,
    /// Reason for match (if matched)
    pub reason: Option<String>,
    /// Evaluation time in microseconds
    pub evaluation_time_us: u64,
}

// ============================================================================
// Configuration Types
// ============================================================================

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Helius RPC configuration
    pub helius: HeliusConfig,
    /// ClickHouse configuration
    pub clickhouse: ClickHouseConfig,
    /// Protocols to monitor
    pub protocols: Vec<Protocol>,
    /// Rules configuration
    pub rules: Vec<Rule>,
    /// General settings
    pub settings: Settings,
}

/// Helius RPC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeliusConfig {
    /// API key
    pub api_key: String,
    /// WebSocket URL
    pub ws_url: String,
    /// REST API URL
    pub rest_url: String,
    /// Connection pool size
    pub pool_size: usize,
    /// Rate limit (requests per second)
    pub rate_limit: u32,
}

/// ClickHouse configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickHouseConfig {
    /// Database URL
    pub url: String,
    /// Database name
    pub database: String,
    /// Username
    pub username: Option<String>,
    /// Password
    pub password: Option<String>,
    /// Connection pool size
    pub pool_size: usize,
    /// Batch size for inserts
    pub batch_size: usize,
    /// Batch timeout in seconds
    pub batch_timeout_secs: u64,
}

/// General application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Log level
    pub log_level: String,
    /// Enable metrics export
    pub enable_metrics: bool,
    /// Metrics port
    pub metrics_port: u16,
    /// TUI refresh rate (milliseconds)
    pub tui_refresh_ms: u64,
}

// ============================================================================
// Event Types (for inter-module communication)
// ============================================================================

/// Events for updating TUI
#[derive(Debug, Clone)]
pub enum UpdateEvent {
    /// New transaction received
    TransactionReceived(ParsedTransaction),
    /// Transaction flagged
    TransactionFlagged(FlaggedTransaction),
    /// Statistics update
    StatsUpdate(Statistics),
    /// Error occurred
    Error(String),
    /// Status change
    StatusChange(String),
}

/// Statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statistics {
    /// Total transactions processed
    pub total_transactions: u64,
    /// Total flagged transactions
    pub total_flagged: u64,
    /// Transactions per second
    pub tx_per_second: f64,
    /// Average parse time (milliseconds)
    pub avg_parse_time_ms: f64,
    /// Average rule evaluation time (milliseconds)
    pub avg_rule_eval_time_ms: f64,
    /// Active connections
    pub active_connections: usize,
    /// Buffer utilization (0.0-1.0)
    pub buffer_utilization: f64,
    /// Uptime in seconds
    pub uptime_seconds: u64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instruction_discriminator_calculation() {
        // Create a test instruction
        let instruction = IDLInstruction {
            name: "initialize".to_string(),
            args: vec![],
            accounts: vec![],
            returns: None,
            discriminator: vec![],
            docs: vec![],
        };

        let disc = instruction.compute_discriminator();

        // The discriminator should be 8 bytes
        assert_eq!(disc.len(), 8);

        // Verify it's deterministic
        let disc2 = instruction.compute_discriminator();
        assert_eq!(disc, disc2);

        // Known discriminator for "initialize" instruction
        // This is from Anchor's standard: SHA256("global:initialize")[..8]
        // You can verify with: echo -n "global:initialize" | sha256sum
        let expected = [175, 175, 109, 31, 13, 152, 155, 237];
        assert_eq!(disc, expected, "Discriminator mismatch for 'initialize'");
    }

    #[test]
    fn test_different_instructions_different_discriminators() {
        let init = IDLInstruction {
            name: "initialize".to_string(),
            args: vec![],
            accounts: vec![],
            returns: None,
            discriminator: vec![],
            docs: vec![],
        };

        let swap = IDLInstruction {
            name: "swap".to_string(),
            args: vec![],
            accounts: vec![],
            returns: None,
            discriminator: vec![],
            docs: vec![],
        };

        let init_disc = init.compute_discriminator();
        let swap_disc = swap.compute_discriminator();

        // Different instructions should have different discriminators
        assert_ne!(init_disc, swap_disc);
    }

    #[test]
    fn test_protocol_program_pubkey() {
        let protocol = Protocol::new("Test".to_string(), "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4".to_string());

        let pubkey = protocol.program_pubkey();
        assert!(pubkey.is_ok());
    }

    #[test]
    fn test_protocol_invalid_pubkey() {
        let protocol = Protocol::new("Test".to_string(), "invalid_pubkey".to_string());

        let pubkey = protocol.program_pubkey();
        assert!(pubkey.is_err());
    }
}
