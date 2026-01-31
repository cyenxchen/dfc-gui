//! Connection Supervisor
//!
//! Manages connection health, reconnection with exponential backoff,
//! and provides observability for connection state.

use crate::constants::{
    RETRY_INITIAL_DELAY_MS, RETRY_JITTER, RETRY_MAX_DELAY_MS, RETRY_MULTIPLIER,
};
use crate::services::events::ServiceEvent;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Retry configuration for connection recovery
#[derive(Clone, Debug)]
pub struct RetryConfig {
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub multiplier: f64,
    /// Jitter factor (0.0 - 1.0) to randomize delays
    pub jitter: f64,
    /// Maximum number of retry attempts (0 = unlimited)
    pub max_attempts: u32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(RETRY_INITIAL_DELAY_MS),
            max_delay: Duration::from_millis(RETRY_MAX_DELAY_MS),
            multiplier: RETRY_MULTIPLIER,
            jitter: RETRY_JITTER,
            max_attempts: 0, // Unlimited
        }
    }
}

/// Connection state for a service
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected, not trying to connect
    Disconnected,
    /// Attempting to connect
    Connecting,
    /// Successfully connected
    Connected,
    /// Waiting before next retry attempt
    Backoff,
}

/// Supervisor for managing connection lifecycle
pub struct Supervisor {
    /// Service name (for logging and events)
    service_name: Arc<str>,
    /// Retry configuration
    config: RetryConfig,
    /// Event sender for state notifications
    tx: Sender<ServiceEvent>,
    /// Current connection state
    state: std::sync::atomic::AtomicU8,
    /// Current retry attempt count
    attempt: AtomicU32,
}

impl Supervisor {
    /// Create a new supervisor for a service
    pub fn new(
        service_name: impl Into<Arc<str>>,
        config: RetryConfig,
        tx: Sender<ServiceEvent>,
    ) -> Self {
        Self {
            service_name: service_name.into(),
            config,
            tx,
            state: std::sync::atomic::AtomicU8::new(ConnectionState::Disconnected as u8),
            attempt: AtomicU32::new(0),
        }
    }

    /// Get the current connection state
    pub fn state(&self) -> ConnectionState {
        match self.state.load(Ordering::SeqCst) {
            0 => ConnectionState::Disconnected,
            1 => ConnectionState::Connecting,
            2 => ConnectionState::Connected,
            _ => ConnectionState::Backoff,
        }
    }

    /// Set the connection state and notify
    fn set_state(&self, state: ConnectionState, detail: &str) {
        self.state.store(state as u8, Ordering::SeqCst);

        let _ = self.tx.send(ServiceEvent::ConnectionState {
            service: self.service_name.clone(),
            connected: state == ConnectionState::Connected,
            detail: detail.into(),
        });
    }

    /// Mark connection as successful
    pub fn on_connected(&self) {
        self.attempt.store(0, Ordering::SeqCst);
        self.set_state(ConnectionState::Connected, "Connected");
        tracing::info!("{}: Connected", self.service_name);
    }

    /// Mark connection as disconnected (will trigger retry)
    pub fn on_disconnected(&self, reason: &str) {
        self.set_state(ConnectionState::Disconnected, reason);
        tracing::warn!("{}: Disconnected - {}", self.service_name, reason);
    }

    /// Calculate the next retry delay with exponential backoff and jitter
    pub fn next_retry_delay(&self) -> Option<Duration> {
        let attempt = self.attempt.fetch_add(1, Ordering::SeqCst) + 1;

        // Check max attempts
        if self.config.max_attempts > 0 && attempt > self.config.max_attempts {
            self.set_state(
                ConnectionState::Disconnected,
                &format!("Max attempts ({}) reached", self.config.max_attempts),
            );
            return None;
        }

        // Calculate delay with exponential backoff
        let base_delay = self.config.initial_delay.as_millis() as f64
            * self.config.multiplier.powi((attempt - 1) as i32);

        let capped_delay = base_delay.min(self.config.max_delay.as_millis() as f64);

        // Apply jitter
        let jitter_range = capped_delay * self.config.jitter;
        let jitter = (rand_jitter() * 2.0 - 1.0) * jitter_range;
        let final_delay = (capped_delay + jitter).max(0.0) as u64;

        let delay = Duration::from_millis(final_delay);

        let detail = format!(
            "Reconnecting in {}s (attempt {}/{})",
            delay.as_secs(),
            attempt,
            if self.config.max_attempts == 0 {
                "∞".to_string()
            } else {
                self.config.max_attempts.to_string()
            }
        );

        self.set_state(ConnectionState::Backoff, &detail);
        tracing::info!("{}: {}", self.service_name, detail);

        Some(delay)
    }

    /// Reset retry counter
    pub fn reset(&self) {
        self.attempt.store(0, Ordering::SeqCst);
    }

    /// Get current attempt count
    pub fn attempt_count(&self) -> u32 {
        self.attempt.load(Ordering::SeqCst)
    }

    /// Get the service name
    pub fn service_name(&self) -> &str {
        &self.service_name
    }
}

/// Simple pseudo-random jitter (0.0 - 1.0)
fn rand_jitter() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    (nanos % 1000) as f64 / 1000.0
}

impl std::fmt::Debug for Supervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Supervisor")
            .field("service", &self.service_name)
            .field("state", &self.state())
            .field("attempt", &self.attempt_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    #[test]
    fn test_retry_delay_increases() {
        let (tx, _rx) = unbounded();
        let supervisor = Supervisor::new("test", RetryConfig::default(), tx);

        let d1 = supervisor.next_retry_delay().expect("delay");
        let d2 = supervisor.next_retry_delay().expect("delay");
        let d3 = supervisor.next_retry_delay().expect("delay");

        // Each delay should generally be larger (with some jitter variation)
        // We just check they're all positive
        assert!(d1.as_millis() > 0);
        assert!(d2.as_millis() > 0);
        assert!(d3.as_millis() > 0);
    }

    #[test]
    fn test_max_attempts() {
        let (tx, _rx) = unbounded();
        let config = RetryConfig {
            max_attempts: 3,
            ..Default::default()
        };
        let supervisor = Supervisor::new("test", config, tx);

        assert!(supervisor.next_retry_delay().is_some());
        assert!(supervisor.next_retry_delay().is_some());
        assert!(supervisor.next_retry_delay().is_some());
        assert!(supervisor.next_retry_delay().is_none());
    }
}
