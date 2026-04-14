//! Application auto-update state and workflows.

use crate::error::Error;
use gpui::{App, Entity, Global};
use tracing::{debug, error, info};

type Result<T, E = Error> = std::result::Result<T, E>;

const GITHUB_RELEASES_LATEST_URL: &str = "https://github.com/cyenxchen/dfc-gui/releases/latest";
const GITHUB_RELEASES_BASE_URL: &str = "https://github.com/cyenxchen/dfc-gui/releases";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTO_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(12 * 60 * 60);
const RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5 * 60);

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub version: semver::Version,
    pub tag_name: String,
    pub body: String,
    pub html_url: String,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    Available(Box<ReleaseInfo>),
    Downloading {
        downloaded: u64,
        total: u64,
    },
    Installing,
    Installed,
    UpToDate,
    Error(String),
}

#[derive(Debug, Clone)]
pub(crate) enum AutoCheckOutcome {
    UpToDate,
    UpdateAvailable,
    Failed,
    Skipped,
    Dismissed,
    TimerReset,
}

enum DownloadMsg {
    Progress { downloaded: u64 },
    Complete { written: u64 },
    Error(String),
}

#[derive(Default)]
pub struct DfcUpdateState {
    pub status: UpdateStatus,
    pub(crate) outcome_tx: Option<futures::channel::mpsc::UnboundedSender<AutoCheckOutcome>>,
    pub(crate) dialog_window: Option<gpui::AnyWindowHandle>,
}

impl DfcUpdateState {
    fn send_outcome(&self, outcome: AutoCheckOutcome) {
        if let Some(tx) = &self.outcome_tx {
            let _ = tx.unbounded_send(outcome);
        }
    }

    fn send_check_outcome(&self, manual: bool, auto_outcome: AutoCheckOutcome) {
        self.send_outcome(if manual {
            AutoCheckOutcome::TimerReset
        } else {
            auto_outcome
        });
    }
}

#[derive(Clone)]
pub struct DfcUpdateStore {
    state: Entity<DfcUpdateState>,
}

impl DfcUpdateStore {
    pub fn new(state: Entity<DfcUpdateState>) -> Self {
        Self { state }
    }

    pub fn state(&self) -> Entity<DfcUpdateState> {
        self.state.clone()
    }
}

impl Global for DfcUpdateStore {}

fn platform_asset_suffix() -> Option<&'static str> {
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        Some("-macos-arm64.dmg")
    } else if cfg!(target_os = "windows") {
        Some("-windows-x86.zip")
    } else {
        None
    }
}

fn get_platform_asset(assets: &[ReleaseAsset]) -> Option<&ReleaseAsset> {
    let suffix = platform_asset_suffix()?;
    assets.iter().find(|asset| asset.name.ends_with(suffix))
}

pub fn has_compatible_download(release: &ReleaseInfo) -> bool {
    get_platform_asset(&release.assets).is_some()
}

pub fn current_version() -> &'static str {
    CURRENT_VERSION
}

fn fetch_latest_release() -> Result<ReleaseInfo> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("DFC-GUI")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let response = client
        .get(GITHUB_RELEASES_LATEST_URL)
        .send()?
        .error_for_status()?;
    let final_url = response.url().clone();
    let html = response.text()?;
    let tag_name = parse_release_tag(final_url.as_str())?;
    let version_str = tag_name.strip_prefix('v').unwrap_or(&tag_name);
    let version = semver::Version::parse(version_str).map_err(|err| Error::Update {
        message: format!("Invalid version tag '{tag_name}': {err}"),
    })?;

    Ok(ReleaseInfo {
        version,
        tag_name: tag_name.clone(),
        body: extract_release_notes(&html).unwrap_or_default(),
        html_url: format!("{GITHUB_RELEASES_BASE_URL}/tag/{tag_name}"),
        assets: build_release_assets(&client, &tag_name),
    })
}

fn parse_release_tag(url: &str) -> Result<String> {
    let marker = "/releases/tag/";
    let index = url.find(marker).ok_or_else(|| Error::Update {
        message: format!("Unexpected latest release redirect URL: {url}"),
    })?;
    let tag = &url[index + marker.len()..];
    let tag = tag.split(['?', '#']).next().unwrap_or(tag);
    if tag.is_empty() {
        return Err(Error::Update {
            message: "Release tag is empty".to_string(),
        });
    }
    Ok(tag.to_string())
}

fn build_release_assets(client: &reqwest::blocking::Client, tag_name: &str) -> Vec<ReleaseAsset> {
    let Some(asset_name) = current_platform_asset_name(tag_name) else {
        return Vec::new();
    };

    let download_url = format!("{GITHUB_RELEASES_BASE_URL}/download/{tag_name}/{asset_name}");
    let Some(size) = fetch_asset_size(client, &download_url) else {
        return Vec::new();
    };

    vec![ReleaseAsset {
        name: asset_name,
        download_url,
        size,
    }]
}

