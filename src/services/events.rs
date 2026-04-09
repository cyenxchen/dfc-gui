//! Service Events
//!
//! Domain events emitted by the service layer to be consumed by the state layer.
//! These events represent changes in device state, telemetry data, alarms, etc.

use std::sync::Arc;

/// Unique identifier for a device
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct DeviceId(pub Arc<str>);

impl DeviceId {
    /// Create a new DeviceId from a string
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }

    /// Get the underlying string reference
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DeviceId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for DeviceId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Device metadata information
#[derive(Clone, Debug)]
pub struct DeviceMeta {
    /// Unique device identifier
    pub id: DeviceId,
    /// Human-readable device name
    pub name: Arc<str>,
    /// Site/location of the device
    pub site: Option<Arc<str>>,
    /// Device model/type
    pub model: Option<Arc<str>>,
    /// Device firmware version
    pub firmware: Option<Arc<str>>,
    /// Additional tags/labels
    pub tags: Vec<Arc<str>>,
}

impl DeviceMeta {
    /// Create a new DeviceMeta with minimal info
    pub fn new(id: impl Into<Arc<str>>, name: impl Into<Arc<str>>) -> Self {
        Self {
            id: DeviceId::new(id),
            name: name.into(),
            site: None,
            model: None,
            firmware: None,
            tags: Vec::new(),
        }
    }
}

/// A single telemetry data point
#[derive(Clone, Debug)]
pub struct TelemetryPoint {
    /// Metric identifier (maps to a name via dictionary from Redis)
    pub key: u16,
    /// Metric value
    pub value: f64,
}

/// Alarm severity levels
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlarmSeverity {
    /// Informational message
    Info = 0,
    /// Warning condition
    Warning = 1,
    /// Error condition
    Error = 2,
    /// Critical condition requiring immediate attention
    Critical = 3,
}

impl From<u8> for AlarmSeverity {
    fn from(value: u8) -> Self {
        match value {
            0 => AlarmSeverity::Info,
            1 => AlarmSeverity::Warning,
            2 => AlarmSeverity::Error,
            _ => AlarmSeverity::Critical,
        }
    }
}

/// Command execution status
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandStatus {
    /// Command is pending execution
    Pending,
    /// Command executed successfully
    Success,
    /// Command failed
    Failed,
    /// Command timed out
    Timeout,
}

/// Events emitted by the service layer
#[derive(Clone, Debug)]
pub enum ServiceEvent {
    // ==================== Device Metadata ====================
    /// Device metadata was created or updated
    DeviceMetaUpsert(DeviceMeta),

    /// Device was removed from the fleet
    DeviceRemoved(DeviceId),

    // ==================== Telemetry ====================
    /// Telemetry data received from a device
    Telemetry {
        /// Source device
        device: DeviceId,
        /// Timestamp in milliseconds since epoch
        ts_ms: i64,
        /// Telemetry data points
        points: Vec<TelemetryPoint>,
    },

    // ==================== Alarms ====================
    /// Alarm raised by a device
    Alarm {
        /// Source device
        device: DeviceId,
        /// Timestamp in milliseconds since epoch
        ts_ms: i64,
        /// Alarm code
        code: u32,
        /// Alarm message
        message: Arc<str>,
        /// Alarm severity
        severity: AlarmSeverity,
    },

    // ==================== Device State ====================
    /// Device online status changed
    DeviceOnlineChanged {
        /// Device identifier
        device: DeviceId,
        /// New online status
        online: bool,
        /// Timestamp in milliseconds since epoch
        ts_ms: i64,
    },

    // ==================== Commands ====================
    /// Command acknowledgment received
    CommandAck {
        /// Correlation ID to match request
        correlation_id: Arc<str>,
        /// Whether command succeeded
        success: bool,
        /// Optional response payload
        payload: Option<Arc<str>>,
        /// Error message if failed
        error: Option<Arc<str>>,
    },

    // ==================== Connection State ====================
    /// Service connection state changed
    ConnectionState {
        /// Service name (e.g., "redis", "pulsar")
        service: Arc<str>,
        /// Whether connected
        connected: bool,
        /// Additional detail (e.g., "Reconnecting in 8s (attempt 4/10)")
        detail: Arc<str>,
    },

    // ==================== Dictionary ====================
    /// Metric dictionary updated (key -> name mapping)
    MetricDictionary {
        /// Metric ID to name mapping
        entries: Vec<(u16, Arc<str>)>,
    },
}
