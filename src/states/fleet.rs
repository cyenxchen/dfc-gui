//! Fleet State
//!
//! Manages the state of all devices in the fleet. This is the single source of truth
//! for device data including metadata, telemetry, alarms, and online status.

use crate::constants::{
    DEVICE_ALARMS_CAPACITY, DEVICE_EVENTS_CAPACITY, INGEST_BATCH_SIZE, INGEST_INTERVAL_MS,
};
use crate::helpers::BoundedDeque;
use crate::services::{
    AlarmSeverity, CommandStatus, DeviceId, DeviceMeta, ServiceEvent, ServiceHub,
};
use crate::states::UIEvent;
use ahash::AHashMap;
use crossbeam_channel::Receiver;
use gpui::{Context, Entity, EventEmitter, Task};
use std::sync::Arc;
use std::time::Duration;

/// Runtime state for a single device
#[derive(Clone, Debug)]
pub struct DeviceRuntimeState {
    /// Device metadata
    pub meta: DeviceMeta,
    /// Current online status
    pub online: bool,
    /// Last seen timestamp (milliseconds since epoch)
    pub last_seen_ts_ms: Option<i64>,
    /// Latest telemetry values (metric_id -> value)
    pub latest: AHashMap<u16, f64>,
    /// Recent events (bounded buffer)
    pub events: BoundedDeque<DeviceEvent>,
    /// Recent alarms (bounded buffer)
    pub alarms: BoundedDeque<DeviceAlarm>,
    /// Whether this device is "watched" (full history retained)
    pub watched: bool,
}

impl DeviceRuntimeState {
    /// Create a new runtime state from metadata
    pub fn from_meta(meta: DeviceMeta) -> Self {
        Self {
            meta,
            online: false,
            last_seen_ts_ms: None,
            latest: AHashMap::new(),
            events: BoundedDeque::new(DEVICE_EVENTS_CAPACITY),
            alarms: BoundedDeque::new(DEVICE_ALARMS_CAPACITY),
            watched: false,
        }
    }
}

/// A device event record
#[derive(Clone, Debug)]
pub struct DeviceEvent {
    pub ts_ms: i64,
    pub message: Arc<str>,
}

/// A device alarm record
#[derive(Clone, Debug)]
pub struct DeviceAlarm {
    pub ts_ms: i64,
    pub code: u32,
    pub message: Arc<str>,
    pub severity: AlarmSeverity,
}

/// Pending command state
#[derive(Clone, Debug)]
pub struct PendingCommand {
    pub device: DeviceId,
    pub method: Arc<str>,
    pub status: CommandStatus,
    pub sent_at_ms: i64,
}

/// Fleet state - manages all devices
pub struct FleetState {
    /// Device states indexed by ID
    devices: AHashMap<DeviceId, DeviceRuntimeState>,
    /// Ordered list of device IDs (for consistent display order)
    order: Vec<DeviceId>,
    /// Currently selected device
    selected_device: Option<DeviceId>,
    /// Metric dictionary (ID -> name)
    metric_names: AHashMap<u16, Arc<str>>,
    /// Pending commands
    pending_commands: AHashMap<Arc<str>, PendingCommand>,
    /// Background ingest task
    ingest_task: Option<Task<()>>,
    /// Loading state
    is_loading: bool,
}

impl FleetState {
    /// Create a new fleet state
    pub fn new() -> Self {
        Self {
            devices: AHashMap::new(),
            order: Vec::new(),
            selected_device: None,
            metric_names: AHashMap::new(),
            pending_commands: AHashMap::new(),
            ingest_task: None,
            is_loading: false,
        }
    }

    /// Start the event ingest loop
    ///
    /// This consumes events from the service layer and applies them in batches
    /// to minimize UI updates.
    pub fn start_ingest(&mut self, rx: Receiver<ServiceEvent>, cx: &mut Context<Self>) {
        if self.ingest_task.is_some() {
            tracing::warn!("Ingest task already running");
            return;
        }

        let task = cx.spawn(async move |handle, cx| {
            loop {
                // Wait for batch interval
                cx.background_executor()
                    .timer(Duration::from_millis(INGEST_INTERVAL_MS))
                    .await;

                // Collect batch of events
                let mut batch = Vec::with_capacity(INGEST_BATCH_SIZE);
                while let Ok(ev) = rx.try_recv() {
                    batch.push(ev);
                    if batch.len() >= INGEST_BATCH_SIZE {
                        break;
                    }
                }

                if batch.is_empty() {
                    continue;
                }

                // Apply batch on main thread
                let _ = handle.update(cx, |this, cx| {
                    this.apply_batch(batch, cx);
                });
            }
        });

        self.ingest_task = Some(task);
        tracing::info!("Started fleet ingest task");
    }

    /// Stop the ingest task
    pub fn stop_ingest(&mut self) {
        if let Some(task) = self.ingest_task.take() {
            drop(task);
            tracing::info!("Stopped fleet ingest task");
        }
    }

    /// Apply a batch of service events
    fn apply_batch(&mut self, batch: Vec<ServiceEvent>, cx: &mut Context<Self>) {
        for event in batch {
            self.apply_event(event, cx);
        }
        cx.notify(); // Single notification for entire batch
    }

