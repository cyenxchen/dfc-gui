//! Colors - DFC Theme Colors

use gpui::{rgb, Hsla, Rgba};

/// DFC color palette - All colors are accessed via associated functions
pub struct DfcColors;

impl DfcColors {
    // Primary colors
    /// Header background - Cyan/Teal
    pub fn header_bg() -> Rgba { rgb(0x2cb3b8) }
    /// Primary accent - Yellow (for main buttons)
    pub fn accent() -> Rgba { rgb(0xf5c518) }
    /// Secondary accent - Blue
    pub fn accent_blue() -> Rgba { rgb(0x3b82f6) }

    // Background colors
    /// Main background
    pub fn background() -> Rgba { rgb(0xf5f5f5) }
    /// Content area background
    pub fn content_bg() -> Rgba { rgb(0xffffff) }
    /// Sidebar background
    pub fn sidebar_bg() -> Rgba { rgb(0xffffff) }
    /// Log panel background - Dark blue
    pub fn log_panel_bg() -> Rgba { rgb(0x1a2332) }

    // Text colors
    /// Primary text
    pub fn text_primary() -> Rgba { rgb(0x1f2937) }
    /// Secondary text
    pub fn text_secondary() -> Rgba { rgb(0x6b7280) }
    /// Muted text
    pub fn text_muted() -> Rgba { rgb(0x9ca3af) }
    /// Light text (on dark backgrounds)
    pub fn text_light() -> Rgba { rgb(0xffffff) }
    /// Header text
    pub fn text_header() -> Rgba { rgb(0xffffff) }

    // Status colors
    /// Success - Green
    pub fn success() -> Rgba { rgb(0x22c55e) }
    /// Warning - Amber
    pub fn warning() -> Rgba { rgb(0xf59e0b) }
    /// Error/Danger - Red
    pub fn danger() -> Rgba { rgb(0xef4444) }
    /// Info - Blue
    pub fn info() -> Rgba { rgb(0x3b82f6) }

    // Border colors
    /// Default border
    pub fn border() -> Rgba { rgb(0xe5e7eb) }
    /// Focused border
    pub fn border_focus() -> Rgba { rgb(0x3b82f6) }

    // Button colors
    /// Primary button background
    pub fn button_primary_bg() -> Rgba { rgb(0xf5c518) }
    /// Primary button text
    pub fn button_primary_text() -> Rgba { rgb(0x1f2937) }
    /// Danger button background
    pub fn button_danger_bg() -> Rgba { rgb(0xec4899) }
    /// Danger button text
    pub fn button_danger_text() -> Rgba { rgb(0xffffff) }
    /// Ghost button text
    pub fn button_ghost_text() -> Rgba { rgb(0x6b7280) }

    // Table colors
    /// Table header background
    pub fn table_header_bg() -> Rgba { rgb(0xf9fafb) }
    /// Table row hover
    pub fn table_row_hover() -> Rgba { rgb(0xf3f4f6) }
    /// Table row alternate
    pub fn table_row_alt() -> Rgba { rgb(0xf9fafb) }

    // Input colors
    /// Input background
    pub fn input_bg() -> Rgba { rgb(0xffffff) }
    /// Input border
    pub fn input_border() -> Rgba { rgb(0xd1d5db) }
    /// Input placeholder
    pub fn input_placeholder() -> Rgba { rgb(0x9ca3af) }
}

/// Convert Rgba to Hsla for certain GPUI operations
impl DfcColors {
    pub fn header_bg_hsla() -> Hsla {
        Hsla::from(Self::header_bg())
    }

    pub fn accent_hsla() -> Hsla {
        Hsla::from(Self::accent())
    }
}
