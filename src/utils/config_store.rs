//! ConfigStore - Local Configuration Storage

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};

/// Get the application data directory
pub fn app_data_dir() -> Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find local data directory"))?
        .join("dfc-gui");

    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    Ok(dir)
}

/// Load a JSON config file
pub fn load_config<T: DeserializeOwned + Default>(filename: &str) -> Result<T> {
    let path = app_data_dir()?.join(filename);

    if !path.exists() {
        return Ok(T::default());
    }

    let content = fs::read_to_string(&path)?;
    let config: T = serde_json::from_str(&content)?;
    Ok(config)
}

/// Save a JSON config file
pub fn save_config<T: Serialize>(filename: &str, config: &T) -> Result<()> {
    let path = app_data_dir()?.join(filename);
    let content = serde_json::to_string_pretty(config)?;
    fs::write(&path, content)?;
    Ok(())
}

/// Delete a config file
pub fn delete_config(filename: &str) -> Result<()> {
    let path = app_data_dir()?.join(filename);
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// List all config files matching a pattern
pub fn list_configs(extension: &str) -> Result<Vec<String>> {
    let dir = app_data_dir()?;
    let mut files = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == extension {
                    if let Some(name) = path.file_name() {
                        files.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    Ok(files)
}
