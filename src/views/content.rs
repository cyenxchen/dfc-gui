//! Main Content Area
//!
//! Routes to different views based on the current application route.

use crate::assets::CustomIconName;
use crate::states::{DfcGlobalStore, FleetState, Route, UIEvent, i18n_common, i18n_sidebar};
use gpui::{Context, Entity, Subscription, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Icon, IconName,
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState},
    label::Label,
    v_flex,
};

/// Width of the keyword search input
const KEYWORD_INPUT_WIDTH: f32 = 200.0;

/// Main content container component
pub struct DfcContent {
    /// Current route
    current_route: Route,
    /// Fleet state entity
    fleet_state: Entity<FleetState>,
    /// Keyword search input state
    keyword_state: Entity<InputState>,
    /// Subscriptions
    _subscriptions: Vec<Subscription>,
}

impl DfcContent {
    /// Create a new content view
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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

        // Initialize keyword search input
        let keyword_state = cx.new(|cx| {
            InputState::new(window, cx)
                .clean_on_escape()
                .placeholder(i18n_common(cx, "filter_placeholder"))
        });

        // Subscribe to input events
        subscriptions.push(cx.subscribe(&keyword_state, |this, _, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.handle_filter(cx);
            }
        }));

        Self {
            current_route,
            fleet_state,
            keyword_state,
            _subscriptions: subscriptions,
        }
    }

    /// Handle filter action
    fn handle_filter(&mut self, cx: &mut Context<Self>) {
        let keyword = self.keyword_state.read(cx).value();
        tracing::info!("Filter keyword: {}", keyword);
        // TODO: Implement actual filtering logic
        cx.notify();
    }

    /// Handle add action
    fn handle_add(&mut self, _cx: &mut Context<Self>) {
        tracing::info!("Add button clicked");
        // TODO: Implement add dialog/action
    }

    /// Render the bottom toolbar
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let search_btn = Button::new("home-search-btn")
            .ghost()
            .icon(IconName::Search)
            .tooltip(i18n_sidebar(cx, "search"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.handle_filter(cx);
            }));

        h_flex()
            .w_full()
            .p_2()
            .border_t_1()
            .border_color(cx.theme().border)
            // Left side: Add button and search input
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("add-btn")
                            .icon(Icon::from(CustomIconName::FilePlusCorner))
                            .tooltip(i18n_common(cx, "add"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.handle_add(cx);
                            })),
                    )
                    .child(
                        gpui_component::input::Input::new(&self.keyword_state)
                            .w(px(KEYWORD_INPUT_WIDTH))
                            .suffix(search_btn)
                            .cleanable(true),
                    )
                    .flex_1(),
            )
    }

    /// Render the home view
    fn render_home(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            // Main content area (empty for now)
            .child(div().size_full().flex_1())
            // Bottom toolbar
            .child(self.render_toolbar(cx))
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
