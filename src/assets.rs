//! Embedded assets for DFC-GUI
//!
//! Uses rust-embed to bundle icons and other assets at compile time.

use gpui::{AssetSource, Result, SharedString};
use gpui_component::Icon;
use gpui_component_assets::Assets as ComponentAssets;
use rust_embed::RustEmbed;
use std::borrow::Cow;

/// Embedded assets from the assets directory
#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        // Try component assets first
        if let Some(f) = ComponentAssets::get(path) {
            return Ok(Some(f.data));
        }
        // Then try our own assets
        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow::anyhow!(r#"could not find asset at path "{path}""#))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut files: Vec<SharedString> = ComponentAssets::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect();

        files.extend(
            Self::iter()
                .filter_map(|p| p.starts_with(path).then(|| p.into()))
                .collect::<Vec<_>>(),
        );

        Ok(files)
    }
}

/// Custom icon names for DFC-GUI
pub enum CustomIconName {
    /// Device icon
    Device,
    /// Online status indicator
    Online,
    /// Offline status indicator
    Offline,
    /// Alarm/warning icon
    Alarm,
    /// Command icon
    Command,
    /// Properties icon
    Properties,
    /// Events icon
    Events,
    /// Languages icon
    Languages,
    /// Connection icon
    Connection,
    /// File plus corner icon (for add button)
    FilePlusCorner,
}

impl CustomIconName {
    /// Get the SVG path for this icon
    pub fn path(self) -> SharedString {
        match self {
            CustomIconName::Device => "icons/device.svg",
            CustomIconName::Online => "icons/online.svg",
            CustomIconName::Offline => "icons/offline.svg",
            CustomIconName::Alarm => "icons/alarm.svg",
            CustomIconName::Command => "icons/command.svg",
            CustomIconName::Properties => "icons/properties.svg",
            CustomIconName::Events => "icons/events.svg",
            CustomIconName::Languages => "icons/languages.svg",
            CustomIconName::Connection => "icons/connection.svg",
            CustomIconName::FilePlusCorner => "icons/file-plus-corner.svg",
        }
        .into()
    }
}

impl From<CustomIconName> for Icon {
    fn from(val: CustomIconName) -> Self {
        Icon::empty().path(val.path())
    }
}
