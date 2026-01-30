//! Command - Device Command Request/Response

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A command request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    /// Unique request ID
    pub request_id: String,
    /// Device ID
    pub device_id: String,
    /// Service name
    pub service: String,
    /// Method name
    pub method: String,
    /// Parameters (JSON string)
    pub params: String,
    /// Timeout in seconds
    pub timeout: u32,
    /// Request timestamp
    pub created_time: DateTime<Utc>,
}

impl Default for CommandRequest {
    fn default() -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            device_id: String::new(),
            service: String::new(),
            method: String::new(),
            params: "{}".to_string(),
            timeout: 30,
            created_time: Utc::now(),
        }
    }
}

/// A command response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    /// Request ID this is responding to
    pub request_id: String,
    /// Response code (0 = success)
    pub code: i32,
    /// Response message
    pub message: String,
    /// Response data (JSON string)
    pub data: Option<String>,
    /// Response timestamp
    pub response_time: DateTime<Utc>,
}

impl Default for CommandResponse {
    fn default() -> Self {
        Self {
            request_id: String::new(),
            code: 0,
            message: String::new(),
            data: None,
            response_time: Utc::now(),
        }
    }
}
