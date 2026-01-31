//! Main Content Area
//!
//! Routes to different views based on the current application route.

use crate::states::{DfcGlobalStore, FleetState, Route, UIEvent, i18n_common};
use gpui::{Context, Entity, Subscription, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme,
    label::Label,
    v_flex,
};

/// Main content container component
pub struct DfcContent {
    /// Current route
    current_route: Route,
    /// Fleet state entity
    fleet_state: Entity<FleetState>,
    /// Subscriptions
    _subscriptions: Vec<Subscription>,
}

impl DfcContent {
    /// Create a new content view
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = cx.global::<DfcGlobalStore>();
        let current_route = store.read(cx).route();
        let fleet_state = store.fleet_state();

        let mut subscriptions = Vec::new();

        // Subscribe to route changes
        let app_state = store.app_state();
        subscriptions.push(cx.observe(&app_state, |this, model, cx| {
            let route = model.read(cx).route();
            if this.current_route != route {
                this.current_route = route;
                cx.notify();
            }
        }));

        // Subscribe to UI events from fleet state
        subscriptions.push(cx.subscribe(&fleet_state, |_this, _state, event, cx| {
            match event {
                UIEvent::Toast { message, is_error } => {
                    // TODO: Show notification
                    tracing::info!("Toast: {} (error: {})", message, is_error);
                }
                UIEvent::ConnectionStateChanged { service, connected, detail } => {
                    tracing::info!(
                        "Connection: {} - {} ({})",
                        service,
                        if *connected { "connected" } else { "disconnected" },
                        detail
                    );
                }
                _ => {}
            }
            cx.notify();
        }));

        Self {
            current_route,
            fleet_state,
            _subscriptions: subscriptions,
        }
    }

    /// Render the home view
    fn render_home(&self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }

    /// Render the properties view
    fn render_properties(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_4()
            .child(Label::new(i18n_common(cx, "properties")).text_xl())
            .child(
                div()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(Label::new(i18n_common(cx, "select_device")).text_color(cx.theme().muted_foreground)),
            )
    }

    /// Render the events view
    fn render_events(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_4()
            .child(Label::new(i18n_common(cx, "events")).text_xl())
            .child(
                div()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(Label::new(i18n_common(cx, "no_events")).text_color(cx.theme().muted_foreground)),
            )
    }

    /// Render the commands view
    fn render_commands(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_4()
            .child(Label::new(i18n_common(cx, "commands")).text_xl())
            .child(
                div()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(Label::new(i18n_common(cx, "select_device")).text_color(cx.theme().muted_foreground)),
            )
    }

    /// Render the settings view
    fn render_settings(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_4()
            .child(Label::new(i18n_common(cx, "settings")).text_xl())
            .child(
                div()
                    .flex_1()
                    .p_4()
                    .child(Label::new(i18n_common(cx, "settings_placeholder")).text_color(cx.theme().muted_foreground)),
            )
    }
}

impl Render for DfcContent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.current_route {
            Route::Home => self.render_home(window, cx).into_any_element(),
            Route::Properties => self.render_properties(window, cx).into_any_element(),
            Route::Events => self.render_events(window, cx).into_any_element(),
            Route::Commands => self.render_commands(window, cx).into_any_element(),
            Route::Settings => self.render_settings(window, cx).into_any_element(),
        };

        div()
            .id("content")
            .flex_1()
            .h_full()
            .bg(cx.theme().background)
            .child(content)
    }
}
