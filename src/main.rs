//! DFC-GUI - Device Fleet Control GUI
//!
//! A native GUI client for monitoring and controlling device fleets.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::constants::SIDEBAR_WIDTH;
use crate::helpers::{MenuAction, is_development, new_key_bindings};
use crate::services::ServiceHub;
use crate::states::{
    DfcAppState, DfcGlobalStore, FleetState, FontSize, FontSizeAction, LocaleAction,
    Route, SettingsAction, ThemeAction, UIEvent, update_app_state_and_save,
};
use crate::views::{DfcContent, DfcSidebar, DfcTitleBar};
use gpui::{
    App, Application, Bounds, Entity, Menu, MenuItem, Pixels, Task, TitlebarOptions, Window,
    WindowAppearance, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use gpui_component::{ActiveTheme, Root, Theme, ThemeMode, WindowExt, h_flex, v_flex};
use std::env;
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

// Initialize i18n
rust_i18n::i18n!("locales", fallback = "en");

mod assets;
mod constants;
mod error;
mod helpers;
mod services;
mod states;
mod views;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Main application component
pub struct DfcApp {
    /// Sidebar component
    sidebar: Entity<DfcSidebar>,
    /// Content component
    content: Entity<DfcContent>,
    /// Title bar component
    title_bar: Option<Entity<DfcTitleBar>>,
    /// Last window bounds (for persistence)
    last_bounds: Bounds<Pixels>,
    /// Save task handle
    save_task: Option<Task<()>>,
    /// Theme update task
    theme_update_task: Option<Task<()>>,
}

impl DfcApp {
    /// Create a new application instance
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|cx| DfcSidebar::new(window, cx));
        let content = cx.new(|cx| DfcContent::new(window, cx));
        let title_bar = Some(cx.new(|cx| DfcTitleBar::new(window, cx)));

        // Subscribe to theme changes
        cx.observe_window_appearance(window, |this, _window, cx| {
            if cx.global::<DfcGlobalStore>().read(cx).theme().is_none() {
                this.theme_update_task = Some(cx.spawn(async move |_this, cx| {
                    let _ = cx.update(|cx| {
                        Theme::change(cx.window_appearance(), None, cx);
                        cx.refresh_windows();
                    });
                }));
            }
        })
        .detach();

        Self {
            sidebar,
            content,
            title_bar,
            last_bounds: Bounds::default(),
            save_task: None,
            theme_update_task: None,
        }
    }

    /// Persist window state when bounds change
    fn persist_window_state(&mut self, new_bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
        self.last_bounds = new_bounds;
        let store = cx.global::<DfcGlobalStore>().clone();
        let mut value = store.value(cx);
        value.set_bounds(new_bounds);

        let task = cx.spawn(async move |_, cx| {
            // Debounce: wait 500ms before saving
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;

            let result = store.update(cx, move |state, cx| {
                state.set_bounds(new_bounds);
                cx.notify();
            });

            if let Err(e) = result {
                error!(error = %e, "Failed to update window bounds");
                return;
            };

            cx.background_spawn(async move {
                if let Err(e) = states::save_app_state(&value) {
                    error!(error = %e, "Failed to save window bounds");
                } else {
                    info!(bounds = ?new_bounds, "Window bounds saved");
                }
            })
            .await;
        });

        self.save_task = Some(task);
    }

    /// Render the title bar
    fn render_titlebar(&self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        match self.title_bar.as_ref() {
            Some(title_bar) => title_bar.clone().into_any_element(),
            None => h_flex().into_any_element(),
        }
    }
}

impl Render for DfcApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        // Persist bounds if changed
        let current_bounds = window.bounds();
        if current_bounds != self.last_bounds {
            self.persist_window_state(current_bounds, cx);
        }

        // Apply font size
        if let Some(font_size) = cx.global::<DfcGlobalStore>().read(cx).font_size().to_pixels() {
            window.set_rem_size(font_size);
        }

        let content = v_flex()
            .id(PKG_NAME)
            .size_full()
            .child(self.render_titlebar(window, cx))
            .child(
                h_flex()
                    .id("main-layout")
                    .bg(cx.theme().background)
                    .size_full()
                    .child(self.sidebar.clone())
                    .child(self.content.clone())
                    .children(dialog_layer)
                    .children(notification_layer),
            );

        // Action handlers
        content
            .on_action(cx.listener(|_this, e: &ThemeAction, _window, cx| {
                let mode = match e {
                    ThemeAction::Light => Some(ThemeMode::Light),
                    ThemeAction::Dark => Some(ThemeMode::Dark),
                    ThemeAction::System => None,
                };

                let render_mode = match mode {
                    Some(m) => m,
                    None => match cx.window_appearance() {
                        WindowAppearance::Light => ThemeMode::Light,
                        _ => ThemeMode::Dark,
                    },
                };

                Theme::change(render_mode, None, cx);

                update_app_state_and_save(cx, "save_theme", move |state, _cx| {
                    state.set_theme(mode);
                });
            }))
            .on_action(cx.listener(|_this, e: &LocaleAction, _window, cx| {
                let locale = match e {
                    LocaleAction::Zh => "zh",
                    LocaleAction::En => "en",
                };

                update_app_state_and_save(cx, "save_locale", move |state, _cx| {
                    state.set_locale(locale.to_string());
                });
            }))
            .on_action(cx.listener(|_this, e: &FontSizeAction, _window, cx| {
                let font_size = match e {
                    FontSizeAction::Large => Some(FontSize::Large),
                    FontSizeAction::Small => Some(FontSize::Small),
                    FontSizeAction::Medium => None,
                };

                update_app_state_and_save(cx, "save_font_size", move |state, _cx| {
                    state.set_font_size(font_size);
                });
            }))
            .on_action(cx.listener(|_this, _e: &SettingsAction, _window, cx| {
                cx.update_global::<DfcGlobalStore, ()>(|store, cx| {
                    store.update(cx, |state, cx| {
                        state.go_to(Route::Settings, cx);
                    });
                });
            }))
    }
}

