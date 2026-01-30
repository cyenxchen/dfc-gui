//! Properties Controller
//!
//! Handles properties data loading and filtering.

use gpui::App;

use crate::app::entities::AppEntities;
use crate::domain::property::Property;
use crate::eventing::app_event::AppEvent;
use crate::services::service_hub::ServiceHub;

/// Properties page controller
pub struct PropertiesController {
    entities: AppEntities,
}

impl PropertiesController {
    /// Create a new controller
    pub fn new(entities: AppEntities) -> Self {
        Self { entities }
    }

    /// Refresh properties data
    pub fn refresh(&self, cx: &mut App) {
        self.entities.properties.update(cx, |state, cx| {
            state.set_loading(true);
            cx.notify();
        });

        if let Some(hub) = cx.try_global::<ServiceHub>() {
            hub.log(AppEvent::info("Refreshing properties data..."));
        }

        // TODO: Actually fetch data from service
        // For now, simulate with sample data
        let sample_properties = vec![
            Property {
                id: "1".to_string(),
                device_id: "DOC00006".to_string(),
                name: "WindTurbine/WROT/WindSpdAHHS1-V1-S1-1".to_string(),
                topic: "windturbine.property".to_string(),
                mms: "MMS Path".to_string(),
                hmi: "HMI Path".to_string(),
                value: "8.5".to_string(),
                prev_value: Some("8.4".to_string()),
                quality: 0,
                data_time: chrono::Utc::now(),
                created_time: chrono::Utc::now(),
                source: "Pulsar".to_string(),
            },
            Property {
                id: "2".to_string(),
                device_id: "DOC00006".to_string(),
                name: "WindTurbine/WGEN/ActPwr-V1-S1-1".to_string(),
                topic: "windturbine.property".to_string(),
                mms: "MMS Path".to_string(),
                hmi: "HMI Path".to_string(),
                value: "2500.0".to_string(),
                prev_value: Some("2480.0".to_string()),
                quality: 0,
                data_time: chrono::Utc::now(),
                created_time: chrono::Utc::now(),
                source: "Pulsar".to_string(),
            },
        ];

        self.entities.properties.update(cx, |state, cx| {
            state.update_properties(sample_properties);
            cx.notify();
        });

        if let Some(hub) = cx.try_global::<ServiceHub>() {
            hub.log(AppEvent::info("Properties data refreshed"));
        }
    }

    /// Set filter text
    pub fn set_filter(&self, filter: String, cx: &mut App) {
        self.entities.properties.update(cx, |state, cx| {
            state.set_filter(filter);
            cx.notify();
        });
    }
}
