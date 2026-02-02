//! Sidebar Navigation Component
//!
//! Fixed-width navigation sidebar with route switching and connected servers.

use crate::assets::CustomIconName;
use crate::constants::SIDEBAR_WIDTH;
use crate::states::{ConfigState, DfcAppState, DfcGlobalStore, KeysState, Route, i18n_sidebar};
use gpui::{Context, Entity, Subscription, Window, div, prelude::*, px};
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
    /// App state entity for server selection
    app_state: Entity<DfcAppState>,
    /// Config state entity for clearing selection
    config_state: Entity<ConfigState>,
    /// Keys state entity for connected servers
    keys_state: Entity<KeysState>,
    /// Subscriptions
    _subscriptions: Vec<Subscription>,
}

impl DfcSidebar {
    /// Create a new sidebar
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut subscriptions = Vec::new();

        let store = cx.global::<DfcGlobalStore>();
        let app_state = store.app_state();
        let config_state = store.config_state();
        let keys_state = store.keys_state();
        let current_route = store.read(cx).route();

        // Subscribe to route changes
        subscriptions.push(cx.observe(&app_state, |this, model, cx| {
            let route = model.read(cx).route();
            if this.current_route != route {
                this.current_route = route;
                cx.notify();
            }
            // Also re-render when selected server changes (for connected config tab)
            cx.notify();
        }));

        // Subscribe to keys state changes (for connected servers)
        subscriptions.push(cx.observe(&keys_state, |_this, _model, cx| {
            cx.notify();
        }));

        Self {
            current_route,
            app_state,
            config_state,
            keys_state,
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
        let is_home = route == Route::Home;
        let app_state = self.app_state.clone();
        let config_state = self.config_state.clone();

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
                if is_home {
                    // Always reset selection to show server list, even if we are already on Home.
                    config_state.update(cx, |state, cx| {
                        state.clear(cx);
                    });
                    app_state.update(cx, |state, cx| {
                        state.select_server(None, cx);
                    });
                }
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

    fn render_connected_config_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let app_state = self.app_state.read(cx);
        let server = app_state.selected_server();

        let Some(server) = server else {
            return div().into_any_element();
        };

        let server_id = server.id.clone();
        let server_name = server.name.clone();
        let tooltip_label = server_name.clone();
        let list_active = cx.theme().list_active;
        let list_active_border = cx.theme().list_active_border;
        let config_state = self.config_state.clone();
        let app_state = self.app_state.clone();

        let btn = Button::new("connected-config-tab")
            .ghost()
            .w_full()
            .h(px(56.0))
            .child(
                v_flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .child(Icon::from(CustomIconName::DatabaseZap))
                    .child(
                        Label::new(server_name)
                            .text_xs()
                            .text_ellipsis()
                            .max_w(px(70.0)),
                    ),
            )
            .on_click(move |_, _, cx| {
                config_state.update(cx, |state, cx| {
                    state.set_connected_server(Some(server_id.clone()), cx);
                });
                app_state.update(cx, |state, cx| {
                    state.select_server(Some(server_id.clone()), cx);
                });
                cx.update_global::<DfcGlobalStore, ()>(|store, cx| {
                    store.update(cx, |state, cx| {
                        state.go_to(Route::Home, cx);
                    });
                });
            });

        div()
            .id("connected-config-tab-wrapper")
            .tooltip(move |window, cx| Tooltip::new(tooltip_label.clone()).build(window, cx))
            .bg(list_active)
            .border_r_2()
            .border_color(list_active_border)
            .child(btn)
            .into_any_element()
    }

    /// Render a connected server item
    fn render_server_item(
        &self,
        index: usize,
        server_id: String,
        server_name: String,
        is_active: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let list_active = cx.theme().list_active;
        let list_active_border = cx.theme().list_active_border;
        let keys_state = self.keys_state.clone();
        let server_id_for_click = server_id.clone();
        let server_name_for_tooltip = server_name.clone();

        let btn = Button::new(("server-nav", index))
            .ghost()
            .w_full()
            .h(px(56.0))
            .child(
                v_flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .child(Icon::from(CustomIconName::DatabaseZap))
                    .child(
                        Label::new(server_name.clone())
                            .text_xs()
                            .text_ellipsis()
                            .max_w(px(70.0)),
                    ),
            )
            .on_click(move |_, _, cx| {
                let server_id = server_id_for_click.clone();
                keys_state.update(cx, |state, cx| {
                    state.set_active_server(Some(server_id), cx);
                });
                // Navigate to home to show keys browser
                cx.update_global::<DfcGlobalStore, ()>(|store, cx| {
                    store.update(cx, |state, cx| {
                        state.go_to(Route::Home, cx);
                    });
                });
            });

        div()
            .id(("server-item", index))
            .tooltip(move |window, cx| Tooltip::new(server_name_for_tooltip.clone()).build(window, cx))
            .when(is_active, |this| {
                this.bg(list_active)
                    .border_r_2()
                    .border_color(list_active_border)
            })
            .child(btn)
    }

    /// Render connected servers section
    fn render_connected_servers(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let keys_state = self.keys_state.read(cx);
        let connected_servers = keys_state.connected_servers();
        let active_server_id = keys_state.active_server_id();

        if connected_servers.is_empty() {
            return div().into_any_element();
        }

        // Collect data before borrowing cx mutably
        let servers_data: Vec<_> = connected_servers
            .iter()
            .enumerate()
            .map(|(index, server)| {
                let is_active = active_server_id == Some(&server.server_id);
                (index, server.server_id.clone(), server.server_name.clone(), is_active)
            })
            .collect();

        let border_color = cx.theme().border;

        let mut items = Vec::new();
        for (index, server_id, server_name, is_active) in servers_data {
            items.push(self.render_server_item(index, server_id, server_name, is_active, cx));
        }

        v_flex()
            .mt_2()
            .border_t_1()
            .border_color(border_color)
            .pt_2()
            .gap_1()
            .children(items)
            .into_any_element()
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
                    .child(self.render_connected_config_tab(cx))
                    // Connected servers
                    .child(self.render_connected_servers(cx)),
            )
            // Settings button at bottom
            .child(self.render_settings_button(cx))
    }
}
