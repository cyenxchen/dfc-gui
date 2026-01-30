//! Shell Component
//!
//! The main application shell that wraps the entire layout.

use gpui::{div, prelude::*, px, App, IntoElement, ParentElement, RenderOnce, Styled, Window};

use crate::theme::colors::DfcColors;

/// Application shell wrapper
#[derive(IntoElement)]
pub struct Shell {
    children: Vec<gpui::AnyElement>,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Shell {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(DfcColors::background())
            .children(self.children)
    }
}
