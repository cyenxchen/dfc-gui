//! Configuration View
//!
//! Displays Redis configuration with left-right split layout:
//! - Left panel: TopicAgentId list
//! - Right panel: Topic tabs for selected TopicAgentId

use crate::assets::CustomIconName;
use crate::connection::{ConfigItem, ConfigLoadState, ConnectedServerInfo};
use crate::services::spawn_named_in_tokio;
use crate::states::{ConfigState, DfcAppState, DfcGlobalStore, KeysState, PropRow, PropTableLoadState, PropTableState};
use chrono::Local;
use crossbeam_channel::{Receiver, Sender};
use futures::StreamExt;
use gpui::{Action, App, Context, Corner, Entity, StatefulInteractiveElement as _, Subscription, Task, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Colorize, Disableable, Icon, IconName, Sizable,
    button::{Button, ButtonVariants, DropdownButton},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    tooltip::Tooltip,
    v_flex,
};
use prost::Message as _;
use rust_i18n::t;
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// Width of the left agent list panel
const AGENT_LIST_WIDTH: f32 = 320.0;
/// Height of the top bar for left/right panels (keeps alignment)
const PANEL_TOPBAR_HEIGHT: f32 = 48.0;

#[derive(Debug)]
enum PropStreamEvent {
    Rows(Vec<PropRow>),
    Error(String),
}

fn topic_display_name(topic_path: &str) -> String {
    if topic_path.contains("thing_service-BZ-RESPONSE") && topic_path.contains("thing_service-BZ-REQUEST") {
        return "service".to_string();
    }

    if let Some(start) = topic_path.find("prop_data-BZ-") {
        let after_prefix = start + "prop_data-BZ-".len();
        if let Some(realdev_pos) = topic_path[after_prefix..].find("-realdev-") {
            let a = &topic_path[after_prefix..after_prefix + realdev_pos];
            let b_start = after_prefix + realdev_pos + "-realdev-".len();
            if let Some(last_dash) = topic_path.rfind('-') {
                if last_dash > b_start {
                    let b = &topic_path[b_start..last_dash];
                    if !a.is_empty() && !b.is_empty() {
                        return format!("{a}_{b}");
                    }
                }
            }
        }
    }

    let mut last = topic_path.rsplit('/').next().unwrap_or(topic_path).to_string();
    if let Some(last_dash) = last.rfind('-') {
        let tail = &last[last_dash + 1..];
        if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
            last.truncate(last_dash);
        }
    }
    last
}

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
enum AgentQueryMode {
    All,
    Prefix,
    Exact,
}

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
enum PropPageSize {
    S10,
    S20,
    S50,
    S100,
}

impl PropPageSize {
    fn value(self) -> usize {
        match self {
            Self::S10 => 10,
            Self::S20 => 20,
            Self::S50 => 50,
            Self::S100 => 100,
        }
    }

    fn from_value(value: usize) -> Self {
        match value {
            10 => Self::S10,
            50 => Self::S50,
            100 => Self::S100,
            _ => Self::S20,
        }
    }
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
    /// Prop topic table state (for `prop_data` topics)
    prop_table_state: Entity<PropTableState>,
    /// Active prop topic path (to avoid restarting streams on every notify)
    active_prop_topic: Option<String>,
    /// Stop signal for the active prop topic stream (tokio)
    prop_stream_stop: Option<watch::Sender<bool>>,
    /// UI-side ingest task draining prop rows from channel
    prop_ingest_task: Option<Task<()>>,
    /// Monotonic row id generator
    prop_row_uid: Arc<AtomicU64>,
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

        // Prop topic table state
        let prop_table_state = cx.new(|_| PropTableState::new());
        let prop_row_uid = Arc::new(AtomicU64::new(1));

        subscriptions.push(cx.observe(&prop_table_state, |_this, _model, cx| {
            cx.notify();
        }));

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
            this.sync_prop_stream_with_selection(cx);
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
            prop_table_state,
            active_prop_topic: None,
            prop_stream_stop: None,
            prop_ingest_task: None,
            prop_row_uid,
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