/// Initialize logging
fn init_logger() {
    let mut level = Level::INFO;
    if let Ok(log_level) = env::var("RUST_LOG") {
        if let Ok(value) = log_level.parse() {
            level = value;
        }
    }

    let timer = tracing_subscriber::fmt::time::OffsetTime::local_rfc_3339().unwrap_or_else(|_| {
        tracing_subscriber::fmt::time::OffsetTime::new(
            time::UtcOffset::from_hms(0, 0, 0).unwrap_or(time::UtcOffset::UTC),
            time::format_description::well_known::Rfc3339,
        )
    });

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_timer(timer)
        .with_ansi(is_development())
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

fn main() {
    init_logger();
    info!("Starting {} v{}", PKG_NAME, PKG_VERSION);

    let app = Application::new().with_assets(assets::Assets);

    // Load or create app state
    let app_state = DfcAppState::try_load().unwrap_or_else(|e| {
        error!(error = %e, "Failed to load app state, using default");
        DfcAppState::new()
    });

    // Create service hub
    let services = ServiceHub::with_defaults().unwrap_or_else(|e| {
        error!(error = %e, "Failed to create service hub");
        panic!("Cannot start without service hub");
    });

    app.run(move |cx| {
        // Initialize GPUI components
        gpui_component::init(cx);

        cx.activate(true);

        // Determine window bounds
        let window_bounds = if let Some(bounds) = app_state.bounds() {
            info!(bounds = ?bounds, "Restoring window bounds");
            *bounds
        } else {
            let mut window_size = size(
                px(constants::DEFAULT_WINDOW_WIDTH),
                px(constants::DEFAULT_WINDOW_HEIGHT),
            );
            if let Some(display) = cx.primary_display() {
                let display_size = display.bounds().size;
                window_size.width = window_size.width.min(display_size.width * 0.85);
                window_size.height = window_size.height.min(display_size.height * 0.85);
            }
            Bounds::centered(None, window_size, cx)
        };

        // Create state entities
        let app_state_entity = cx.new(|_| app_state);
        let fleet_state_entity = cx.new(|_| FleetState::new());

        // Start event ingestion
        let event_rx = services.events();
        fleet_state_entity.update(cx, |state, cx| {
            state.start_ingest(event_rx, cx);
        });

        // Create global store
        let global_store = DfcGlobalStore::new(
            app_state_entity.clone(),
            fleet_state_entity.clone(),
            services.clone(),
        );

        // Apply theme
        if let Some(theme) = global_store.read(cx).theme() {
            Theme::change(theme, None, cx);
        }

        cx.set_global(global_store);
        cx.bind_keys(new_key_bindings());

        // Set up menu actions
        cx.on_action(|e: &MenuAction, cx: &mut App| match e {
            MenuAction::Quit => cx.quit(),
            MenuAction::About => {
                // TODO: Open about dialog
                info!("About dialog");
            }
        });

        // Set up application menu
        cx.set_menus(vec![Menu {
            name: "DFC-GUI".into(),
            items: vec![
                MenuItem::action("About DFC-GUI", MenuAction::About),
                MenuItem::action("Quit", MenuAction::Quit),
            ],
        }]);

        // Start services
        services.start();

        // Open main window
        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                    #[cfg(not(target_os = "linux"))]
                    titlebar: Some(TitlebarOptions {
                        title: None,
                        appears_transparent: true,
                        traffic_light_position: Some(gpui::point(px(9.0), px(9.0))),
                    }),
                    show: true,
                    window_min_size: Some(size(
                        px(constants::MIN_WINDOW_WIDTH),
                        px(constants::MIN_WINDOW_HEIGHT),
                    )),
                    ..Default::default()
                },
                |window, cx| {
                    #[cfg(target_os = "macos")]
                    window.on_window_should_close(cx, move |_window, cx| {
                        cx.quit();
                        true
                    });

                    let app_view = cx.new(|cx| DfcApp::new(window, cx));
                    cx.new(|cx| Root::new(app_view, window, cx))
                },
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