fn current_platform_asset_name(tag_name: &str) -> Option<String> {
    let safe_tag = tag_name.replace('/', "-");
    let suffix = platform_asset_suffix()?;
    Some(format!("dfc-gui-{safe_tag}{suffix}"))
}

fn fetch_asset_size(client: &reqwest::blocking::Client, download_url: &str) -> Option<u64> {
    let response = client
        .head(download_url)
        .send()
        .ok()?
        .error_for_status()
        .ok()?;
    response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

fn extract_release_notes(html: &str) -> Option<String> {
    let marker = "markdown-body";
    let idx = html.find(marker)?;
    let after_marker = &html[idx..];
    let start = after_marker.find('>')? + 1;
    let remainder = &after_marker[start..];
    let end = remainder.find("</div>")?;
    let fragment = &remainder[..end];
    let text = html_to_text(fragment);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn html_to_text(fragment: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut entity = String::new();
    let mut in_entity = false;

    for ch in fragment.chars() {
        match ch {
            '<' => {
                in_tag = true;
                in_entity = false;
            }
            '>' => {
                in_tag = false;
            }
            '&' if !in_tag => {
                in_entity = true;
                entity.clear();
            }
            ';' if in_entity => {
                in_entity = false;
                out.push_str(match entity.as_str() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "#39" => "'",
                    _ => "",
                });
            }
            _ if in_tag => {}
            _ if in_entity => entity.push(ch),
            _ => out.push(ch),
        }
    }

    out.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn download_file(
    url: &str,
    path: &std::path::Path,
    tx: futures::channel::mpsc::UnboundedSender<DownloadMsg>,
) -> Result<u64> {
    use std::io::{Read, Write};

    let client = reqwest::blocking::Client::builder()
        .user_agent("DFC-GUI")
        .timeout(std::time::Duration::from_secs(600))
        .build()?;
    let mut response = client.get(url).send()?.error_for_status()?;
    let mut file = std::fs::File::create(path)?;
    let mut downloaded = 0u64;
    let mut buffer = vec![0u8; 65536];
    let mut last_report = std::time::Instant::now();

    loop {
        let read = response.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])?;
        downloaded += read as u64;

        if last_report.elapsed() >= std::time::Duration::from_millis(50) {
            let _ = tx.unbounded_send(DownloadMsg::Progress { downloaded });
            last_report = std::time::Instant::now();
        }
    }

    Ok(downloaded)
}

/// Check for updates. Manual checks always surface a dialog.
pub fn check_for_updates(manual: bool, cx: &App) {
    let store = cx.global::<DfcUpdateStore>().clone();
    let state_entity = store.state();

    {
        let state = state_entity.read(cx);
        if matches!(
            state.status,
            UpdateStatus::Checking | UpdateStatus::Downloading { .. } | UpdateStatus::Installing
        ) {
            state.send_check_outcome(manual, AutoCheckOutcome::Skipped);
            return;
        }
    }

    if !manual {
        let last_check = cx
            .global::<super::DfcGlobalStore>()
            .read(cx)
            .last_update_check()
            .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok());
        if let Some(last) = last_check {
            let elapsed = chrono::Utc::now().signed_duration_since(last);
            if elapsed.num_hours() < 1 {
                debug!(
                    "Skipping auto-check (last check was {}m ago)",
                    elapsed.num_minutes()
                );
                state_entity
                    .read(cx)
                    .send_outcome(AutoCheckOutcome::Skipped);
                return;
            }
        }
    }

    cx.spawn(async move |cx| {
        let _ = state_entity.update(cx, |state, cx| {
            state.status = UpdateStatus::Checking;
            cx.notify();
        });

        if manual {
            cx.update(crate::views::open_update_dialog).ok();
        }

        let (tx, rx) = futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            let _ = tx.send(fetch_latest_release());
        });

        let result = rx.await.unwrap_or_else(|_| {
            Err(Error::Update {
                message: "Failed to check for updates".to_string(),
            })
        });

        match result {
            Ok(release) => {
                let current = semver::Version::parse(CURRENT_VERSION)
                    .unwrap_or(semver::Version::new(0, 0, 0));
                if release.version > current {
                    if !manual {
                        let skipped = cx
                            .update(|cx| {
                                cx.global::<super::DfcGlobalStore>()
                                    .read(cx)
                                    .skipped_version()
                                    .map(ToString::to_string)
                            })
                            .ok()
                            .flatten();
                        if skipped.as_deref() == Some(release.tag_name.as_str()) {
                            debug!(version = %release.tag_name, "Skipping update (user skipped)");
                            let _ = state_entity.update(cx, |state, cx| {
                                state.status = UpdateStatus::Idle;
                                state.send_outcome(AutoCheckOutcome::Skipped);
                                cx.notify();
                            });
                            return;
                        }
                    }

                    info!(current = %current, new = %release.version, "Update available");
                    let _ = state_entity.update(cx, |state, cx| {
                        state.status = UpdateStatus::Available(Box::new(release));
                        state.send_check_outcome(manual, AutoCheckOutcome::UpdateAvailable);
                        cx.notify();
                    });
                    cx.update(crate::views::open_update_dialog).ok();
                } else {
                    info!("Already up to date ({current})");
                    let _ = state_entity.update(cx, |state, cx| {
                        state.status = UpdateStatus::UpToDate;
                        state.send_check_outcome(manual, AutoCheckOutcome::UpToDate);
                        cx.notify();
                    });
                    if manual {
                        cx.update(crate::views::open_update_dialog).ok();
                    }
                }

                cx.update(|cx| {
                    super::update_app_state_and_save(cx, "save_last_update_check", |state, _cx| {
                        state.set_last_update_check(chrono::Utc::now().to_rfc3339());
                    });
                })
                .ok();
            }
            Err(err) => {
                error!(error = %err, "Failed to check for updates");
                let message = err.to_string();
                let _ = state_entity.update(cx, |state, cx| {
                    state.status = UpdateStatus::Error(message);
                    state.send_check_outcome(manual, AutoCheckOutcome::Failed);
                    cx.notify();
                });
                if manual {
                    cx.update(crate::views::open_update_dialog).ok();
                }
            }
        }
    })
    .detach();
}

