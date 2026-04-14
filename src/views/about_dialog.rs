//! About dialog window.

use crate::states::i18n_about;
use chrono::{Datelike, Local};
use gpui::{
    App, Bounds, TitlebarOptions, Window, WindowBounds, WindowKind, WindowOptions, prelude::*, px,
    size,
};
use gpui_component::{ActiveTheme, Root, h_flex, label::Label, v_flex};

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

struct AboutDialog;

impl AboutDialog {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for AboutDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let year = Local::now().year().to_string();
        let years = if year == "2026" {
            "2026".to_string()
        } else {
            format!("2026 - {year}")
        };

        v_flex()
            .size_full()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .gap_4()
            .child(
                h_flex().items_center().justify_center().child(
                    Label::new("DFC-GUI")
                        .text_xl()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(cx.theme().foreground),
                ),
            )
            .child(
                Label::new(format!("{} {}", i18n_about(cx, "version"), PKG_VERSION))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                Label::new(i18n_about(cx, "description"))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                Label::new(format!("© {years} Goldwind. All rights reserved."))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
    }
}

pub fn open_about_dialog(cx: &mut App) {
    let width = px(360.);
    let height = px(220.);
    let window_size = size(width, height);

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            window_size,
            cx,
        ))),
        is_movable: false,
        is_resizable: false,
        titlebar: Some(TitlebarOptions {
            title: Some(i18n_about(cx, "title")),
            appears_transparent: true,
            ..Default::default()
        }),
        focus: true,
        kind: WindowKind::Normal,
        ..Default::default()
    };

    let _ = cx.open_window(options, |window, cx| {
        let dialog = cx.new(|cx| AboutDialog::new(window, cx));
        cx.new(|cx| Root::new(dialog, window, cx))
    });
}
