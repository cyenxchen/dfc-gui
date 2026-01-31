//! Header Component
//!
//! The application header with logo, title, and language switcher.

use gpui::{
    div, px, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window,
};

use crate::app::entities::AppEntities;
use crate::i18n::t;
use crate::state::connection_state::ConnectionTarget;
use crate::theme::colors::DfcColors;

/// Header component
pub struct Header {
    entities: AppEntities,
}

impl Header {
    pub fn new(entities: AppEntities, cx: &mut Context<Self>) -> Self {
        // Observe i18n changes
        cx.observe(&entities.i18n, |_this, _, cx| cx.notify())
            .detach();

        // Observe connection changes
        cx.observe(&entities.connection, |_this, _, cx| cx.notify())
            .detach();

        Self { entities }
    }

    fn render_connection_indicator(
        &self,
        target: ConnectionTarget,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let conn = self.entities.connection.read(cx);
        let connected = conn.is_connected(target);

        let (color, status) = if connected {
            (DfcColors::success(), "●")
        } else {
            (gpui::rgba(0x9ca3afff), "○")
        };

        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .text_color(color)
                    .text_size(px(10.0))
                    .child(status),
            )
            .child(
                div()
                    .text_color(DfcColors::text_light())
                    .text_size(px(12.0))
                    .child(target.label()),
            )
    }
}

impl Render for Header {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.entities.i18n.read(cx).locale;
        let title = t(locale, "app-title");
        let lang_label = locale.display_name();

        let entities = self.entities.clone();

        div()
            .h(px(48.0))
            .w_full()
            .bg(DfcColors::header_bg())
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            // Left side: Logo and title
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    // Logo placeholder
                    .child(
                        div()
                            .size(px(32.0))
                            .rounded_md()
                            .bg(gpui::rgba(0xffffffcc))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(DfcColors::header_bg())
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("R"),
                    )
                    .child(
                        div()
                            .text_color(DfcColors::text_header())
                            .text_size(px(18.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title),
                    ),
            )
            // Right side: Connection status and language switcher
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_6()
                    // Connection indicators
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(self.render_connection_indicator(ConnectionTarget::Redis, cx))
                            .child(self.render_connection_indicator(ConnectionTarget::Pulsar, cx))
                            .child(self.render_connection_indicator(ConnectionTarget::Database, cx)),
                    )
                    // Language switcher
                    .child(
                        div()
                            .id("lang-switcher")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(gpui::rgba(0xffffff22))
                            .text_color(DfcColors::text_header())
                            .text_size(px(13.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(gpui::rgba(0xffffff44)))
                            .on_click(move |_event: &ClickEvent, _window, cx| {
                                entities.i18n.update(cx, |i18n, cx| {
                                    i18n.toggle_locale();
                                    cx.notify();
                                });
                            })
                            .child(lang_label),
                    ),
            )
    }
}
