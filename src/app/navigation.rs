//! Navigation - Active Page and Tab Management
//!
//! Defines the pages available in the application and tab navigation state.

use gpui::SharedString;
use serde::{Deserialize, Serialize};

/// Available pages in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ActivePage {
    /// Home page with configuration
    #[default]
    Home,
    /// Properties page - device properties list
    Properties,
    /// Events page - device events list
    Events,
    /// Commands page - send commands to devices
    Commands,
    /// Power curve data page
    Curve,
    /// One minute aggregation data page
    OneMin,
    /// Ten minute aggregation data page
    TenMin,
}

impl ActivePage {
    /// Get the icon name for the page
    pub fn icon(&self) -> &'static str {
        match self {
            ActivePage::Home => "home",
            ActivePage::Properties => "list",
            ActivePage::Events => "bell",
            ActivePage::Commands => "terminal",
            ActivePage::Curve => "trending-up",
            ActivePage::OneMin => "clock",
            ActivePage::TenMin => "calendar",
        }
    }

    /// Get the translation key for the page title
    pub fn title_key(&self) -> &'static str {
        match self {
            ActivePage::Home => "nav-home",
            ActivePage::Properties => "nav-properties",
            ActivePage::Events => "nav-events",
            ActivePage::Commands => "nav-commands",
            ActivePage::Curve => "nav-curve",
            ActivePage::OneMin => "nav-one-min",
            ActivePage::TenMin => "nav-ten-min",
        }
    }

    /// Get all available pages for sidebar
    pub fn all() -> &'static [ActivePage] {
        &[
            ActivePage::Home,
            ActivePage::Properties,
            ActivePage::Events,
            ActivePage::Commands,
            ActivePage::Curve,
            ActivePage::OneMin,
            ActivePage::TenMin,
        ]
    }
}

/// Represents an open tab
#[derive(Debug, Clone)]
pub struct Tab {
    /// Unique identifier for the tab
    pub id: u64,
    /// The page this tab displays
    pub page: ActivePage,
    /// Custom title (if any)
    pub title: Option<SharedString>,
    /// Whether this tab can be closed
    pub closable: bool,
}

impl Tab {
    /// Create a new tab for a page
    pub fn new(id: u64, page: ActivePage) -> Self {
        Self {
            id,
            page,
            title: None,
            closable: page != ActivePage::Home,
        }
    }

    /// Create a new tab with a custom title
    pub fn with_title(id: u64, page: ActivePage, title: impl Into<SharedString>) -> Self {
        Self {
            id,
            page,
            title: Some(title.into()),
            closable: true,
        }
    }
}
