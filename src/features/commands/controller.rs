//! Commands Controller
//!
//! Handles command sending and response handling.

use gpui::App;

use crate::app::entities::AppEntities;
use crate::domain::command::CommandRequest;
use crate::eventing::app_event::AppEvent;
use crate::services::service_hub::ServiceHub;

/// Commands page controller
pub struct CommandsController {
    entities: AppEntities,
}

impl CommandsController {
    /// Create a new controller
    pub fn new(entities: AppEntities) -> Self {
        Self { entities }
    }

    /// Send a command
    pub fn send_command(&self, request: CommandRequest, cx: &mut App) {
        // Add to history
        self.entities.commands.update(cx, |state, cx| {
            state.add_to_history(request.clone());
            state.set_sending(true);
            cx.notify();
        });

        // Log the action
        if let Some(hub) = cx.try_global::<ServiceHub>() {
            hub.log(AppEvent::info(format!(
                "Sending command: {}.{} to {}",
                request.service, request.method, request.device_id
            )));

            // Send via service hub
            hub.send_command(request);
        }
    }

    /// Clear command history
    pub fn clear_history(&self, cx: &mut App) {
        self.entities.commands.update(cx, |state, cx| {
            state.clear_history();
            cx.notify();
        });

        if let Some(hub) = cx.try_global::<ServiceHub>() {
            hub.log(AppEvent::info("Command history cleared"));
        }
    }
}
