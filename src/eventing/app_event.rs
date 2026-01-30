//! AppEvent - Application Event Enum
//!
//! All events that can be sent from services to the UI layer.

use chrono::{DateTime, Local};

use crate::domain::command::CommandResponse;
use crate::domain::config::AppConfig;
use crate::domain::event_log::EventLog;
use crate::domain::property::Property;
use crate::state::connection_state::ConnectionTarget;
use crate::state::log_state::LogLevel;

/// Application events for service -> UI communication
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Log message
    Log {
        level: LogLevel,
        message: String,
        timestamp: DateTime<Local>,
    },

    /// Connection status changed
    ConnectionChanged {
        target: ConnectionTarget,
        connected: bool,
        detail: Option<String>,
    },

    /// Configuration loaded
    ConfigLoaded {
        config: AppConfig,
    },

    /// Properties data updated
    PropertiesUpdated {
        properties: Vec<Property>,
    },

    /// Events data updated
    EventsUpdated {
        events: Vec<EventLog>,
    },

    /// Command response received
    CommandResponse {
        request_id: String,
        response: CommandResponse,
    },
}

impl AppEvent {
    /// Create a log event with current timestamp
    pub fn log(level: LogLevel, message: impl Into<String>) -> Self {
        Self::Log {
            level,
            message: message.into(),
            timestamp: Local::now(),
        }
    }

    /// Create an info log event
    pub fn info(message: impl Into<String>) -> Self {
        Self::log(LogLevel::Info, message)
    }

    /// Create a warning log event
    pub fn warn(message: impl Into<String>) -> Self {
        Self::log(LogLevel::Warn, message)
    }

    /// Create an error log event
    pub fn error(message: impl Into<String>) -> Self {
        Self::log(LogLevel::Error, message)
    }

    /// Create a debug log event
    pub fn debug(message: impl Into<String>) -> Self {
        Self::log(LogLevel::Debug, message)
    }
}
