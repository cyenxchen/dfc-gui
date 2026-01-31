//! Sidebar Navigation Component
//!
//! Fixed-width navigation sidebar with route switching.

use crate::constants::SIDEBAR_WIDTH;
use crate::states::{DfcGlobalStore, Route, i18n_sidebar};
use gpui::{Context, Subscription, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Icon, IconName,
    button::{Button, ButtonVariants},
    label::Label,
    tooltip::Tooltip,
    v_flex,
};

/// Sidebar navigation component
pub struct DfcSidebar {
    /// Current route for highlighting
    current_route: Route,
    /// Subscriptions
    _subscriptions: Vec<Subscription>,
}

impl DfcSidebar {
    /// Create a new sidebar
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut subscriptions = Vec::new();

        // Subscribe to route changes
        let app_state = cx.global::<DfcGlobalStore>().app_state();
        subscriptions.push(cx.observe(&app_state, |this, model, cx| {
            let route = model.read(cx).route();
            if this.current_route != route {
                this.current_route = route;
                cx.notify();
            }
        }));

        let current_route = cx.global::<DfcGlobalStore>().read(cx).route();

        Self {
            current_route,
            _subscriptions: subscriptions,
        }
    }

    /// Render a navigation button
    fn render_nav_button(
        &self,
        id: &'static str,
        route: Route,
        icon: IconName,
        label_key: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = self.current_route == route;
        let label = i18n_sidebar(cx, label_key);
        let tooltip_label = label.clone();
        let list_active = cx.theme().list_active;
        let list_active_border = cx.theme().list_active_border;

        let btn = Button::new(id)
            .ghost()
            .w_full()
            .h(px(56.0))
            .child(
                v_flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .child(Icon::new(icon))
                    .child(Label::new(label).text_xs()),
            )
            .on_click(move |_, _, cx| {
                cx.update_global::<DfcGlobalStore, ()>(|store, cx| {
                    store.update(cx, |state, cx| {
                        state.go_to(route, cx);
                    });
                });
            });

        div()
            .id(id)
            .tooltip(move |window, cx| Tooltip::new(tooltip_label.clone()).build(window, cx))
            .when(is_active, |this| {
                this.bg(list_active)
                    .border_r_2()
                    .border_color(list_active_border)
            })
            .child(btn)
    }

    /// Render the settings button at the bottom
    fn render_settings_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_active = self.current_route == Route::Settings;
        let label = i18n_sidebar(cx, "settings");
        let tooltip_label = label.clone();
        let border_color = cx.theme().border;
        let list_active = cx.theme().list_active;
        let list_active_border = cx.theme().list_active_border;

        let btn = Button::new("settings-nav")
            .ghost()
            .w_full()
            .h(px(48.0))
            .child(
                v_flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::Settings)),
            )
            .on_click(move |_, _, cx| {
                cx.update_global::<DfcGlobalStore, ()>(|store, cx| {
                    store.update(cx, |state, cx| {
                        state.go_to(Route::Settings, cx);
                    });
                });
            });

        div()
            .id("nav-settings")
            .tooltip(move |window, cx| Tooltip::new(tooltip_label.clone()).build(window, cx))
            .border_t_1()
            .border_color(border_color)
            .when(is_active, |this| {
                this.bg(list_active)
                    .border_r_2()
                    .border_color(list_active_border)
            })
            .child(btn)
    }
}

impl Render for DfcSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_color = cx.theme().border;
        let sidebar_bg = cx.theme().sidebar;

        v_flex()
            .id("sidebar")
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(border_color)
            .bg(sidebar_bg)
            // Navigation items
            .child(
                v_flex()
                    .flex_1()
                    .pt_2()
                    .child(self.render_nav_button("home", Route::Home, IconName::LayoutDashboard, "home", cx))
                    .child(self.render_nav_button("properties", Route::Properties, IconName::File, "properties", cx))
                    .child(self.render_nav_button("events", Route::Events, IconName::Bell, "events", cx))
                    .child(self.render_nav_button("commands", Route::Commands, IconName::SquareTerminal, "commands", cx)),
            )
            // Settings button at bottom
            .child(self.render_settings_button(cx))
    }
}
