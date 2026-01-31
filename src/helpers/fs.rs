//! File System Utilities
//!
//! Configuration directory management and file operations.

use crate::error::{Error, Result};
use directories::ProjectDirs;
use home::home_dir;
use std::fs;
use std::path::{Path, PathBuf};

/// Get or create the application's configuration directory
///
/// Platform-specific locations:
/// - **Linux**: `~/.config/dfc-gui/` or `$XDG_CONFIG_HOME/dfc-gui/`
/// - **macOS**: `~/Library/Application Support/com.goldwind.dfc-gui/`
/// - **Windows**: `C:\Users\<User>\AppData\Roaming\goldwind\dfc-gui\config\`
pub fn get_or_create_config_dir() -> Result<PathBuf> {
    let Some(project_dirs) = ProjectDirs::from("com", "goldwind", "dfc-gui") else {
        return Err(Error::Invalid {
            message: "Could not determine project directories".to_string(),
        });
    };

    let config_dir = project_dirs.config_dir();

    // Create config directory if it doesn't exist
    if !config_dir.exists() {
        fs::create_dir_all(config_dir)?;
    }

    // Handle migration from old location if needed
    if let Some(home) = home_dir() {
        let old_config_path = home.join(".dfc-gui");
        if old_config_path.exists() {
            // Copy files from old location (ignore errors)
            let _ = copy_dir_files(&old_config_path, config_dir);
            // Clean up old directory
            let _ = fs::remove_dir_all(&old_config_path);
        }
    }

    Ok(config_dir.to_path_buf())
}

/// Copy files (not directories) from source to destination
fn copy_dir_files(src: &PathBuf, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        // Skip subdirectories
        if file_type.is_dir() {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        fs::copy(&src_path, &dst_path)?;
    }
    Ok(())
}

/// Check if running from App Store (macOS)
///
/// Detected by presence of `_MASReceipt/receipt` in app bundle.
pub fn is_app_store_build() -> bool {
    #[cfg(target_os = "macos")]
    {
        let Ok(exe_path) = std::env::current_exe() else {
            return false;
        };

        let mut receipt_path = exe_path;

        // Navigate: MacOS/executable -> Contents/ -> _MASReceipt/receipt
        if !receipt_path.pop() || !receipt_path.pop() {
            return false;
        }

        receipt_path.push("_MASReceipt");
        receipt_path.push("receipt");

        receipt_path.exists()
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Check if running in development mode
pub fn is_development() -> bool {
    cfg!(debug_assertions)
}

/// Check if running on Windows
pub fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

/// Check if running on macOS
pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Get the data directory for storing larger files
///
/// Platform-specific locations:
/// - **Linux**: `~/.local/share/dfc-gui/`
/// - **macOS**: `~/Library/Application Support/com.goldwind.dfc-gui/`
/// - **Windows**: `C:\Users\<User>\AppData\Roaming\goldwind\dfc-gui\data\`
pub fn get_or_create_data_dir() -> Result<PathBuf> {
    let Some(project_dirs) = ProjectDirs::from("com", "goldwind", "dfc-gui") else {
        return Err(Error::Invalid {
            message: "Could not determine project directories".to_string(),
        });
    };

    let data_dir = project_dirs.data_dir();

    if !data_dir.exists() {
        fs::create_dir_all(data_dir)?;
    }

    Ok(data_dir.to_path_buf())
}

/// Get the cache directory for temporary files
///
/// Platform-specific locations:
/// - **Linux**: `~/.cache/dfc-gui/`
/// - **macOS**: `~/Library/Caches/com.goldwind.dfc-gui/`
/// - **Windows**: `C:\Users\<User>\AppData\Local\goldwind\dfc-gui\cache\`
pub fn get_or_create_cache_dir() -> Result<PathBuf> {
    let Some(project_dirs) = ProjectDirs::from("com", "goldwind", "dfc-gui") else {
        return Err(Error::Invalid {
            message: "Could not determine project directories".to_string(),
        });
    };

    let cache_dir = project_dirs.cache_dir();

    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir)?;
    }

    Ok(cache_dir.to_path_buf())
}
