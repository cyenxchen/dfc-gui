//! Property - Device Property Data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A device property
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    /// Unique ID
    pub id: String,
    /// Device ID
    pub device_id: String,
    /// Property name
    pub name: String,
    /// Topic path
    pub topic: String,
    /// MMS (Manufacturing Message Specification) path
    pub mms: String,
    /// HMI path
    pub hmi: String,
    /// Current value
    pub value: String,
    /// Previous value
    pub prev_value: Option<String>,
    /// Quality (0 = good, others = bad)
    pub quality: i32,
    /// Data timestamp
    pub data_time: DateTime<Utc>,
    /// Created timestamp
    pub created_time: DateTime<Utc>,
    /// Source system
    pub source: String,
}

impl Default for Property {
    fn default() -> Self {
        Self {
            id: String::new(),
            device_id: String::new(),
            name: String::new(),
            topic: String::new(),
            mms: String::new(),
            hmi: String::new(),
            value: String::new(),
            prev_value: None,
            quality: 0,
            data_time: Utc::now(),
            created_time: Utc::now(),
            source: String::new(),
        }
    }
}