    fn stop_prop_stream(&mut self) {
        if let Some(stop) = self.prop_stream_stop.take() {
            let _ = stop.send(true);
        }
        if let Some(task) = self.prop_ingest_task.take() {
            drop(task);
        }
        self.active_prop_topic = None;
    }

    fn is_prop_topic_path(topic_path: &str) -> bool {
        topic_path.contains("prop_data-BZ-")
    }

    fn sync_prop_stream_with_selection(&mut self, cx: &mut Context<Self>) {
        let selected_topic_path: Option<String> = {
            let state = self.config_state.read(cx);
            match (state.selected_agent(), state.selected_topic_index()) {
                (Some(agent), Some(idx)) => agent.topics.get(idx as usize).map(|t| t.path.clone()),
                _ => None,
            }
        };

        let selected_prop_topic = selected_topic_path
            .as_deref()
            .filter(|path| Self::is_prop_topic_path(path))
            .map(|s| s.to_string());

        if self.active_prop_topic == selected_prop_topic {
            return;
        }

        // Stop any existing stream and reset state if needed.
        self.stop_prop_stream();

        let Some(topic_path) = selected_prop_topic else {
            let _ = self.prop_table_state.update(cx, |state, cx| {
                state.reset_for_topic(None);
                cx.notify();
            });
            return;
        };

        // Locate the Pulsar service URL + cfgid for this topic.
        let (service_url, cfgid) = {
            let config_state = self.config_state.read(cx);
            match find_topic_origin(config_state.configs(), &topic_path) {
                Some(v) => v,
                None => {
                    let _ = self.prop_table_state.update(cx, |state, cx| {
                        state.reset_for_topic(Some(topic_path.clone()));
                        state.set_error("无法定位该 Topic 对应的 service_url/cfgid");
                        cx.notify();
                    });
                    return;
                }
            }
        };

        let token = cx
            .global::<DfcGlobalStore>()
            .read(cx)
            .selected_server()
            .and_then(|s| s.pulsar_token.clone())
            .filter(|t| !t.trim().is_empty());

        self.active_prop_topic = Some(topic_path.clone());

        let _ = self.prop_table_state.update(cx, |state, cx| {
            state.reset_for_topic(Some(topic_path.clone()));
            cx.notify();
        });

        self.start_prop_stream(service_url, cfgid, topic_path, token, cx);
    }

    fn start_prop_stream(
        &mut self,
        service_url: String,
        cfgid: String,
        topic_path: String,
        token: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let (tx, rx): (Sender<PropStreamEvent>, Receiver<PropStreamEvent>) =
            crossbeam_channel::unbounded();
        let (stop_tx, stop_rx) = watch::channel(false);
        self.prop_stream_stop = Some(stop_tx);

        let redis = cx.global::<DfcGlobalStore>().services().redis().clone();
        let uid = self.prop_row_uid.clone();

        spawn_named_in_tokio("prop-topic-stream", async move {
            run_prop_topic_stream(service_url, topic_path, token, cfgid, redis, stop_rx, tx, uid)
                .await;
        });

        let prop_state = self.prop_table_state.clone();
        let task = cx.spawn(async move |_, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;

            let mut rows: Vec<PropRow> = Vec::new();
            let mut error: Option<String> = None;

            while let Ok(ev) = rx.try_recv() {
                match ev {
                    PropStreamEvent::Rows(mut batch) => rows.append(&mut batch),
                    PropStreamEvent::Error(msg) => error = Some(msg),
                }
            }

            if let Some(msg) = error {
                let _ = prop_state.update(cx, |state, cx| {
                    state.set_error(msg);
                    cx.notify();
                });
                continue;
            }

            if rows.is_empty() {
                continue;
            }

            let _ = prop_state.update(cx, |state, cx| {
                state.push_rows_front(rows);
                cx.notify();
            });
        });

        self.prop_ingest_task = Some(task);
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
        topic_count: usize,
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

