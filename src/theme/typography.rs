//! Typography - Font Sizes and Weights

/// Typography constants
pub struct Typography;

impl Typography {
    // Font sizes
    pub const TEXT_XS: f32 = 12.0;
    pub const TEXT_SM: f32 = 14.0;
    pub const TEXT_BASE: f32 = 16.0;
    pub const TEXT_LG: f32 = 18.0;
    pub const TEXT_XL: f32 = 20.0;
    pub const TEXT_2XL: f32 = 24.0;
    pub const TEXT_3XL: f32 = 30.0;

    // Line heights
    pub const LEADING_TIGHT: f32 = 1.25;
    pub const LEADING_NORMAL: f32 = 1.5;
    pub const LEADING_RELAXED: f32 = 1.625;

    // Font weights (not directly used in GPUI but for reference)
    pub const FONT_NORMAL: u32 = 400;
    pub const FONT_MEDIUM: u32 = 500;
    pub const FONT_SEMIBOLD: u32 = 600;
    pub const FONT_BOLD: u32 = 700;
}
