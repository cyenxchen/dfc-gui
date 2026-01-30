//! Repository
//!
//! Data access methods for properties and events.
//! Note: sqlez has a different API than rusqlite.
//! For now, we'll simplify the implementation.

use anyhow::Result;

use crate::domain::event_log::EventLog;
use crate::domain::property::Property;

use super::connection::DatabaseConnection;

/// Repository for data access
pub struct Repository {
    _db: DatabaseConnection,
}

impl Repository {
    /// Create a new repository
    pub fn new(db: DatabaseConnection) -> Self {
        Self { _db: db }
    }

    /// Get all properties (simplified - returns empty for now)
    pub async fn get_properties(&self, _device_id: Option<&str>) -> Result<Vec<Property>> {
        // TODO: Implement with sqlez when schema is ready
        Ok(Vec::new())
    }

    /// Get all events (simplified - returns empty for now)
    pub async fn get_events(&self, _device_id: Option<&str>) -> Result<Vec<EventLog>> {
        // TODO: Implement with sqlez when schema is ready
        Ok(Vec::new())
    }

    /// Clear all data
    pub async fn clear_all(&self) -> Result<()> {
        // TODO: Implement with sqlez
        Ok(())
    }
}
