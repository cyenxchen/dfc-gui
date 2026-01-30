//! EventsState - Device Events State

use crate::domain::event_log::EventLog;

/// State for device events
#[derive(Debug, Clone, Default)]
pub struct EventsState {
    /// All events
    pub events: Vec<EventLog>,
    /// Filter text
    pub filter: String,
    /// Total count
    pub total_count: usize,
    /// Whether data is loading
    pub loading: bool,
}

impl EventsState {
    /// Update events data
    pub fn update_events(&mut self, events: Vec<EventLog>) {
        self.total_count = events.len();
        self.events = events;
        self.loading = false;
    }

    /// Set filter text
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }

    /// Get filtered events
    pub fn filtered_events(&self) -> Vec<&EventLog> {
        if self.filter.is_empty() {
            self.events.iter().collect()
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.events
                .iter()
                .filter(|e| {
                    e.event_code.to_lowercase().contains(&filter_lower)
                        || e.device_id.to_lowercase().contains(&filter_lower)
                })
                .collect()
        }
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }
}
