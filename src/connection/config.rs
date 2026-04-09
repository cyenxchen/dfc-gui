//! Server Configuration
//!
//! DFC server configuration data structures and persistence.

use crate::error::{Error, Result};
use crate::helpers::{decrypt, encrypt, get_or_create_config_dir};
use serde::{Deserialize, Serialize};
use smol::fs;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use tracing::info;

/// DFC Server configuration
#[derive(Debug, Default, Deserialize, Clone, Serialize, Hash, Eq, PartialEq)]
pub struct DfcServerConfig {
    /// Unique identifier (UUID)
    pub id: String,
    /// Server name (user-visible)
    pub name: String,
    /// Redis IP address
    pub host: String,
    /// Redis port number
    pub port: u16,
    /// Redis password (encrypted storage)
    pub password: Option<String>,
    /// Restricted cfgid (e.g., "{DCC0006}")
    pub cfgid: Option<String>,
    /// Device filter
    pub device_filter: Option<String>,
    /// Pulsar Token (encrypted storage)
    pub pulsar_token: Option<String>,
    /// Last update timestamp (RFC3339)
    pub updated_at: Option<String>,
}

/// TOML wrapper structure for server list
#[derive(Debug, Default, Deserialize, Clone, Serialize)]
pub(crate) struct DfcServers {
    servers: Vec<DfcServerConfig>,
}

impl DfcServerConfig {
    /// Generate a hash for this server configuration
    pub fn get_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Generate Redis connection URL
    pub fn redis_url(&self) -> String {
        match &self.password {
            Some(pwd) if !pwd.is_empty() => {
                format!("redis://:{}@{}:{}", pwd, self.host, self.port)
            }
            _ => format!("redis://{}:{}", self.host, self.port),
        }
    }

    /// Generate display name (e.g., "Local Test (127.0.0.1:6379)")
    pub fn display_name(&self) -> String {
        if self.name.is_empty() {
            format!("{}:{}", self.host, self.port)
        } else {
            format!("{} ({}:{})", self.name, self.host, self.port)
        }
    }
}

/// Get or create the server configuration file path
fn get_server_config_path() -> Result<PathBuf> {
    let config_dir = get_or_create_config_dir()?;
    let path = config_dir.join("servers.toml");

    #[cfg(debug_assertions)]
    info!("Server config file: {}", path.display());

    if !path.exists() {
        std::fs::write(&path, "")?;
    }

    Ok(path)
}

/// Load all server configurations from file
pub fn get_servers() -> Result<Vec<DfcServerConfig>> {
    let path = get_server_config_path()?;
    let value = std::fs::read_to_string(&path)?;

    if value.trim().is_empty() {
        return Ok(vec![]);
    }

    let configs: DfcServers = toml::from_str(&value)?;
    let mut servers = configs.servers;

    // Decrypt sensitive fields
    for server in servers.iter_mut() {
        if let Some(pwd) = &server.password {
            server.password = Some(decrypt(pwd).unwrap_or_else(|_| pwd.clone()));
        }
        if let Some(token) = &server.pulsar_token {
            server.pulsar_token = Some(decrypt(token).unwrap_or_else(|_| token.clone()));
        }
    }

    Ok(servers)
}

/// Save server configurations to file
pub async fn save_servers(mut servers: Vec<DfcServerConfig>) -> Result<()> {
    // Encrypt sensitive fields
    for server in servers.iter_mut() {
        if let Some(pwd) = &server.password {
            if !pwd.is_empty() {
                server.password = Some(encrypt(pwd)?);
            }
        }
        if let Some(token) = &server.pulsar_token {
            if !token.is_empty() {
                server.pulsar_token = Some(encrypt(token)?);
            }
        }
    }

    let path = get_server_config_path()?;
    let content = toml::to_string_pretty(&DfcServers { servers })?;
    fs::write(&path, content).await?;

    Ok(())
}

/// Get a single server configuration by ID
pub fn get_server_by_id(id: &str) -> Result<DfcServerConfig> {
    let servers = get_servers()?;
    servers
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| Error::Invalid {
            message: format!("Server not found: {id}"),
        })
}
