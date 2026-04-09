//! Keys Browser View
//!
//! A three-column layout for browsing Redis keys:
//! - Left: Search input + Keys list with type badges
//! - Right: Selected key's value display

use crate::connection::{RedisKeyItem, RedisKeyType, RedisKeyValue};
use crate::states::{DfcGlobalStore, KeysState};
use gpui::{App, Context, Entity, Subscription, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Colorize, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    v_flex,
};
use rust_i18n::t;

/// Keys browser view component
pub struct KeysBrowserView {
    /// Keys state entity
    keys_state: Entity<KeysState>,
    /// Search input state
    search_state: Entity<InputState>,
    /// Subscriptions
    _subscriptions: Vec<Subscription>,
}

impl KeysBrowserView {
    /// Create a new keys browser view
    pub fn new(
        keys_state: Entity<KeysState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        // Subscribe to keys state changes
        subscriptions.push(cx.observe(&keys_state, |_this, _model, cx| {
            cx.notify();
        }));

        // Create search input
        let search_state = cx.new(|cx| {
            let locale = cx.global::<DfcGlobalStore>().read(cx).locale().to_string();
            let placeholder = t!("keys.search_placeholder", locale = &locale).to_string();
            InputState::new(window, cx)
                .clean_on_escape()
                .placeholder(placeholder)
        });

        // Subscribe to search input for filtering
        let keys_state_clone = keys_state.clone();
        subscriptions.push(cx.subscribe(&search_state, move |_this, state, event, cx| {
            if matches!(event, InputEvent::Change) {
                let pattern = state.read(cx).value().to_string();
                keys_state_clone.update(cx, |state, cx| {
                    state.set_filter_pattern(pattern, cx);
                });
            }
        }));

        Self {
            keys_state,
            search_state,
            _subscriptions: subscriptions,
        }
    }

    /// Get the locale string
    fn locale(&self, cx: &App) -> String {
        cx.global::<DfcGlobalStore>().read(cx).locale().to_string()
    }

    /// Render type badge with color
    fn render_type_badge(&self, key_type: RedisKeyType, cx: &mut Context<Self>) -> impl IntoElement {
        let (bg_color, text_color) = match key_type {
            RedisKeyType::String => (cx.theme().success.opacity(0.2), cx.theme().success),
            RedisKeyType::Hash => (cx.theme().warning.opacity(0.2), cx.theme().warning),
            RedisKeyType::List => (cx.theme().info.opacity(0.2), cx.theme().info),
            RedisKeyType::Set => (cx.theme().primary.opacity(0.2), cx.theme().primary),
            RedisKeyType::ZSet => (cx.theme().danger.opacity(0.2), cx.theme().danger),
            RedisKeyType::Stream => (cx.theme().accent.opacity(0.2), cx.theme().accent_foreground),
            RedisKeyType::Unknown => (cx.theme().muted.opacity(0.2), cx.theme().muted_foreground),
        };

        div()
            .px_1()
            .py_px()
            .rounded_sm()
            .bg(bg_color)
            .child(
                Label::new(key_type.short_name())
                    .text_xs()
                    .text_color(text_color),
            )
    }

    /// Render a single key item
    fn render_key_item(
        &self,
        index: usize,
        key_item: &RedisKeyItem,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let key = key_item.key.clone();
        let keys_state = self.keys_state.clone();

        let bg = if is_selected {
            cx.theme().accent
        } else if index % 2 == 0 {
            if cx.theme().is_dark() {
                cx.theme().background.lighten(0.3)
            } else {
                cx.theme().background.darken(0.01)
            }
        } else {
            cx.theme().background
        };

        let text_color = if is_selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        };

