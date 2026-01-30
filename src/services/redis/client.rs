//! Redis Client
//!
//! Manages Redis connection for configuration lookup.

use anyhow::Result;

use crate::domain::config::RedisConfig;

/// Redis client wrapper
pub struct RedisClient {
    config: RedisConfig,
    // connection: Option<redis::aio::ConnectionManager>,
}

impl RedisClient {
    /// Create a new Redis client
    pub fn new(config: RedisConfig) -> Self {
        Self {
            config,
            // connection: None,
        }
    }

    /// Connect to Redis
    pub async fn connect(&mut self) -> Result<()> {
        let url = if let Some(ref password) = self.config.password {
            format!(
                "redis://:{}@{}:{}/",
                password, self.config.ip, self.config.port
            )
        } else {
            format!("redis://{}:{}/", self.config.ip, self.config.port)
        };

        tracing::info!("Connecting to Redis at {}:{}", self.config.ip, self.config.port);

        // TODO: Implement actual connection
        // let client = redis::Client::open(url)?;
        // self.connection = Some(redis::aio::ConnectionManager::new(client).await?);

        Ok(())
    }

    /// Disconnect from Redis
    pub async fn disconnect(&mut self) {
        // self.connection = None;
        tracing::info!("Disconnected from Redis");
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        // self.connection.is_some()
        false
    }
}
