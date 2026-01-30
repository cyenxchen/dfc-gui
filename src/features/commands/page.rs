//! Commands Page
//!
//! Form for sending commands to devices with response display.

use gpui::{
    div, prelude::*, px, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window,
};

use crate::app::entities::AppEntities;
use crate::components::primitives::button::Button;
use crate::domain::command::CommandRequest;
use crate::features::commands::controller::CommandsController;
use crate::i18n::{t, Locale};
use crate::theme::colors::DfcColors;
use crate::utils::format::format_datetime;

/// Commands page component
pub struct CommandsPage {
    entities: AppEntities,
    controller: CommandsController,
    // Form state
    device_id: String,
    service: String,
    method: String,
    params: String,
    timeout: String,
}

impl CommandsPage {
    pub fn new(entities: AppEntities, cx: &mut Context<Self>) -> Self {
        let controller = CommandsController::new(entities.clone());

        // Observe commands state
        cx.observe(&entities.commands, |_this, _, cx| cx.notify())
            .detach();

        // Observe i18n changes
        cx.observe(&entities.i18n, |_this, _, cx| cx.notify())
            .detach();

        // Observe config for device ID
        cx.observe(&entities.config, |this, config, cx| {
            let config = config.read(cx);
            if !config.config.device.device_id.is_empty() {
                this.device_id = config.config.device.device_id.clone();
                cx.notify();
            }
        })
        .detach();

        Self {
            entities,
            controller,
            device_id: "DOC00006".to_string(),
            service: "serviceCall".to_string(),
            method: "query".to_string(),
            params: "{}".to_string(),
            timeout: "30".to_string(),
        }
    }

    fn render_form_row(
        &self,
        label: SharedString,
        value: &str,
        _field_id: &str,
    ) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w(px(100.0))
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

    fn build_request(&self) -> CommandRequest {
        CommandRequest {
            request_id: uuid::Uuid::new_v4().to_string(),
            device_id: self.device_id.clone(),
            service: self.service.clone(),
            method: self.method.clone(),
            params: self.params.clone(),
            timeout: self.timeout.parse().unwrap_or(30),
            created_time: chrono::Utc::now(),
        }
    }
}

impl Render for CommandsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.entities.i18n.read(cx).locale;
        let commands_state = self.entities.commands.read(cx);
        let history = &commands_state.history;
        let sending = commands_state.sending;

        div()
            .size_full()
            .flex()
            .flex_col()
            .p_4()
            .gap_4()
            // Header
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(t(locale, "commands-title")),
            )
            // Command Form
            .child(
                div()
                    .w_full()
                    .bg(DfcColors::content_bg())
                    .border_1()
                    .border_color(DfcColors::border())
                    .rounded_md()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(self.render_form_row(
                        t(locale, "col-device-id"),
                        &self.device_id,
                        "cmd_device_id",
                    ))
                    .child(self.render_form_row(
                        t(locale, "commands-service"),
                        &self.service,
                        "cmd_service",
                    ))
                    .child(self.render_form_row(
                        t(locale, "commands-method"),
                        &self.method,
                        "cmd_method",
                    ))
                    .child(self.render_form_row(
                        t(locale, "commands-params"),
                        &self.params,
                        "cmd_params",
                    ))
                    .child(self.render_form_row(
                        t(locale, "commands-timeout"),
                        &self.timeout,
                        "cmd_timeout",
                    ))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_3()
                            .pt_2()
                            .child(
                                Button::primary("send-cmd-btn", t(locale, "action-send"))
                                    .loading(sending)
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        let request = this.build_request();
                                        this.controller.send_command(request, cx);
                                    })),
                            )
                            .child(
                                Button::ghost("clear-history-btn", t(locale, "action-clear"))
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        this.controller.clear_history(cx);
                                    })),
                            ),
                    ),
            )
            // History Section
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(t(locale, "commands-history")),
                    )
                    .child(
                        div()
                            .id("command-history")
                            .flex_1()
                            .bg(DfcColors::content_bg())
                            .border_1()
                            .border_color(DfcColors::border())
                            .rounded_md()
                            .overflow_y_scroll()
                            .p_4()
                            .when(history.is_empty(), |el| {
                                el.flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(DfcColors::text_muted())
                                    .child(t(locale, "table-no-data"))
                            })
                            .when(!history.is_empty(), |el| {
                                el.flex().flex_col().gap_2().children(
                                    history.iter().rev().map(|req| {
                                        let response = commands_state.get_response(&req.request_id);

                                        div()
                                            .w_full()
                                            .p_3()
                                            .bg(DfcColors::table_row_alt())
                                            .rounded_md()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(gpui::FontWeight::MEDIUM)
                                                            .child(format!(
                                                                "{}.{}",
                                                                req.service, req.method
                                                            )),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(DfcColors::text_muted())
                                                            .child(format_datetime(
                                                                &req.created_time,
                                                            )),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(DfcColors::text_secondary())
                                                    .child(format!(
                                                        "Device: {} | Timeout: {}s",
                                                        req.device_id, req.timeout
                                                    )),
                                            )
                                            .when_some(response, |el, resp| {
                                                let (color, text) = if resp.code == 0 {
                                                    (DfcColors::success(), "Success")
                                                } else {
                                                    (DfcColors::danger(), "Error")
                                                };
                                                el.child(
                                                    div()
                                                        .pt_1()
                                                        .flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(color)
                                                                .child(text),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(DfcColors::text_muted())
                                                                .child(resp.message.clone()),
                                                        ),
                                                )
                                            })
                                    }),
                                )
                            }),
                    ),
            )
    }
}
