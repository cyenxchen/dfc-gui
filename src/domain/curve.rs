//! Curve - Power Curve Data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Power curve data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurveData {
    /// Unique ID
    pub id: String,
    /// Device ID
    pub device_id: String,
    /// Wind speed (m/s)
    pub wind_speed: f64,
    /// Active power (kW)
    pub active_power: f64,
    /// Reactive power (kVar)
    pub reactive_power: f64,
    /// Data timestamp
    pub data_time: DateTime<Utc>,
    /// Created timestamp
    pub created_time: DateTime<Utc>,
}

impl Default for CurveData {
    fn default() -> Self {
        Self {
            id: String::new(),
            device_id: String::new(),
            wind_speed: 0.0,
            active_power: 0.0,
            reactive_power: 0.0,
            data_time: Utc::now(),
            created_time: Utc::now(),
        }
    }
}
