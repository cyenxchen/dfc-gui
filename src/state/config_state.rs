//! ConfigState - Application Configuration State

use crate::domain::config::AppConfig;

/// State for application configuration
#[derive(Debug, Clone, Default)]
pub struct ConfigState {
    /// Current configuration
    pub config: AppConfig,
    /// Whether config has been loaded
    pub loaded: bool,
    /// Whether config is being saved
    pub saving: bool,
}

impl ConfigState {
    /// Update configuration
    pub fn update_config(&mut self, config: AppConfig) {
        self.config = config;
        self.loaded = true;
    }

    /// Set saving state
    pub fn set_saving(&mut self, saving: bool) {
        self.saving = saving;
    }
}
