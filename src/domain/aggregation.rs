//! Aggregation - One Minute and Ten Minute Data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One minute aggregation data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneMinData {
    /// Unique ID
    pub id: String,
    /// Device ID
    pub device_id: String,
    /// Wind speed average (m/s)
    pub wind_speed_avg: f64,
    /// Wind speed max (m/s)
    pub wind_speed_max: f64,
    /// Wind speed min (m/s)
    pub wind_speed_min: f64,
    /// Active power average (kW)
    pub active_power_avg: f64,
    /// Active power max (kW)
    pub active_power_max: f64,
    /// Active power min (kW)
    pub active_power_min: f64,
    /// Data timestamp (start of minute)
    pub data_time: DateTime<Utc>,
    /// Created timestamp
    pub created_time: DateTime<Utc>,
}

impl Default for OneMinData {
    fn default() -> Self {
        Self {
            id: String::new(),
            device_id: String::new(),
            wind_speed_avg: 0.0,
            wind_speed_max: 0.0,
            wind_speed_min: 0.0,
            active_power_avg: 0.0,
            active_power_max: 0.0,
            active_power_min: 0.0,
            data_time: Utc::now(),
            created_time: Utc::now(),
        }
    }
}

/// Ten minute aggregation data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenMinData {
    /// Unique ID
    pub id: String,
    /// Device ID
    pub device_id: String,
    /// Wind speed average (m/s)
    pub wind_speed_avg: f64,
    /// Wind speed max (m/s)
    pub wind_speed_max: f64,
    /// Wind speed min (m/s)
    pub wind_speed_min: f64,
    /// Wind speed std deviation
    pub wind_speed_std: f64,
    /// Active power average (kW)
    pub active_power_avg: f64,
    /// Active power max (kW)
    pub active_power_max: f64,
    /// Active power min (kW)
    pub active_power_min: f64,
    /// Active power std deviation
    pub active_power_std: f64,
    /// Generator speed average (rpm)
    pub generator_speed_avg: f64,
    /// Rotor speed average (rpm)
    pub rotor_speed_avg: f64,
    /// Pitch angle average (degrees)
    pub pitch_angle_avg: f64,
    /// Nacelle direction average (degrees)
    pub nacelle_direction_avg: f64,
    /// Ambient temperature average (°C)
    pub ambient_temp_avg: f64,
    /// Data timestamp (start of 10-minute period)
    pub data_time: DateTime<Utc>,
    /// Created timestamp
    pub created_time: DateTime<Utc>,
}

impl Default for TenMinData {
    fn default() -> Self {
        Self {
            id: String::new(),
            device_id: String::new(),
            wind_speed_avg: 0.0,
            wind_speed_max: 0.0,
            wind_speed_min: 0.0,
            wind_speed_std: 0.0,
            active_power_avg: 0.0,
            active_power_max: 0.0,
            active_power_min: 0.0,
            active_power_std: 0.0,
            generator_speed_avg: 0.0,
            rotor_speed_avg: 0.0,
            pitch_angle_avg: 0.0,
            nacelle_direction_avg: 0.0,
            ambient_temp_avg: 0.0,
            data_time: Utc::now(),
            created_time: Utc::now(),
        }
    }
}
