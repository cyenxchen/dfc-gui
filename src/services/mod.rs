//! Service Layer
//!
//! The service layer provides abstraction over external services (Redis, Pulsar)
//! and handles async operations, connection management, and event streaming.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      ServiceHub                              │
//! │  ┌─────────────┐  ┌─────────────┐  ┌──────────────────┐    │
//! │  │  RedisRepo  │  │  PulsarBus  │  │    Supervisor    │    │
//! │  │  (metadata) │  │  (events)   │  │  (health/retry)  │    │
//! │  └─────────────┘  └─────────────┘  └──────────────────┘    │
//! └─────────────────────────────────────────────────────────────┘
//!                            │
//!                            ▼ ServiceEvent
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      State Layer                             │
//! │                   (FleetState, etc.)                         │
//! └─────────────────────────────────────────────────────────────┘
//! ```

mod events;
mod hub;
mod pulsar_bus;
mod redis_repo;
mod runtime;
mod supervisor;

pub use events::*;
pub use hub::*;
pub use pulsar_bus::*;
pub use redis_repo::*;
pub use runtime::*;
pub use supervisor::*;
