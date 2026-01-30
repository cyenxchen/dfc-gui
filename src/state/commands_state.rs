//! CommandsState - Commands State

use std::collections::HashMap;

use crate::domain::command::{CommandRequest, CommandResponse};

/// State for commands
#[derive(Debug, Clone, Default)]
pub struct CommandsState {
    /// Command history
    pub history: Vec<CommandRequest>,
    /// Pending responses (request_id -> response)
    pub responses: HashMap<String, CommandResponse>,
    /// Current request being edited
    pub current_request: Option<CommandRequest>,
    /// Whether a command is being sent
    pub sending: bool,
}

impl CommandsState {
    /// Add a command to history
    pub fn add_to_history(&mut self, request: CommandRequest) {
        self.history.push(request);
    }

    /// Set response for a request
    pub fn set_response(&mut self, request_id: String, response: CommandResponse) {
        self.responses.insert(request_id, response);
        self.sending = false;
    }

    /// Get response for a request
    pub fn get_response(&self, request_id: &str) -> Option<&CommandResponse> {
        self.responses.get(request_id)
    }

    /// Set sending state
    pub fn set_sending(&mut self, sending: bool) {
        self.sending = sending;
    }

    /// Clear history
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.responses.clear();
    }
}
