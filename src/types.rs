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

/// IDL Schema (Anchor format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLSchema {
    /// IDL version
    pub version: String,
    /// Program name
    pub name: String,
    /// Instructions defined in IDL
    pub instructions: Vec<IDLInstruction>,
    /// Account types
    pub accounts: Vec<IDLAccount>,
    /// Custom types
    pub types: Vec<IDLType>,
    /// Error codes
    pub errors: Option<Vec<IDLError>>,
}

/// IDL Instruction definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLInstruction {
    /// Instruction name
    pub name: String,
    /// Instruction arguments
    pub args: Vec<IDLField>,
    /// Accounts required
    pub accounts: Vec<IDLAccountMeta>,
}

/// IDL Account definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLAccount {
    /// Account name
    pub name: String,
    /// Account type
    #[serde(rename = "type")]
    pub type_def: IDLType,
}

/// IDL Account metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLAccountMeta {
    /// Account name
    pub name: String,
    /// Whether account is mutable
    #[serde(rename = "isMut")]
    pub is_mut: bool,
    /// Whether account is signer
    #[serde(rename = "isSigner")]
    pub is_signer: bool,
}

/// IDL Field (argument or struct field)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLField {
    /// Field name
    pub name: String,
    /// Field type
    #[serde(rename = "type")]
    pub type_name: IDLType,
}

/// IDL Type definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IDLType {
    /// Primitive type (string)
    Primitive(String),
    /// Struct type
    Struct { kind: String, fields: Vec<IDLField> },
    /// Enum type
    Enum { kind: String, variants: Vec<IDLField> },
}

/// IDL Error definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDLError {
    /// Error code
    pub code: u32,
    /// Error name
    pub name: String,
    /// Error message
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
