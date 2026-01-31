//! TextInput Component

use gpui::{
    div, prelude::*, px, Context, ElementId, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, Styled, Window,
};

use crate::theme::colors::DfcColors;

/// A text input component
pub struct TextInput {
    id: ElementId,
    value: String,
    placeholder: SharedString,
    disabled: bool,
    focus_handle: FocusHandle,
    on_change: Option<Box<dyn Fn(&str, &mut Context<Self>) + 'static>>,
}

impl TextInput {
    /// Create a new text input
    pub fn new(id: impl Into<ElementId>, cx: &mut Context<Self>) -> Self {
        Self {
            id: id.into(),
            value: String::new(),
            placeholder: SharedString::default(),
            disabled: false,
            focus_handle: cx.focus_handle(),
            on_change: None,
        }
    }

    /// Set the value
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
    }

    /// Get the value
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Set the placeholder
    pub fn set_placeholder(&mut self, placeholder: impl Into<SharedString>) {
        self.placeholder = placeholder.into();
    }

    /// Set disabled state
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    /// Set the change handler
    pub fn on_change(&mut self, handler: impl Fn(&str, &mut Context<Self>) + 'static) {
        self.on_change = Some(Box::new(handler));
    }

    /// Handle text input
    #[allow(dead_code)]
    fn handle_input(&mut self, text: &str, cx: &mut Context<Self>) {
        self.value.push_str(text);
        if let Some(ref handler) = self.on_change {
            handler(&self.value, cx);
        }
        cx.notify();
    }

    /// Handle backspace
    #[allow(dead_code)]
    fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        self.value.pop();
        if let Some(ref handler) = self.on_change {
            handler(&self.value, cx);
        }
        cx.notify();
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus_handle.is_focused(window);
        let border_color = if is_focused {
            DfcColors::border_focus()
        } else {
            DfcColors::input_border()
        };

        let display_text = if self.value.is_empty() {
            self.placeholder.clone()
        } else {
            SharedString::from(self.value.clone())
        };

        let text_color = if self.value.is_empty() {
            DfcColors::input_placeholder()
        } else {
            DfcColors::text_primary()
        };

        div()
            .id(self.id.clone())
            .track_focus(&self.focus_handle)
            .px_3()
            .py_2()
            .bg(DfcColors::input_bg())
            .border_1()
            .border_color(border_color)
            .rounded_md()
            .text_color(text_color)
            .text_sm()
            .min_w(px(200.0))
            .child(display_text)
    }
}

/// Create a simple text input entity
pub fn text_input<V: 'static>(
    id: impl Into<ElementId>,
    value: impl Into<String>,
    placeholder: impl Into<SharedString>,
    cx: &mut Context<V>,
) -> Entity<TextInput> {
    let id = id.into();
    let value = value.into();
    let placeholder = placeholder.into();

    cx.new(|cx| {
        let mut input = TextInput::new(id, cx);
        input.set_value(value);
        input.set_placeholder(placeholder);
        input
    })
}
