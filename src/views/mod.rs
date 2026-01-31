//! View Components
//!
//! UI components for the DFC-GUI application.
//!
//! ## Layout Structure
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                        TitleBar                              │
//! ├────────┬────────────────────────────────────────────────────┤
//! │        │                                                     │
//! │        │                                                     │
//! │ Side   │                    Content                          │
//! │ bar    │                                                     │
//! │ (80px) │                                                     │
//! │        │                                                     │
//! │        │                                                     │
//! ├────────┴────────────────────────────────────────────────────┤
//! │                       StatusBar                              │
//! └─────────────────────────────────────────────────────────────┘
//! ```

mod content;
mod sidebar;
mod title_bar;

pub use content::*;
pub use sidebar::*;
pub use title_bar::*;
