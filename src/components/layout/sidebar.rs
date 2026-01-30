//! Sidebar Component
//!
//! Navigation sidebar with page links.

use gpui::{
    div, prelude::*, px, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, VisualContext, Window,
};

use crate::app::entities::AppEntities;
use crate::app::navigation::ActivePage;
use crate::i18n::{t, Locale};
use crate::theme::colors::DfcColors;

/// Sidebar component
pub struct Sidebar {
    entities: AppEntities,
    collapsed: bool,
}

impl Sidebar {
    pub fn new(entities: AppEntities, cx: &mut Context<Self>) -> Self {
        // Observe i18n changes
        cx.observe(&entities.i18n, |_this, _, cx| cx.notify())
            .detach();

        // Observe tabs changes
        cx.observe(&entities.tabs, |_this, _, cx| cx.notify())
            .detach();

        Self {
            entities,
            collapsed: false,
        }
    }

    fn render_nav_item(
        &self,
        page: ActivePage,
        locale: Locale,
        active_page: ActivePage,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let is_active = page == active_page;
        let label = t(locale, page.title_key());
        let entities = self.entities.clone();

        let bg_color = if is_active {
            gpui::rgba(0x2cb3b822)
        } else {
            gpui::rgba(0x00000000)
        };

        let text_color = if is_active {
            DfcColors::header_bg()
        } else {
            DfcColors::text_secondary()
        };

        let border_color = if is_active {
            DfcColors::header_bg()
        } else {
            gpui::rgba(0x00000000)
        };

        div()
            .id(SharedString::from(format!("nav-{:?}", page)))
            .w_full()
            .px_4()
            .py_2()
            .bg(bg_color)
            .border_l_2()
            .border_color(border_color)
            .text_color(text_color)
            .text_size(px(14.0))
            .cursor_pointer()
            .hover(|s| s.bg(gpui::rgba(0x2cb3b811)))
            .on_click(move |_event: &ClickEvent, _window, cx| {
                entities.tabs.update(cx, |tabs, cx| {
                    tabs.set_active_page(page);
                    cx.notify();
                });
            })
            .child(label)
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.entities.i18n.read(cx).locale;
        let active_page = self.entities.tabs.read(cx).active_page;

        let width = if self.collapsed { px(48.0) } else { px(180.0) };

        div()
            .w(width)
            .h_full()
            .bg(DfcColors::sidebar_bg())
            .border_r_1()
            .border_color(DfcColors::border())
            .flex()
            .flex_col()
            .pt_4()
            .children(
                ActivePage::all()
                    .iter()
                    .map(|page| self.render_nav_item(*page, locale, active_page, cx)),
            )
    }
}
