//! Pagination Component
//!
//! Page navigation for the DataTable.

use gpui::{
    div, prelude::*, px, App, ClickEvent, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window,
};

use crate::theme::colors::DfcColors;

/// Pagination component
#[derive(IntoElement)]
pub struct Pagination {
    current_page: usize,
    total_pages: usize,
    total_items: usize,
    items_label: SharedString,
    on_page_change: Option<Box<dyn Fn(usize, &mut App) + 'static>>,
}

impl Pagination {
    /// Create a new pagination component
    pub fn new(current_page: usize, total_pages: usize, total_items: usize) -> Self {
        Self {
            current_page,
            total_pages,
            total_items,
            items_label: "items".into(),
            on_page_change: None,
        }
    }

    /// Set the items label
    pub fn items_label(mut self, label: impl Into<SharedString>) -> Self {
        self.items_label = label.into();
        self
    }

    /// Set the page change handler
    pub fn on_page_change(mut self, handler: impl Fn(usize, &mut App) + 'static) -> Self {
        self.on_page_change = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Pagination {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let current = self.current_page;
        let total = self.total_pages;
        let can_prev = current > 1;
        let can_next = current < total;

        div()
            .w_full()
            .px_4()
            .py_2()
            .flex()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(DfcColors::border())
            // Item count
            .child(
                div()
                    .text_sm()
                    .text_color(DfcColors::text_secondary())
                    .child(format!("{} {}", self.total_items, self.items_label)),
            )
            // Page navigation
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    // Previous button
                    .child({
                        let on_change = self.on_page_change.as_ref().map(|h| {
                            let handler = unsafe {
                                std::mem::transmute::<
                                    &Box<dyn Fn(usize, &mut App) + 'static>,
                                    &'static Box<dyn Fn(usize, &mut App) + 'static>,
                                >(h)
                            };
                            handler
                        });

                        let mut btn = div()
                            .id("prev-page")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_sm()
                            .text_color(if can_prev {
                                DfcColors::text_primary()
                            } else {
                                DfcColors::text_muted()
                            })
                            .child("←");

                        if can_prev {
                            btn = btn
                                .cursor_pointer()
                                .hover(|s| s.bg(DfcColors::table_row_hover()));
                        }

                        btn
                    })
                    // Page info
                    .child(
                        div()
                            .text_sm()
                            .text_color(DfcColors::text_primary())
                            .child(format!("{} / {}", current, total)),
                    )
                    // Next button
                    .child({
                        let mut btn = div()
                            .id("next-page")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_sm()
                            .text_color(if can_next {
                                DfcColors::text_primary()
                            } else {
                                DfcColors::text_muted()
                            })
                            .child("→");

                        if can_next {
                            btn = btn
                                .cursor_pointer()
                                .hover(|s| s.bg(DfcColors::table_row_hover()));
                        }

                        btn
                    }),
            )
    }
}
