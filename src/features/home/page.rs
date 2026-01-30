//! Home Page
//!
//! Main configuration page with device settings, Redis/Pulsar config, and filters.

use gpui::{
    div, prelude::*, px, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, VisualContext, Window,
};

use crate::app::entities::AppEntities;
use crate::components::primitives::button::{Button, ButtonVariant};
use crate::components::primitives::checkbox::Checkbox;
use crate::domain::config::{AppConfig, DeviceConfig, FilterConfig, PulsarConfig, RedisConfig};
use crate::features::home::controller::HomeController;
use crate::i18n::{t, Locale};
use crate::services::service_hub::ServiceHub;
use crate::theme::colors::DfcColors;

/// Home page component
pub struct HomePage {
    entities: AppEntities,
    controller: HomeController,
    // Form state
    device_id: String,
    cfgid: String,
    running: bool,
    redis_ip: String,
    redis_port: String,
    redis_password: String,
    pulsar_ip: String,
    pulsar_port: String,
    pulsar_redis_ip: String,
    pulsar_redis_port: String,
    // Filter options
    limit_device: bool,
    limit_cfgid: bool,
    use_token: bool,
    use_time_range: bool,
}

impl HomePage {
    pub fn new(entities: AppEntities, cx: &mut Context<Self>) -> Self {
        let controller = HomeController::new(entities.clone());

        // Observe config changes
        cx.observe(&entities.config, |this, config, cx| {
            let config = config.read(cx);
            this.load_from_config(&config.config);
            cx.notify();
        })
        .detach();

        // Observe i18n changes
        cx.observe(&entities.i18n, |_this, _, cx| cx.notify())
            .detach();

        // Set default values
        let default_redis = RedisConfig::default();
        let default_pulsar = PulsarConfig::default();

        Self {
            entities,
            controller,
            device_id: "DOC00006".to_string(),
            cfgid: String::new(),
            running: false,
            redis_ip: default_redis.ip,
            redis_port: default_redis.port.to_string(),
            redis_password: String::new(),
            pulsar_ip: default_pulsar.ip,
            pulsar_port: default_pulsar.port.to_string(),
            pulsar_redis_ip: default_pulsar.redis_ip,
            pulsar_redis_port: default_pulsar.redis_port.to_string(),
            limit_device: false,
            limit_cfgid: false,
            use_token: false,
            use_time_range: false,
        }
    }

    fn load_from_config(&mut self, config: &AppConfig) {
        self.device_id = config.device.device_id.clone();
        self.cfgid = config.device.cfgid.clone();
        self.running = config.device.running;
        self.redis_ip = config.redis.ip.clone();
        self.redis_port = config.redis.port.to_string();
        self.redis_password = config.redis.password.clone().unwrap_or_default();
        self.pulsar_ip = config.pulsar.ip.clone();
        self.pulsar_port = config.pulsar.port.to_string();
        self.pulsar_redis_ip = config.pulsar.redis_ip.clone();
        self.pulsar_redis_port = config.pulsar.redis_port.to_string();
        self.limit_device = config.filter.limit_device;
        self.limit_cfgid = config.filter.limit_cfgid;
        self.use_token = config.filter.use_token;
        self.use_time_range = config.filter.use_time_range;
    }

    fn build_config(&self) -> AppConfig {
        AppConfig {
            device: DeviceConfig {
                device_id: self.device_id.clone(),
                cfgid: self.cfgid.clone(),
                running: self.running,
                start_time: None,
                end_time: None,
            },
            redis: RedisConfig {
                ip: self.redis_ip.clone(),
                port: self.redis_port.parse().unwrap_or(10060),
                password: if self.redis_password.is_empty() {
                    None
                } else {
                    Some(self.redis_password.clone())
                },
            },
            pulsar: PulsarConfig {
                ip: self.pulsar_ip.clone(),
                port: self.pulsar_port.parse().unwrap_or(6678),
                redis_ip: self.pulsar_redis_ip.clone(),
                redis_port: self.pulsar_redis_port.parse().unwrap_or(6603),
                ..Default::default()
            },
            filter: FilterConfig {
                limit_device: self.limit_device,
                limit_cfgid: self.limit_cfgid,
                use_token: self.use_token,
                use_time_range: self.use_time_range,
                ..Default::default()
            },
        }
    }

    fn render_section_header(&self, title: SharedString) -> impl IntoElement {
        div()
            .w_full()
            .px_4()
            .py_2()
            .bg(DfcColors::header_bg())
            .text_color(DfcColors::text_header())
            .text_sm()
            .font_weight(gpui::FontWeight::MEDIUM)
            .child(title)
    }

