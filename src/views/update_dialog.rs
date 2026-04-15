//! Standalone update dialog window.

use crate::helpers::{WindowAction, handle_window_action};
use crate::states::i18n_update;
use crate::states::update::{
    DfcUpdateState, DfcUpdateStore, ReleaseInfo, UpdateStatus, check_for_updates, current_version,
    download_update, has_compatible_download, reset_status, restart_app, skip_version,
};
use gpui::{
    App, Bounds, Entity, TitlebarOptions, Window, WindowBounds, WindowKind, WindowOptions,
    prelude::*, px, size,
};
use gpui_component::{
    ActiveTheme, Root,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    v_flex,
};

struct UpdateDialog {
    state: Entity<DfcUpdateState>,
}

impl UpdateDialog {
    fn new(state: Entity<DfcUpdateState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();
        Self { state }
    }

    fn render_available(
        &self,
        release: &ReleaseInfo,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let version_text = format!("{} -> {}", current_version(), release.version);
        let release_notes = if release.body.is_empty() {
            version_text
        } else {
            release.body.clone()
        };
        let can_download = has_compatible_download(release);

        let mut actions = h_flex()
            .gap_2()
            .justify_end()
            .child(
                Button::new("remind-later")
                    .label(i18n_update(cx, "remind_later"))
                    .on_click(|_, window, cx| {
                        reset_status(cx);
                        window.remove_window();
                    }),
            )
            .child(
                Button::new("skip-version")
                    .label(i18n_update(cx, "skip_version"))
                    .on_click(|_, window, cx| {
                        skip_version(cx);
                        reset_status(cx);
                        window.remove_window();
                    }),
            );

        if can_download {
            actions = actions.child(
                Button::new("download-install")
                    .primary()
                    .label(i18n_update(cx, "download_install"))
                    .on_click(|_, _window, cx| {
                        download_update(cx);
                    }),
            );
        }

        let mut layout = v_flex().gap_3().size_full().px_4().pb_4().pt(px(16.0));

        #[cfg(target_os = "macos")]
        {
            layout = layout.pt(px(28.0));
        }

        let mut layout = layout
            .child(
                Label::new(i18n_update(cx, "new_version_available"))
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground),
            )
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        Label::new(i18n_update(cx, "current_version"))
                            .text_sm()
                            .text_color(cx.theme().foreground),
                    )
                    .child(
                        Label::new(current_version())
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new("->")
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(i18n_update(cx, "new_version"))
                            .text_sm()
                            .text_color(cx.theme().foreground),
                    )
                    .child(
                        Label::new(release.version.to_string())
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(cx.theme().link),
                    ),
            )
            .child(
                Label::new(i18n_update(cx, "release_notes"))
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .font_weight(gpui::FontWeight::MEDIUM),
            )
            .child(
                gpui::div()
                    .id("release-notes")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_2()
                    .rounded(px(4.))
                    .bg(cx.theme().muted)
                    .child(
                        Label::new(release_notes)
                            .text_sm()
                            .text_color(cx.theme().foreground),
                    ),
            );

        if !can_download {
            layout = layout.child(
                Label::new(i18n_update(cx, "download_unavailable"))
                    .text_sm()
                    .text_color(cx.theme().danger),
            );
        }

        layout.child(actions)
    }

    fn render_downloading(
        &self,
        downloaded: u64,
        total: u64,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let progress = if total > 0 {
            downloaded as f32 / total as f32
        } else {
            0.0
        };
        let percent = (progress * 100.0) as u32;
        let progress_text = if total > 0 {
            format!(
                "{} / {} ({}%)",
                format_bytes(downloaded),
                format_bytes(total),
                percent
            )
        } else {
            format!("{percent}%")
        };

        v_flex()
            .gap_3()
            .size_full()
            .p_4()
            .items_center()
            .justify_center()
            .child(
                Label::new(i18n_update(cx, "downloading"))
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground),
            )
            .child(
                v_flex()
                    .w_full()
                    .child(
                        gpui::div()
                            .h(px(8.))
                            .w_full()
                            .rounded(px(4.))
                            .bg(cx.theme().muted)
                            .child(
                                gpui::div()
                                    .h_full()
                                    .w(gpui::relative(progress))
                                    .rounded(px(4.))
                                    .bg(cx.theme().primary),
                            ),
                    )
                    .child(
                        h_flex().justify_center().pt_1().child(
                            Label::new(progress_text)
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        ),
                    ),
            )
    }

    fn render_installing(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .size_full()
            .p_4()
            .items_center()
            .justify_center()
            .child(
                Label::new(i18n_update(cx, "installing"))
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground),
            )
            .child(
                Label::new(i18n_update(cx, "please_wait"))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
    }

    fn render_installed(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut layout = v_flex()
            .gap_3()
            .size_full()
            .p_4()
            .items_center()
            .justify_center()
            .child(
                Label::new(i18n_update(cx, "restart_required"))
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground),
            );

        #[cfg(target_os = "macos")]
        {
            layout = layout
                .child(
                    Label::new(i18n_update(cx, "restart_required_detail"))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("restart-later")
                                .label(i18n_update(cx, "restart_later"))
                                .on_click(|_, window, cx| {
                                    reset_status(cx);
                                    window.remove_window();
                                }),
                        )
                        .child(
                            Button::new("restart-now")
                                .primary()
                                .label(i18n_update(cx, "restart_now"))
                                .on_click(|_, _window, cx| {
                                    restart_app(cx);
                                }),
                        ),
                );
        }

        #[cfg(not(target_os = "macos"))]
        {
            layout = layout
                .child(
                    Label::new(i18n_update(cx, "restart_required_detail_manual"))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    Button::new("close-installed")
                        .primary()
                        .label(i18n_update(cx, "close"))
                        .on_click(|_, window, cx| {
                            reset_status(cx);
                            window.remove_window();
                        }),
                );
        }

        layout
    }

    fn render_up_to_date(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .size_full()
            .p_4()
            .items_center()
            .justify_center()
            .child(
                Label::new(format!(
                    "{} {}",
                    i18n_update(cx, "up_to_date"),
                    current_version()
                ))
                .text_lg()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground),
            )
            .child(
                Button::new("close")
                    .label(i18n_update(cx, "close"))
                    .on_click(|_, window, cx| {
                        reset_status(cx);
                        window.remove_window();
                    }),
            )
    }

    fn render_checking(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .size_full()
            .p_4()
            .items_center()
            .justify_center()
            .child(
                Label::new(i18n_update(cx, "checking"))
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground),
            )
            .child(
                Label::new(i18n_update(cx, "please_wait"))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
    }

    fn render_error(
        &self,
        message: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .gap_3()
            .size_full()
            .p_4()
            .items_center()
            .justify_center()
            .child(
                Label::new(i18n_update(cx, "update_error"))
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground),
            )
            .child(
                Label::new(message.to_string())
                    .text_sm()
                    .text_color(cx.theme().danger),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("close-error")
                            .label(i18n_update(cx, "close"))
                            .on_click(|_, window, cx| {
                                reset_status(cx);
                                window.remove_window();
                            }),
                    )
                    .child(
                        Button::new("retry")
                            .primary()
                            .label(i18n_update(cx, "retry"))
                            .on_click(|_, _window, cx| {
                                check_for_updates(true, cx);
                            }),
                    ),
            )
    }
}

