//! Title Bar Component
//!
//! Custom title bar with settings menu and branding.

use crate::states::{
    DfcGlobalStore, FontSize, FontSizeAction, LocaleAction, SettingsAction, ThemeAction,
    i18n_sidebar,
};
use gpui::{App, Context, Corner, Window, prelude::*};
use gpui_component::{
    Icon, IconName, Sizable, ThemeMode, TitleBar,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    menu::{DropdownMenu, PopupMenu},
};

/// Title bar component
pub struct DfcTitleBar;

impl DfcTitleBar {
    /// Create a new title bar
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }

    /// Render the settings dropdown menu
    fn render_settings_menu(menu: PopupMenu, _window: &mut Window, cx: &App) -> PopupMenu {
        let store = cx.global::<DfcGlobalStore>().read(cx);
        let (font_size, locale, theme) = (store.font_size(), store.locale(), store.theme());

        menu
            // Font size section
            .label(i18n_sidebar(cx, "font_size"))
            .menu_with_check(
                i18n_sidebar(cx, "font_size_large"),
                font_size == FontSize::Large,
                Box::new(FontSizeAction::Large),
            )
            .menu_with_check(
                i18n_sidebar(cx, "font_size_medium"),
                font_size == FontSize::Medium,
                Box::new(FontSizeAction::Medium),
            )
            .menu_with_check(
                i18n_sidebar(cx, "font_size_small"),
                font_size == FontSize::Small,
                Box::new(FontSizeAction::Small),
            )
            .separator()
            // Language section
            .label(i18n_sidebar(cx, "language"))
            .menu_with_check("中文", locale == "zh", Box::new(LocaleAction::Zh))
            .menu_with_check("English", locale == "en", Box::new(LocaleAction::En))
            .separator()
            // Theme section
            .label(i18n_sidebar(cx, "theme"))
            .menu_with_check(
                i18n_sidebar(cx, "light"),
                theme == Some(ThemeMode::Light),
                Box::new(ThemeAction::Light),
            )
            .menu_with_check(
                i18n_sidebar(cx, "dark"),
                theme == Some(ThemeMode::Dark),
                Box::new(ThemeAction::Dark),
            )
            .menu_with_check(
                i18n_sidebar(cx, "system"),
                theme.is_none(),
                Box::new(ThemeAction::System),
            )
            .separator()
            // Other settings
            .menu_element_with_icon(
                Icon::new(IconName::Settings2),
                Box::new(SettingsAction::Open),
                move |_window, cx| Label::new(i18n_sidebar(cx, "settings")),
            )
    }
}

impl Render for DfcTitleBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Right side actions
        let right_actions = h_flex()
            .items_center()
            .justify_end()
            .px_2()
            .gap_2()
            .mr_2();

        TitleBar::new()
            // Left side - app name/logo placeholder
            .child(
                h_flex()
                    .flex_1()
                    .items_center()
                    .pl_4()
                    .child(Label::new("DFC-GUI").text_sm()),
            )
            // Right side - settings and info
            .child(
                right_actions
                    .child(
                        Button::new("settings")
                            .tooltip(i18n_sidebar(cx, "settings"))
                            .icon(IconName::Settings2)
                            .small()
                            .ghost()
                            .dropdown_menu(move |menu, window, cx| Self::render_settings_menu(menu, window, cx))
                            .anchor(Corner::TopRight),
                    )
                    .child(
                        Button::new("info")
                            .tooltip(i18n_sidebar(cx, "about"))
                            .icon(IconName::Info)
                            .small()
                            .ghost()
                            .on_click(|_, _, _cx| {
                                // TODO: Open about dialog
                            }),
                    ),
            )
    }
}
