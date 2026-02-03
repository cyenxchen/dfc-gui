//! State Management Layer
//!
//! Centralized application state using GPUI's Entity system.
//! Follows a unidirectional data flow pattern:
//!
//! ```text
//! UI Action → State Method → spawn Service Call → Service Event → State Update → notify → UI Refresh
//! ```

mod app;
mod config;
mod fleet;
mod i18n;
mod keys;
mod prop_table;
mod ui_event;

pub use app::*;
pub use config::*;
pub use fleet::*;
pub use i18n::*;
pub use keys::*;
pub use prop_table::*;
pub use ui_event::*;
