//! Pulsar Producer
//!
//! Publishes messages to Pulsar topics for sending commands.

use anyhow::Result;

/// Pulsar message producer
pub struct PulsarProducer {
    // producer: Option<pulsar::Producer<...>>,
}

impl PulsarProducer {
    /// Create a new producer
    pub fn new() -> Self {
        Self {
            // producer: None,
        }
    }

    /// Send a message
    pub async fn send(&self, _message: &[u8]) -> Result<()> {
        // TODO: Implement message sending
        Ok(())
    }
}

impl Default for PulsarProducer {
    fn default() -> Self {
        Self::new()
    }
}
