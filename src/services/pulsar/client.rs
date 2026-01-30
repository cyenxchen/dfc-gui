//! Pulsar Client
//!
//! Manages Pulsar connection for message bus.

use anyhow::Result;

use crate::domain::config::PulsarConfig;

/// Pulsar client wrapper
pub struct PulsarClient {
    config: PulsarConfig,
    // client: Option<pulsar::Pulsar<pulsar::TokioExecutor>>,
}

impl PulsarClient {
    /// Create a new Pulsar client
    pub fn new(config: PulsarConfig) -> Self {
        Self {
            config,
            // client: None,
        }
    }

    /// Connect to Pulsar
    pub async fn connect(&mut self) -> Result<()> {
        let url = format!("pulsar://{}:{}", self.config.ip, self.config.port);

        tracing::info!("Connecting to Pulsar at {}", url);

        // TODO: Implement actual connection
        // let mut builder = Pulsar::builder(url, TokioExecutor);
        // if let Some(token) = &self.config.token {
        //     builder = builder.with_auth(Authentication {
        //         name: "token".to_string(),
        //         data: token.as_bytes().to_vec(),
        //     });
        // }
        // self.client = Some(builder.build().await?);

        Ok(())
    }

    /// Disconnect from Pulsar
    pub async fn disconnect(&mut self) {
        // self.client = None;
        tracing::info!("Disconnected from Pulsar");
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        // self.client.is_some()
        false
    }

    /// Get the topic for a message type
    pub fn topic(&self, message_type: &str) -> String {
        format!(
            "persistent://{}/{}/{}",
            self.config.tenant, self.config.namespace, message_type
        )
    }
}
