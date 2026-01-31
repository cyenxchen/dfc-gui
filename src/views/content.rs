//! Main Content Area
//!
//! Routes to different views based on the current application route.

use crate::assets::CustomIconName;
use crate::connection::{DfcServerConfig, credentials_to_text, text_to_credentials};
use crate::states::{
    DfcAppState, DfcGlobalStore, FleetState, Route, UIEvent, i18n_common, i18n_servers,
    i18n_settings, i18n_sidebar, update_app_state_and_save,
};
use gpui::{App, Context, Entity, SharedString, Subscription, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Colorize, Icon, IconName, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    form::{field, v_form},
    h_flex,
    input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
    label::Label,
    scroll::ScrollableElement,
    v_flex,
};
use rust_i18n::t;
use std::cell::Cell;
use std::rc::Rc;

/// Width of the keyword search input
const KEYWORD_INPUT_WIDTH: f32 = 200.0;

/// Default Redis port
const DEFAULT_REDIS_PORT: u16 = 6379;

/// Viewport breakpoints for responsive grid
const VIEWPORT_BREAKPOINT_SMALL: f32 = 800.0;
const VIEWPORT_BREAKPOINT_MEDIUM: f32 = 1200.0;

/// Card background color adjustments
const THEME_LIGHTEN_AMOUNT_DARK: f32 = 1.0;
const THEME_DARKEN_AMOUNT_LIGHT: f32 = 0.02;

/// Main content container component
pub struct DfcContent {
    /// Current route
    current_route: Route,
    /// Fleet state entity
    fleet_state: Entity<FleetState>,
    /// App state entity
    app_state: Entity<DfcAppState>,
    /// Keyword search input state
    keyword_state: Entity<InputState>,
    /// Filter keyword for servers
    filter_keyword: SharedString,

    // Server form input states
    name_state: Entity<InputState>,
    host_state: Entity<InputState>,
    port_state: Entity<InputState>,
    password_state: Entity<InputState>,
    cfgid_state: Entity<InputState>,
    device_filter_state: Entity<InputState>,
    pulsar_token_state: Entity<InputState>,
    /// Current server ID being edited (empty for new)
    editing_server_id: String,

    // Settings form input states
    preset_credentials_state: Entity<InputState>,

    /// Subscriptions
    _subscriptions: Vec<Subscription>,
}

impl DfcContent {
    /// Create a new content view
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = cx.global::<DfcGlobalStore>();
        let current_route = store.read(cx).route();
        let fleet_state = store.fleet_state();
        let app_state = store.app_state();

        let mut subscriptions = Vec::new();

        // Subscribe to route changes
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

