//! Log Panel Component
//!
//! Displays application logs at the bottom of the screen.

use gpui::{
    div, prelude::*, px, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, VisualContext, Window,
};

use crate::app::entities::AppEntities;
use crate::i18n::{t, Locale};
use crate::state::log_state::LogLevel;
use crate::theme::colors::DfcColors;
use crate::utils::format::format_time_ms;

/// Log panel component
pub struct LogPanel {
    entities: AppEntities,
    expanded: bool,
}

impl LogPanel {
    pub fn new(entities: AppEntities, cx: &mut Context<Self>) -> Self {
        // Observe log changes
        cx.observe(&entities.logs, |_this, _, cx| cx.notify())
            .detach();

        // Observe i18n changes
        cx.observe(&entities.i18n, |_this, _, cx| cx.notify())
            .detach();

        Self {
            entities,
            expanded: true,
        }
    }

    fn toggle_expanded(&mut self, cx: &mut Context<Self>) {
        self.expanded = !self.expanded;
        cx.notify();
    }

    fn render_log_entry(&self, entry: &crate::state::log_state::LogEntry) -> impl IntoElement {
        let time = format_time_ms(&entry.timestamp);
        let level_color = entry.level.color();
        let level_label = entry.level.label();

        div()
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .py_px()
            .child(
                div()
                    .text_color(DfcColors::text_muted())
                    .text_size(px(11.0))
                    .min_w(px(85.0))
                    .child(time),
            )
            .child(
                div()
                    .text_color(level_color)
                    .text_size(px(11.0))
                    .min_w(px(45.0))
                    .child(level_label),
            )
            .child(
                div()
                    .text_color(DfcColors::text_light())
                    .text_size(px(12.0))
                    .flex_1()
                    .child(entry.message.clone()),
            )
    }
}

impl Render for LogPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.entities.i18n.read(cx).locale;
        let logs = self.entities.logs.read(cx);
        let title = t(locale, "log-title");
        let clear_label = t(locale, "log-clear");

        let height = if self.expanded { px(150.0) } else { px(32.0) };

        let entities = self.entities.clone();

        let mut panel = div()
            .h(height)
            .w_full()
            .bg(DfcColors::log_panel_bg())
            .flex()
            .flex_col()
            // Header
            .child(
                div()
                    .h(px(32.0))
                    .w_full()
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(gpui::rgba(0xffffff22))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_color(DfcColors::text_light())
                                    .text_size(px(13.0))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_color(DfcColors::text_muted())
                                    .text_size(px(11.0))
                                    .child(format!("({})", logs.len())),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            // Clear button
                            .child(
                                div()
                                    .id("clear-logs")
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .text_color(DfcColors::text_muted())
                                    .text_size(px(11.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(gpui::rgba(0xffffff22)))
                                    .on_click(move |_event: &ClickEvent, _window, cx| {
                                        entities.logs.update(cx, |logs, cx| {
                                            logs.clear();
                                            cx.notify();
                                        });
                                    })
                                    .child(clear_label),
                            )
                            // Toggle button
                            .child(
                                div()
                                    .id("toggle-logs")
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .text_color(DfcColors::text_muted())
                                    .text_size(px(11.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(gpui::rgba(0xffffff22)))
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        this.toggle_expanded(cx);
                                    }))
                                    .child(if self.expanded { "▼" } else { "▲" }),
                            ),
                    ),
            );

        // Log entries (only when expanded)
        if self.expanded {
            let entries: Vec<_> = logs.entries().iter().rev().take(50).collect();

            panel = panel.child(
                div()
                    .id("log-entries")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_4()
                    .py_1()
                    .children(entries.into_iter().map(|entry| self.render_log_entry(entry))),
            );
        }

        panel
    }
}
