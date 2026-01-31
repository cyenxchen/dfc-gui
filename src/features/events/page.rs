//! Events Page
//!
//! Displays device events in a data table.

use gpui::{
    div, prelude::*, ClickEvent, Context, Entity, IntoElement, ParentElement,
    Render, Styled, Window,
};

use crate::app::entities::AppEntities;
use crate::components::composite::data_table::column::Column;
use crate::components::composite::data_table::data_table::DataTable;
use crate::components::primitives::button::Button;
use crate::domain::event_log::EventLog;
use crate::features::events::controller::EventsController;
use crate::i18n::{t, Locale};
use crate::theme::colors::DfcColors;
use crate::utils::format::format_datetime;

/// Events page component
pub struct EventsPage {
    entities: AppEntities,
    controller: EventsController,
    table: Entity<DataTable<EventLog>>,
}

impl EventsPage {
    pub fn new(entities: AppEntities, cx: &mut Context<Self>) -> Self {
        let controller = EventsController::new(entities.clone());

        // Create table with columns
        let table = cx.new(|cx| {
            let mut table = DataTable::<EventLog>::new(cx);
            table.set_columns(Self::create_columns(Locale::ZhCN));
            table
        });

        // Observe events state
        let table_clone = table.clone();
        cx.observe(&entities.events, move |_this, events, cx| {
            // Extract data first to avoid borrow conflicts
            let (rows, loading) = {
                let evts = events.read(cx);
                (evts.events.clone(), evts.loading)
            };
            table_clone.update(cx, |table, cx| {
                table.set_rows(rows);
                table.set_loading(loading);
                cx.notify();
            });
        })
        .detach();

        // Observe i18n changes
        let table_clone = table.clone();
        cx.observe(&entities.i18n, move |_this, i18n, cx| {
            let locale = {
                i18n.read(cx).locale
            };
            table_clone.update(cx, |table, cx| {
                table.set_columns(Self::create_columns(locale));
                cx.notify();
            });
        })
        .detach();

        Self {
            entities,
            controller,
            table,
        }
    }

    fn create_columns(locale: Locale) -> Vec<Column<EventLog>> {
        vec![
            Column::new(
                "device_id",
                t(locale, "col-device-id"),
                |row: &EventLog| {
                    div()
                        .text_sm()
                        .child(row.device_id.clone())
                        .into_any_element()
                },
            )
            .fixed_width(100.0),
            Column::new(
                "event_code",
                t(locale, "col-event-code"),
                |row: &EventLog| {
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(row.event_code.clone())
                        .into_any_element()
                },
            )
            .fixed_width(100.0),
            Column::new(
                "description",
                t(locale, "col-description"),
                |row: &EventLog| {
                    div()
                        .text_sm()
                        .child(row.description.clone())
                        .into_any_element()
                },
            )
            .fixed_width(200.0),
            Column::new("level", t(locale, "col-level"), |row: &EventLog| {
                let (color, label) = match row.level {
                    0 => (DfcColors::info(), "Info"),
                    1 => (DfcColors::warning(), "Warning"),
                    2 => (DfcColors::danger(), "Error"),
                    3 => (gpui::rgba(0x7c3aedff), "Critical"),
                    _ => (DfcColors::text_muted(), "Unknown"),
                };
                div()
                    .text_sm()
                    .text_color(color)
                    .child(label)
                    .into_any_element()
            })
            .fixed_width(80.0),
            Column::new("state", t(locale, "col-state"), |row: &EventLog| {
                let (color, label) = if row.state == 0 {
                    (DfcColors::danger(), "Active")
                } else {
                    (DfcColors::success(), "Cleared")
                };
                div()
                    .text_sm()
                    .text_color(color)
                    .child(label)
                    .into_any_element()
            })
            .fixed_width(80.0),
            Column::new(
                "event_time",
                t(locale, "col-event-time"),
                |row: &EventLog| {
                    div()
                        .text_sm()
                        .text_color(DfcColors::text_secondary())
                        .child(format_datetime(&row.event_time))
                        .into_any_element()
                },
            )
            .fixed_width(160.0),
        ]
    }
}

impl Render for EventsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.entities.i18n.read(cx).locale;
        let events = self.entities.events.read(cx);
        let count = events.events.len();

        div()
            .size_full()
            .flex()
            .flex_col()
            .p_4()
            .gap_4()
            // Header
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(format!("{} {}", count, t(locale, "nav-events"))),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::primary("refresh-btn", t(locale, "action-refresh"))
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        this.controller.refresh(cx);
                                    })),
                            ),
                    ),
            )
            // Table
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.table.clone()),
            )
    }
}
