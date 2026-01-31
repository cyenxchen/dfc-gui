//! Application State
//!
//! Global application state including routing, theme, locale, and window bounds.

use crate::connection::{DfcServerConfig, get_servers, save_servers};
use crate::error::{Error, Result};
use crate::helpers::get_or_create_config_dir;
use crate::services::ServiceHub;
use crate::states::FleetState;
use chrono::Local;
use gpui::{Action, App, AppContext, Bounds, Context, Entity, Global, Pixels};
use gpui_component::ThemeMode;
use locale_config::Locale;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{error, info};
use uuid::Uuid;

/// Application routes
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Route {
    /// Home page - device list overview
    #[default]
    Home,
    /// Device properties view
    Properties,
    /// Device events view
    Events,
    /// Command interface
    Commands,
    /// Application settings
    Settings,
}

/// Font size options
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl FontSize {
    /// Convert to pixel size (returns None for default/Medium)
    pub fn to_pixels(self) -> Option<f32> {
        match self {
            FontSize::Small => Some(14.0),
            FontSize::Medium => None, // Use system default
            FontSize::Large => Some(18.0),
        }
    }
}

// ==================== Actions ====================

/// Theme selection action
#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
pub enum ThemeAction {
    Light,
    Dark,
    System,
}

/// Locale selection action
#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
pub enum LocaleAction {
    En,
    Zh,
}

/// Font size action
#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
pub enum FontSizeAction {
    Large,
    Medium,
    Small,
}

/// Settings action
#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
pub enum SettingsAction {
    Open,
}

// ==================== Persisted State ====================

const LIGHT_THEME_MODE: &str = "light";
const DARK_THEME_MODE: &str = "dark";

fn get_config_path() -> Result<PathBuf> {
    let config_dir = get_or_create_config_dir()?;
    let path = config_dir.join("dfc-gui.toml");
    if !path.exists() {
        std::fs::write(&path, "")?;
    }
    Ok(path)
}

/// Persisted application state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DfcAppState {
    route: Route,
    locale: Option<String>,
    bounds: Option<Bounds<Pixels>>,
    theme: Option<String>,
    font_size: Option<FontSize>,
    /// Selected device ID
    selected_device: Option<String>,
    /// Server configurations (loaded separately, not serialized here)
    #[serde(skip)]
    servers: Vec<DfcServerConfig>,
    /// Currently selected server ID
    #[serde(skip)]
    selected_server_id: Option<String>,
}

impl DfcAppState {
    /// Load state from config file
    pub fn try_load() -> Result<Self> {
        let path = get_config_path()?;
        info!(path = ?path, "Loading config file");
        let value = std::fs::read_to_string(&path)?;

        if value.trim().is_empty() {
            return Ok(Self::new());
        }

        let mut state: Self = toml::from_str(&value).map_err(|e| {
            error!(error = %e, path = ?path, "Failed to parse config file");
            e
        })?;

        // Detect system locale if not set
        if state.locale.as_ref().map_or(true, |l| l.is_empty()) {
            if let Some((lang, _)) = Locale::current().to_string().split_once("-") {
                state.locale = Some(lang.to_string());
            }
        }

        // Always start at home
        state.route = Route::Home;

        Ok(state)
    }

    /// Create new default state
    pub fn new() -> Self {
        Self::default()
    }

    // ==================== Getters ====================

    pub fn route(&self) -> Route {
        self.route
    }

    pub fn bounds(&self) -> Option<&Bounds<Pixels>> {
        self.bounds.as_ref()
    }

    pub fn font_size(&self) -> FontSize {
        self.font_size.unwrap_or(FontSize::Medium)
    }

    pub fn theme(&self) -> Option<ThemeMode> {
        match self.theme.as_deref() {
            Some(LIGHT_THEME_MODE) => Some(ThemeMode::Light),
            Some(DARK_THEME_MODE) => Some(ThemeMode::Dark),
            _ => None,
        }
    }

    pub fn locale(&self) -> &str {
        self.locale.as_deref().unwrap_or("en")
    }

    pub fn selected_device(&self) -> Option<&str> {
        self.selected_device.as_deref()
    }

    // ==================== Setters ====================

    pub fn go_to(&mut self, route: Route, cx: &mut Context<Self>) {
        if self.route != route {
            self.route = route;
            cx.notify();
        }
    }

    pub fn set_bounds(&mut self, bounds: Bounds<Pixels>) {
        self.bounds = Some(bounds);
    }

    pub fn set_theme(&mut self, theme: Option<ThemeMode>) {
        self.theme = match theme {
            Some(ThemeMode::Light) => Some(LIGHT_THEME_MODE.to_string()),
            Some(ThemeMode::Dark) => Some(DARK_THEME_MODE.to_string()),
            _ => None,
        };
    }

    pub fn set_locale(&mut self, locale: String) {
        self.locale = Some(locale);
    }

    pub fn set_font_size(&mut self, font_size: Option<FontSize>) {
        self.font_size = font_size;
    }

    pub fn set_selected_device(&mut self, device_id: Option<String>) {
        self.selected_device = device_id;
    }

    // ==================== Server Management ====================

