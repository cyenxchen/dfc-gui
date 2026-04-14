//! Sidebar Navigation Component
//!
//! Fixed-width navigation sidebar with route switching and connected servers.

use crate::assets::CustomIconName;
use crate::constants::SIDEBAR_WIDTH;
use crate::helpers::ServerAction;
use crate::states::{
    ConfigState, DfcAppState, DfcGlobalStore, KeysState, Route, i18n_common, i18n_sidebar,
};
use gpui::{AnyElement, Context, Entity, Subscription, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Icon, IconName,
    button::{Button, ButtonVariants},
    label::Label,
    menu::{ContextMenuExt, PopupMenuItem},
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

        // Subscribe to config state changes (for connected config tab)
        subscriptions.push(cx.observe(&config_state, |_this, _model, cx| {
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
        let keys_state = self.keys_state.clone();

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
                    app_state.update(cx, |state, cx| {
                        state.select_server(None, cx);
                    });
                    // Hide keys browser on Home (keep connected servers)
                    keys_state.update(cx, |state, cx| {
                        state.clear_keys(cx);
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

    /// Render all connected config server tabs
    fn render_connected_config_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let server_ids: Vec<String> = self.config_state.read(cx).connected_server_ids().to_vec();

        if server_ids.is_empty() {
            return div().into_any_element();
        }

        let app = self.app_state.read(cx);
        let selected_server_id = app.selected_server_id().map(|s| s.to_string());
        let servers_data: Vec<_> = server_ids
            .iter()
            .enumerate()
            .map(|(index, server_id)| {
                let server_name = app
                    .server(server_id)
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| server_id.clone());
                let is_active = selected_server_id.as_deref() == Some(server_id.as_str());
                (index, server_id.clone(), server_name, is_active)
            })
            .collect();

        let mut items: Vec<AnyElement> = Vec::new();
        for (index, server_id, server_name, is_active) in servers_data {
            let store_for_click = cx.global::<DfcGlobalStore>().clone();
            let server_id_for_click = server_id.clone();
            let config_state = self.config_state.clone();
            let app_state = self.app_state.clone();
            let server_id_for_close = server_id.clone();

            items.push(self.render_server_tab(
                ("config-tab", index),
                ("config-tab-wrapper", index),
                server_id,
                server_name,
                is_active,
                move |_, window, cx| {
                    store_for_click.set_pending_server(server_id_for_click.clone());
                    window.dispatch_action(Box::new(ServerAction::Reconnect), cx);
                },
                move |_, _, cx| {
                    config_state.update(cx, |state, cx| {
                        state.remove_connected_server(&server_id_for_close, cx);
                    });
                    app_state.update(cx, |state, cx| {
                        if state.selected_server_id() == Some(server_id_for_close.as_str()) {
                            state.select_server(None, cx);
                        }
                    });
                },
                cx,
            ));
        }

        v_flex().children(items).into_any_element()
    }

    /// Render a server tab with icon, label, active styling, and context menu.
    /// Shared between config tabs and keys browser tabs.
    fn render_server_tab(
        &self,
        btn_id: impl Into<gpui::ElementId>,
        wrapper_id: impl Into<gpui::ElementId>,
        server_id: String,
        server_name: String,
        is_active: bool,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
        on_close: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let list_active = cx.theme().list_active;
        let list_active_border = cx.theme().list_active_border;
        let store = cx.global::<DfcGlobalStore>().clone();
        let server_id_for_edit = server_id.clone();
        let server_id_for_reconnect = server_id;
        let edit_label = i18n_common(cx, "edit");
        let reconnect_label = i18n_sidebar(cx, "reconnect");
        let close_label = i18n_common(cx, "close");

        let btn = Button::new(btn_id)
            .ghost()
            .w_full()
            .h(px(56.0))
            .tooltip(server_name.clone())
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
            .on_click(on_click);

        div()
            .id(wrapper_id)
            .when(is_active, |this| {
                this.bg(list_active)
                    .border_r_2()
                    .border_color(list_active_border)
            })
            .child(btn)
            .context_menu(move |menu, _, _| {
                let store_for_edit = store.clone();
                let store_for_reconnect = store.clone();
                let sid_edit = server_id_for_edit.clone();
                let sid_reconnect = server_id_for_reconnect.clone();
                let on_close = on_close.clone();

                menu.item(
                    PopupMenuItem::new(edit_label.clone())
                        .icon(Icon::from(CustomIconName::FilePenLine))
                        .on_click(move |_, window, cx| {
                            store_for_edit.set_pending_server(sid_edit.clone());
                            window.dispatch_action(Box::new(ServerAction::Edit), cx);
                        }),
                )
                .item(
                    PopupMenuItem::new(reconnect_label.clone())
                        .icon(Icon::new(IconName::Redo2))
                        .on_click(move |_, window, cx| {
                            store_for_reconnect.set_pending_server(sid_reconnect.clone());
                            window.dispatch_action(Box::new(ServerAction::Reconnect), cx);
                        }),
                )
                .item(
                    PopupMenuItem::new(close_label.clone())
                        .icon(Icon::new(IconName::Close))
                        .on_click(on_close),
                )
            })
            .into_any_element()
    }

    /// Render a connected server item (keys browser)
    fn render_server_item(
        &self,
        index: usize,
        server_id: String,
        server_name: String,
        is_active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let keys_state_for_click = self.keys_state.clone();
        let keys_state_for_close = self.keys_state.clone();
        let server_id_for_click = server_id.clone();
        let server_id_for_close = server_id.clone();

        self.render_server_tab(
            ("server-nav", index),
            ("server-item", index),
            server_id,
            server_name,
            is_active,
            move |_, _, cx| {
                let server_id = server_id_for_click.clone();
                keys_state_for_click.update(cx, |state, cx| {
                    state.set_active_server(Some(server_id), cx);
                });
                cx.update_global::<DfcGlobalStore, ()>(|store, cx| {
                    store.update(cx, |state, cx| {
                        state.go_to(Route::Home, cx);
                    });
                });
            },
            move |_, _, cx| {
                keys_state_for_close.update(cx, |state, cx| {
                    state.remove_connected_server(&server_id_for_close, cx);
                });
            },
            cx,
        )
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
                (
                    index,
                    server.server_id.clone(),
                    server.server_name.clone(),
                    is_active,
                )
            })
            .collect();

        let border_color = cx.theme().border;

        let mut items: Vec<AnyElement> = Vec::new();
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
                    .child(self.render_nav_button(
                        "home",
                        Route::Home,
                        IconName::LayoutDashboard,
                        "home",
                        cx,
                    ))
                    .child(self.render_connected_config_tabs(cx))
                    // Connected servers
                    .child(self.render_connected_servers(cx)),
            )
    }
}
