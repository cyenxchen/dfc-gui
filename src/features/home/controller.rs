//! Home Controller
//!
//! Handles configuration loading, saving, and service management.

use gpui::{App, Context, Entity};

use crate::app::entities::AppEntities;
use crate::domain::config::AppConfig;
use crate::eventing::app_event::AppEvent;
use crate::services::service_hub::ServiceHub;

/// Home page controller
pub struct HomeController {
    entities: AppEntities,
}

impl HomeController {
    /// Create a new controller
    pub fn new(entities: AppEntities) -> Self {
        Self { entities }
    }

    /// Start services with the current configuration
    pub fn start_services(&self, config: AppConfig, cx: &mut App) {
        if let Some(hub) = cx.try_global::<ServiceHub>() {
            hub.start(config);
        }
    }

    /// Stop services
    pub fn stop_services(&self, cx: &mut App) {
        if let Some(hub) = cx.try_global::<ServiceHub>() {
            hub.stop();
        }
    }

    /// Update configuration
    pub fn update_config(&self, config: AppConfig, cx: &mut App) {
        // Update config state
        self.entities.config.update(cx, |state, cx| {
            state.update_config(config.clone());
            cx.notify();
        });

        // Send to service hub
        if let Some(hub) = cx.try_global::<ServiceHub>() {
            hub.update_config(config);
        }
    }

    /// Load saved configuration
    pub fn load_config(&self, cx: &mut App) {
        // Try to load from local storage
        match crate::utils::config_store::load_config::<AppConfig>("config.json") {
            Ok(config) => {
                self.entities.config.update(cx, |state, cx| {
                    state.update_config(config);
                    cx.notify();
                });

                if let Some(hub) = cx.try_global::<ServiceHub>() {
                    hub.log(AppEvent::info("Configuration loaded from local storage"));
                }
            }
            Err(e) => {
                if let Some(hub) = cx.try_global::<ServiceHub>() {
                    hub.log(AppEvent::warn(format!("Failed to load config: {}", e)));
                }
            }
        }
    }

    /// Save configuration
    pub fn save_config(&self, config: &AppConfig, cx: &mut App) {
        match crate::utils::config_store::save_config("config.json", config) {
            Ok(()) => {
                if let Some(hub) = cx.try_global::<ServiceHub>() {
                    hub.log(AppEvent::info("Configuration saved"));
                }
            }
            Err(e) => {
                if let Some(hub) = cx.try_global::<ServiceHub>() {
                    hub.log(AppEvent::error(format!("Failed to save config: {}", e)));
                }
            }
        }
    }
}