        let count_color = if is_selected {
            cx.theme().accent_foreground.opacity(0.9)
        } else {
            cx.theme().muted_foreground
        };

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
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        Label::new(agent_id.clone())
                            .text_sm()
                            .text_color(text_color)
                            .text_ellipsis()
                            .flex_1(),
                    )
                    .child(
                        Label::new(format!("{} topics", topic_count))
                            .text_xs()
                            .text_color(count_color),
                    ),
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
        let agents_data: Vec<(usize, String, usize, bool)> = {
            let config_state = self.config_state.read(cx);
            let topic_agents = config_state.topic_agents();
            let selected_agent_id = config_state.selected_agent_id();

            topic_agents
                .iter()
                .filter(|agent| query.is_empty() || self.agent_id_matches_query(&agent.agent_id, &query))
                .enumerate()
                .map(|(idx, agent)| {
                    let is_selected = selected_agent_id == Some(&agent.agent_id);
                    (idx, agent.agent_id.clone(), agent.topics.len(), is_selected)
                })
                .collect()
        };

        let mut agent_items: Vec<gpui::Stateful<gpui::Div>> = Vec::new();
        for (index, agent_id, topic_count, is_selected) in agents_data {
            agent_items.push(self.render_agent_item(index, agent_id, topic_count, is_selected, cx));
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

        let (_agent_id, _topic_count) = agent_info.expect("checked above");

        let muted_fg = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let secondary_bg = cx.theme().secondary;
        let no_topic_selected = t!("config.no_topic_selected", locale = &locale).to_string();

        let selected_topic_index = selected_topic_index.filter(|idx| (*idx as usize) < topic_paths.len());
        let selected_topic_path = selected_topic_index
            .and_then(|idx| topic_paths.get(idx as usize))
            .cloned();
        let is_prop_topic = selected_topic_path
            .as_deref()
            .map(Self::is_prop_topic_path)
            .unwrap_or(false);

        // Build tab buttons
        let mut tabs = Vec::new();
        for (pos, path) in topic_paths.iter().enumerate() {
            let label = topic_display_name(path);
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
                    .items_center()
                    .px_4()
                    .gap_2()
                    .bg(secondary_bg)
                    .border_b_1()
                    .border_color(border)
                    .child(
                        h_flex()
                            .id("agent-tabs-scroll")
                            .flex_1()
                            .gap_2()
                            .flex_nowrap()
                            .justify_center()
                            .overflow_x_scroll()
                            .children(tabs),
                    ),
            )
            // Content area
            .child(
                div()
                    .flex_1()
                    .child(match (selected_topic_path.as_deref(), is_prop_topic) {
                        (Some(topic_path), true) => self.render_prop_table(topic_path, cx).into_any_element(),
                        (Some(topic_path), false) => self.render_unsupported_topic(topic_path, cx).into_any_element(),
                        (None, _) => div().flex_1().into_any_element(),
                    }),
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
                    .child(if is_prop_topic {
                        self.render_prop_pagination(cx).into_any_element()
                    } else {
                        Label::new(no_topic_selected)
                            .text_color(muted_fg)
                            .into_any_element()
                    }),
            )
            .into_any_element()
    }

    fn render_unsupported_topic(&self, topic_path: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let muted_fg = cx.theme().muted_foreground;
        let border = cx.theme().border;

        v_flex()
            .flex_1()
            .p_4()
            .gap_2()
            .child(
                Label::new("当前仅实现 prop_data Topic 的内容展示")
                    .text_color(muted_fg),
            )
            .child(
                div()
                    .border_1()
                    .border_color(border)
                    .rounded_md()
                    .p_3()
                    .child(
                        Label::new(topic_path.to_string())
                            .text_sm()
                            .text_color(muted_fg)
                            .text_ellipsis(),
                    ),
            )
    }

    fn render_prop_table(&self, selected_topic_path: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let header_bg = cx.theme().secondary;
        let muted_fg = cx.theme().muted_foreground;

        let (topic_path, load_state, page_rows, total_rows) = {
            let state = self.prop_table_state.read(cx);
            (
                state.topic_path().map(|s| s.to_string()),
                state.load_state().clone(),
                state.page_rows().cloned().collect::<Vec<PropRow>>(),
                state.rows_len(),
            )
        };

        if topic_path.as_deref() != Some(selected_topic_path) {
            return div()
                .flex_1()
                .p_4()
                .child(Label::new("正在切换 Topic…").text_color(muted_fg))
                .into_any_element();
        }

        match &load_state {
            PropTableLoadState::Error(msg) => {
                return div()
                    .flex_1()
                    .p_4()
                    .child(Label::new(format!("加载失败: {msg}")).text_color(cx.theme().danger))
                    .into_any_element();
            }
            PropTableLoadState::Loading if total_rows == 0 => {
                return div()
                    .flex_1()
                    .p_4()
                    .child(Label::new("等待数据…").text_color(muted_fg))
                    .into_any_element();
            }
            _ => {}
        }

        // Build rows
        let mut rows = Vec::new();
        for (idx, row) in page_rows.iter().enumerate() {
            let bg = if idx % 2 == 0 {
                if cx.theme().is_dark() {
                    cx.theme().background.lighten(0.3)
                } else {
                    cx.theme().background.darken(0.01)
                }
            } else {
                cx.theme().background
            };

            rows.push(
                h_flex()
                    .id(("prop-row", row.uid as usize))
                    .w_full()
                    .bg(bg)
                    .border_b_1()
                    .border_color(border)
                    .child(self.render_prop_cell(180.0, &row.global_uuid, cx))
                    .child(self.render_prop_cell(110.0, &row.device, cx))
                    .child(self.render_prop_cell(320.0, &row.imr, cx))
                    .child(self.render_prop_cell(90.0, &row.imid.to_string(), cx))
                    .child(self.render_prop_cell(120.0, &row.value, cx))
                    .child(self.render_prop_cell(90.0, &row.quality.to_string(), cx))
                    .child(self.render_prop_cell(140.0, &row.bcrid, cx))
                    .child(self.render_prop_cell(180.0, &row.time, cx))
                    .child(self.render_prop_cell(180.0, &row.message_time, cx))
                    .child(self.render_prop_cell(240.0, &row.summary, cx)),
            );
        }

        // Horizontal scroll wrapper
        div()
            .flex_1()
            .p_3()
            .child(
                div()
                    .w_full()
                    .h_full()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .overflow_hidden()
                    .child(
                        div()
                            .id("prop-table-x-scroll")
                            .flex_1()
                            .overflow_x_scroll()
                            .child(
                                v_flex()
                                    .min_w(px(1_650.0))
                                    .w(px(1_650.0))
                                    // Header
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .bg(header_bg)
                                            .border_b_1()
                                            .border_color(border)
                                            .child(self.render_prop_header_cell(180.0, "全局UUID", cx))
                                            .child(self.render_prop_header_cell(110.0, "设备号", cx))
                                            .child(self.render_prop_header_cell(320.0, "IMR", cx))
                                            .child(self.render_prop_header_cell(90.0, "IMID", cx))
                                            .child(self.render_prop_header_cell(120.0, "值", cx))
                                            .child(self.render_prop_header_cell(90.0, "数据质量", cx))
                                            .child(self.render_prop_header_cell(140.0, "BCRID", cx))
                                            .child(self.render_prop_header_cell(180.0, "数据时间", cx))
                                            .child(self.render_prop_header_cell(180.0, "报文时间", cx))
                                            .child(self.render_prop_header_cell(240.0, "报文摘要", cx)),
                                    )
                                    // Body
                                    .child(
                                        div()
                                            .id("prop-table-y-scroll")
                                            .flex_1()
                                            .overflow_y_scroll()
                                            .children(rows),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_prop_header_cell(&self, w: f32, text: &str, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(w))
            .px_2()
            .py_2()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                Label::new(text.to_string())
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .text_ellipsis(),
            )
    }

    fn render_prop_cell(&self, w: f32, text: &str, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(w))
            .px_2()
            .py_2()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                Label::new(text.to_string())
                    .text_sm()
                    .text_ellipsis(),
            )
    }

    fn render_prop_pagination(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (total, start, end, pages, page_index, page_size) = {
            let state = self.prop_table_state.read(cx);
            let total = state.rows_len();
            let (start, end) = state.page_range();
            (
                total,
                start,
                end,
                state.total_pages(),
                state.page_index(),
                state.page_size(),
            )
        };

        let display_start = if total == 0 { 0 } else { start + 1 };
        let display_end = if total == 0 { 0 } else { end };

        let info = format!("显示第 {display_start} 到第 {display_end} 条记录，总共 {total} 条记录");
        let page_label = format!("第 {} / {} 页", page_index + 1, pages);

        let current_size = PropPageSize::from_value(page_size);
        let dropdown = DropdownButton::new("prop-page-size-dropdown")
            .button(
                Button::new("prop-page-size-btn")
                    .ghost()
                    .compact()
                    .label(format!("{page_size}")),
            )
            .dropdown_menu_with_anchor(Corner::TopLeft, move |menu, _, _| {
                let menu = menu
                    .menu_element_with_check(
                        current_size == PropPageSize::S10,
                        Box::new(PropPageSize::S10),
                        move |_, _cx| Label::new("10").ml_2().text_xs(),
                    )
                    .menu_element_with_check(
                        current_size == PropPageSize::S20,
                        Box::new(PropPageSize::S20),
                        move |_, _cx| Label::new("20").ml_2().text_xs(),
                    )
                    .menu_element_with_check(
                        current_size == PropPageSize::S50,
                        Box::new(PropPageSize::S50),
                        move |_, _cx| Label::new("50").ml_2().text_xs(),
                    )
                    .menu_element_with_check(
                        current_size == PropPageSize::S100,
                        Box::new(PropPageSize::S100),
                        move |_, _cx| Label::new("100").ml_2().text_xs(),
                    );
                menu
            });

        let prev_disabled = page_index == 0;
        let next_disabled = page_index + 1 >= pages;

        let prev_btn = Button::new("prop-page-prev")
            .ghost()
            .icon(IconName::ChevronLeft)
            .disabled(prev_disabled)
            .on_click(cx.listener(|this, _, _, cx| {
                this.prop_table_state.update(cx, |state, cx| {
                    let current = state.page_index();
                    state.set_page_index(current.saturating_sub(1));
                    cx.notify();
                });
            }));

        let next_btn = Button::new("prop-page-next")
            .ghost()
            .icon(IconName::ChevronRight)
            .disabled(next_disabled)
            .on_click(cx.listener(|this, _, _, cx| {
                this.prop_table_state.update(cx, |state, cx| {
                    let current = state.page_index();
                    state.set_page_index(current + 1);
                    cx.notify();
                });
            }));

        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(Label::new(info).text_xs().text_color(cx.theme().muted_foreground))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(Label::new("每页显示").text_xs().text_color(cx.theme().muted_foreground))
                            .child(dropdown)
                            .child(Label::new("条记录").text_xs().text_color(cx.theme().muted_foreground)),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(prev_btn)
                    .child(Label::new(page_label).text_xs().text_color(cx.theme().muted_foreground))
                    .child(next_btn),
            )
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
        let tab_bg = if is_selected {
            cx.theme().primary.opacity(0.12)
        } else {
            cx.theme().background
        };
        let border_color = if is_selected { cx.theme().primary } else { cx.theme().border };
        let text_color = if is_selected { cx.theme().foreground } else { cx.theme().muted_foreground };
        let tooltip_label = topic_path.to_string();

        div()
            .id(("agent-topic-tab", index as usize))
            .px_4()
            .py_2()
            .bg(tab_bg)
            .cursor_pointer()
            .flex_none()
            .rounded_md()
            .border_2()
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
                    .id("config-tabs-scroll")
                    .gap_1()
                    .flex_nowrap()
                    .overflow_x_scroll()
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

