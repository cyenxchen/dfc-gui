//! Properties Page
//!
//! Displays device properties in a data table.

use gpui::{
    div, prelude::*, ClickEvent, Context, Entity, IntoElement, ParentElement,
    Render, Styled, Window,
};

use crate::app::entities::AppEntities;
use crate::components::composite::data_table::column::Column;
use crate::components::composite::data_table::data_table::DataTable;
use crate::components::primitives::button::Button;
use crate::domain::property::Property;
use crate::features::properties::controller::PropertiesController;
use crate::i18n::{t, Locale};
use crate::theme::colors::DfcColors;
use crate::utils::format::format_datetime;

/// Properties page component
pub struct PropertiesPage {
    entities: AppEntities,
    controller: PropertiesController,
    table: Entity<DataTable<Property>>,
}

impl PropertiesPage {
    pub fn new(entities: AppEntities, cx: &mut Context<Self>) -> Self {
        let controller = PropertiesController::new(entities.clone());

        // Create table with columns
        let table = cx.new(|cx| {
            let mut table = DataTable::<Property>::new(cx);
            table.set_columns(Self::create_columns(Locale::ZhCN));
            table
        });

        // Observe properties state
        let table_clone = table.clone();
        cx.observe(&entities.properties, move |_this, properties, cx| {
            // Extract data first to avoid borrow conflicts
            let (rows, loading) = {
                let props = properties.read(cx);
                (props.properties.clone(), props.loading)
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

    fn create_columns(locale: Locale) -> Vec<Column<Property>> {
        vec![
            Column::new(
                "device_id",
                t(locale, "col-device-id"),
                |row: &Property| {
                    div()
                        .text_sm()
                        .child(row.device_id.clone())
                        .into_any_element()
                },
            )
            .fixed_width(100.0),
            Column::new("name", t(locale, "col-name"), |row: &Property| {
                div()
                    .text_sm()
                    .child(row.name.clone())
                    .into_any_element()
            })
            .fixed_width(300.0),
            Column::new("topic", t(locale, "col-topic"), |row: &Property| {
                div()
                    .text_sm()
                    .text_color(DfcColors::text_secondary())
                    .child(row.topic.clone())
                    .into_any_element()
            })
            .fixed_width(150.0),
            Column::new("value", t(locale, "col-value"), |row: &Property| {
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(row.value.clone())
                    .into_any_element()
            })
            .fixed_width(100.0),
            Column::new("quality", t(locale, "col-quality"), |row: &Property| {
                let (color, label) = if row.quality == 0 {
                    (DfcColors::success(), "Good")
                } else {
                    (DfcColors::danger(), "Bad")
                };
                div()
                    .text_sm()
                    .text_color(color)
                    .child(label)
                    .into_any_element()
            })
            .fixed_width(80.0),
            Column::new("data_time", t(locale, "col-data-time"), |row: &Property| {
                div()
                    .text_sm()
                    .text_color(DfcColors::text_secondary())
                    .child(format_datetime(&row.data_time))
                    .into_any_element()
            })
            .fixed_width(160.0),
        ]
    }
}

impl Render for PropertiesPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.entities.i18n.read(cx).locale;
        let props = self.entities.properties.read(cx);
        let count = props.properties.len();

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
                                    .child(format!("{} {}", count, t(locale, "nav-properties"))),
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
