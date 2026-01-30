//! Favorite - User Favorites/Presets

use serde::{Deserialize, Serialize};

use crate::domain::config::AppConfig;

/// A saved favorite configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Favorite {
    /// Unique ID
    pub id: String,
    /// Display name
    pub name: String,
    /// What parts of the config to save
    pub options: FavoriteOptions,
    /// The saved configuration values
    pub config: FavoriteConfig,
    /// Created timestamp
    pub created_time: String,
}

/// Options for what to save in a favorite
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FavoriteOptions {
    /// Save Redis configuration
    pub save_redis: bool,
    /// Save running state
    pub save_running: bool,
    /// Save device filter
    pub save_device_filter: bool,
    /// Save cfgid filter
    pub save_cfgid_filter: bool,
    /// Save Pulsar token
    pub save_pulsar_token: bool,
    /// Save time range
    pub save_time_range: bool,
}

/// The actual saved configuration values
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FavoriteConfig {
    /// Redis IP
    pub redis_ip: Option<String>,
    /// Redis port
    pub redis_port: Option<u16>,
    /// Redis password
    pub redis_password: Option<String>,
    /// Running state
    pub running: Option<bool>,
    /// Device ID filter
    pub device_id: Option<String>,
    /// CFG ID filter
    pub cfgid: Option<String>,
    /// Pulsar token
    pub pulsar_token: Option<String>,
    /// Time range start
    pub time_range_start: Option<String>,
    /// Time range end
    pub time_range_end: Option<String>,
}

impl Favorite {
    /// Create a new favorite from current config
    pub fn from_config(name: String, options: FavoriteOptions, config: &AppConfig) -> Self {
        let mut fav_config = FavoriteConfig::default();

        if options.save_redis {
            fav_config.redis_ip = Some(config.redis.ip.clone());
            fav_config.redis_port = Some(config.redis.port);
            fav_config.redis_password = config.redis.password.clone();
        }

        if options.save_running {
            fav_config.running = Some(config.device.running);
        }

        if options.save_device_filter {
            fav_config.device_id = Some(config.filter.device_filter.clone());
        }

        if options.save_cfgid_filter {
            fav_config.cfgid = Some(config.filter.cfgid_filter.clone());
        }

        if options.save_pulsar_token {
            fav_config.pulsar_token = Some(config.filter.token_filter.clone());
        }

        if options.save_time_range {
            fav_config.time_range_start = config.filter.time_range_start.clone();
            fav_config.time_range_end = config.filter.time_range_end.clone();
        }

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            options,
            config: fav_config,
            created_time: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Apply this favorite to a config
    pub fn apply_to(&self, config: &mut AppConfig) {
        if self.options.save_redis {
            if let Some(ip) = &self.config.redis_ip {
                config.redis.ip = ip.clone();
            }
            if let Some(port) = self.config.redis_port {
                config.redis.port = port;
            }
            config.redis.password = self.config.redis_password.clone();
        }

        if self.options.save_running {
            if let Some(running) = self.config.running {
                config.device.running = running;
            }
        }

        if self.options.save_device_filter {
            if let Some(device_id) = &self.config.device_id {
                config.filter.device_filter = device_id.clone();
                config.filter.limit_device = !device_id.is_empty();
            }
        }

        if self.options.save_cfgid_filter {
            if let Some(cfgid) = &self.config.cfgid {
                config.filter.cfgid_filter = cfgid.clone();
                config.filter.limit_cfgid = !cfgid.is_empty();
            }
        }

        if self.options.save_pulsar_token {
            if let Some(token) = &self.config.pulsar_token {
                config.filter.token_filter = token.clone();
                config.filter.use_token = !token.is_empty();
            }
        }

        if self.options.save_time_range {
            config.filter.time_range_start = self.config.time_range_start.clone();
            config.filter.time_range_end = self.config.time_range_end.clone();
            config.filter.use_time_range =
                self.config.time_range_start.is_some() || self.config.time_range_end.is_some();
        }
    }
}
