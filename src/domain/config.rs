//! Config - Application Configuration

use serde::{Deserialize, Serialize};

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// Device configuration
    pub device: DeviceConfig,
    /// Redis configuration
    pub redis: RedisConfig,
    /// Pulsar configuration
    pub pulsar: PulsarConfig,
    /// Filter options
    pub filter: FilterConfig,
}

/// Device configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceConfig {
    /// Device ID (e.g., "DOC00006")
    pub device_id: String,
    /// CFG ID
    pub cfgid: String,
    /// Whether running
    pub running: bool,
    /// Start time for data query
    pub start_time: Option<String>,
    /// End time for data query
    pub end_time: Option<String>,
}

/// Redis connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis host IP
    pub ip: String,
    /// Redis port
    pub port: u16,
    /// Redis password (optional)
    pub password: Option<String>,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            ip: "10.15.204.120".to_string(),
            port: 10060,
            password: None,
        }
    }
}

/// Pulsar connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulsarConfig {
    /// Pulsar broker IP
    pub ip: String,
    /// Pulsar broker port (native protocol)
    pub port: u16,
    /// Redis for Pulsar (for token/config lookup)
    pub redis_ip: String,
    /// Redis port for Pulsar
    pub redis_port: u16,
    /// Pulsar tenant
    pub tenant: String,
    /// Pulsar namespace
    pub namespace: String,
    /// Pulsar token
    pub token: Option<String>,
}

impl Default for PulsarConfig {
    fn default() -> Self {
        Self {
            ip: "10.15.84.63".to_string(),
            port: 6678,
            redis_ip: "10.15.84.63".to_string(),
            redis_port: 6603,
            tenant: "iothub-simulation".to_string(),
            namespace: "iothub-simulation".to_string(),
            token: None,
        }
    }
}

/// Filter configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilterConfig {
    /// Limit to specific device
    pub limit_device: bool,
    /// Device ID for filtering
    pub device_filter: String,
    /// Limit to specific cfgid
    pub limit_cfgid: bool,
    /// CFG ID for filtering
    pub cfgid_filter: String,
    /// Use specified Pulsar token
    pub use_token: bool,
    /// Pulsar token for filtering
    pub token_filter: String,
    /// Use specified time range
    pub use_time_range: bool,
    /// Data time range start
    pub time_range_start: Option<String>,
    /// Data time range end
    pub time_range_end: Option<String>,
}
