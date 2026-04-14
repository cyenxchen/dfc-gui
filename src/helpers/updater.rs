//! Platform-specific auto-update installers.

use crate::error::Error;
use std::path::Path;
use std::process::Command;
use tracing::{debug, info};

type Result<T, E = Error> = std::result::Result<T, E>;

/// Install a downloaded update archive using platform-specific logic.
pub fn install_update(downloaded_path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        install_macos(downloaded_path)
    }
    #[cfg(target_os = "windows")]
    {
        install_windows(downloaded_path)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = downloaded_path;
        Err(Error::Update {
            message: "Unsupported platform for auto-update".to_string(),
        })
    }
}

#[cfg(target_os = "macos")]
fn install_macos(dmg_path: &Path) -> Result<()> {
    let app_bundle = super::get_app_bundle_path().ok_or_else(|| Error::Update {
        message: "Cannot determine app bundle path".to_string(),
    })?;

    info!(bundle = ?app_bundle, "Installing update to app bundle");

    let mount_point = std::env::temp_dir().join("dfc-gui-update-mount");
    if mount_point.exists() {
        let _ = Command::new("hdiutil")
            .args(["detach", "-force"])
            .arg(&mount_point)
            .output();
    }

    let output = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-mountpoint"])
        .arg(&mount_point)
        .arg(dmg_path)
        .output()?;
    if !output.status.success() {
        return Err(Error::Update {
            message: format!(
                "Failed to mount DMG: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    debug!("DMG mounted at {:?}", mount_point);

    let source = mount_point.join("DFC-GUI.app");
    if !source.exists() {
        let _ = Command::new("hdiutil")
            .args(["detach", "-force"])
            .arg(&mount_point)
            .output();
        return Err(Error::Update {
            message: "DFC-GUI.app not found in DMG".to_string(),
        });
    }

    let output = Command::new("rsync")
        .args(["-a", "--delete"])
        .arg(format!("{}/", source.display()))
        .arg(format!("{}/", app_bundle.display()))
        .output()?;
    if !output.status.success() {
        let _ = Command::new("hdiutil")
            .args(["detach", "-force"])
            .arg(&mount_point)
            .output();
        return Err(Error::Update {
            message: format!(
                "Failed to copy app bundle: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    info!("App bundle updated via rsync");

    let _ = Command::new("hdiutil")
        .args(["detach", "-force"])
        .arg(&mount_point)
        .output();
    let _ = std::fs::remove_file(dmg_path);

    Ok(())
}

#[cfg(target_os = "windows")]
fn install_windows(zip_path: &Path) -> Result<()> {
    let extract_dir = std::env::temp_dir().join("dfc-gui-update-extract");
    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir)?;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                zip_path.display(),
                extract_dir.display()
            ),
        ])
        .output()?;
    if !output.status.success() {
        return Err(Error::Update {
            message: format!(
                "Failed to extract zip: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    let exe = std::env::current_exe()?;
    let new_exe = extract_dir.join("dfc-gui.exe");
    if !new_exe.exists() {
        return Err(Error::Update {
            message: "dfc-gui.exe not found in archive".to_string(),
        });
    }

    let old_exe = exe.with_extension("exe.old");
    let _ = std::fs::remove_file(&old_exe);
    std::fs::rename(&exe, &old_exe)?;
    if let Err(err) = std::fs::copy(&new_exe, &exe) {
        let _ = std::fs::rename(&old_exe, &exe);
        return Err(Error::Update {
            message: format!("Failed to replace executable: {err}"),
        });
    }

    let _ = std::fs::remove_dir_all(&extract_dir);
    let _ = std::fs::remove_file(zip_path);

    Ok(())
}
