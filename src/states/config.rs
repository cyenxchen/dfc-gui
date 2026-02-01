//! Configuration State
//!
//! Manages the state of Redis configuration items and their loading status.

use crate::connection::{ConfigItem, ConfigLoadState, DetailItem};
use gpui::Context;
use std::sync::Arc;

/// Configuration state for managing Redis config items
pub struct ConfigState {
    /// List of configuration items
    configs: Vec<ConfigItem>,
    /// Current loading state
    load_state: ConfigLoadState,
    /// Currently selected config group ID
    selected_config_id: Option<i32>,
    /// Currently selected topic index within the config
    selected_topic_index: Option<i32>,
    /// ID of the connected server
    connected_server_id: Option<String>,
}

impl ConfigState {
    /// Create a new empty config state
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
            load_state: ConfigLoadState::Idle,
            selected_config_id: None,
            selected_topic_index: None,
            connected_server_id: None,
        }
    }

    // ==================== Getters ====================

    /// Get all configuration items
    pub fn configs(&self) -> &[ConfigItem] {
        &self.configs
    }

    /// Get the current loading state
    pub fn load_state(&self) -> &ConfigLoadState {
        &self.load_state
    }

    /// Check if currently loading
    pub fn is_loading(&self) -> bool {
        self.load_state.is_loading()
    }

    /// Get the selected config group ID
    pub fn selected_config_id(&self) -> Option<i32> {
        self.selected_config_id
    }

    /// Get the selected topic index
    pub fn selected_topic_index(&self) -> Option<i32> {
        self.selected_topic_index
    }

    /// Get the connected server ID
    pub fn connected_server_id(&self) -> Option<&str> {
        self.connected_server_id.as_deref()
    }

    /// Get the currently selected config item
    pub fn selected_config(&self) -> Option<&ConfigItem> {
        self.selected_config_id
            .and_then(|id| self.configs.iter().find(|c| c.group_id == id))
    }

    /// Get the currently selected topic detail
    pub fn selected_topic(&self) -> Option<&DetailItem> {
        self.selected_config().and_then(|config| {
            self.selected_topic_index
                .and_then(|idx| config.details.iter().find(|d| d.index == idx))
        })
    }

    // ==================== Setters ====================

    /// Set configuration items
    pub fn set_configs(&mut self, configs: Vec<ConfigItem>, cx: &mut Context<Self>) {
        self.configs = configs;
        self.load_state = ConfigLoadState::Loaded;
        cx.notify();
    }

    /// Set loading state
    pub fn set_loading(&mut self, cx: &mut Context<Self>) {
        self.load_state = ConfigLoadState::Loading;
        cx.notify();
    }

    /// Set error state
    pub fn set_error(&mut self, message: impl Into<Arc<str>>, cx: &mut Context<Self>) {
        self.load_state = ConfigLoadState::Error(message.into());
        cx.notify();
    }

    /// Select a configuration by group ID
    pub fn select_config(&mut self, group_id: Option<i32>, cx: &mut Context<Self>) {
        self.selected_config_id = group_id;
        // Reset topic selection when changing config
        if group_id.is_some() {
            self.selected_topic_index = Some(0);
        } else {
            self.selected_topic_index = None;
        }
        cx.notify();
    }

    /// Select a topic by index
    pub fn select_topic(&mut self, index: Option<i32>, cx: &mut Context<Self>) {
        self.selected_topic_index = index;
        cx.notify();
    }

    /// Set the connected server ID
    pub fn set_connected_server(&mut self, server_id: Option<String>, cx: &mut Context<Self>) {
        self.connected_server_id = server_id;
        cx.notify();
    }

    /// Clear all state and reset to initial
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.configs.clear();
        self.load_state = ConfigLoadState::Idle;
        self.selected_config_id = None;
        self.selected_topic_index = None;
        self.connected_server_id = None;
        cx.notify();
    }

    /// Go back to config list (deselect config)
    pub fn back_to_list(&mut self, cx: &mut Context<Self>) {
        self.selected_config_id = None;
        self.selected_topic_index = None;
        cx.notify();
    }
}

impl Default for ConfigState {
    fn default() -> Self {
        Self::new()
    }
}
