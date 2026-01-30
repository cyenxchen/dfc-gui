//! LogState - Log Messages with Ring Buffer

use chrono::{DateTime, Local};
use std::collections::VecDeque;

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

impl LogLevel {
    pub fn label(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Debug => "DEBUG",
        }
    }

    pub fn color(&self) -> gpui::Rgba {
        match self {
            LogLevel::Info => gpui::rgba(0x22c55eff), // Green
            LogLevel::Warn => gpui::rgba(0xf59e0bff), // Yellow/Amber
            LogLevel::Error => gpui::rgba(0xef4444ff), // Red
            LogLevel::Debug => gpui::rgba(0x6b7280ff), // Gray
        }
    }
}

/// A single log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub id: u64,
    pub level: LogLevel,
    pub message: String,
    pub timestamp: DateTime<Local>,
}

/// State for log messages using a ring buffer
#[derive(Debug)]
pub struct LogState {
    entries: VecDeque<LogEntry>,
    capacity: usize,
    next_id: u64,
    /// Whether auto-scroll is enabled
    pub auto_scroll: bool,
}

impl LogState {
    /// Create a new log state with the specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
            next_id: 1,
            auto_scroll: true,
        }
    }

    /// Push a new log entry
    pub fn push(&mut self, level: LogLevel, message: impl Into<String>, timestamp: DateTime<Local>) {
        let entry = LogEntry {
            id: self.next_id,
            level,
            message: message.into(),
            timestamp,
        };
        self.next_id += 1;

        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Push a log entry with current timestamp
    pub fn push_now(&mut self, level: LogLevel, message: impl Into<String>) {
        self.push(level, message, Local::now());
    }

    /// Get all log entries
    pub fn entries(&self) -> &VecDeque<LogEntry> {
        &self.entries
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Toggle auto-scroll
    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
    }
}

impl Default for LogState {
    fn default() -> Self {
        Self::new(1000)
    }
}
