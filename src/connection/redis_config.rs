//! Redis Configuration Data Structures
//!
//! Data structures for storing configuration items loaded from Redis.

use std::sync::Arc;

/// Configuration item loaded from Redis
#[derive(Debug, Clone)]
pub struct ConfigItem {
    /// Group ID (sequence number)
    pub group_id: i32,
    /// Service URL (Pulsar URL)
    pub service_url: String,
    /// Configuration source (Redis key path)
    pub source: String,
    /// Topic details list
    pub details: Vec<DetailItem>,
}

/// Topic detail item
#[derive(Debug, Clone)]
pub struct DetailItem {
    /// Index within the config group
    pub index: i32,
    /// Topic path
    pub path: String,
    /// Visibility flag
    pub visibility: bool,
    /// Parent config group ID
    pub group_id: i32,
}

/// Configuration loading state
#[derive(Debug, Clone, Default)]
pub enum ConfigLoadState {
    /// Not loading
    #[default]
    Idle,
    /// Currently loading
    Loading,
    /// Successfully loaded
    Loaded,
    /// Failed to load
    Error(Arc<str>),
}

impl ConfigLoadState {
    /// Check if currently loading
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    /// Check if loaded successfully
    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }

    /// Check if there was an error
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Get error message if any
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Error(msg) => Some(msg),
            _ => None,
        }
    }
}

/// Redis key patterns for configuration lookup
pub const REDIS_KEY_PATTERNS: &[&str] = &[
    "CMC_*_sg.og.output.iothub",
    "CMC_*_sg.input.iothub",
    "CMC_*_sg.io.iothub",
    "CMC_*_sg.bus",
];

// ==================== Redis Key Types ====================

/// Redis key type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedisKeyType {
    String,
    Hash,
    List,
    Set,
    ZSet,
    Stream,
    #[default]
    Unknown,
}

impl RedisKeyType {
    /// Get short display name for the type
    pub fn short_name(&self) -> &'static str {
        match self {
            RedisKeyType::String => "STR",
            RedisKeyType::Hash => "HASH",
            RedisKeyType::List => "LIST",
            RedisKeyType::Set => "SET",
            RedisKeyType::ZSet => "ZSET",
            RedisKeyType::Stream => "STREAM",
            RedisKeyType::Unknown => "?",
        }
    }

    /// Parse from Redis TYPE command response
    pub fn from_type_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "string" => RedisKeyType::String,
            "hash" => RedisKeyType::Hash,
            "list" => RedisKeyType::List,
            "set" => RedisKeyType::Set,
            "zset" => RedisKeyType::ZSet,
            "stream" => RedisKeyType::Stream,
            _ => RedisKeyType::Unknown,
        }
    }
}

/// Redis key item with metadata
#[derive(Debug, Clone)]
pub struct RedisKeyItem {
    /// The key name
    pub key: String,
    /// The key type
    pub key_type: RedisKeyType,
    /// TTL in seconds (-1 means no expiry, -2 means key doesn't exist)
    pub ttl: i64,
}

impl RedisKeyItem {
    /// Create a new key item
    pub fn new(key: String, key_type: RedisKeyType, ttl: i64) -> Self {
        Self { key, key_type, ttl }
    }
}

/// Redis key value representation
#[derive(Debug, Clone)]
pub enum RedisKeyValue {
    /// String value
    String(String),
    /// Hash value (field-value pairs)
    Hash(Vec<(String, String)>),
    /// List value (ordered elements)
    List(Vec<String>),
    /// Set value (unordered unique elements)
    Set(Vec<String>),
    /// Sorted set value (elements with scores)
    ZSet(Vec<(String, f64)>),
    /// Loading state
    Loading,
    /// Error state
    Error(String),
    /// Empty/not loaded
    Empty,
}

impl Default for RedisKeyValue {
    fn default() -> Self {
        Self::Empty
    }
}

/// Connected server information for sidebar display
#[derive(Debug, Clone)]
pub struct ConnectedServerInfo {
    /// Server ID (matches DfcServerConfig.id)
    pub server_id: String,
    /// Server display name
    pub server_name: String,
    /// Selected config source (Redis key)
    pub config_source: Option<String>,
}
