//! PropertiesState - Device Properties State

use crate::domain::property::Property;

/// State for device properties
#[derive(Debug, Clone, Default)]
pub struct PropertiesState {
    /// All properties
    pub properties: Vec<Property>,
    /// Filter text
    pub filter: String,
    /// Total count (may differ from properties.len() if filtered)
    pub total_count: usize,
    /// Whether data is loading
    pub loading: bool,
}

impl PropertiesState {
    /// Update properties data
    pub fn update_properties(&mut self, properties: Vec<Property>) {
        self.total_count = properties.len();
        self.properties = properties;
        self.loading = false;
    }

    /// Set filter text
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }

    /// Get filtered properties
    pub fn filtered_properties(&self) -> Vec<&Property> {
        if self.filter.is_empty() {
            self.properties.iter().collect()
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.properties
                .iter()
                .filter(|p| {
                    p.name.to_lowercase().contains(&filter_lower)
                        || p.topic.to_lowercase().contains(&filter_lower)
                })
                .collect()
        }
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }
}
