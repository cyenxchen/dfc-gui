//! Events Controller
//!
//! Handles events data loading and filtering.

use gpui::App;

use crate::app::entities::AppEntities;
use crate::domain::event_log::EventLog;
use crate::eventing::app_event::AppEvent;
use crate::services::service_hub::ServiceHub;

/// Events page controller
pub struct EventsController {
    entities: AppEntities,
}

impl EventsController {
    /// Create a new controller
    pub fn new(entities: AppEntities) -> Self {
        Self { entities }
    }

    /// Refresh events data
    pub fn refresh(&self, cx: &mut App) {
        self.entities.events.update(cx, |state, cx| {
            state.set_loading(true);
            cx.notify();
        });

        if let Some(hub) = cx.try_global::<ServiceHub>() {
            hub.log(AppEvent::info("Refreshing events data..."));
        }

        // TODO: Actually fetch data from service
        // For now, simulate with sample data
        let sample_events = vec![
            EventLog {
                id: "1".to_string(),
                device_id: "DOC00006".to_string(),
                event_code: "E001".to_string(),
                description: "Wind speed high".to_string(),
                level: 1,
                state: 0,
                event_time: chrono::Utc::now(),
                created_time: chrono::Utc::now(),
                source: "Pulsar".to_string(),
            },
            EventLog {
                id: "2".to_string(),
                device_id: "DOC00006".to_string(),
                event_code: "E002".to_string(),
                description: "Generator overtemp".to_string(),
                level: 2,
                state: 0,
                event_time: chrono::Utc::now(),
                created_time: chrono::Utc::now(),
                source: "Pulsar".to_string(),
            },
        ];

        self.entities.events.update(cx, |state, cx| {
            state.update_events(sample_events);
            cx.notify();
        });

        if let Some(hub) = cx.try_global::<ServiceHub>() {
            hub.log(AppEvent::info("Events data refreshed"));
        }
    }

    /// Set filter text
    pub fn set_filter(&self, filter: String, cx: &mut App) {
        self.entities.events.update(cx, |state, cx| {
            state.set_filter(filter);
            cx.notify();
        });
    }
}