fn find_topic_origin(configs: &[ConfigItem], topic_path: &str) -> Option<(String, String)> {
    for config in configs {
        for agent in &config.topic_agents {
            for topic in &agent.topics {
                if topic.path == topic_path {
                    let cfgid = extract_cfgid_from_source(&config.source)?;
                    return Some((config.service_url.clone(), cfgid));
                }
            }
        }
    }
    None
}

fn extract_cfgid_from_source(source: &str) -> Option<String> {
    if !source.starts_with("CMC_") {
        return None;
    }

    if let (Some(start), Some(end)) = (source.find('{'), source.find('}')) {
        if start < end {
            let inner = &source[start + 1..end];
            if !inner.is_empty() {
                return Some(inner.to_string());
            }
        }
    }

    if let Some(sg_pos) = source.find("_sg") {
        let raw = &source["CMC_".len()..sg_pos];
        let trimmed = raw.trim_start_matches('{').trim_end_matches('}');
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    None
}

fn split_summary_and_proto(data: &[u8]) -> (String, &[u8]) {
    if data.len() < 3 {
        return (String::new(), data);
    }

    let summary_len = data[2] as usize;
    let start = 3usize.saturating_add(summary_len);
    if start >= data.len() {
        return (String::new(), data);
    }

    let summary = if summary_len == 0 {
        String::new()
    } else {
        String::from_utf8_lossy(&data[3..start]).to_string()
    };

    (summary, &data[start..])
}

fn format_clock_time(clock: Option<&crate::proto::iothub::ClockTime>) -> String {
    let Some(clock) = clock else {
        return String::new();
    };

    let secs = i64::from(clock.t);
    let Some(dt_utc) = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0) else {
        return String::new();
    };

    dt_utc
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string()
}