        div()
            .id(("key-item", index))
            .w_full()
            .px_2()
            .py_1()
            .bg(bg)
            .cursor_pointer()
            .hover(|this| this.bg(cx.theme().accent.opacity(0.5)))
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(self.render_type_badge(key_item.key_type, cx))
                    .child(
                        Label::new(key_item.key.clone())
                            .text_sm()
                            .text_color(text_color)
                            .text_ellipsis()
                            .flex_1(),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                let key_clone = key.clone();
                this.keys_state.update(cx, |state, cx| {
                    state.select_key(Some(key_clone.clone()), cx);
                });

                // Fetch the key value
                let store = cx.global::<DfcGlobalStore>().clone();
                let keys_state = this.keys_state.clone();
                let key_for_fetch = key_clone.clone();

                cx.spawn(async move |_, cx| {
                    let redis = store.services().redis();
                    match redis.get_key_value(&key_for_fetch).await {
                        Ok(value) => {
                            let _ = keys_state.update(cx, |state, cx| {
                                state.set_selected_value(value, cx);
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to get key value: {}", e);
                            let _ = keys_state.update(cx, |state, cx| {
                                state.set_selected_value(
                                    RedisKeyValue::Error(e.to_string()),
                                    cx,
                                );
                            });
                        }
                    }
                })
                .detach();
            }))
    }

    /// Render the keys list panel
    fn render_keys_list(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let keys_state = self.keys_state.read(cx);
        let selected_key = keys_state.selected_key().map(|s| s.to_string());
        let is_loading = keys_state.is_loading();
        let has_more = keys_state.has_more_keys();

        // Collect key data before mutable borrow
        let key_data: Vec<_> = keys_state
            .filtered_keys()
            .iter()
            .enumerate()
            .map(|(index, key_item)| {
                let is_selected = selected_key.as_deref() == Some(&key_item.key);
                (index, (*key_item).clone(), is_selected)
            })
            .collect();

        let keys_count = key_data.len();

        let search_btn = Button::new("keys-search-btn")
            .ghost()
            .small()
            .icon(IconName::Search);

        // Build key items
        let mut key_items = Vec::new();
        for (index, key_item, is_selected) in key_data {
            key_items.push(self.render_key_item(index, &key_item, is_selected, cx));
        }
        let count_label = t!("keys.items", count = keys_count, locale = &locale).to_string();

        v_flex()
            .w(px(300.0))
            .h_full()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            // Search input
            .child(
                div()
                    .w_full()
                    .p_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Input::new(&self.search_state)
                            .w_full()
                            .suffix(search_btn)
                            .cleanable(true),
                    ),
            )
            // Count label
            .child(
                div()
                    .w_full()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Label::new(count_label)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            // Keys list
            .child(
                div()
                    .id("keys-list-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .children(key_items)
                    // Load more button
                    .when(has_more && !is_loading, |this| {
                        let load_more_label = t!("keys.load_more", locale = &locale).to_string();
                        this.child(
                            div()
                                .w_full()
                                .p_2()
                                .child(
                                    Button::new("load-more-btn")
                                        .ghost()
                                        .w_full()
                                        .label(load_more_label)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            let cursor = this.keys_state.read(cx).scan_cursor();
                                            let store = cx.global::<DfcGlobalStore>().clone();
                                            let keys_state = this.keys_state.clone();

                                            cx.spawn(async move |_, cx| {
                                                let redis = store.services().redis();
                                                match redis.scan_keys("*", cursor, 100).await {
                                                    Ok((keys, next_cursor)) => {
                                                        let _ = keys_state.update(cx, |state, cx| {
                                                            state.append_keys(keys, next_cursor, cx);
                                                        });
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to load more keys: {}", e);
                                                    }
                                                }
                                            })
                                            .detach();
                                        })),
                                ),
                        )
                    })
                    // Loading indicator
                    .when(is_loading, |this| {
                        let loading_label = t!("keys.loading", locale = &locale).to_string();
                        this.child(
                            div()
                                .w_full()
                                .p_4()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .justify_center()
                                        .child(Icon::new(IconName::Loader).size_4())
                                        .child(
                                            Label::new(loading_label)
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground),
                                        ),
                                ),
                        )
                    }),
            )
    }

    /// Render string value
    fn render_string_value(&self, value: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let value_label = t!("keys.value", locale = &locale).to_string();

        v_flex()
            .gap_2()
            .child(
                Label::new(value_label)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .w_full()
                    .p_2()
                    .rounded_md()
                    .bg(cx.theme().secondary)
                    .child(Label::new(value.to_string()).text_sm()),
            )
    }

    /// Render hash value
    fn render_hash_value(&self, pairs: &[(String, String)], cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let field_label = t!("keys.field", locale = &locale).to_string();
        let value_label = t!("keys.value", locale = &locale).to_string();
        let count_label = t!("keys.hash_fields", count = pairs.len(), locale = &locale).to_string();

        let header_bg = cx.theme().secondary;
        let border_color = cx.theme().border;

        // Build rows
        let mut rows = Vec::new();
        for (index, (field, value)) in pairs.iter().enumerate() {
            let bg = if index % 2 == 0 {
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
                    .w_full()
                    .bg(bg)
                    .border_b_1()
                    .border_color(border_color)
                    .child(
                        div()
                            .w(px(200.0))
                            .px_2()
                            .py_1()
                            .border_r_1()
                            .border_color(border_color)
                            .child(
                                Label::new(field.clone())
                                    .text_sm()
                                    .text_ellipsis(),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .px_2()
                            .py_1()
                            .child(
                                Label::new(value.clone())
                                    .text_sm()
                                    .text_ellipsis(),
                            ),
                    ),
            );
        }

        v_flex()
            .gap_2()
            .child(
                Label::new(count_label)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .w_full()
                    .rounded_md()
                    .border_1()
                    .border_color(border_color)
                    .overflow_hidden()
                    // Table header
                    .child(
                        h_flex()
                            .w_full()
                            .bg(header_bg)
                            .border_b_1()
                            .border_color(border_color)
                            .child(
                                div()
                                    .w(px(200.0))
                                    .px_2()
                                    .py_1()
                                    .border_r_1()
                                    .border_color(border_color)
                                    .child(
                                        Label::new(field_label)
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .px_2()
                                    .py_1()
                                    .child(
                                        Label::new(value_label)
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                            ),
                    )
                    // Table body
                    .child(
                        div()
                            .id("hash-table-scroll")
                            .max_h(px(400.0))
                            .overflow_y_scroll()
                            .children(rows),
                    ),
            )
    }

    /// Render list value
    fn render_list_value(&self, items: &[String], cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let index_label = t!("keys.index", locale = &locale).to_string();
        let value_label = t!("keys.value", locale = &locale).to_string();
        let count_label = t!("keys.list_elements", count = items.len(), locale = &locale).to_string();

        let header_bg = cx.theme().secondary;
        let border_color = cx.theme().border;

        // Build rows
        let mut rows = Vec::new();
        for (index, value) in items.iter().enumerate() {
            let bg = if index % 2 == 0 {
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
                    .w_full()
                    .bg(bg)
                    .border_b_1()
                    .border_color(border_color)
                    .child(
                        div()
                            .w(px(60.0))
                            .px_2()
                            .py_1()
                            .border_r_1()
                            .border_color(border_color)
                            .child(
                                Label::new(format!("{}", index))
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .px_2()
                            .py_1()
                            .child(
                                Label::new(value.clone())
                                    .text_sm()
                                    .text_ellipsis(),
                            ),
                    ),
            );
        }

        v_flex()
            .gap_2()
            .child(
                Label::new(count_label)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .w_full()
                    .rounded_md()
                    .border_1()
                    .border_color(border_color)
                    .overflow_hidden()
                    // Table header
                    .child(
                        h_flex()
                            .w_full()
                            .bg(header_bg)
                            .border_b_1()
                            .border_color(border_color)
                            .child(
                                div()
                                    .w(px(60.0))
                                    .px_2()
                                    .py_1()
                                    .border_r_1()
                                    .border_color(border_color)
                                    .child(
                                        Label::new(index_label)
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .px_2()
                                    .py_1()
                                    .child(
                                        Label::new(value_label)
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                            ),
                    )
                    // Table body
                    .child(
                        div()
                            .id("list-table-scroll")
                            .max_h(px(400.0))
                            .overflow_y_scroll()
                            .children(rows),
                    ),
            )
    }

    /// Render set value
    fn render_set_value(&self, members: &[String], cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let member_label = t!("keys.member", locale = &locale).to_string();
        let count_label = t!("keys.set_members", count = members.len(), locale = &locale).to_string();

        let header_bg = cx.theme().secondary;
        let border_color = cx.theme().border;

        // Build rows
        let mut rows = Vec::new();
        for (index, member) in members.iter().enumerate() {
            let bg = if index % 2 == 0 {
                if cx.theme().is_dark() {
                    cx.theme().background.lighten(0.3)
                } else {
                    cx.theme().background.darken(0.01)
                }
            } else {
                cx.theme().background
            };

            rows.push(
                div()
                    .w_full()
                    .bg(bg)
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(border_color)
                    .child(
                        Label::new(member.clone())
                            .text_sm()
                            .text_ellipsis(),
                    ),
            );
        }

        v_flex()
            .gap_2()
            .child(
                Label::new(count_label)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .w_full()
                    .rounded_md()
                    .border_1()
                    .border_color(border_color)
                    .overflow_hidden()
                    // Table header
                    .child(
                        div()
                            .w_full()
                            .bg(header_bg)
                            .px_2()
                            .py_1()
                            .border_b_1()
                            .border_color(border_color)
                            .child(
                                Label::new(member_label)
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    // Table body
                    .child(
                        div()
                            .id("set-table-scroll")
                            .max_h(px(400.0))
                            .overflow_y_scroll()
                            .children(rows),
                    ),
            )
    }

    /// Render sorted set value
    fn render_zset_value(&self, members: &[(String, f64)], cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let member_label = t!("keys.member", locale = &locale).to_string();
        let score_label = t!("keys.score", locale = &locale).to_string();
        let count_label = t!("keys.zset_members", count = members.len(), locale = &locale).to_string();

        let header_bg = cx.theme().secondary;
        let border_color = cx.theme().border;

        // Build rows
        let mut rows = Vec::new();
        for (index, (member, score)) in members.iter().enumerate() {
            let bg = if index % 2 == 0 {
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
                    .w_full()
                    .bg(bg)
                    .border_b_1()
                    .border_color(border_color)
                    .child(
                        div()
                            .w(px(100.0))
                            .px_2()
                            .py_1()
                            .border_r_1()
                            .border_color(border_color)
                            .child(
                                Label::new(format!("{}", score))
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .px_2()
                            .py_1()
                            .child(
                                Label::new(member.clone())
                                    .text_sm()
                                    .text_ellipsis(),
                            ),
                    ),
            );
        }

        v_flex()
            .gap_2()
            .child(
                Label::new(count_label)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .w_full()
                    .rounded_md()
                    .border_1()
                    .border_color(border_color)
                    .overflow_hidden()
                    // Table header
                    .child(
                        h_flex()
                            .w_full()
                            .bg(header_bg)
                            .border_b_1()
                            .border_color(border_color)
                            .child(
                                div()
                                    .w(px(100.0))
                                    .px_2()
                                    .py_1()
                                    .border_r_1()
                                    .border_color(border_color)
                                    .child(
                                        Label::new(score_label)
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .px_2()
                                    .py_1()
                                    .child(
                                        Label::new(member_label)
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                            ),
                    )
                    // Table body
                    .child(
                        div()
                            .id("zset-table-scroll")
                            .max_h(px(400.0))
                            .overflow_y_scroll()
                            .children(rows),
                    ),
            )
    }

    /// Render the value panel
    fn render_value_panel(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let keys_state = self.keys_state.read(cx);
        let selected_key = keys_state.selected_key().map(|s| s.to_string());
        let selected_value = keys_state.selected_value().clone();

        // Find the key item for type and TTL info
        let key_item = selected_key.as_ref().and_then(|key| {
            keys_state.keys().iter().find(|k| &k.key == key).cloned()
        });

        let content = match (&selected_key, &selected_value) {
            (None, _) => {
                let select_key_label = t!("keys.select_key", locale = &locale).to_string();
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Label::new(select_key_label)
                            .text_color(cx.theme().muted_foreground),
                    )
                    .into_any_element()
            }
            (Some(_), RedisKeyValue::Loading) => {
                let loading_label = t!("keys.loading", locale = &locale).to_string();
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::Loader).size_4())
                            .child(
                                Label::new(loading_label)
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .into_any_element()
            }
            (Some(_), RedisKeyValue::Error(err)) => {
                let error_label = t!("keys.error", locale = &locale).to_string();
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
                                    .child(
                                        Icon::new(IconName::CircleX)
                                            .size_5()
                                            .text_color(cx.theme().danger),
                                    )
                                    .child(
                                        Label::new(error_label)
                                            .text_color(cx.theme().danger),
                                    ),
                            )
                            .child(
                                Label::new(err.clone())
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .into_any_element()
            }
            (Some(_), RedisKeyValue::String(value)) => {
                self.render_string_value(value, cx).into_any_element()
            }
            (Some(_), RedisKeyValue::Hash(pairs)) => {
                self.render_hash_value(pairs, cx).into_any_element()
            }
            (Some(_), RedisKeyValue::List(items)) => {
                self.render_list_value(items, cx).into_any_element()
            }
            (Some(_), RedisKeyValue::Set(members)) => {
                self.render_set_value(members, cx).into_any_element()
            }
            (Some(_), RedisKeyValue::ZSet(members)) => {
                self.render_zset_value(members, cx).into_any_element()
            }
            (Some(_), RedisKeyValue::Empty) => {
                let select_key_label = t!("keys.select_key", locale = &locale).to_string();
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Label::new(select_key_label)
                            .text_color(cx.theme().muted_foreground),
                    )
                    .into_any_element()
            }
        };

        let key_label = t!("keys.key", locale = &locale).to_string();
        let type_label = t!("keys.type", locale = &locale).to_string();
        let ttl_label = t!("keys.ttl", locale = &locale).to_string();
        let no_expiry = t!("keys.no_expiry", locale = &locale).to_string();
        let border_color = cx.theme().border;
        let bg_color = cx.theme().background;
        let muted_fg = cx.theme().muted_foreground;

        // Build header if we have a key item
        let header = key_item.map(|item| {
            let ttl_display = if item.ttl < 0 {
                no_expiry.clone()
            } else {
                t!("keys.expires_in", seconds = item.ttl, locale = &locale).to_string()
            };

            div()
                .w_full()
                .p_2()
                .border_b_1()
                .border_color(border_color)
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Label::new(key_label.clone())
                                        .text_sm()
                                        .text_color(muted_fg),
                                )
                                .child(Label::new(item.key.clone()).text_sm()),
                        )
                        .child(
                            h_flex()
                                .gap_4()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            Label::new(type_label.clone())
                                                .text_xs()
                                                .text_color(muted_fg),
                                        )
                                        .child(self.render_type_badge(item.key_type, cx)),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            Label::new(ttl_label.clone())
                                                .text_xs()
                                                .text_color(muted_fg),
                                        )
                                        .child(
                                            Label::new(ttl_display)
                                                .text_xs()
                                                .text_color(muted_fg),
                                        ),
                                ),
                        ),
                )
        });

        v_flex()
            .flex_1()
            .h_full()
            .bg(bg_color)
            // Key info header
            .when_some(header, |this, h| this.child(h))
            // Value content
            .child(
                div()
                    .id("value-panel-scroll")
                    .flex_1()
                    .p_4()
                    .overflow_y_scroll()
                    .child(content),
            )
    }
}

impl KeysBrowserView {
    /// Render the header with back button and server info
    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let keys_state = self.keys_state.read(cx);

        // Get active server name
        let server_name = keys_state
            .active_server()
            .map(|s| s.server_name.clone())
            .unwrap_or_else(|| t!("keys.title", locale = &locale).to_string());

        let back_label = t!("config.back", locale = &locale).to_string();

        let back_btn = Button::new("keys-back-btn")
            .ghost()
            .icon(IconName::ArrowLeft)
            .label(back_label)
            .on_click(cx.listener(move |this, _, _, cx| {
                // Remove all connected servers and go back to server list
                this.keys_state.update(cx, |state, cx| {
                    state.clear(cx);
                });
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
}

impl Render for KeysBrowserView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(self.render_header(cx))
            .child(
                h_flex()
                    .flex_1()
                    .child(self.render_keys_list(window, cx))
                    .child(self.render_value_panel(window, cx)),
            )
    }
}