/// Download the current available update and install it.
pub fn download_update(cx: &App) {
    let store = cx.global::<DfcUpdateStore>().clone();
    let state_entity = store.state();

    let release = {
        let state = state_entity.read(cx);
        match &state.status {
            UpdateStatus::Available(release) => (**release).clone(),
            _ => return,
        }
    };

    let Some(asset) = get_platform_asset(&release.assets).cloned() else {
        cx.spawn(async move |cx| {
            let _ = state_entity.update(cx, |state, cx| {
                state.status = UpdateStatus::Error(
                    "No compatible download found for this platform".to_string(),
                );
                cx.notify();
            });
        })
        .detach();
        return;
    };

    let url = asset.download_url.clone();
    let file_name = asset.name.clone();
    let total_size = asset.size;

    cx.spawn(async move |cx| {
        let _ = state_entity.update(cx, |state, cx| {
            state.status = UpdateStatus::Downloading {
                downloaded: 0,
                total: total_size,
            };
            cx.notify();
        });

        let download_dir = std::env::temp_dir().join("dfc-gui-update");
        if let Err(err) = std::fs::create_dir_all(&download_dir) {
            let message = format!("Failed to create download directory: {err}");
            let _ = state_entity.update(cx, |state, cx| {
                state.status = UpdateStatus::Error(message);
                cx.notify();
            });
            return;
        }

        let download_path = download_dir.join(&file_name);
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let download_path_clone = download_path.clone();

        std::thread::spawn(move || {
            let result = download_file(&url, &download_path_clone, tx.clone());
            match result {
                Ok(written) => {
                    let _ = tx.unbounded_send(DownloadMsg::Complete { written });
                }
                Err(err) => {
                    let _ = tx.unbounded_send(DownloadMsg::Error(err.to_string()));
                }
            }
        });

        use futures::StreamExt;

        let mut last_percent: u32 = 0;
        while let Some(message) = rx.next().await {
            match message {
                DownloadMsg::Progress { downloaded } => {
                    let percent = if total_size > 0 {
                        (downloaded as f64 / total_size as f64 * 100.0) as u32
                    } else {
                        0
                    };
                    if percent != last_percent {
                        last_percent = percent;
                        let _ = state_entity.update(cx, |state, cx| {
                            state.status = UpdateStatus::Downloading {
                                downloaded,
                                total: total_size,
                            };
                            cx.notify();
                        });
                    }
                }
                DownloadMsg::Complete { written } => {
                    info!(path = ?download_path, "Download complete");
                    if total_size > 0 && written != total_size {
                        let message = format!(
                            "Download size mismatch: expected {total_size} bytes, got {written} bytes"
                        );
                        error!("{message}");
                        let _ = std::fs::remove_file(&download_path);
                        let _ = state_entity.update(cx, |state, cx| {
                            state.status = UpdateStatus::Error(message);
                            cx.notify();
                        });
                        break;
                    }

                    let _ = state_entity.update(cx, |state, cx| {
                        state.status = UpdateStatus::Installing;
                        cx.notify();
                    });

                    let install_path = download_path.clone();
                    let (install_tx, install_rx) = futures::channel::oneshot::channel();
                    std::thread::spawn(move || {
                        let _ = install_tx.send(crate::helpers::install_update(&install_path));
                    });

                    let install_result = install_rx.await.unwrap_or_else(|_| {
                        Err(Error::Update {
                            message: "Install thread panicked".to_string(),
                        })
                    });

                    match install_result {
                        Ok(()) => {
                            info!("Update installed successfully");
                            let _ = state_entity.update(cx, |state, cx| {
                                state.status = UpdateStatus::Installed;
                                cx.notify();
                            });
                        }
                        Err(err) => {
                            let message = format!("Installation failed: {err}");
                            error!("{message}");
                            let _ = state_entity.update(cx, |state, cx| {
                                state.status = UpdateStatus::Error(message);
                                cx.notify();
                            });
                        }
                    }
                    break;
                }
                DownloadMsg::Error(err) => {
                    error!(error = %err, "Download failed");
                    let _ = std::fs::remove_file(&download_path);
                    let _ = state_entity.update(cx, |state, cx| {
                        state.status = UpdateStatus::Error(err);
                        cx.notify();
                    });
                    break;
                }
            }
        }
    })
    .detach();
}