        // Subscribe to keyword input events for filtering
        subscriptions.push(cx.subscribe(&keyword_state, |this, state, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.filter_keyword = state.read(cx).value();
                cx.notify();
            }
        }));

        // Initialize server form input states
        let name_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder(i18n_servers(cx, "name_placeholder"))
        });
        let host_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder(i18n_servers(cx, "host_placeholder"))
        });
        let port_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder(i18n_servers(cx, "port_placeholder"))
        });
        let password_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(i18n_servers(cx, "password_placeholder"))
                .masked(true)
        });
        let cfgid_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder(i18n_servers(cx, "cfgid_placeholder"))
        });
        let device_filter_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder(i18n_servers(cx, "device_filter_placeholder"))
        });
        let pulsar_token_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(i18n_servers(cx, "pulsar_token_placeholder"))
                .auto_grow(2, 10)
        });

        // Initialize preset credentials input with existing data
        let existing_credentials = app_state.read(cx).preset_credentials();
        let preset_credentials_state = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .placeholder(i18n_settings(cx, "preset_credentials_placeholder"))
                .auto_grow(3, 10);
            if !existing_credentials.is_empty() {
                state.set_value(credentials_to_text(&existing_credentials), window, cx);
            }
            state
        });

        // Subscribe to preset credentials input for auto-save on blur
        subscriptions.push(cx.subscribe(&preset_credentials_state, |this, state, event, cx| {
            if matches!(event, InputEvent::Blur) {
                let text = state.read(cx).value();
                let credentials = text_to_credentials(&text);
                update_app_state_and_save(cx, "set_preset_credentials", move |state, _| {
                    state.set_preset_credentials(credentials.clone());
                });
            }
        }));

        // Subscribe to port input for stepping
        subscriptions.push(cx.subscribe_in(&port_state, window, |_this, state, event, window, cx| {
            let NumberInputEvent::Step(action) = event;
            let Ok(current_val) = state.read(cx).value().parse::<u16>() else {
                return;
            };
            let new_val = match action {
                StepAction::Increment => current_val.saturating_add(1),
                StepAction::Decrement => current_val.saturating_sub(1),
            };
            if new_val != current_val {
                state.update(cx, |input, cx| {
                    input.set_value(new_val.to_string(), window, cx);
                });
            }
        }));

        Self {
            current_route,
            fleet_state,
            app_state,
            keyword_state,
            filter_keyword: SharedString::default(),
            name_state,
            host_state,
            port_state,
            password_state,
            cfgid_state,
            device_filter_state,
            pulsar_token_state,
            editing_server_id: String::new(),
            preset_credentials_state,
            _subscriptions: subscriptions,
        }
    }

    /// Check if a server matches the current filter keyword
    fn server_matches_filter(&self, server: &DfcServerConfig) -> bool {
        if self.filter_keyword.is_empty() {
            return true;
        }
        let keyword = self.filter_keyword.to_lowercase();
        let name_matches = server.name.to_lowercase().contains(&keyword);
        let host_matches = server.host.to_lowercase().contains(&keyword);
        let cfgid_matches = server
            .cfgid
            .as_ref()
            .is_some_and(|c| c.to_lowercase().contains(&keyword));
        name_matches || host_matches || cfgid_matches
    }

    /// Fill input fields with server data for editing
    fn fill_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>, server: &DfcServerConfig) {
        self.editing_server_id = server.id.clone();

        self.name_state.update(cx, |state, cx| {
            state.set_value(server.name.clone(), window, cx);
        });
        self.host_state.update(cx, |state, cx| {
            state.set_value(server.host.clone(), window, cx);
        });
        if server.port != 0 {
            self.port_state.update(cx, |state, cx| {
                state.set_value(server.port.to_string(), window, cx);
            });
        } else {
            self.port_state.update(cx, |state, cx| {
                state.set_value(String::new(), window, cx);
            });
        }
        self.password_state.update(cx, |state, cx| {
            state.set_value(server.password.clone().unwrap_or_default(), window, cx);
        });
        self.cfgid_state.update(cx, |state, cx| {
            state.set_value(server.cfgid.clone().unwrap_or_default(), window, cx);
        });
        self.device_filter_state.update(cx, |state, cx| {
            state.set_value(server.device_filter.clone().unwrap_or_default(), window, cx);
        });
        self.pulsar_token_state.update(cx, |state, cx| {
            state.set_value(server.pulsar_token.clone().unwrap_or_default(), window, cx);
        });
    }

    /// Clear all input fields (for adding new server)
    /// Auto-fills Pulsar token from first preset credential if available
    fn clear_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_server_id = String::new();
        self.name_state.update(cx, |state, cx| {
            state.set_value(String::new(), window, cx);
        });
        self.host_state.update(cx, |state, cx| {
            state.set_value(String::new(), window, cx);
        });
        self.port_state.update(cx, |state, cx| {
            state.set_value(String::new(), window, cx);
        });
        self.password_state.update(cx, |state, cx| {
            state.set_value(String::new(), window, cx);
        });
        self.cfgid_state.update(cx, |state, cx| {
            state.set_value(String::new(), window, cx);
        });
        self.device_filter_state.update(cx, |state, cx| {
            state.set_value(String::new(), window, cx);
        });

        // Auto-fill Pulsar token from first preset credential
        let default_token = self
            .app_state
            .read(cx)
            .preset_credentials()
            .first()
            .map(|c| c.password.clone())
            .unwrap_or_default();
        self.pulsar_token_state.update(cx, |state, cx| {
            state.set_value(default_token, window, cx);
        });
    }

    /// Remove server with confirmation dialog
    fn remove_server(&mut self, window: &mut Window, cx: &mut Context<Self>, server_id: &str) {
        let server_name = self
            .app_state
            .read(cx)
            .server(server_id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "--".to_string());

        let locale = cx.global::<DfcGlobalStore>().read(cx).locale().to_string();
        let app_state = self.app_state.clone();
        let server_id = server_id.to_string();

        window.open_dialog(cx, move |dialog, _, cx| {
            let message = t!("servers.remove_prompt", server = &server_name, locale = &locale).to_string();
            let app_state = app_state.clone();
            let server_id = server_id.clone();

            dialog
                .confirm()
                .title(i18n_servers(cx, "remove_title"))
                .child(message)
                .on_ok(move |_, window, cx| {
                    app_state.update(cx, |state, cx| {
                        state.remove_server(&server_id, cx);
                    });
                    window.close_dialog(cx);
                    true
                })
        });
    }

    /// Open add/edit server dialog
    fn open_server_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app_state = self.app_state.clone();
        let name_state = self.name_state.clone();
        let host_state = self.host_state.clone();
        let port_state = self.port_state.clone();
        let password_state = self.password_state.clone();
        let cfgid_state = self.cfgid_state.clone();
        let device_filter_state = self.device_filter_state.clone();
        let pulsar_token_state = self.pulsar_token_state.clone();
        let server_id = self.editing_server_id.clone();
        let is_new = server_id.is_empty();

        // Clone states for submit handler
        let name_state_clone = name_state.clone();
        let host_state_clone = host_state.clone();
        let port_state_clone = port_state.clone();
        let password_state_clone = password_state.clone();
        let cfgid_state_clone = cfgid_state.clone();
        let device_filter_state_clone = device_filter_state.clone();
        let pulsar_token_state_clone = pulsar_token_state.clone();
        let app_state_clone = app_state.clone();
        let server_id_clone = server_id.clone();

        let handle_submit = Rc::new(move |_window: &mut Window, cx: &mut App| {
            let name = name_state_clone.read(cx).value();
            let host = host_state_clone.read(cx).value();
            let port = port_state_clone
                .read(cx)
                .value()
                .parse::<u16>()
                .unwrap_or(DEFAULT_REDIS_PORT);

            if name.is_empty() || host.is_empty() {
                return false;
            }

            let password_val = password_state_clone.read(cx).value();
            let password = if password_val.is_empty() {
                None
            } else {
                Some(password_val.to_string())
            };

            let cfgid_val = cfgid_state_clone.read(cx).value();
            let cfgid = if cfgid_val.is_empty() {
                None
            } else {
                Some(cfgid_val.to_string())
            };

            let device_filter_val = device_filter_state_clone.read(cx).value();
            let device_filter = if device_filter_val.is_empty() {
                None
            } else {
                Some(device_filter_val.to_string())
            };

            let pulsar_token_val = pulsar_token_state_clone.read(cx).value();
            let pulsar_token = if pulsar_token_val.is_empty() {
                None
            } else {
                Some(pulsar_token_val.to_string())
            };

            app_state_clone.update(cx, |state, cx| {
                state.upsert_server(
                    DfcServerConfig {
                        id: server_id_clone.clone(),
                        name: name.to_string(),
                        host: host.to_string(),
                        port,
                        password,
                        cfgid,
                        device_filter,
                        pulsar_token,
                        updated_at: None, // Will be set by upsert_server
                    },
                    cx,
                );
            });

            true
        });

        let focus_handle_done = Cell::new(false);

        window.open_dialog(cx, move |dialog, window, cx| {
            let title = if is_new {
                i18n_servers(cx, "add_title")
            } else {
                i18n_servers(cx, "edit_title")
            };

            // Focus the name field
            if !focus_handle_done.get() {
                name_state.clone().update(cx, |this, cx| {
                    this.focus(window, cx);
                });
                focus_handle_done.set(true);
            }

            let name_label = i18n_servers(cx, "name");
            let host_label = i18n_servers(cx, "host");
            let port_label = i18n_servers(cx, "port");
            let password_label = i18n_servers(cx, "password");
            let cfgid_label = i18n_servers(cx, "cfgid");
            let device_filter_label = i18n_servers(cx, "device_filter");
            let pulsar_token_label = i18n_servers(cx, "pulsar_token");

            dialog
                .title(title)
                .overlay(true)
                .child({
                    let form = v_form()
                        .child(field().label(name_label).child(Input::new(&name_state)))
                        .child(field().label(host_label).child(Input::new(&host_state)))
                        .child(field().label(port_label).child(NumberInput::new(&port_state)))
                        .child(field().label(password_label).child(Input::new(&password_state).mask_toggle()))
                        .child(field().label(cfgid_label).child(Input::new(&cfgid_state)))
                        .child(field().label(device_filter_label).child(Input::new(&device_filter_state)))
                        .child(
                            field()
                                .label(pulsar_token_label)
                                .child(Input::new(&pulsar_token_state)),
                        );

                    div()
                        .id("server-dialog-content")
                        .max_h(px(500.0))
                        .child(form)
                        .overflow_y_scrollbar()
                })
                .on_ok({
                    let handle = handle_submit.clone();
                    move |_, window, cx| handle(window, cx)
                })
                .footer({
                    let handle = handle_submit.clone();
                    move |_, _, _, cx| {
                        let submit_label = i18n_servers(cx, "save_config");
                        let cancel_label = i18n_common(cx, "cancel");

                        vec![
                            Button::new("cancel")
                                .label(cancel_label)
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                }),
                            Button::new("ok")
                                .primary()
                                .label(submit_label)
                                .on_click({
                                    let handle = handle.clone();
                                    move |_, window, cx| {
                                        if handle.clone()(window, cx) {
                                            window.close_dialog(cx);
                                        }
                                    }
                                }),
                        ]
                    }
                })
        });
    }

    /// Render the bottom toolbar
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let search_btn = Button::new("home-search-btn")
            .ghost()
            .xsmall()
            .icon(IconName::Search)
            .tooltip(i18n_sidebar(cx, "search"));

        h_flex()
            .w_full()
            .p_2()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("add-btn")
                            .icon(Icon::from(CustomIconName::FilePlusCorner))
                            .tooltip(i18n_servers(cx, "add_tooltip"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.clear_inputs(window, cx);
                                this.open_server_dialog(window, cx);
                            })),
                    )
                    .child(
                        Input::new(&self.keyword_state)
                            .w(px(KEYWORD_INPUT_WIDTH))
                            .suffix(search_btn)
                            .cleanable(true),
                    )
                    .flex_1(),
            )
    }

    /// Render a single server card
    fn render_server_card(
        &self,
        index: usize,
        server: &DfcServerConfig,
        bg: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let server_id = server.id.clone();
        let update_server = server.clone();
        let remove_server_id = server.id.clone();

        let updated_at = server
            .updated_at
            .as_ref()
            .map(|s| s.chars().take(10).collect::<String>())
            .unwrap_or_default();

        let title = format!("{} ({}:{})", server.name, server.host, server.port);

        let edit_btn = Button::new(("server-edit", index))
            .ghost()
            .xsmall()
            .tooltip(i18n_servers(cx, "edit_tooltip"))
            .icon(Icon::from(CustomIconName::FilePenLine))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.fill_inputs(window, cx, &update_server);
                this.open_server_dialog(window, cx);
            }));

        let delete_btn = Button::new(("server-delete", index))
            .ghost()
            .xsmall()
            .tooltip(i18n_servers(cx, "delete_tooltip"))
            .icon(Icon::from(CustomIconName::FileXCorner))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.remove_server(window, cx, &remove_server_id);
            }));

        div()
            .id(("server-card", index))
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(bg)
            .cursor_pointer()
            .hover(|this| this.border_color(cx.theme().primary))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .justify_between()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(Icon::new(CustomIconName::DatabaseZap).size_4())
                                    .child(
                                        Label::new(title)
                                            .text_sm()
                                            .text_ellipsis()
                                            .max_w(px(200.0)),
                                    ),
                            )
                            .child(h_flex().gap_1().child(edit_btn).child(delete_btn)),
                    )
                    .when(!updated_at.is_empty(), |this| {
                        this.child(
                            Label::new(updated_at)
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        )
                    }),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                tracing::info!("Server card clicked: {}", server_id);
                this.app_state.update(cx, |state, cx| {
                    state.select_server(Some(server_id.clone()), cx);
                });
            }))
    }

    /// Render the home view with server cards
    fn render_home(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = window.viewport_size().width;

        // Responsive grid columns
        let cols = match width {
            width if width < px(VIEWPORT_BREAKPOINT_SMALL) => 1,
            width if width < px(VIEWPORT_BREAKPOINT_MEDIUM) => 2,
            _ => 3,
        };

        // Card background color
        let bg = if cx.theme().is_dark() {
            cx.theme().background.lighten(THEME_LIGHTEN_AMOUNT_DARK)
        } else {
            cx.theme().background.darken(THEME_DARKEN_AMOUNT_LIGHT)
        };

        // Build server cards - collect first to avoid borrow issues
        let servers: Vec<_> = self
            .app_state
            .read(cx)
            .servers()
            .iter()
            .filter(|s| self.server_matches_filter(s))
            .cloned()
            .collect();

        let mut children = Vec::new();
        for (index, server) in servers.iter().enumerate() {
            children.push(self.render_server_card(index, server, bg, cx));
        }

        let grid = div()
            .grid()
            .grid_cols(cols)
            .gap_2()
            .p_2()
            .w_full()
            .children(children);

        v_flex()
            .size_full()
            .child(
                div()
                    .id("servers-grid-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(grid),
            )
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
        let preset_credentials_label = i18n_settings(cx, "preset_credentials");

        v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .child(Label::new(i18n_common(cx, "settings")).text_xl())
            .child(
                v_form().child(
                    field()
                        .label(preset_credentials_label)
                        .child(Input::new(&self.preset_credentials_state)),
                ),
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