impl Render for UpdateDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.state.read(cx).status.clone();

        let content: gpui::AnyElement = match status {
            UpdateStatus::Checking => self.render_checking(window, cx).into_any_element(),
            UpdateStatus::Available(release) => self
                .render_available(&release, window, cx)
                .into_any_element(),
            UpdateStatus::Downloading { downloaded, total } => self
                .render_downloading(downloaded, total, window, cx)
                .into_any_element(),
            UpdateStatus::Installing => self.render_installing(window, cx).into_any_element(),
            UpdateStatus::Installed => self.render_installed(window, cx).into_any_element(),
            UpdateStatus::UpToDate => self.render_up_to_date(window, cx).into_any_element(),
            UpdateStatus::Error(message) => {
                self.render_error(&message, window, cx).into_any_element()
            }
            UpdateStatus::Idle => self.render_up_to_date(window, cx).into_any_element(),
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .capture_action(cx.listener(|_this, e: &WindowAction, window, cx| {
                handle_window_action(e, window);
                cx.stop_propagation();
            }))
            .child(content)
    }
}

pub fn open_update_dialog(cx: &mut App) {
    let store = cx.global::<DfcUpdateStore>().clone();
    let state = store.state();

    if let Some(handle) = state.read(cx).dialog_window {
        if handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
        {
            return;
        }
        let _ = state.update(cx, |state, _cx| {
            state.dialog_window = None;
        });
    }

    let width = px(500.);
    let height = px(420.);
    let window_size = size(width, height);

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            window_size,
            cx,
        ))),
        is_movable: true,
        is_resizable: false,
        titlebar: Some(TitlebarOptions {
            title: Some(i18n_update(cx, "check_for_updates")),
            appears_transparent: true,
            ..Default::default()
        }),
        focus: true,
        kind: WindowKind::Normal,
        ..Default::default()
    };

    if let Ok(window_handle) = cx.open_window(options, |window, cx| {
        window.on_window_should_close(cx, |_window, cx| {
            reset_status(cx);
            true
        });
        let dialog = cx.new(|cx| UpdateDialog::new(state.clone(), window, cx));
        cx.new(|cx| Root::new(dialog, window, cx))
    }) {
        let _ = state.update(cx, |state, _cx| {
            state.dialog_window = Some(window_handle.into());
        });
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
