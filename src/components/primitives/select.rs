//! Select Component

use gpui::{
    div, prelude::*, px, App, ElementId, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window,
};

use crate::theme::colors::DfcColors;

/// A select option
#[derive(Debug, Clone)]
pub struct SelectOption {
    pub value: String,
    pub label: SharedString,
}

impl SelectOption {
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

/// A select/dropdown component
#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    selected: Option<String>,
    options: Vec<SelectOption>,
    placeholder: SharedString,
    disabled: bool,
    // Note: Actual dropdown requires more complex state management
    // This is a simplified version that just shows the current selection
}

impl Select {
    /// Create a new select
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            selected: None,
            options: Vec::new(),
            placeholder: "Select...".into(),
            disabled: false,
        }
    }

    /// Set the selected value
    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.selected = Some(value.into());
        self
    }

    /// Set the options
    pub fn options(mut self, options: Vec<SelectOption>) -> Self {
        self.options = options;
        self
    }

    /// Set the placeholder
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for Select {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let display_text = self
            .selected
            .as_ref()
            .and_then(|val| {
                self.options
                    .iter()
                    .find(|opt| &opt.value == val)
                    .map(|opt| opt.label.clone())
            })
            .unwrap_or(self.placeholder);

        let text_color = if self.selected.is_some() {
            DfcColors::text_primary()
        } else {
            DfcColors::input_placeholder()
        };

        let opacity = if self.disabled { 0.5 } else { 1.0 };

        div()
            .id(self.id)
            .px_3()
            .py_2()
            .bg(DfcColors::input_bg())
            .border_1()
            .border_color(DfcColors::input_border())
            .rounded_md()
            .text_color(text_color)
            .text_sm()
            .min_w(px(150.0))
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .opacity(opacity)
            .child(display_text)
            .child(
                div()
                    .text_color(DfcColors::text_muted())
                    .text_size(px(10.0))
                    .child("▼"),
            )
    }
}
