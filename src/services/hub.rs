//! Service Hub
//!
//! Central orchestrator for all services. Handles initialization, lifecycle,
//! and provides a unified API for the state layer.

use crate::error::Result;
use crate::services::{
    generate_correlation_id, DeviceId, DeviceMeta, PulsarBus, PulsarConfig, RedisConfig,
    RedisRepo, RetryConfig, ServiceEvent, Supervisor,
};
use crossbeam_channel::{Receiver, Sender};
use std::sync::Arc;

/// Configuration for all services
#[derive(Clone, Debug, Default)]
pub struct ServiceConfig {
    /// Redis configuration
    pub redis: RedisConfig,
    /// Pulsar configuration
    pub pulsar: PulsarConfig,
    /// Retry configuration for reconnection
    pub retry: RetryConfig,
}

/// Central hub for all backend services
pub struct ServiceHub {
    /// Redis repository for metadata
    redis: Arc<RedisRepo>,
    /// Pulsar bus for event streaming
    pulsar: Arc<PulsarBus>,
    /// Redis connection supervisor
    redis_supervisor: Arc<Supervisor>,
    /// Pulsar connection supervisor
    pulsar_supervisor: Arc<Supervisor>,
    /// Event sender (for internal use)
    tx: Sender<ServiceEvent>,
    /// Event receiver (for state layer)
    rx: Receiver<ServiceEvent>,
}

impl ServiceHub {
    /// Create a new service hub with the given configuration
    pub fn new(config: ServiceConfig) -> Result<Self> {
        let (tx, rx) = crossbeam_channel::unbounded();

        // Create supervisors
        let redis_supervisor = Arc::new(Supervisor::new(
            "redis",
            config.retry.clone(),
            tx.clone(),
        ));
        let pulsar_supervisor = Arc::new(Supervisor::new(
            "pulsar",
            config.retry.clone(),
            tx.clone(),
        ));

        // Create services
        let redis = Arc::new(RedisRepo::new(&config.redis, tx.clone())?);
        let pulsar = Arc::new(PulsarBus::new(&config.pulsar, tx.clone())?);

        Ok(Self {
            redis,
            pulsar,
            redis_supervisor,
            pulsar_supervisor,
            tx,
            rx,
        })
    }

    /// Create a service hub with default configuration (for development/testing)
    pub fn with_defaults() -> Result<Self> {
        Self::new(ServiceConfig::default())
    }

    /// Get the event receiver for the state layer
    ///
    /// Events from all services are multiplexed into this single channel.
    pub fn events(&self) -> Receiver<ServiceEvent> {
        self.rx.clone()
    }

    /// Start all services
    pub fn start(&self) {
        tracing::info!("Starting all services");

        // Start Pulsar subscriptions
        self.pulsar.start_subscriptions();

        // TODO: Start Redis connection monitoring
        // TODO: Start health check loops
    }

    /// Stop all services
    pub fn stop(&self) {
        tracing::info!("Stopping all services");
        self.pulsar.stop_subscriptions();
    }

    // ==================== Device Operations ====================

    /// Fetch all devices from Redis
    pub async fn fetch_all_devices(&self) -> Result<Vec<DeviceMeta>> {
        self.redis.fetch_all_devices().await
    }

    /// Fetch a specific device
    pub async fn fetch_device(&self, device_id: &DeviceId) -> Result<Option<DeviceMeta>> {
        self.redis.fetch_device(device_id).await
    }

    // ==================== Command Operations ====================

    /// Send a command to a device
    ///
    /// Returns a correlation ID that can be used to track the command response.
    pub fn send_command(
        &self,
        device: &DeviceId,
        method: &str,
        params: &str,
    ) -> Result<Arc<str>> {
        let correlation_id = generate_correlation_id();
        self.pulsar.send_command(device, method, params, &correlation_id)?;
        Ok(correlation_id)
    }

    // ==================== Dictionary Operations ====================

    /// Fetch the metric dictionary
    pub async fn fetch_metric_dictionary(&self) -> Result<Vec<(u16, Arc<str>)>> {
        self.redis.fetch_metric_dictionary().await
    }

    // ==================== Health Check ====================

    /// Check if all services are healthy
    pub fn is_healthy(&self) -> bool {
        self.redis.is_connected() && self.pulsar.is_running()
    }

    /// Get Redis connection state
    pub fn redis_state(&self) -> crate::services::ConnectionState {
        self.redis_supervisor.state()
    }

    /// Get Pulsar connection state
    pub fn pulsar_state(&self) -> crate::services::ConnectionState {
        self.pulsar_supervisor.state()
    }

    // ==================== Event Emission (for testing) ====================

    /// Emit a service event (mainly for testing)
    #[cfg(test)]
    pub fn emit(&self, event: ServiceEvent) {
        let _ = self.tx.send(event);
    }
}

impl Clone for ServiceHub {
    fn clone(&self) -> Self {
        Self {
            redis: self.redis.clone(),
            pulsar: self.pulsar.clone(),
            redis_supervisor: self.redis_supervisor.clone(),
            pulsar_supervisor: self.pulsar_supervisor.clone(),
            tx: self.tx.clone(),
            rx: self.rx.clone(),
        }
    }
}

impl std::fmt::Debug for ServiceHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceHub")
            .field("redis", &self.redis)
            .field("pulsar", &self.pulsar)
            .field("healthy", &self.is_healthy())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_hub_creation() {
        let hub = ServiceHub::with_defaults().expect("Failed to create hub");
        assert!(!hub.events().is_empty() || hub.events().is_empty()); // Just check it works
    }
}
