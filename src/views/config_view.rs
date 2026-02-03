//! Configuration View
//!
//! Displays Redis configuration with left-right split layout:
//! - Left panel: TopicAgentId list
//! - Right panel: Topic tabs for selected TopicAgentId

use crate::assets::CustomIconName;
use crate::connection::{ConfigItem, ConfigLoadState, ConnectedServerInfo};
use crate::states::{ConfigState, DfcAppState, DfcGlobalStore, KeysState};
use gpui::{Action, App, Context, Corner, Entity, Subscription, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Colorize, Icon, IconName, Sizable,
    button::{Button, ButtonVariants, DropdownButton},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    scroll::ScrollableElement,
    tooltip::Tooltip,
    v_flex,
};
use rust_i18n::t;
use schemars::JsonSchema;
use serde::Deserialize;

/// Width of the left agent list panel
const AGENT_LIST_WIDTH: f32 = 320.0;
/// Height of the top bar for left/right panels (keeps alignment)
const PANEL_TOPBAR_HEIGHT: f32 = 56.0;

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
enum AgentQueryMode {
    All,
    Prefix,
    Exact,
}

impl Default for AgentQueryMode {
    fn default() -> Self {
        Self::All
    }
}

/// Configuration view component
pub struct ConfigView {
    /// App state entity
    app_state: Entity<DfcAppState>,
    /// Config state entity
    config_state: Entity<ConfigState>,
    /// Keys state entity
    keys_state: Entity<KeysState>,
    /// Search input state for filtering TopicAgentIds
    agent_search_state: Entity<InputState>,
    agent_query_mode: AgentQueryMode,
    /// Subscriptions
    _subscriptions: Vec<Subscription>,
}

impl ConfigView {
    /// Create a new config view
    pub fn new(
        app_state: Entity<DfcAppState>,
        config_state: Entity<ConfigState>,
        keys_state: Entity<KeysState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        // Create agent search input
        let agent_search_state = cx.new(|cx| {
            let locale = cx.global::<DfcGlobalStore>().read(cx).locale().to_string();
            let placeholder = t!("config.search_agent_placeholder", locale = &locale).to_string();
            InputState::new(window, cx)
                .clean_on_escape()
                .placeholder(placeholder)
        });

        // Subscribe to config state changes
        subscriptions.push(cx.observe(&config_state, |this, _model, cx| {
            let query = this.agent_search_state.read(cx).value().trim().to_string();
            if !query.is_empty() {
                let selected = this
                    .config_state
                    .read(cx)
                    .selected_agent_id()
                    .map(|s| s.to_string());
                if let Some(selected) = selected {
                    if !this.agent_id_matches_query(&selected, &query) {
                        this.config_state.update(cx, |state, cx| {
                            state.select_agent(None, cx);
                        });
                    }
                }
            }
            cx.notify();
        }));

        // Subscribe to agent search input changes for filtering and selection clearing
        subscriptions.push(cx.subscribe(&agent_search_state, |this, state, event, cx| {
            if matches!(event, InputEvent::Change | InputEvent::PressEnter { .. }) {
                let query = state.read(cx).value().trim().to_string();

                if !query.is_empty() {
                    let selected = this
                        .config_state
                        .read(cx)
                        .selected_agent_id()
                        .map(|s| s.to_string());
                    if let Some(selected) = selected {
                        if !this.agent_id_matches_query(&selected, &query) {
                            this.config_state.update(cx, |state, cx| {
                                state.select_agent(None, cx);
                            });
                        }
                    }
                }

                cx.notify();
            }
        }));

        Self {
            app_state,
            config_state,
            keys_state,
            agent_search_state,
            agent_query_mode: AgentQueryMode::default(),
            _subscriptions: subscriptions,
        }
    }

    /// Get the locale string
    fn locale(&self, cx: &App) -> String {
        cx.global::<DfcGlobalStore>().read(cx).locale().to_string()
    }