    fn render_form_row(
        &self,
        label: SharedString,
        value: &str,
        _field_id: &str,
    ) -> impl IntoElement {
        div()
            .w_full()
            .px_4()
            .py_2()
            .flex()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w(px(120.0))
                    .text_sm()
                    .text_color(DfcColors::text_secondary())
                    .child(label),
            )
            .child(
                div()
                    .flex_1()
                    .px_3()
                    .py_2()
                    .bg(DfcColors::input_bg())
                    .border_1()
                    .border_color(DfcColors::input_border())
                    .rounded_md()
                    .text_sm()
                    .text_color(DfcColors::text_primary())
                    .child(value.to_string()),
            )
    }
}

impl Render for HomePage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.entities.i18n.read(cx).locale;
        let entities = self.entities.clone();

        div()
            .id("home-page")
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .p_4()
            .gap_4()
            // Basic Configuration Section
            .child(
                div()
                    .w_full()
                    .bg(DfcColors::content_bg())
                    .border_1()
                    .border_color(DfcColors::border())
                    .rounded_md()
                    .overflow_hidden()
                    .child(self.render_section_header(t(locale, "home-basic-config")))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(self.render_form_row(
                                t(locale, "home-device-id"),
                                &self.device_id,
                                "device_id",
                            ))
                            .child(self.render_form_row(
                                t(locale, "home-cfgid"),
                                &self.cfgid,
                                "cfgid",
                            ))
                            .child(
                                div()
                                    .w_full()
                                    .px_4()
                                    .py_2()
                                    .flex()
                                    .items_center()
                                    .gap_4()
                                    .child(
                                        div()
                                            .w(px(120.0))
                                            .text_sm()
                                            .text_color(DfcColors::text_secondary())
                                            .child(t(locale, "home-running")),
                                    )
                                    .child({
                                        let running = self.running;
                                        let entities = self.entities.clone();
                                        Checkbox::new("running-checkbox")
                                            .checked(running)
                                            .on_change(move |_checked, _window, _cx| {
                                                // TODO: Handle running state change
                                            })
                                    }),
                            ),
                    ),
            )
            // Redis Configuration Section
            .child(
                div()
                    .w_full()
                    .bg(DfcColors::content_bg())
                    .border_1()
                    .border_color(DfcColors::border())
                    .rounded_md()
                    .overflow_hidden()
                    .child(self.render_section_header(t(locale, "home-redis-config")))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(self.render_form_row(
                                t(locale, "home-ip"),
                                &self.redis_ip,
                                "redis_ip",
                            ))
                            .child(self.render_form_row(
                                t(locale, "home-port"),
                                &self.redis_port,
                                "redis_port",
                            ))
                            .child(self.render_form_row(
                                t(locale, "home-password"),
                                "********",
                                "redis_password",
                            )),
                    ),
            )
            // Pulsar Configuration Section
            .child(
                div()
                    .w_full()
                    .bg(DfcColors::content_bg())
                    .border_1()
                    .border_color(DfcColors::border())
                    .rounded_md()
                    .overflow_hidden()
                    .child(self.render_section_header(t(locale, "home-pulsar-config")))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(self.render_form_row(
                                t(locale, "home-ip"),
                                &self.pulsar_ip,
                                "pulsar_ip",
                            ))
                            .child(self.render_form_row(
                                t(locale, "home-port"),
                                &self.pulsar_port,
                                "pulsar_port",
                            ))
                            .child(self.render_form_row(
                                t(locale, "home-pulsar-redis-ip"),
                                &self.pulsar_redis_ip,
                                "pulsar_redis_ip",
                            ))
                            .child(self.render_form_row(
                                t(locale, "home-pulsar-redis-port"),
                                &self.pulsar_redis_port,
                                "pulsar_redis_port",
                            )),
                    ),
            )
            // Action Buttons
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::primary("start-btn", t(locale, "action-start")).on_click(
                            cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                                let config = this.build_config();
                                this.controller.start_services(config, cx);
                            }),
                        ),
                    )
                    .child(
                        Button::secondary("stop-btn", t(locale, "action-stop")).on_click(
                            cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                                this.controller.stop_services(cx);
                            }),
                        ),
                    )
                    .child(
                        Button::ghost("save-btn", t(locale, "action-save")).on_click(cx.listener(
                            move |this, _event: &ClickEvent, _window, cx| {
                                let config = this.build_config();
                                this.controller.save_config(&config, cx);
                            },
                        )),
                    )
                    .child(
                        Button::ghost("favorite-btn", t(locale, "action-save-favorite")).on_click(
                            cx.listener(move |_this, _event: &ClickEvent, _window, _cx| {
                                // TODO: Show favorite dialog
                            }),
                        ),
                    ),
            )
    }
}
