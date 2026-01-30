//! State - GPUI Entity State Modules
//!
//! Each state module represents a distinct piece of application state,
//! split by update frequency to avoid unnecessary re-renders.

pub mod commands_state;
pub mod config_state;
pub mod connection_state;
pub mod data_state;
pub mod events_state;
pub mod i18n_state;
pub mod log_state;
pub mod properties_state;
pub mod tabs_state;
