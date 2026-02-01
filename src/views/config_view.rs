//! Configuration View
//!
//! Displays Redis configuration list and Topic tabs.

use crate::assets::CustomIconName;
use crate::connection::{ConfigItem, ConfigLoadState, ConnectedServerInfo};
use crate::states::{ConfigState, DfcAppState, DfcGlobalStore, KeysState};
use gpui::{App, Context, Entity, Subscription, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Colorize, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    scroll::ScrollableElement,
    v_flex,
};
use rust_i18n::t;


/// Configuration view component
pub struct ConfigView {
    /// App state entity
    app_state: Entity<DfcAppState>,
    /// Config state entity
    config_state: Entity<ConfigState>,
    /// Keys state entity
    keys_state: Entity<KeysState>,
    /// Subscriptions
    _subscriptions: Vec<Subscription>,
}

impl ConfigView {
    /// Create a new config view
    pub fn new(
        app_state: Entity<DfcAppState>,
        config_state: Entity<ConfigState>,
        keys_state: Entity<KeysState>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        // Subscribe to config state changes
        subscriptions.push(cx.observe(&config_state, |_this, _model, cx| {
            cx.notify();
        }));

        Self {
            app_state,
            config_state,
            keys_state,
            _subscriptions: subscriptions,
        }
    }

    /// Get the locale string
    fn locale(&self, cx: &App) -> String {
        cx.global::<DfcGlobalStore>().read(cx).locale().to_string()
    }

    /// Render the header with back button and server info
    fn render_header(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let config_state = self.config_state.read(cx);
        let app_state = self.app_state.read(cx);

        let server_name = app_state
            .selected_server()
            .map(|s| s.name.clone())
            .unwrap_or_default();

        let has_selected_config = config_state.selected_config_id().is_some();

        let back_label = t!("config.back", locale = &locale).to_string();

        let back_btn = Button::new("back-btn")
            .ghost()
            .icon(IconName::ArrowLeft)
            .label(back_label)
            .on_click(cx.listener(move |this, _, _, cx| {
                let config_state = this.config_state.clone();
                let app_state = this.app_state.clone();
                let has_config = config_state.read(cx).selected_config_id().is_some();

                if has_config {
                    // Go back to config list
                    config_state.update(cx, |state, cx| {
                        state.back_to_list(cx);
                    });
                } else {
                    // Go back to server list
                    config_state.update(cx, |state, cx| {
                        state.clear(cx);
                    });
                    app_state.update(cx, |state, cx| {
                        state.select_server(None, cx);
                    });
                }
            }));

        h_flex()
            .w_full()
            .p_2()
            .gap_4()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(back_btn)
            .child(
                Label::new(server_name)
                    .text_lg()
                    .text_color(cx.theme().foreground),
            )
    }

