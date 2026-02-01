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
