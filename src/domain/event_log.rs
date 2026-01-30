//! EventLog - Device Event Data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A device event log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLog {
    /// Unique ID
    pub id: String,
    /// Device ID
    pub device_id: String,
    /// Event code
    pub event_code: String,
    /// Event description
    pub description: String,
    /// Event level (0=Info, 1=Warning, 2=Error, 3=Critical)
    pub level: i32,
    /// Event state (0=Active, 1=Cleared)
    pub state: i32,
    /// Event timestamp
    pub event_time: DateTime<Utc>,
    /// Created timestamp
    pub created_time: DateTime<Utc>,
    /// Source system
    pub source: String,
}

impl Default for EventLog {
    fn default() -> Self {
        Self {
            id: String::new(),
            device_id: String::new(),
            event_code: String::new(),
            description: String::new(),
            level: 0,
            state: 0,
            event_time: Utc::now(),
            created_time: Utc::now(),
            source: String::new(),
        }
    }
}

impl EventLog {
    pub fn level_label(&self) -> &'static str {
        match self.level {
            0 => "Info",
            1 => "Warning",
            2 => "Error",
            3 => "Critical",
            _ => "Unknown",
        }
    }

    pub fn state_label(&self) -> &'static str {
        match self.state {
            0 => "Active",
            1 => "Cleared",
            _ => "Unknown",
        }
    }
}