fn any_value_to_string(v: Option<&crate::proto::iothub::AnyValue>) -> String {
    let Some(v) = v else {
        return String::new();
    };

    use crate::proto::iothub::any_value::V;
    match v.v.as_ref() {
        Some(V::DoubleV(x)) => x.to_string(),
        Some(V::FloatV(x)) => x.to_string(),
        Some(V::Int32V(x)) => x.to_string(),
        Some(V::Uint32V(x)) => x.to_string(),
        Some(V::Uint64V(x)) => x.to_string(),
        Some(V::Sint32V(x)) => x.to_string(),
        Some(V::Sint64V(x)) => x.to_string(),
        Some(V::Fixed32V(x)) => x.to_string(),
        Some(V::Fixed64V(x)) => x.to_string(),
        Some(V::Sfixed32V(x)) => x.to_string(),
        Some(V::Sfixed64V(x)) => x.to_string(),
        Some(V::BoolV(x)) => x.to_string(),
        Some(V::StringV(s)) => s.clone(),
        Some(V::BytesV(b)) => format!("{} bytes", b.len()),
        Some(V::AnyV(a)) => format!("any({}) {} bytes", a.type_url, a.value.len()),
        Some(V::NullV(_)) => String::new(),
        Some(V::JsonV(s)) => s.clone(),
        Some(V::MsgPackV(b)) => format!("msgpack {} bytes", b.len()),
        None => String::new(),
    }
}

