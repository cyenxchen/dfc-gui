//! Pulsar Consumer
//!
//! Subscribes to Pulsar topics for receiving device data.

use anyhow::Result;

/// Pulsar message consumer
pub struct PulsarConsumer {
    // consumer: Option<pulsar::Consumer<...>>,
}

impl PulsarConsumer {
    /// Create a new consumer
    pub fn new() -> Self {
        Self {
            // consumer: None,
        }
    }

    /// Subscribe to a topic
    pub async fn subscribe(&mut self, _topic: &str, _subscription: &str) -> Result<()> {
        // TODO: Implement subscription
        Ok(())
    }

    /// Unsubscribe
    pub async fn unsubscribe(&mut self) -> Result<()> {
        // TODO: Implement unsubscription
        Ok(())
    }
}

impl Default for PulsarConsumer {
    fn default() -> Self {
        Self::new()
    }
}