    /// Load server configurations from file
    pub fn load_servers(&mut self) {
        match get_servers() {
            Ok(servers) => {
                info!(count = servers.len(), "Loaded server configurations");
                self.servers = servers;
            }
            Err(e) => {
                error!(error = %e, "Failed to load server configurations");
                self.servers = vec![];
            }
        }
    }

    /// Get server list
    pub fn servers(&self) -> &[DfcServerConfig] {
        &self.servers
    }

    /// Get server by ID
    pub fn server(&self, id: &str) -> Option<&DfcServerConfig> {
        self.servers.iter().find(|s| s.id == id)
    }

    /// Get currently selected server
    pub fn selected_server(&self) -> Option<&DfcServerConfig> {
        self.selected_server_id
            .as_ref()
            .and_then(|id| self.servers.iter().find(|s| &s.id == id))
    }

    /// Get selected server ID
    pub fn selected_server_id(&self) -> Option<&str> {
        self.selected_server_id.as_deref()
    }

    /// Add or update a server configuration
    pub fn upsert_server(&mut self, mut server: DfcServerConfig, cx: &mut Context<Self>) {
        // Generate ID if empty (new server)
        if server.id.is_empty() {
            server.id = Uuid::now_v7().to_string();
        }

        // Update timestamp
        server.updated_at = Some(Local::now().to_rfc3339());

        // Find existing or insert new
        if let Some(existing) = self.servers.iter_mut().find(|s| s.id == server.id) {
            *existing = server;
        } else {
            self.servers.push(server);
        }

        // Save asynchronously
        let servers = self.servers.clone();
        cx.spawn(async move |_, _| {
            if let Err(e) = save_servers(servers).await {
                error!(error = %e, "Failed to save servers");
            } else {
                info!("Servers saved successfully");
            }
        })
        .detach();

        cx.notify();
    }

    /// Remove a server by ID
    pub fn remove_server(&mut self, id: &str, cx: &mut Context<Self>) {
        self.servers.retain(|s| s.id != id);

        // Clear selection if removed server was selected
        if self.selected_server_id.as_deref() == Some(id) {
            self.selected_server_id = None;
        }

        // Save asynchronously
        let servers = self.servers.clone();
        cx.spawn(async move |_, _| {
            if let Err(e) = save_servers(servers).await {
                error!(error = %e, "Failed to save servers after removal");
            } else {
                info!("Server removed and saved");
            }
        })
        .detach();

        cx.notify();
    }

    /// Select a server
    pub fn select_server(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.selected_server_id = id;
        cx.notify();
    }
}

// ==================== Global Store ====================

/// Global store accessible via `cx.global::<DfcGlobalStore>()`
#[derive(Clone)]
pub struct DfcGlobalStore {
    app_state: Entity<DfcAppState>,
    fleet_state: Entity<FleetState>,
    services: ServiceHub,
}

impl DfcGlobalStore {
    /// Create a new global store
    pub fn new(
        app_state: Entity<DfcAppState>,
        fleet_state: Entity<FleetState>,
        services: ServiceHub,
    ) -> Self {
        Self {
            app_state,
            fleet_state,
            services,
        }
    }

    /// Get the app state entity
    pub fn app_state(&self) -> Entity<DfcAppState> {
        self.app_state.clone()
    }

    /// Get the fleet state entity
    pub fn fleet_state(&self) -> Entity<FleetState> {
        self.fleet_state.clone()
    }

    /// Get the service hub
    pub fn services(&self) -> &ServiceHub {
        &self.services
    }

    /// Read app state
    pub fn read<'a>(&self, cx: &'a App) -> &'a DfcAppState {
        self.app_state.read(cx)
    }

    /// Update app state
    pub fn update<R, C: AppContext>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut DfcAppState, &mut Context<DfcAppState>) -> R,
    ) -> C::Result<R> {
        self.app_state.update(cx, update)
    }

    /// Get a clone of current app state
    pub fn value(&self, cx: &App) -> DfcAppState {
        self.app_state.read(cx).clone()
    }
}

impl Global for DfcGlobalStore {}

// ==================== Persistence ====================

/// Save app state to disk
pub fn save_app_state(state: &DfcAppState) -> Result<()> {
    let path = get_config_path()?;
    let value = toml::to_string(state)?;
    std::fs::write(path, value)?;
    Ok(())
}

/// Update app state and save to disk asynchronously
pub fn update_app_state_and_save<F>(cx: &App, action_name: &'static str, mutation: F)
where
    F: FnOnce(&mut DfcAppState, &App) + Send + 'static + Clone,
{
    let store = cx.global::<DfcGlobalStore>().clone();

    cx.spawn(async move |cx| {
        // Step 1: Update global state
        let current_state = store.update(cx, |state, cx| {
            mutation(state, cx);
            state.clone()
        });

        // Step 2: Persist to disk in background
        if let Ok(state) = current_state {
            cx.background_executor()
                .spawn(async move {
                    if let Err(e) = save_app_state(&state) {
                        error!(error = %e, action = action_name, "Failed to save state");
                    } else {
                        info!(action = action_name, "State saved successfully");
                    }
                })
                .await;
        }

        // Step 3: Refresh windows
        cx.update(|cx| cx.refresh_windows()).ok();
    })
    .detach();
}
