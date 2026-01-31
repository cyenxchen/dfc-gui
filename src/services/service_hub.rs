//! ServiceHub - Unified Service Management
//!
//! Manages all background services (Redis, Pulsar, Database) and provides
//! a single point of control for starting, stopping, and reconnecting.

use std::sync::Arc;

use gpui::Global;
use parking_lot::RwLock;

use crate::domain::command::CommandRequest;
use crate::domain::config::AppConfig;
use crate::eventing::app_event::AppEvent;
use crate::state::connection_state::ConnectionTarget;

/// Commands that can be sent to services
#[derive(Debug, Clone)]
pub enum ServiceCommand {
    /// Start all services with the given config
    Start(AppConfig),
    /// Stop all services
    Stop,
    /// Reconnect a specific service
    Reconnect(ConnectionTarget),
    /// Send a command to a device
    SendCommand(CommandRequest),
    /// Update configuration
    UpdateConfig(AppConfig),
}

/// ServiceHub manages all background services
pub struct ServiceHub {
    /// Channel to send events to UI
    event_tx: flume::Sender<AppEvent>,
    /// Channel to send commands to services
    command_tx: flume::Sender<ServiceCommand>,
    /// Current configuration
    config: Arc<RwLock<Option<AppConfig>>>,
    /// Whether services are running
    running: Arc<RwLock<bool>>,
}

impl Global for ServiceHub {}

impl ServiceHub {
    /// Create a new service hub
    pub fn new(event_tx: flume::Sender<AppEvent>) -> Self {
        let (command_tx, command_rx) = flume::unbounded::<ServiceCommand>();
        let config = Arc::new(RwLock::new(None));
        let running = Arc::new(RwLock::new(false));

        let hub = Self {
            event_tx: event_tx.clone(),
            command_tx,
            config: config.clone(),
            running: running.clone(),
        };

        // Start command handler in background
        hub.start_command_handler(command_rx, config, running, event_tx);

        // Send initial log
        let _ = hub.event_tx.send(AppEvent::info("ServiceHub initialized"));

        hub
    }

    /// Start the command handler task
    fn start_command_handler(
        &self,
        command_rx: flume::Receiver<ServiceCommand>,
        config: Arc<RwLock<Option<AppConfig>>>,
        running: Arc<RwLock<bool>>,
        event_tx: flume::Sender<AppEvent>,
    ) {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime");

            rt.block_on(async move {
                while let Ok(cmd) = command_rx.recv_async().await {
                    match cmd {
                        ServiceCommand::Start(app_config) => {
                            let _ = event_tx.send(AppEvent::info(format!(
                                "Starting services with device {}",
                                app_config.device.device_id
                            )));

                            // Store config
                            *config.write() = Some(app_config.clone());
                            *running.write() = true;

                            // TODO: Actually start Redis/Pulsar/Database connections
                            // For now, simulate connection
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                            let _ = event_tx.send(AppEvent::ConnectionChanged {
                                target: ConnectionTarget::Redis,
                                connected: true,
                                detail: Some(format!(
                                    "{}:{}",
                                    app_config.redis.ip, app_config.redis.port
                                )),
                            });

                            let _ = event_tx.send(AppEvent::ConnectionChanged {
                                target: ConnectionTarget::Database,
                                connected: true,
                                detail: Some("In-memory SQLite".to_string()),
                            });

                            let _ = event_tx.send(AppEvent::info("Services started"));
                        }
                        ServiceCommand::Stop => {
                            let _ = event_tx.send(AppEvent::info("Stopping services..."));
                            *running.write() = false;

                            // Simulate disconnection
                            let _ = event_tx.send(AppEvent::ConnectionChanged {
                                target: ConnectionTarget::Redis,
                                connected: false,
                                detail: None,
                            });
                            let _ = event_tx.send(AppEvent::ConnectionChanged {
                                target: ConnectionTarget::Pulsar,
                                connected: false,
                                detail: None,
                            });
                            let _ = event_tx.send(AppEvent::ConnectionChanged {
                                target: ConnectionTarget::Database,
                                connected: false,
                                detail: None,
                            });

                            let _ = event_tx.send(AppEvent::info("Services stopped"));
                        }
                        ServiceCommand::Reconnect(target) => {
                            let _ = event_tx.send(AppEvent::info(format!(
                                "Reconnecting {:?}...",
                                target
                            )));

                            // Simulate reconnection
                            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

                            let _ = event_tx.send(AppEvent::ConnectionChanged {
                                target,
                                connected: true,
                                detail: Some("Reconnected".to_string()),
                            });
                        }
                        ServiceCommand::SendCommand(request) => {
                            let _ = event_tx.send(AppEvent::info(format!(
                                "Sending command: {}.{} to {}",
                                request.service, request.method, request.device_id
                            )));

                            // TODO: Actually send command via Pulsar
                            // For now, simulate response
                            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                            let response = crate::domain::command::CommandResponse {
                                request_id: request.request_id.clone(),
                                code: 0,
                                message: "Success (simulated)".to_string(),
                                data: Some("{}".to_string()),
                                response_time: chrono::Utc::now(),
                            };

                            let _ = event_tx.send(AppEvent::CommandResponse {
                                request_id: request.request_id,
                                response,
                            });
                        }
                        ServiceCommand::UpdateConfig(new_config) => {
                            let _ = event_tx.send(AppEvent::info("Configuration updated"));
                            *config.write() = Some(new_config.clone());
                            let _ = event_tx.send(AppEvent::ConfigLoaded { config: new_config });
                        }
                    }
                }
            });
        });
    }

    /// Send a command to the services
    pub fn send(&self, cmd: ServiceCommand) {
        let _ = self.command_tx.send(cmd);
    }

    /// Start services with config
    pub fn start(&self, config: AppConfig) {
        self.send(ServiceCommand::Start(config));
    }

    /// Stop all services
    pub fn stop(&self) {
        self.send(ServiceCommand::Stop);
    }

    /// Send a device command
    pub fn send_command(&self, request: CommandRequest) {
        self.send(ServiceCommand::SendCommand(request));
    }

    /// Update configuration
    pub fn update_config(&self, config: AppConfig) {
        self.send(ServiceCommand::UpdateConfig(config));
    }

    /// Check if services are running
    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    /// Get current config
    pub fn config(&self) -> Option<AppConfig> {
        self.config.read().clone()
    }

    /// Send a log event
    pub fn log(&self, event: AppEvent) {
        let _ = self.event_tx.send(event);
    }
}
