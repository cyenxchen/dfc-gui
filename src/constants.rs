//! UI Constants
//!
//! Centralized UI constants for consistent layout across the application.

/// Sidebar navigation width in pixels
pub const SIDEBAR_WIDTH: f32 = 80.0;

/// Content panel minimum width
pub const CONTENT_MIN_WIDTH: f32 = 400.0;

/// Device list panel width constraints
pub const DEVICE_LIST_MIN_WIDTH: f32 = 250.0;
pub const DEVICE_LIST_MAX_WIDTH: f32 = 500.0;
pub const DEVICE_LIST_DEFAULT_WIDTH: f32 = 300.0;

/// Status bar height
pub const STATUS_BAR_HEIGHT: f32 = 28.0;

/// Default window dimensions
pub const DEFAULT_WINDOW_WIDTH: f32 = 1200.0;
pub const DEFAULT_WINDOW_HEIGHT: f32 = 750.0;
pub const MIN_WINDOW_WIDTH: f32 = 800.0;
pub const MIN_WINDOW_HEIGHT: f32 = 500.0;

/// Bounded cache capacities
pub const DEVICE_EVENTS_CAPACITY: usize = 200;
pub const DEVICE_ALARMS_CAPACITY: usize = 200;
pub const TELEMETRY_HISTORY_CAPACITY: usize = 1000;
pub const GLOBAL_LOG_CAPACITY: usize = 5000;

/// Batch processing thresholds
pub const INGEST_BATCH_SIZE: usize = 2048;
pub const INGEST_INTERVAL_MS: u64 = 100;

/// Retry configuration
pub const RETRY_INITIAL_DELAY_MS: u64 = 1000;
pub const RETRY_MAX_DELAY_MS: u64 = 60000;
pub const RETRY_MULTIPLIER: f64 = 2.0;
pub const RETRY_JITTER: f64 = 0.1;

/// Command timeout
pub const COMMAND_TIMEOUT_SECS: u64 = 30;