    /// Apply a single service event
    fn apply_event(&mut self, event: ServiceEvent, cx: &mut Context<Self>) {
        match event {
            ServiceEvent::DeviceMetaUpsert(meta) => {
                let id = meta.id.clone();
                if let Some(state) = self.devices.get_mut(&id) {
                    state.meta = meta;
                } else {
                    self.devices.insert(id.clone(), DeviceRuntimeState::from_meta(meta));
                    self.order.push(id);
                }
            }

            ServiceEvent::DeviceRemoved(id) => {
                self.devices.remove(&id);
                self.order.retain(|d| d != &id);
                if self.selected_device.as_ref() == Some(&id) {
                    self.selected_device = None;
                }
            }

            ServiceEvent::Telemetry { device, ts_ms, points } => {
                if let Some(state) = self.devices.get_mut(&device) {
                    state.last_seen_ts_ms = Some(ts_ms);
                    for point in points {
                        state.latest.insert(point.key, point.value);
                    }
                }
            }

            ServiceEvent::Alarm { device, ts_ms, code, message, severity } => {
                if let Some(state) = self.devices.get_mut(&device) {
                    state.alarms.push(DeviceAlarm {
                        ts_ms,
                        code,
                        message,
                        severity,
                    });
                }
                // Emit UI event for high severity alarms
                if severity >= AlarmSeverity::Error {
                    cx.emit(UIEvent::AlarmReceived { device, code, severity });
                }
            }

            ServiceEvent::DeviceOnlineChanged { device, online, ts_ms } => {
                if let Some(state) = self.devices.get_mut(&device) {
                    state.online = online;
                    state.last_seen_ts_ms = Some(ts_ms);
                }
            }

            ServiceEvent::CommandAck { correlation_id, success, payload, error } => {
                if let Some(cmd) = self.pending_commands.get_mut(&correlation_id) {
                    cmd.status = if success {
                        CommandStatus::Success
                    } else {
                        CommandStatus::Failed
                    };

                    // Emit UI notification
                    let message = if success {
                        format!("Command {} succeeded", cmd.method)
                    } else {
                        format!("Command {} failed: {}", cmd.method, error.as_deref().unwrap_or("Unknown error"))
                    };

                    cx.emit(UIEvent::Toast {
                        message: message.into(),
                        is_error: !success,
                    });
                }
            }

            ServiceEvent::ConnectionState { service, connected, detail } => {
                cx.emit(UIEvent::ConnectionStateChanged {
                    service,
                    connected,
                    detail,
                });
            }

            ServiceEvent::MetricDictionary { entries } => {
                for (id, name) in entries {
                    self.metric_names.insert(id, name);
                }
            }
        }
    }

    // ==================== Getters ====================

    /// Get all devices in display order
    pub fn devices(&self) -> impl Iterator<Item = &DeviceRuntimeState> {
        self.order.iter().filter_map(|id| self.devices.get(id))
    }

    /// Get device count
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Get online device count
    pub fn online_count(&self) -> usize {
        self.devices.values().filter(|d| d.online).count()
    }

    /// Get a specific device
    pub fn device(&self, id: &DeviceId) -> Option<&DeviceRuntimeState> {
        self.devices.get(id)
    }

    /// Get the selected device
    pub fn selected_device(&self) -> Option<&DeviceRuntimeState> {
        self.selected_device
            .as_ref()
            .and_then(|id| self.devices.get(id))
    }

    /// Get the selected device ID
    pub fn selected_device_id(&self) -> Option<&DeviceId> {
        self.selected_device.as_ref()
    }

    /// Get metric name by ID
    pub fn metric_name(&self, id: u16) -> Option<&Arc<str>> {
        self.metric_names.get(&id)
    }

    /// Check if loading
    pub fn is_loading(&self) -> bool {
        self.is_loading
    }

    // ==================== Setters ====================

    /// Select a device
    pub fn select_device(&mut self, id: Option<DeviceId>, cx: &mut Context<Self>) {
        if self.selected_device != id {
            self.selected_device = id;
            cx.notify();
        }
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        if self.is_loading != loading {
            self.is_loading = loading;
            cx.notify();
        }
    }

    /// Toggle device watch status
    pub fn toggle_watch(&mut self, id: &DeviceId, cx: &mut Context<Self>) {
        if let Some(state) = self.devices.get_mut(id) {
            state.watched = !state.watched;
            cx.notify();
        }
    }

    // ==================== Commands ====================

    /// Send a command to a device
    pub fn send_command(
        &mut self,
        services: &ServiceHub,
        device: &DeviceId,
        method: &str,
        params: &str,
        cx: &mut Context<Self>,
    ) {
        match services.send_command(device, method, params) {
            Ok(correlation_id) => {
                self.pending_commands.insert(
                    correlation_id,
                    PendingCommand {
                        device: device.clone(),
                        method: method.into(),
                        status: CommandStatus::Pending,
                        sent_at_ms: chrono_now_ms(),
                    },
                );
                cx.notify();
            }
            Err(e) => {
                cx.emit(UIEvent::Toast {
                    message: format!("Failed to send command: {}", e).into(),
                    is_error: true,
                });
            }
        }
    }

    /// Get pending command status
    pub fn pending_command(&self, correlation_id: &str) -> Option<&PendingCommand> {
        self.pending_commands.get(correlation_id)
    }

    // ==================== Bulk Operations ====================

    /// Set all devices from a list of metadata
    pub fn set_devices(&mut self, metas: Vec<DeviceMeta>, cx: &mut Context<Self>) {
        self.devices.clear();
        self.order.clear();

        for meta in metas {
            let id = meta.id.clone();
            self.devices.insert(id.clone(), DeviceRuntimeState::from_meta(meta));
            self.order.push(id);
        }

        cx.notify();
    }
}

impl Default for FleetState {
    fn default() -> Self {
        Self::new()
    }
}

impl EventEmitter<UIEvent> for FleetState {}

/// Get current time in milliseconds
fn chrono_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
