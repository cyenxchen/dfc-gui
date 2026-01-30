//! Modal Component
//!
//! A modal dialog component.

use gpui::{
    div, prelude::*, px, App, ClickEvent, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window,
};

use crate::theme::colors::DfcColors;

/// Modal component
#[derive(IntoElement)]
pub struct Modal {
    title: SharedString,
    children: Vec<gpui::AnyElement>,
    on_close: Option<Box<dyn Fn(&mut App) + 'static>>,
    show_close_button: bool,
}

impl Modal {
    /// Create a new modal
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            children: Vec::new(),
            on_close: None,
            show_close_button: true,
        }
    }

    /// Add a child element
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// Set the close handler
    pub fn on_close(mut self, handler: impl Fn(&mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }

    /// Hide the close button
    pub fn hide_close_button(mut self) -> Self {
        self.show_close_button = false;
        self
    }
}

impl RenderOnce for Modal {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_close = self.on_close;

        // Backdrop
        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000088))
            .flex()
            .items_center()
            .justify_center()
            .child(
                // Modal container
                div()
                    .bg(DfcColors::content_bg())
                    .rounded_lg()
                    .shadow_lg()
                    .min_w(px(400.0))
                    .max_w(px(600.0))
                    .flex()
                    .flex_col()
                    // Header
                    .child(
                        div()
                            .px_6()
                            .py_4()
                            .border_b_1()
                            .border_color(DfcColors::border())
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(DfcColors::text_primary())
                                    .child(self.title),
                            )
                            .when(self.show_close_button, |el| {
                                el.child(
                                    div()
                                        .id("modal-close")
                                        .size(px(24.0))
                                        .rounded_sm()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(DfcColors::text_muted())
                                        .text_size(px(16.0))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(DfcColors::table_row_hover()))
                                        .when_some(on_close, |el, handler| {
                                            el.on_click(move |_event: &ClickEvent, _window, cx| {
                                                handler(cx);
                                            })
                                        })
                                        .child("×"),
                                )
                            }),
                    )
                    // Content
                    .child(
                        div()
                            .px_6()
                            .py_4()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .children(self.children),
                    ),
            )
    }
}