    /// Render loading state
    fn render_loading(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let loading_text = t!("config.loading", locale = &locale).to_string();

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                h_flex()
                    .gap_2()
                    .child(Icon::new(IconName::Loader).size_5())
                    .child(Label::new(loading_text).text_color(cx.theme().muted_foreground)),
            )
    }

    /// Render error state
    fn render_error(&self, message: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let error_title = t!("config.error", locale = &locale).to_string();

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::CircleX).size_5().text_color(cx.theme().danger))
                            .child(Label::new(error_title).text_color(cx.theme().danger)),
                    )
                    .child(
                        Label::new(message.to_string())
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
    }

    /// Render config table row
    fn render_config_row(
        &self,
        index: usize,
        config: &ConfigItem,
        bg: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let group_id = config.group_id;
        let topic_count = config.details.len();
        let locale = self.locale(cx);
        let browse_keys_label = t!("keys.browse_keys", locale = &locale).to_string();
        let config_source = config.source.clone();

        // Browse Keys button
        let keys_state = self.keys_state.clone();
        let app_state = self.app_state.clone();
        let config_source_for_click = config_source.clone();

        let browse_btn = Button::new(("browse-keys", index))
            .ghost()
            .small()
            .icon(Icon::from(CustomIconName::DatabaseZap))
            .tooltip(browse_keys_label)
            .on_click(cx.listener(move |this, _, _, cx| {
                // Stop propagation to prevent row click
                cx.stop_propagation();

                // Get server info
                let server = this.app_state.read(cx).selected_server().cloned();
                let config_source = config_source_for_click.clone();

                if let Some(server) = server {
                    // Add to connected servers
                    let server_info = ConnectedServerInfo {
                        server_id: server.id.clone(),
                        server_name: server.name.clone(),
                        config_source: Some(config_source.clone()),
                    };

                    this.keys_state.update(cx, |state, cx| {
                        state.add_connected_server(server_info, cx);
                        state.set_loading(cx);
                    });

                    // Load keys for this config pattern
                    let keys_state = this.keys_state.clone();
                    let store = cx.global::<DfcGlobalStore>().clone();

                    cx.spawn(async move |_, cx| {
                        let redis = store.services().redis();

                        // Use the config source as pattern or scan all keys
                        // For now, scan all keys with pattern *
                        match redis.scan_keys("*", 0, 100).await {
                            Ok((keys, cursor)) => {
                                tracing::info!("Loaded {} keys, cursor: {}", keys.len(), cursor);
                                let _ = keys_state.update(cx, |state, cx| {
                                    state.set_keys(keys, cursor, cx);
                                });
                            }
                            Err(e) => {
                                tracing::error!("Failed to scan keys: {}", e);
                                let _ = keys_state.update(cx, |state, cx| {
                                    state.set_error(e.to_string(), cx);
                                });
                            }
                        }
                    })
                    .detach();
                }
            }));

        div()
            .id(("config-row", index))
            .w_full()
            .px_4()
            .py_2()
            .bg(bg)
            .cursor_pointer()
            .border_b_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .child(
                h_flex()
                    .w_full()
                    .gap_4()
                    .items_center()
                    // Group ID column
                    .child(
                        div()
                            .w(px(60.0))
                            .child(Label::new(format!("{}", config.group_id)).text_sm()),
                    )
                    // Service URL column
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                Label::new(config.service_url.clone())
                                    .text_sm()
                                    .text_ellipsis(),
                            ),
                    )
                    // Source column
                    .child(
                        div()
                            .w(px(250.0))
                            .overflow_hidden()
                            .child(
                                Label::new(config.source.clone())
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .text_ellipsis(),
                            ),
                    )
                    // Topic count badge
                    .child(
                        div()
                            .w(px(80.0))
                            .child(
                                Label::new(format!("{} topics", topic_count))
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    // Browse keys button
                    .child(browse_btn)
                    // Arrow icon
                    .child(Icon::new(IconName::ChevronRight).size_4().text_color(cx.theme().muted_foreground)),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.config_state.update(cx, |state, cx| {
                    state.select_config(Some(group_id), cx);
                });
            }))
    }

    /// Render the config table header
    fn render_table_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);

        h_flex()
            .w_full()
            .px_4()
            .py_2()
            .bg(cx.theme().secondary)
            .border_b_1()
            .border_color(cx.theme().border)
            .gap_4()
            .child(
                div()
                    .w(px(60.0))
                    .child(
                        Label::new(t!("config.group_id", locale = &locale).to_string())
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .child(
                        Label::new(t!("config.service_url", locale = &locale).to_string())
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                div()
                    .w(px(250.0))
                    .child(
                        Label::new(t!("config.source", locale = &locale).to_string())
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                div()
                    .w(px(80.0))
                    .child(
                        Label::new(t!("config.topics", locale = &locale).to_string())
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(div().w(px(16.0))) // Spacer for arrow
    }

    /// Render the configuration list table
    fn render_config_table(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let config_state = self.config_state.read(cx);
        let configs: Vec<_> = config_state.configs().to_vec();

        // Alternate row colors
        let bg_even = if cx.theme().is_dark() {
            cx.theme().background.lighten(0.5)
        } else {
            cx.theme().background.darken(0.01)
        };
        let bg_odd = cx.theme().background;

        let mut rows = Vec::new();
        for (index, config) in configs.iter().enumerate() {
            let bg = if index % 2 == 0 { bg_even } else { bg_odd };
            rows.push(self.render_config_row(index, config, bg, cx));
        }

        v_flex()
            .size_full()
            .child(self.render_table_header(cx))
            .child(
                div()
                    .id("config-table-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .children(rows),
            )
    }

    /// Render topic tab item
    fn render_topic_tab(
        &self,
        index: i32,
        path: &str,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let tab_bg = if is_selected {
            cx.theme().accent
        } else {
            cx.theme().secondary
        };

        let text_color = if is_selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().muted_foreground
        };

        // Extract short name from path
        let short_name = path
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .chars()
            .take(20)
            .collect::<String>();

        div()
            .id(("topic-tab", index as usize))
            .px_3()
            .py_1()
            .bg(tab_bg)
            .cursor_pointer()
            .rounded_t_md()
            .border_1()
            .border_color(cx.theme().border)
            .when(!is_selected, |this| this.border_b_0())
            .child(
                Label::new(short_name)
                    .text_sm()
                    .text_color(text_color),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.config_state.update(cx, |state, cx| {
                    state.select_topic(Some(index), cx);
                });
            }))
    }

    /// Render topic content
    fn render_topic_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let config_state = self.config_state.read(cx);

        let content = if let Some(topic) = config_state.selected_topic() {
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .gap_2()
                        .child(Label::new("Path:").text_sm().text_color(cx.theme().muted_foreground))
                        .child(Label::new(topic.path.clone()).text_sm()),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(Label::new("Visibility:").text_sm().text_color(cx.theme().muted_foreground))
                        .child(Label::new(if topic.visibility { "Visible" } else { "Hidden" }).text_sm()),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(Label::new("Index:").text_sm().text_color(cx.theme().muted_foreground))
                        .child(Label::new(format!("{}", topic.index)).text_sm()),
                )
        } else {
            v_flex().child(
                Label::new("No topic selected")
                    .text_color(cx.theme().muted_foreground),
            )
        };

        div()
            .p_4()
            .flex_1()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_b_md()
            .rounded_tr_md()
            .child(content)
    }

    /// Render topic tabs view
    fn render_topic_tabs(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let config_state = self.config_state.read(cx);
        let selected_topic_index = config_state.selected_topic_index().unwrap_or(0);

        let details = config_state
            .selected_config()
            .map(|c| c.details.clone())
            .unwrap_or_default();

        let config_info = config_state
            .selected_config()
            .map(|c| (c.service_url.clone(), c.source.clone()))
            .unwrap_or_default();

        // Build tab buttons
        let mut tabs = Vec::new();
        for detail in &details {
            let is_selected = detail.index == selected_topic_index;
            tabs.push(self.render_topic_tab(detail.index, &detail.path, is_selected, cx));
        }

        v_flex()
            .size_full()
            .p_4()
            .gap_2()
            // Config info header
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        Label::new(config_info.0)
                            .text_lg(),
                    )
                    .child(
                        Label::new(config_info.1)
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            // Tab bar
            .child(
                h_flex()
                    .gap_1()
                    .overflow_x_scrollbar()
                    .children(tabs),
            )
            // Tab content
            .child(self.render_topic_content(cx))
    }

    /// Render the main content based on state
    fn render_content(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Clone the state values first to avoid borrow issues
        let load_state = self.config_state.read(cx).load_state().clone();
        let selected_config_id = self.config_state.read(cx).selected_config_id();

        match load_state {
            ConfigLoadState::Loading => self.render_loading(cx).into_any_element(),
            ConfigLoadState::Error(msg) => self.render_error(&msg, cx).into_any_element(),
            ConfigLoadState::Loaded | ConfigLoadState::Idle => {
                if selected_config_id.is_some() {
                    // Show topic tabs
                    self.render_topic_tabs(window, cx).into_any_element()
                } else {
                    // Show config table
                    self.render_config_table(window, cx).into_any_element()
                }
            }
        }
    }
}

impl Render for ConfigView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(self.render_header(window, cx))
            .child(self.render_content(window, cx))
    }
}