fn parse_prop_rows_from_payload(
    payload: &[u8],
    imid2imr: &std::collections::HashMap<(String, u32), String>,
    uid: &AtomicU64,
) -> Vec<PropRow> {
    let (summary, proto) = split_summary_and_proto(payload);

    let df = match crate::proto::iothub::DataFrame::decode(proto) {
        Ok(df) => df,
        Err(e) => {
            tracing::debug!("Failed to decode DataFrame: {}", e);
            return Vec::new();
        }
    };

    let mut out = Vec::new();
    for set in df.frame {
        let Some(header) = set.header.as_ref() else {
            continue;
        };

        let global_uuid = header.im_global_uuid.clone();
        let device = header.source_device.clone();
        let message_time = format_clock_time(header.t.as_ref());

        for record in &set.data {
            let (imid, imr) = match record.k.as_ref() {
                Some(crate::proto::iothub::data_record::K::Im2id(id)) => {
                    let key = (global_uuid.clone(), *id);
                    let imr = imid2imr
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| "Unknown Imr".to_string());
                    (i32::try_from(*id).unwrap_or(0), imr)
                }
                Some(crate::proto::iothub::data_record::K::Imr(imr_ref)) => (0, imr_ref.path.clone()),
                None => (0, "Unknown Imr".to_string()),
            };

            let time = format_clock_time(record.device_time.as_ref());

            out.push(PropRow {
                uid: uid.fetch_add(1, Ordering::Relaxed),
                global_uuid: global_uuid.clone(),
                device: device.clone(),
                imr,
                imid,
                value: any_value_to_string(record.v.as_ref()),
                quality: i32::try_from(record.q).unwrap_or(0),
                bcrid: record.bcr_uuid.clone(),
                time,
                message_time: message_time.clone(),
                summary: summary.clone(),
            });
        }
    }

    out
}

