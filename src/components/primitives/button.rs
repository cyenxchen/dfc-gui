//! Button Component

use gpui::{
    div, prelude::*, px, App, AnyElement, ClickEvent, ElementId, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
};

use crate::theme::colors::DfcColors;

/// Button variant
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Primary action button (yellow)
    #[default]
    Primary,
    /// Secondary button (gray)
    Secondary,
    /// Danger button (pink/red)
    Danger,
    /// Ghost button (transparent)
    Ghost,
}

/// Button size
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonSize {
    /// Small button
    Small,
    /// Medium button (default)
    #[default]
    Medium,
    /// Large button
    Large,
}

/// A styled button component
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
    loading: bool,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl Button {
    /// Create a new button
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant: ButtonVariant::Primary,
            size: ButtonSize::Medium,
            disabled: false,
            loading: false,
            on_click: None,
        }
    }

    /// Set the button variant
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set the button size
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Set whether the button is disabled
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set whether the button is loading
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Set the click handler
    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// Create a primary button
    pub fn primary(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self::new(id, label).variant(ButtonVariant::Primary)
    }

    /// Create a secondary button
    pub fn secondary(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self::new(id, label).variant(ButtonVariant::Secondary)
    }

    /// Create a danger button
    pub fn danger(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self::new(id, label).variant(ButtonVariant::Danger)
    }

    /// Create a ghost button
    pub fn ghost(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self::new(id, label).variant(ButtonVariant::Ghost)
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (bg_color, text_color, hover_bg) = match self.variant {
            ButtonVariant::Primary => (
                DfcColors::button_primary_bg(),
                DfcColors::button_primary_text(),
                gpui::rgba(0xd4a817ff),
            ),
            ButtonVariant::Secondary => (
                gpui::rgba(0xe5e7ebff),
                DfcColors::text_primary(),
                gpui::rgba(0xd1d5dbff),
            ),
            ButtonVariant::Danger => (
                DfcColors::button_danger_bg(),
                DfcColors::button_danger_text(),
                gpui::rgba(0xdb2777ff),
            ),
            ButtonVariant::Ghost => (
                gpui::rgba(0x00000000),
                DfcColors::button_ghost_text(),
                gpui::rgba(0xf3f4f6ff),
            ),
        };

        let (padding_x, padding_y, font_size) = match self.size {
            ButtonSize::Small => (px(8.0), px(4.0), px(12.0)),
            ButtonSize::Medium => (px(16.0), px(8.0), px(14.0)),
            ButtonSize::Large => (px(24.0), px(12.0), px(16.0)),
        };

        let opacity = if self.disabled || self.loading {
            0.5
        } else {
            1.0
        };

        let label = if self.loading {
            "Loading...".into()
        } else {
            self.label
        };

        let mut element = div()
            .id(self.id)
            .px(padding_x)
            .py(padding_y)
            .bg(bg_color)
            .text_color(text_color)
            .text_size(font_size)
            .rounded_md()
            .cursor_pointer()
            .opacity(opacity)
            .child(label);

        if !self.disabled && !self.loading {
            element = element.hover(|s| s.bg(hover_bg));

            if let Some(handler) = self.on_click {
                element = element.on_click(handler);
            }
        }

        element
    }
}
