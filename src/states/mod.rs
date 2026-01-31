//! State Management Layer
//!
//! Centralized application state using GPUI's Entity system.
//! Follows a unidirectional data flow pattern:
//!
//! ```text
//! UI Action → State Method → spawn Service Call → Service Event → State Update → notify → UI Refresh
//! ```

mod app;
mod fleet;
mod i18n;
mod ui_event;

pub use app::*;
pub use fleet::*;
pub use i18n::*;
pub use ui_event::*;
