//! Internationalization Helpers
//!
//! Provides convenient functions for translating strings based on current locale.

use super::DfcGlobalStore;
use gpui::{App, SharedString};
use rust_i18n::t;

/// Get translated string from "common" namespace
pub fn i18n_common(cx: &App, key: &str) -> SharedString {
    let locale = cx.global::<DfcGlobalStore>().read(cx).locale();
    t!(format!("common.{key}"), locale = locale).into()
}

/// Get translated string from "sidebar" namespace
pub fn i18n_sidebar(cx: &App, key: &str) -> SharedString {
    let locale = cx.global::<DfcGlobalStore>().read(cx).locale();
    t!(format!("sidebar.{key}"), locale = locale).into()
}

/// Get translated string from "devices" namespace
pub fn i18n_devices(cx: &App, key: &str) -> SharedString {
    let locale = cx.global::<DfcGlobalStore>().read(cx).locale();
    t!(format!("devices.{key}"), locale = locale).into()
}

/// Get translated string from "properties" namespace
pub fn i18n_properties(cx: &App, key: &str) -> SharedString {
    let locale = cx.global::<DfcGlobalStore>().read(cx).locale();
    t!(format!("properties.{key}"), locale = locale).into()
}

/// Get translated string from "events" namespace
pub fn i18n_events(cx: &App, key: &str) -> SharedString {
    let locale = cx.global::<DfcGlobalStore>().read(cx).locale();
    t!(format!("events.{key}"), locale = locale).into()
}

/// Get translated string from "commands" namespace
pub fn i18n_commands(cx: &App, key: &str) -> SharedString {
    let locale = cx.global::<DfcGlobalStore>().read(cx).locale();
    t!(format!("commands.{key}"), locale = locale).into()
}

/// Get translated string from "settings" namespace
pub fn i18n_settings(cx: &App, key: &str) -> SharedString {
    let locale = cx.global::<DfcGlobalStore>().read(cx).locale();
    t!(format!("settings.{key}"), locale = locale).into()
}

/// Get translated string from "connection" namespace
pub fn i18n_connection(cx: &App, key: &str) -> SharedString {
    let locale = cx.global::<DfcGlobalStore>().read(cx).locale();
    t!(format!("connection.{key}"), locale = locale).into()
}

/// Get translated string from "servers" namespace
pub fn i18n_servers(cx: &App, key: &str) -> SharedString {
    let locale = cx.global::<DfcGlobalStore>().read(cx).locale();
    t!(format!("servers.{key}"), locale = locale).into()
}

/// Format a translated string with arguments
///
/// # Example
/// ```ignore
/// // With translation "count = "{count} devices"
/// i18n_format(cx, "devices.count", &[("count", "42")])
/// // Returns "42 devices"
/// ```
pub fn i18n_format(cx: &App, key: &str, args: &[(&str, &str)]) -> SharedString {
    let locale = cx.global::<DfcGlobalStore>().read(cx).locale();
    let mut result = t!(key, locale = locale).to_string();

    for (name, value) in args {
        result = result.replace(&format!("{{{name}}}"), value);
    }

    result.into()
}