/// Reset update status to idle after the dialog closes.
pub fn reset_status(cx: &App) {
    let store = cx.global::<DfcUpdateStore>().clone();
    let state_entity = store.state();
    cx.spawn(async move |cx| {
        let _ = state_entity.update(cx, |state, cx| {
            if matches!(
                state.status,
                UpdateStatus::Downloading { .. } | UpdateStatus::Installing
            ) {
                return;
            }
            state.status = UpdateStatus::Idle;
            state.dialog_window = None;
            state.send_outcome(AutoCheckOutcome::Dismissed);
            cx.notify();
        });
    })
    .detach();
}

/// Persist the currently offered version as skipped.
pub fn skip_version(cx: &App) {
    let store = cx.global::<DfcUpdateStore>().clone();
    let tag = {
        let state = store.state().read(cx);
        match &state.status {
            UpdateStatus::Available(release) => release.tag_name.clone(),
            _ => return,
        }
    };

    super::update_app_state_and_save(cx, "skip_version", move |state, _cx| {
        state.set_skipped_version(Some(tag.clone()));
    });
}

/// Start the periodic auto-update scheduler.
pub fn start_auto_update_scheduler(cx: &App) {
    let store = cx.global::<DfcUpdateStore>().clone();
    let state_entity = store.state();
    let (tx, mut rx) = futures::channel::mpsc::unbounded();

    cx.spawn(async move |cx| {
        use futures::{FutureExt, StreamExt};

        let _ = state_entity.update(cx, |state, _cx| {
            state.outcome_tx = Some(tx);
        });

        loop {
            while rx.try_recv().is_ok() {}

            cx.update(|cx| check_for_updates(false, cx)).ok();

            let Some(outcome) = rx.next().await else {
                break;
            };

            let mut wait_duration = match outcome {
                AutoCheckOutcome::UpToDate | AutoCheckOutcome::Skipped => AUTO_CHECK_INTERVAL,
                AutoCheckOutcome::Failed => RETRY_INTERVAL,
                AutoCheckOutcome::UpdateAvailable => {
                    loop {
                        let Some(next) = rx.next().await else {
                            return;
                        };
                        if matches!(
                            next,
                            AutoCheckOutcome::Dismissed | AutoCheckOutcome::TimerReset
                        ) {
                            break;
                        }
                    }
                    AUTO_CHECK_INTERVAL
                }
                AutoCheckOutcome::Dismissed | AutoCheckOutcome::TimerReset => AUTO_CHECK_INTERVAL,
            };

            loop {
                futures::select! {
                    signal = rx.next() => {
                        match signal {
                            Some(AutoCheckOutcome::TimerReset | AutoCheckOutcome::Dismissed) => {
                                wait_duration = AUTO_CHECK_INTERVAL;
                                continue;
                            }
                            Some(_) => continue,
                            None => return,
                        }
                    }
                    _ = cx.background_executor().timer(wait_duration).fuse() => {
                        break;
                    }
                }
            }
        }
    })
    .detach();
}

/// Restart the app after a successful update install.
pub fn restart_app(cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        if let Some(app_bundle) = crate::helpers::get_app_bundle_path() {
            let _ = std::process::Command::new("open")
                .arg("-n")
                .arg(app_bundle)
                .spawn();
        }
        cx.quit();
    }

    #[cfg(not(target_os = "macos"))]
    {
        cx.quit();
    }
}
