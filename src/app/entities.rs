//! AppEntities - Global Entity Handles
//!
//! All global GPUI entities are collected here for easy access and management.
//! This pattern avoids "monolith state" by splitting state by update frequency.

use gpui::{App, AppContext, Entity, Global};

use crate::state::{
    commands_state::CommandsState, config_state::ConfigState, connection_state::ConnectionState,
    data_state::DataState, events_state::EventsState, i18n_state::I18nState, log_state::LogState,
    properties_state::PropertiesState, tabs_state::TabsState,
};

/// Collection of all global Entity handles
#[derive(Clone)]
pub struct AppEntities {
    /// Application configuration state
    pub config: Entity<ConfigState>,
    /// Connection status for Redis/Pulsar/Database
    pub connection: Entity<ConnectionState>,
    /// Log messages (ring buffer)
    pub logs: Entity<LogState>,
    /// Tab navigation state
    pub tabs: Entity<TabsState>,
    /// Internationalization state
    pub i18n: Entity<I18nState>,
    /// Properties data state
    pub properties: Entity<PropertiesState>,
    /// Events data state
    pub events: Entity<EventsState>,
    /// Commands state
    pub commands: Entity<CommandsState>,
    /// Data aggregation state (curve, 1min, 10min)
    pub data: Entity<DataState>,
}

impl Global for AppEntities {}

impl AppEntities {
    /// Initialize all entities with default values
    pub fn init(cx: &mut App) -> Self {
        Self {
            config: cx.new(|_| ConfigState::default()),
            connection: cx.new(|_| ConnectionState::default()),
            logs: cx.new(|_| LogState::new(2000)), // Ring buffer with 2000 entries
            tabs: cx.new(|_| TabsState::default()),
            i18n: cx.new(|_| I18nState::default()),
            properties: cx.new(|_| PropertiesState::default()),
            events: cx.new(|_| EventsState::default()),
            commands: cx.new(|_| CommandsState::default()),
            data: cx.new(|_| DataState::default()),
        }
    }
}
