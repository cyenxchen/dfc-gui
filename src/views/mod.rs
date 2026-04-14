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

mod about_dialog;
mod config_view;
mod content;
mod keys_browser;
mod service_panel;
mod sidebar;
mod title_bar;
mod update_dialog;

pub use about_dialog::*;
pub use config_view::*;
pub use content::*;
pub use keys_browser::*;
pub use sidebar::*;
pub use title_bar::*;
pub use update_dialog::*;
