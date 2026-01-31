//! Checkbox Component

use gpui::{
    div, px, App, ElementId, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
};

use crate::theme::colors::DfcColors;

/// A checkbox component
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    label: Option<SharedString>,
    disabled: bool,
    on_change: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
}

impl Checkbox {
    /// Create a new checkbox
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            label: None,
            disabled: false,
            on_change: None,
        }
    }

    /// Set the checked state
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Set the label
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the change handler
    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let checked = self.checked;
        let disabled = self.disabled;
        let on_change = self.on_change;

        let checkbox_bg = if checked {
            DfcColors::accent_blue()
        } else {
            DfcColors::input_bg()
        };

        let border_color = if checked {
            DfcColors::accent_blue()
        } else {
            DfcColors::input_border()
        };

        let check_mark = if checked { "✓" } else { "" };

        let mut checkbox = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .child(
                div()
                    .size(px(18.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(border_color)
                    .bg(checkbox_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(DfcColors::text_light())
                    .text_size(px(12.0))
                    .child(check_mark),
            );

        if let Some(label) = self.label {
            checkbox = checkbox.child(
                div()
                    .text_sm()
                    .text_color(DfcColors::text_primary())
                    .child(label),
            );
        }

        if !disabled {
            if let Some(handler) = on_change {
                checkbox = checkbox.on_click(move |_event, window, cx| {
                    handler(!checked, window, cx);
                });
            }
        } else {
            checkbox = checkbox.opacity(0.5);
        }

        checkbox
    }
}
