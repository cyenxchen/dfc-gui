//! Keyboard Actions and Shortcuts
//!
//! Defines global keyboard shortcuts and action dispatching.

use gpui::{Action, KeyBinding};
use schemars::JsonSchema;
use serde::Deserialize;

/// Menu actions (application-level)
#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
pub enum MenuAction {
    /// Quit the application
    Quit,
    /// Show about dialog
    About,
}

/// Navigation actions
#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
pub enum NavAction {
    /// Go to home/devices view
    Home,
    /// Go to properties view
    Properties,
    /// Go to events view
    Events,
    /// Go to commands view
    Commands,
    /// Go to settings view
    Settings,
}

/// Device actions
#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
pub enum DeviceAction {
    /// Refresh device list
    Refresh,
    /// Filter/search devices
    Filter,
    /// Select next device
    Next,
    /// Select previous device
    Previous,
}

/// Command actions
#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
pub enum CommandAction {
    /// Send command
    Send,
    /// Cancel pending command
    Cancel,
}

/// Convert a keystroke string to human-readable format
///
/// Platform-specific formatting:
/// - macOS: ⌘ for cmd, ⌥ for alt, ⌃ for ctrl, ⇧ for shift
/// - Others: Ctrl+, Alt+, Shift+
pub fn humanize_keystroke(keystroke: &str) -> String {
    let parts = keystroke.split('-');
    let mut display_text = String::new();

    #[cfg(target_os = "macos")]
    let separator = "";
    #[cfg(not(target_os = "macos"))]
    let separator = "+";

    for (i, part) in parts.enumerate() {
        if i > 0 {
            display_text.push_str(separator);
        }

        let symbol = match part {
            "secondary" | "cmd" => {
                #[cfg(target_os = "macos")]
                { "⌘" }
                #[cfg(not(target_os = "macos"))]
                { "Ctrl" }
            }
            "ctrl" => {
                #[cfg(target_os = "macos")]
                { "⌃" }
                #[cfg(not(target_os = "macos"))]
                { "Ctrl" }
            }
            "alt" => {
                #[cfg(target_os = "macos")]
                { "⌥" }
                #[cfg(not(target_os = "macos"))]
                { "Alt" }
            }
            "shift" => {
                #[cfg(target_os = "macos")]
                { "⇧" }
                #[cfg(not(target_os = "macos"))]
                { "Shift" }
            }
            "enter" => "Enter",
            "space" => "Space",
            "backspace" => {
                #[cfg(target_os = "macos")]
                { "⌫" }
                #[cfg(not(target_os = "macos"))]
                { "Backspace" }
            }
            "escape" => "Esc",
            c => {
                display_text.push_str(&c.to_uppercase());
                continue;
            }
        };
        display_text.push_str(symbol);
    }

    display_text
}

/// Create global keyboard bindings
pub fn new_key_bindings() -> Vec<KeyBinding> {
    vec![
        // Application
        KeyBinding::new("secondary-q", MenuAction::Quit, None),
        // Navigation
        KeyBinding::new("secondary-1", NavAction::Home, None),
        KeyBinding::new("secondary-2", NavAction::Properties, None),
        KeyBinding::new("secondary-3", NavAction::Events, None),
        KeyBinding::new("secondary-4", NavAction::Commands, None),
        KeyBinding::new("secondary-,", NavAction::Settings, None),
        // Device operations
        KeyBinding::new("secondary-r", DeviceAction::Refresh, None),
        KeyBinding::new("secondary-f", DeviceAction::Filter, None),
        KeyBinding::new("down", DeviceAction::Next, None),
        KeyBinding::new("up", DeviceAction::Previous, None),
        // Commands
        KeyBinding::new("secondary-enter", CommandAction::Send, None),
        KeyBinding::new("escape", CommandAction::Cancel, None),
    ]
}
