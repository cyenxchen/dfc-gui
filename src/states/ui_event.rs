//! UI Events
//!
//! Events emitted from state layer to UI layer for notifications,
//! toasts, dialogs, and other user-facing feedback.

use crate::services::{AlarmSeverity, DeviceId};
use std::sync::Arc;

/// UI events for user feedback
#[derive(Clone, Debug)]
pub enum UIEvent {
    /// Display a toast notification
    Toast {
        /// Message to display
        message: Arc<str>,
        /// Whether this is an error (affects styling)
        is_error: bool,
    },

    /// Display a confirmation dialog
    ConfirmDialog {
        /// Dialog title
        title: Arc<str>,
        /// Dialog message
        message: Arc<str>,
        /// Action to take on confirm (stored as string for identification)
        action_id: Arc<str>,
    },

    /// Connection state changed
    ConnectionStateChanged {
        /// Service name
        service: Arc<str>,
        /// Whether connected
        connected: bool,
        /// Detail message
        detail: Arc<str>,
    },

    /// Alarm received (for notification)
    AlarmReceived {
        /// Source device
        device: DeviceId,
        /// Alarm code
        code: u32,
        /// Alarm severity
        severity: AlarmSeverity,
    },

    /// Device selection changed
    DeviceSelected {
        /// Selected device ID (None if deselected)
        device: Option<DeviceId>,
    },

    /// Loading state changed
    LoadingChanged {
        /// Whether loading
        loading: bool,
        /// Optional loading message
        message: Option<Arc<str>>,
    },

    /// Error occurred (for logging/display)
    ErrorOccurred {
        /// Error source/task name
        source: Arc<str>,
        /// Error message
        message: Arc<str>,
    },
}

/// Severity level for UI notifications
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationSeverity {
    /// Informational message (auto-dismiss)
    Info,
    /// Success message (auto-dismiss)
    Success,
    /// Warning message (persist until dismissed)
    Warning,
    /// Error message (persist until dismissed)
    Error,
}

impl From<AlarmSeverity> for NotificationSeverity {
    fn from(severity: AlarmSeverity) -> Self {
        match severity {
            AlarmSeverity::Info => NotificationSeverity::Info,
            AlarmSeverity::Warning => NotificationSeverity::Warning,
            AlarmSeverity::Error => NotificationSeverity::Error,
            AlarmSeverity::Critical => NotificationSeverity::Error,
        }
    }
}
