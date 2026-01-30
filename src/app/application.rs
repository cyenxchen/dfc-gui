//! Application - App Initialization and Window Management
//!
//! Main entry point for the GPUI application.

use gpui::{
    actions, px, App, AppContext, Application, Bounds, WindowBounds, WindowOptions,
};

use crate::app::entities::AppEntities;
use crate::app::workspace::Workspace;
use crate::eventing::app_event::AppEvent;
use crate::services::service_hub::ServiceHub;

actions!(dfc, [Quit]);

/// Run the DFC GUI application
pub fn run_app() {
    Application::new().run(|cx: &mut App| {
        // Set up action handlers
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());

        // Initialize global entities
        let entities = AppEntities::init(cx);
        cx.set_global(entities.clone());

        // Create event channel for service -> UI communication
        let (event_tx, event_rx) = flume::unbounded::<AppEvent>();

        // Initialize service hub
        let service_hub = ServiceHub::new(event_tx.clone());
        cx.set_global(service_hub);

        // Create main window
        let bounds = Bounds::centered(None, gpui::size(px(1400.0), px(900.0)), cx);
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            ..Default::default()
        };

        cx.open_window(window_options, |_window, cx| {
            cx.new(|cx| Workspace::new(entities.clone(), event_rx, cx))
        })
        .unwrap();

        cx.activate(true);
    });
}
