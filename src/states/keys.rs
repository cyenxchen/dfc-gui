//! Keys State
//!
//! Manages the state of Redis keys browsing, including the key list,
//! selected key/value, connected servers, and filter patterns.

use crate::connection::{ConnectedServerInfo, RedisKeyItem, RedisKeyValue};
use gpui::Context;
use std::sync::Arc;

/// Keys loading state
#[derive(Debug, Clone, Default)]
pub enum KeysLoadState {
    /// Not loading
    #[default]
    Idle,
    /// Currently loading keys
    Loading,
    /// Keys loaded successfully
    Loaded,
    /// Failed to load keys
    Error(Arc<str>),
}

impl KeysLoadState {
    /// Check if currently loading
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    /// Check if loaded successfully
    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }
}

/// Keys state for managing Redis keys browsing
pub struct KeysState {
    /// List of Redis keys for the current server
    keys: Vec<RedisKeyItem>,
    /// Current loading state
    load_state: KeysLoadState,
    /// Currently selected key
    selected_key: Option<String>,
    /// Value of the selected key
    selected_value: RedisKeyValue,
    /// Search/filter pattern
    filter_pattern: String,
    /// List of connected servers (supports multiple)
    connected_servers: Vec<ConnectedServerInfo>,
    /// Currently active server ID
    active_server_id: Option<String>,
    /// SCAN cursor for pagination (0 means done)
    scan_cursor: u64,
    /// Whether more keys are available to load
    has_more_keys: bool,
}

impl KeysState {
    /// Create a new empty keys state
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            load_state: KeysLoadState::Idle,
            selected_key: None,
            selected_value: RedisKeyValue::Empty,
            filter_pattern: String::new(),
            connected_servers: Vec::new(),
            active_server_id: None,
            scan_cursor: 0,
            has_more_keys: false,
        }
    }

    // ==================== Getters ====================

    /// Get all keys
    pub fn keys(&self) -> &[RedisKeyItem] {
        &self.keys
    }

    /// Get keys filtered by the current filter pattern
    pub fn filtered_keys(&self) -> Vec<&RedisKeyItem> {
        if self.filter_pattern.is_empty() {
            self.keys.iter().collect()
        } else {
            let pattern = self.filter_pattern.to_lowercase();
            self.keys
                .iter()
                .filter(|k| k.key.to_lowercase().contains(&pattern))
                .collect()
        }
    }

    /// Get the current loading state
    pub fn load_state(&self) -> &KeysLoadState {
        &self.load_state
    }

    /// Check if currently loading
    pub fn is_loading(&self) -> bool {
        self.load_state.is_loading()
    }

    /// Get the selected key
    pub fn selected_key(&self) -> Option<&str> {
        self.selected_key.as_deref()
    }

    /// Get the selected key's value
    pub fn selected_value(&self) -> &RedisKeyValue {
        &self.selected_value
    }

    /// Get the filter pattern
    pub fn filter_pattern(&self) -> &str {
        &self.filter_pattern
    }

    /// Get connected servers list
    pub fn connected_servers(&self) -> &[ConnectedServerInfo] {
        &self.connected_servers
    }

    /// Get the active server ID
    pub fn active_server_id(&self) -> Option<&str> {
        self.active_server_id.as_deref()
    }

    /// Get the active connected server info
    pub fn active_server(&self) -> Option<&ConnectedServerInfo> {
        self.active_server_id.as_ref().and_then(|id| {
            self.connected_servers.iter().find(|s| &s.server_id == id)
        })
    }

    /// Check if there are more keys to load
    pub fn has_more_keys(&self) -> bool {
        self.has_more_keys
    }

    /// Get the current SCAN cursor
    pub fn scan_cursor(&self) -> u64 {
        self.scan_cursor
    }

    // ==================== Setters ====================

    /// Set keys list
    pub fn set_keys(&mut self, keys: Vec<RedisKeyItem>, cursor: u64, cx: &mut Context<Self>) {
        self.keys = keys;
        self.scan_cursor = cursor;
        self.has_more_keys = cursor != 0;
        self.load_state = KeysLoadState::Loaded;
        cx.notify();
    }

    /// Append more keys (for pagination)
    pub fn append_keys(&mut self, keys: Vec<RedisKeyItem>, cursor: u64, cx: &mut Context<Self>) {
        self.keys.extend(keys);
        self.scan_cursor = cursor;
        self.has_more_keys = cursor != 0;
        self.load_state = KeysLoadState::Loaded;
        cx.notify();
    }

    /// Set loading state
    pub fn set_loading(&mut self, cx: &mut Context<Self>) {
        self.load_state = KeysLoadState::Loading;
        cx.notify();
    }

    /// Set error state
    pub fn set_error(&mut self, message: impl Into<Arc<str>>, cx: &mut Context<Self>) {
        self.load_state = KeysLoadState::Error(message.into());
        cx.notify();
    }

    /// Select a key
    pub fn select_key(&mut self, key: Option<String>, cx: &mut Context<Self>) {
        self.selected_key = key;
        // Reset value when changing selection
        self.selected_value = RedisKeyValue::Loading;
        cx.notify();
    }

    /// Set the selected key's value
    pub fn set_selected_value(&mut self, value: RedisKeyValue, cx: &mut Context<Self>) {
        self.selected_value = value;
        cx.notify();
    }

    /// Set the filter pattern
    pub fn set_filter_pattern(&mut self, pattern: String, cx: &mut Context<Self>) {
        self.filter_pattern = pattern;
        cx.notify();
    }

    /// Add a connected server
    pub fn add_connected_server(&mut self, server: ConnectedServerInfo, cx: &mut Context<Self>) {
        // Don't add duplicates
        if !self.connected_servers.iter().any(|s| s.server_id == server.server_id) {
            let server_id = server.server_id.clone();
            self.connected_servers.push(server);
            // Auto-activate the newly connected server
            self.active_server_id = Some(server_id);
            cx.notify();
        }
    }

    /// Remove a connected server
    pub fn remove_connected_server(&mut self, server_id: &str, cx: &mut Context<Self>) {
        self.connected_servers.retain(|s| s.server_id != server_id);

        // Clear active if it was the removed one
        if self.active_server_id.as_deref() == Some(server_id) {
            self.active_server_id = self.connected_servers.first().map(|s| s.server_id.clone());
            // Clear keys when switching servers
            self.clear_keys(cx);
        }

        cx.notify();
    }

    /// Set the active server
    pub fn set_active_server(&mut self, server_id: Option<String>, cx: &mut Context<Self>) {
        if self.active_server_id != server_id {
            self.active_server_id = server_id;
            // Clear keys when switching servers
            self.clear_keys(cx);
            cx.notify();
        }
    }

    /// Clear keys (but keep connected servers)
    pub fn clear_keys(&mut self, cx: &mut Context<Self>) {
        self.keys.clear();
        self.selected_key = None;
        self.selected_value = RedisKeyValue::Empty;
        self.filter_pattern.clear();
        self.load_state = KeysLoadState::Idle;
        self.scan_cursor = 0;
        self.has_more_keys = false;
        cx.notify();
    }

    /// Clear all state
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.keys.clear();
        self.load_state = KeysLoadState::Idle;
        self.selected_key = None;
        self.selected_value = RedisKeyValue::Empty;
        self.filter_pattern.clear();
        self.connected_servers.clear();
        self.active_server_id = None;
        self.scan_cursor = 0;
        self.has_more_keys = false;
        cx.notify();
    }
}

impl Default for KeysState {
    fn default() -> Self {
        Self::new()
    }
}