    fn agent_id_matches_query(&self, agent_id: &str, query: &str) -> bool {
        match self.agent_query_mode {
            AgentQueryMode::All => agent_id.contains(query),
            AgentQueryMode::Prefix => agent_id.starts_with(query),
            AgentQueryMode::Exact => agent_id == query,
        }
    }

    fn clear_selected_agent_if_filtered_out(&self, cx: &mut Context<Self>) {
        let query = self.agent_search_state.read(cx).value().trim().to_string();
        if query.is_empty() {
            return;
        }

        let selected = self
            .config_state
            .read(cx)
            .selected_agent_id()
            .map(|s| s.to_string());
        if let Some(selected) = selected {
            if !self.agent_id_matches_query(&selected, &query) {
                self.config_state.update(cx, |state, cx| {
                    state.select_agent(None, cx);
                });
            }
        }
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

    /// Render a single agent item in the left list
    fn render_agent_item(
        &self,
        index: usize,
        agent_id: String,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let bg = if is_selected {
            cx.theme().accent
        } else {
            cx.theme().background
        };

        let text_color = if is_selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        };

        let hover_color = cx.theme().accent.opacity(0.5);
        let border_color = cx.theme().border;
        let agent_id_for_click = agent_id.clone();

        div()
            .id(("agent-item", index))
            .w_full()
            .px_3()
            .py_2()
            .bg(bg)
            .cursor_pointer()
            .border_b_1()
            .border_color(border_color)
            .hover(|this| this.bg(hover_color))
            .child(
                Label::new(agent_id.clone())
                    .text_sm()
                    .text_color(text_color),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.config_state.update(cx, |state, cx| {
                    state.select_agent(Some(agent_id_for_click.clone()), cx);
                });
            }))
    }

    /// Render the left panel with TopicAgentId list
    fn render_agent_list(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.agent_search_state.read(cx).value().trim().to_string();

        // Collect agent data first to avoid borrow conflicts
        let agents_data: Vec<(usize, String, bool)> = {
            let config_state = self.config_state.read(cx);
            let topic_agents = config_state.topic_agents();
            let selected_agent_id = config_state.selected_agent_id();

            topic_agents
                .iter()
                .filter(|agent| query.is_empty() || self.agent_id_matches_query(&agent.agent_id, &query))
                .enumerate()
                .map(|(idx, agent)| {
                    let is_selected = selected_agent_id == Some(&agent.agent_id);
                    (idx, agent.agent_id.clone(), is_selected)
                })
                .collect()
        };

        let mut agent_items: Vec<gpui::Stateful<gpui::Div>> = Vec::new();
        for (index, agent_id, is_selected) in agents_data {
            agent_items.push(self.render_agent_item(index, agent_id, is_selected, cx));
        }

        let border_color = cx.theme().border;
        let secondary_bg = cx.theme().secondary;

        let locale = self.locale(cx);
        let query_mode_all_label = t!("config.query_mode_all", locale = &locale).to_string();
        let query_mode_prefix_label = t!("config.query_mode_prefix", locale = &locale).to_string();
        let query_mode_exact_label = t!("config.query_mode_exact", locale = &locale).to_string();

        // Select icon based on query mode
        let icon = match self.agent_query_mode {
            AgentQueryMode::All => Icon::new(IconName::Asterisk), // * for all keys
            AgentQueryMode::Prefix => Icon::from(CustomIconName::ChevronUp), // ^ for prefix
            AgentQueryMode::Exact => Icon::from(CustomIconName::Equal), // = for exact match
        };
        let query_mode = self.agent_query_mode;
        let query_mode_dropdown = DropdownButton::new("agent-query-mode-dropdown")
            .button(Button::new("agent-query-mode-btn").ghost().px_2().icon(icon))
            .dropdown_menu_with_anchor(Corner::TopLeft, move |menu, _, _| {
                let query_mode_all_label = query_mode_all_label.clone();
                let query_mode_prefix_label = query_mode_prefix_label.clone();
                let query_mode_exact_label = query_mode_exact_label.clone();

                menu.menu_element_with_check(
                    query_mode == AgentQueryMode::All,
                    Box::new(AgentQueryMode::All),
                    move |_, _cx| Label::new(query_mode_all_label.clone()).ml_2().text_xs(),
                )
                .menu_element_with_check(
                    query_mode == AgentQueryMode::Prefix,
                    Box::new(AgentQueryMode::Prefix),
                    move |_, _cx| Label::new(query_mode_prefix_label.clone()).ml_2().text_xs(),
                )
                .menu_element_with_check(
                    query_mode == AgentQueryMode::Exact,
                    Box::new(AgentQueryMode::Exact),
                    move |_, _cx| Label::new(query_mode_exact_label.clone()).ml_2().text_xs(),
                )
            });

        let search_btn = Button::new("agent-search-btn")
            .ghost()
            .icon(IconName::Search)
            .on_click(cx.listener(|this, _, _, cx| {
                this.clear_selected_agent_if_filtered_out(cx);
                cx.notify();
            }));

        let keyword_input = Input::new(&self.agent_search_state)
            .w_full()
            .flex_1()
            .px_0()
            .prefix(query_mode_dropdown)
            .suffix(search_btn)
            .cleanable(true);

        v_flex()
            .w(px(AGENT_LIST_WIDTH))
            .h_full()
            .bg(secondary_bg)
            // Search input
            .child(
                h_flex()
                    .w_full()
                    .h(px(PANEL_TOPBAR_HEIGHT))
                    .p_2()
                    .items_center()
                    .border_b_1()
                    .border_color(border_color)
                    .child(keyword_input),
            )
            // Agent list
            .child(
                div()
                    .id("agent-list-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .children(agent_items),
            )
    }

    /// Render the right panel with topic tabs for selected agent
    fn render_agent_topics(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);

        // Collect data first to avoid borrow conflicts
        let (agent_info, topic_paths, selected_topic_index): (
            Option<(String, usize)>,
            Vec<String>,
            Option<i32>,
        ) = {
            let config_state = self.config_state.read(cx);
            let selected_topic_idx = config_state.selected_topic_index();
            let selected_agent = config_state.selected_agent();

            match selected_agent {
                Some(agent) => {
                    let info = (agent.agent_id.clone(), agent.topics.len());
                    let topics: Vec<_> = agent.topics.iter().map(|t| t.path.clone()).collect();
                    (Some(info), topics, selected_topic_idx)
                }
                None => (None, Vec::new(), None),
            }
        };

        // No agent selected - show placeholder
        if agent_info.is_none() {
            let no_agent_text = t!("config.no_agent_selected", locale = &locale).to_string();
            let muted_fg = cx.theme().muted_foreground;
            return div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Label::new(no_agent_text)
                        .text_color(muted_fg),
                )
                .into_any_element();
        }

        let (agent_id, topic_count) = agent_info.expect("checked above");

        let muted_fg = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let secondary_bg = cx.theme().secondary;
        let no_topic_selected = t!("config.no_topic_selected", locale = &locale).to_string();

        let selected_topic_index = selected_topic_index.filter(|idx| (*idx as usize) < topic_paths.len());

        // Build tab buttons
        let mut tabs = Vec::new();
        for (pos, path) in topic_paths.iter().enumerate() {
            let label = format!("topic{}", pos + 1);
            let is_selected = selected_topic_index == Some(pos as i32);
            tabs.push(self.render_agent_topic_tab(pos as i32, label, path, is_selected, cx));
        }

        v_flex()
            .flex_1()
            .h_full()
            // Top bar spacer (align with left search bar)
            .child(
                h_flex()
                    .w_full()
                    .h(px(PANEL_TOPBAR_HEIGHT))
                    .bg(secondary_bg)
                    .border_b_1()
                    .border_color(border)
                    .child(div().flex_1()),
            )
            .child(
                v_flex()
                    .flex_1()
                    .p_4()
                    .gap_2()
                    .child(
                        h_flex()
                            .w_full()
                            .items_start()
                            .gap_4()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .flex_none()
                                    .child(Label::new(format!("Agent: {}", agent_id)).text_lg())
                                    .child(
                                        Label::new(format!("{} topics", topic_count))
                                            .text_sm()
                                            .text_color(muted_fg),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .flex_1()
                                    .gap_2()
                                    .overflow_x_scrollbar()
                                    .children(tabs),
                            ),
                    )
                    // Placeholder content area (intentionally blank for now)
                    .child(div().flex_1()),
            )
            // Bottom status bar
            .child(
                h_flex()
                    .h(px(48.0))
                    .items_center()
                    .px_4()
                    .border_t_1()
                    .border_color(border)
                    .bg(secondary_bg)
                    .child(Label::new(no_topic_selected).text_color(muted_fg)),
            )
            .into_any_element()
    }

    /// Render a topic tab for the selected agent
    fn render_agent_topic_tab(
        &self,
        index: i32,
        label: String,
        topic_path: &str,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let tab_bg = if is_selected { cx.theme().secondary } else { cx.theme().background };
        let border_color = if is_selected { cx.theme().accent } else { cx.theme().border };
        let text_color = if is_selected { cx.theme().foreground } else { cx.theme().muted_foreground };
        let tooltip_label = topic_path.to_string();

        div()
            .id(("agent-topic-tab", index as usize))
            .px_4()
            .py_2()
            .bg(tab_bg)
            .cursor_pointer()
            .rounded_md()
            .border_1()
            .border_color(border_color)
            .child(
                Label::new(label)
                    .text_sm()
                    .text_color(text_color),
            )
            .tooltip(move |window, cx| Tooltip::new(tooltip_label.clone()).build(window, cx))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.config_state.update(cx, |state, cx| {
                    state.select_topic(Some(index), cx);
                });
            }))
    }

    /// Render content for selected agent's topic using pre-collected data
    fn render_agent_topic_content_from_data(
        &self,
        topic_data: Option<(i32, String, String)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted_fg = cx.theme().muted_foreground;
        let bg = cx.theme().background;
        let border = cx.theme().border;

        let content = if let Some((_idx, path, topic_type)) = topic_data {
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .gap_2()
                        .child(Label::new("Path:").text_sm().text_color(muted_fg))
                        .child(Label::new(path).text_sm()),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(Label::new("Type:").text_sm().text_color(muted_fg))
                        .child(Label::new(topic_type).text_sm()),
                )
        } else {
            v_flex().child(
                Label::new("No topic selected")
                    .text_color(muted_fg),
            )
        };

        div()
            .p_4()
            .flex_1()
            .bg(bg)
            .border_1()
            .border_color(border)
            .rounded_b_md()
            .rounded_tr_md()
            .child(content)
    }

    /// Render the split panel layout (left: agent list, right: topic tabs)
    fn render_split_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .child(self.render_agent_list(window, cx))
            .child(div().w(px(2.0)).h_full().bg(cx.theme().border))
            .child(self.render_agent_topics(window, cx))
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
        let load_state = self.config_state.read(cx).load_state().clone();
        let (has_configs, has_topic_agents) = {
            let config_state = self.config_state.read(cx);
            (!config_state.configs().is_empty(), !config_state.topic_agents().is_empty())
        };

        match load_state {
            ConfigLoadState::Loading => self.render_loading(cx).into_any_element(),
            ConfigLoadState::Error(msg) => self.render_error(&msg, cx).into_any_element(),
            ConfigLoadState::Loaded | ConfigLoadState::Idle => {
                if has_topic_agents {
                    return self.render_split_panel(window, cx).into_any_element();
                }

                let locale = self.locale(cx);
                let message = if has_configs {
                    t!("config.no_topic_agents", locale = &locale).to_string()
                } else {
                    t!("config.no_config", locale = &locale).to_string()
                };

                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Label::new(message).text_color(cx.theme().muted_foreground))
                    .into_any_element()
            }
        }
    }
}

impl Render for ConfigView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .on_action(cx.listener(|this, mode: &AgentQueryMode, _window, cx| {
                this.agent_query_mode = *mode;
                this.clear_selected_agent_if_filtered_out(cx);
                cx.notify();
            }))
            .child(self.render_content(window, cx))
    }
}
