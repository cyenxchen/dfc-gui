//! ConnectionState - Connection Status for Services

use std::collections::HashMap;

/// Connection targets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionTarget {
    Redis,
    Pulsar,
    Database,
}

impl ConnectionTarget {
    pub fn label(&self) -> &'static str {
        match self {
            ConnectionTarget::Redis => "Redis",
            ConnectionTarget::Pulsar => "Pulsar",
            ConnectionTarget::Database => "SQLite",
        }
    }
}

/// Status of a single connection
#[derive(Debug, Clone, Default)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub detail: Option<String>,
}

/// State for all service connections
#[derive(Debug, Clone, Default)]
pub struct ConnectionState {
    statuses: HashMap<ConnectionTarget, ConnectionStatus>,
}

impl ConnectionState {
    /// Set status for a connection target
    pub fn set_status(&mut self, target: ConnectionTarget, connected: bool, detail: Option<String>) {
        self.statuses.insert(
            target,
            ConnectionStatus { connected, detail },
        );
    }

    /// Get status for a connection target
    pub fn get_status(&self, target: ConnectionTarget) -> Option<&ConnectionStatus> {
        self.statuses.get(&target)
    }

    /// Check if a target is connected
    pub fn is_connected(&self, target: ConnectionTarget) -> bool {
        self.statuses
            .get(&target)
            .map(|s| s.connected)
            .unwrap_or(false)
    }

    /// Check if all targets are connected
    pub fn all_connected(&self) -> bool {
        [ConnectionTarget::Redis, ConnectionTarget::Pulsar, ConnectionTarget::Database]
            .iter()
            .all(|t| self.is_connected(*t))
    }

    /// Get all statuses
    pub fn all_statuses(&self) -> &HashMap<ConnectionTarget, ConnectionStatus> {
        &self.statuses
    }
}