async fn run_prop_topic_stream(
    service_url: String,
    topic_path: String,
    token: Option<String>,
    cfgid: String,
    redis: Arc<crate::services::RedisRepo>,
    mut stop: watch::Receiver<bool>,
    tx: Sender<PropStreamEvent>,
    uid: Arc<AtomicU64>,
) {
    let imid2imr = match redis.fetch_imid2imr(&cfgid).await {
        Ok(map) => map,
        Err(e) => {
            tracing::warn!("Failed to load IMID->IMR mapping: {}", e);
            std::collections::HashMap::new()
        }
    };

    let mut builder = pulsar::Pulsar::builder(service_url.clone(), pulsar::TokioExecutor);
    if let Some(token) = token {
        builder = builder.with_auth(pulsar::Authentication {
            name: "token".to_string(),
            data: token.into_bytes(),
        });
    }

    let client: pulsar::Pulsar<_> = match builder.build().await {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(PropStreamEvent::Error(format!("Pulsar 连接失败: {e}")));
            return;
        }
    };

    let mut reader: pulsar::reader::Reader<Vec<u8>, _> = match client
        .reader()
        .with_topic(&topic_path)
        .with_subscription("dfc-gui-prop-reader")
        .with_consumer_name("dfc-gui-prop-reader")
        .into_reader()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(PropStreamEvent::Error(format!("创建 Reader 失败: {e}")));
            return;
        }
    };

    // Align with DFC default: seek to last 20 minutes for persistent topics.
    if topic_path.starts_with("persistent://") {
        let now_ms = chrono::Utc::now().timestamp_millis();
        if now_ms > 0 {
            let seek_ms = now_ms.saturating_sub(20 * 60 * 1000) as u64;
            let _ = reader.seek(None, Some(seek_ms)).await;
        }
    }

    loop {
        if *stop.borrow() {
            break;
        }

        tokio::select! {
            _ = stop.changed() => {
                if *stop.borrow() {
                    break;
                }
            }
            msg = reader.next() => {
                match msg {
                    Some(Ok(message)) => {
                        let data = message.deserialize();
                        let rows = parse_prop_rows_from_payload(&data, &imid2imr, &uid);
                        if !rows.is_empty() {
                            let _ = tx.send(PropStreamEvent::Rows(rows));
                        }
                    }
                    Some(Err(e)) => {
                        let _ = tx.send(PropStreamEvent::Error(format!("读取消息失败: {e}")));
                        break;
                    }
                    None => break,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::topic_display_name;

    #[test]
    fn topic_display_name_prop_data_rules() {
        assert_eq!(
            topic_display_name("persistent://goldwind/iothub/prop_data-BZ-GRID-realdev-Guarantee-626221420272574464"),
            "GRID_Guarantee"
        );
        assert_eq!(
            topic_display_name("non-persistent://goldwind/iothub/prop_data-BZ-FAST-realdev-Guarantee-626221420272574464"),
            "FAST_Guarantee"
        );
        assert_eq!(
            topic_display_name("persistent://goldwind/iothub/prop_data-BZ-GRID_SECTION-realdev-60-626221420272574464"),
            "GRID_SECTION_60"
        );
        assert_eq!(
            topic_display_name("persistent://goldwind/iothub/prop_data-BZ-GRID_SECTION-realdev-WindPower-626221420272574464"),
            "GRID_SECTION_WindPower"
        );
    }

    #[test]
    fn topic_display_name_service_rule() {
        assert_eq!(
            topic_display_name(
                "persistent://goldwind/iothub/thing_service-BZ-RESPONSE-626221420272574464,persistent://goldwind/iothub/thing_service-BZ-REQUEST-626221420272574464"
            ),
            "service"
        );
    }

    #[test]
    fn topic_display_name_fallback_is_stable_and_not_topic_n() {
        assert_eq!(
            topic_display_name("persistent://goldwind/iothub/thing_event-BZ-626221420272574464"),
            "thing_event-BZ"
        );
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
            .on_action(cx.listener(|this, size: &PropPageSize, _window, cx| {
                let page_size = size.value();
                this.prop_table_state.update(cx, |state, cx| {
                    state.set_page_size(page_size);
                    cx.notify();
                });
            }))
            .child(self.render_content(window, cx))
    }
}
