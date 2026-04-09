//! Tokio Runtime Bridge
//!
//! GPUI uses a smol-like executor, but external clients (Redis/Pulsar) require tokio.
//! This module provides a bridge to run tokio futures from GPUI context.
//!
//! ## Pattern
//!
//! ```text
//! GPUI async task
//!       │
//!       ▼
//! run_in_tokio(async { ... })
//!       │
//!       ▼
//! tokio::Runtime::spawn()
//!       │
//!       ▼
//! Result returned to GPUI
//! ```

use std::future::Future;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

/// Global tokio runtime instance
static TOKIO_RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Get or initialize the global tokio runtime
fn get_runtime() -> &'static Runtime {
    TOKIO_RUNTIME.get_or_init(|| {
        Runtime::new().expect("Failed to create tokio runtime")
    })
}

/// Execute a future in the tokio runtime and wait for the result
///
/// This is used for one-shot RPC calls (e.g., fetching device list from Redis).
///
/// # Example
///
/// ```ignore
/// let devices = run_in_tokio(async {
///     redis_client.get_all_devices().await
/// }).await;
/// ```
pub async fn run_in_tokio<F, T>(future: F) -> T
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let handle = get_runtime().spawn(future);
    match handle.await {
        Ok(result) => result,
        Err(e) => std::panic::resume_unwind(e.into_panic()),
    }
}

/// Spawn a detached task in the tokio runtime
///
/// Used for long-running background tasks like subscription loops.
/// The task runs independently and its result is not awaited.
///
/// # Example
///
/// ```ignore
/// spawn_in_tokio(async move {
///     loop {
///         let msg = consumer.next().await;
///         tx.send(parse_event(msg)).ok();
///     }
/// });
/// ```
pub fn spawn_in_tokio<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    get_runtime().spawn(future);
}

/// Spawn a detached task with a name (for debugging)
pub fn spawn_named_in_tokio<F>(name: &'static str, future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tracing::debug!("Spawning tokio task: {}", name);
    get_runtime().spawn(async move {
        future.await;
        tracing::debug!("Tokio task completed: {}", name);
    });
}

/// Block on a future synchronously (use sparingly, mainly for initialization)
///
/// **Warning**: This blocks the current thread. Use only during app startup
/// or when you absolutely need synchronous execution.
pub fn block_on<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    get_runtime().block_on(future)
}

/// Get a handle to the tokio runtime for advanced use cases
pub fn runtime_handle() -> tokio::runtime::Handle {
    get_runtime().handle().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_in_tokio() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        spawn_in_tokio(async move {
            flag_clone.store(true, Ordering::SeqCst);
        });

        // Give the task time to complete
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(flag.load(Ordering::SeqCst));
    }
}
