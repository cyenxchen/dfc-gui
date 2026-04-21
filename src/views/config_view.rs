//! Configuration View
//!
//! Displays Redis configuration with left-right split layout:
//! - Left panel: TopicAgentId list
//! - Right panel: Topic tabs for selected TopicAgentId

use super::service_panel::{
    self, CUSTOM_TYPE_INDEX, REQUEST_TYPES, ServicePublishRequest, ServiceStreamEvent,
    build_pulsar_client_with_fallbacks, normalize_pulsar_service_url,
    pulsar_service_url_candidates, run_service_topic_stream,
};
use crate::assets::CustomIconName;
use crate::connection::{ConfigItem, ConfigLoadState, ConnectedServerInfo, TopicAgentItem};
use crate::services::spawn_named_in_tokio;
use crate::states::{
    ConfigState, DfcAppState, DfcGlobalStore, EventRow, EventSortColumn, EventTableLoadState,
    EventTableState, KeysState, PropRow, PropSortColumn, PropTableLoadState, PropTableState,
    ServiceRequestRow, ServiceTableLoadState, ServiceTableState, SortDirection,
};
use chrono::Local;
use crossbeam_channel::{Receiver, Sender};
use futures::StreamExt;
use gpui::{
    Action, App, Context, Corner, DragMoveEvent, Entity, FocusHandle, Focusable, MouseButton,
    MouseDownEvent, ScrollHandle, ScrollWheelEvent, StatefulInteractiveElement as _, Subscription,
    Task, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Colorize, Disableable, Icon, IconName, Sizable,
    button::{Button, ButtonVariants, DropdownButton},
    calendar::{Calendar, CalendarEvent, CalendarState, Date},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    popover::Popover,
    radio::Radio,
    scroll::{Scrollbar, ScrollbarShow},
    tooltip::Tooltip,
    v_flex,
};
use rust_i18n::t;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use std::time::Instant;
use tokio::sync::watch;

/// Width of the left agent list panel
const AGENT_LIST_WIDTH: f32 = 320.0;
/// Height of the top bar for left/right panels (keeps alignment)
const PANEL_TOPBAR_HEIGHT: f32 = 48.0;
const TOPIC_FEEDBACK_TICK_MS: u64 = 120;
const TOPIC_SWITCH_FEEDBACK_MS: u64 = 320;
const TOPIC_FEEDBACK_FRAME_COUNT: usize = 6;
const TABLE_COLUMN_MIN_WIDTH: f32 = 72.0;
const TABLE_COLUMN_RESIZE_HANDLE_WIDTH: f32 = 8.0;

const PROP_COLUMN_COUNT: usize = PropSortColumn::Summary as usize + 1;
const EVENT_COLUMN_COUNT: usize = EventSortColumn::Summary as usize + 1;
const SERVICE_REQUEST_COLUMN_COUNT: usize = 12;
const SERVICE_RESPONSE_COLUMN_COUNT: usize = 7;

const PROP_DEFAULT_COLUMN_WIDTHS: [f32; PROP_COLUMN_COUNT] = [
    180.0, 110.0, 320.0, 90.0, 120.0, 90.0, 140.0, 180.0, 180.0, 240.0,
];
const EVENT_DEFAULT_COLUMN_WIDTHS: [f32; EVENT_COLUMN_COUNT] = [
    220.0, 110.0, 320.0, 140.0, 90.0, 160.0, 140.0, 160.0, 180.0, 180.0, 140.0, 260.0, 240.0,
];
const SERVICE_REQUEST_DEFAULT_COLUMN_WIDTHS: [f32; SERVICE_REQUEST_COLUMN_COUNT] = [
    140.0, 280.0, 180.0, 110.0, 90.0, 110.0, 220.0, 280.0, 180.0, 140.0, 110.0, 120.0,
];
const SERVICE_RESPONSE_DEFAULT_COLUMN_WIDTHS: [f32; SERVICE_RESPONSE_COLUMN_COUNT] =
    [280.0, 280.0, 180.0, 140.0, 110.0, 180.0, 370.0];

#[derive(Debug)]
enum PropStreamEvent {
    Rows(Vec<PropRow>),
    Error(String),
}

#[derive(Debug)]
enum EventStreamEvent {
    Rows(Vec<EventRow>),
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TopicFeedbackKind {
    Switching,
    Loading,
}

struct PropTopicRuntime {
    state: PropTableState,
    stream_stop: Option<watch::Sender<bool>>,
    ingest_task: Option<Task<()>>,
}

struct EventTopicRuntime {
    state: EventTableState,
    stream_stop: Option<watch::Sender<bool>>,
    ingest_task: Option<Task<()>>,
}

struct ServiceTopicRuntime {
    state: ServiceTableState,
    stream_stop: Option<watch::Sender<bool>>,
    publish_tx: Option<Sender<ServicePublishRequest>>,
    ingest_task: Option<Task<()>>,
}

impl Default for PropTopicRuntime {
    fn default() -> Self {
        Self {
            state: PropTableState::new(),
            stream_stop: None,
            ingest_task: None,
        }
    }
}

impl Default for EventTopicRuntime {
    fn default() -> Self {
        Self {
            state: EventTableState::new(),
            stream_stop: None,
            ingest_task: None,
        }
    }
}

impl Default for ServiceTopicRuntime {
    fn default() -> Self {
        Self {
            state: ServiceTableState::new(),
            stream_stop: None,
            publish_tx: None,
            ingest_task: None,
        }
    }
}

#[derive(Default)]
struct ServerTopicRuntime {
    prop_topics: BTreeMap<String, PropTopicRuntime>,
    event_topics: BTreeMap<String, EventTopicRuntime>,
    service_topics: BTreeMap<String, ServiceTopicRuntime>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
enum ServiceRequestColumn {
    Device = 0,
    Imr,
    RequestTime,
    TimeoutMs,
    IsTest,
    Requester,
    ArgsJson,
    Uuid,
    ResponseTime,
    ResponseCode,
    Responser,
    Summary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
enum ServiceResponseColumn {
    RequestUuid = 0,
    ResponseUuid,
    ResponseTime,
    ResponseCode,
    Responser,
    ReceiveTime,
    Summary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
enum ResizableTableKind {
    Prop,
    Event,
    ServiceRequest,
    ServiceResponse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ColumnResizeToken {
    table: ResizableTableKind,
    column_ix: usize,
}

#[derive(Clone, Copy, Debug)]
struct ActiveColumnResize {
    token: ColumnResizeToken,
    start_mouse_x: f32,
    start_width: f32,
}

#[derive(Clone, Debug)]
struct TableColumnWidths<const N: usize> {
    widths: [f32; N],
}

impl<const N: usize> TableColumnWidths<N> {
    const fn new(widths: [f32; N]) -> Self {
        Self { widths }
    }

    fn get(&self, index: usize) -> f32 {
        self.widths[index]
    }

    fn total(&self) -> f32 {
        self.widths.iter().sum()
    }

    fn resize_from_drag(&mut self, index: usize, start_width: f32, delta_x: f32) {
        self.widths[index] = (start_width + delta_x).max(TABLE_COLUMN_MIN_WIDTH);
    }
}

#[derive(Default)]
struct ColumnResizeGhost;

impl Render for ColumnResizeGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(0.0)).h(px(0.0))
    }
}

type TopicSelectionKey = (Option<String>, Option<String>, Option<String>);
type AgentTabsSignature = (Option<String>, Option<String>, Vec<String>);

fn topic_paths_by_kind<'a>(
    topic_agents: impl IntoIterator<Item = &'a TopicAgentItem>,
) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    let mut prop_topics = BTreeSet::new();
    let mut event_topics = BTreeSet::new();
    let mut service_topics = BTreeSet::new();

    for agent in topic_agents {
        for topic in &agent.topics {
            let path = topic.path.clone();
            if ConfigView::is_prop_topic_path(&path) {
                prop_topics.insert(path);
            } else if ConfigView::is_event_topic_path(&path) {
                event_topics.insert(path);
            } else if ConfigView::is_service_topic_path(&path) {
                service_topics.insert(path);
            }
        }
    }

    (prop_topics, event_topics, service_topics)
}

fn topic_display_name(topic_path: &str) -> String {
    if topic_path.contains("thing_service-BZ-RESPONSE")
        && topic_path.contains("thing_service-BZ-REQUEST")
    {
        return "service".to_string();
    }

    if let Some(start) = topic_path.find("prop_data-BZ-") {
        let after_prefix = start + "prop_data-BZ-".len();
        if let Some(realdev_pos) = topic_path[after_prefix..].find("-realdev-") {
            let a = &topic_path[after_prefix..after_prefix + realdev_pos];
            let b_start = after_prefix + realdev_pos + "-realdev-".len();
            if let Some(last_dash) = topic_path.rfind('-') {
                if last_dash > b_start {
                    let b = &topic_path[b_start..last_dash];
                    if !a.is_empty() && !b.is_empty() {
                        return format!("{a}_{b}");
                    }
                }
            }
        }
    }

    let mut last = topic_path
        .rsplit('/')
        .next()
        .unwrap_or(topic_path)
        .to_string();
    if let Some(last_dash) = last.rfind('-') {
        let tail = &last[last_dash + 1..];
        if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
            last.truncate(last_dash);
        }
    }
    last
}

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
enum AgentQueryMode {
    All,
    Prefix,
    Exact,
}

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, JsonSchema, Action)]
enum PropPageSize {
    S10,
    S20,
    S50,
    S100,
}

impl PropPageSize {
    fn value(self) -> usize {
        match self {
            Self::S10 => 10,
            Self::S20 => 20,
            Self::S50 => 50,
            Self::S100 => 100,
        }
    }

    fn from_value(value: usize) -> Self {
        match value {
            10 => Self::S10,
            50 => Self::S50,
            100 => Self::S100,
            _ => Self::S20,
        }
    }
}

impl Default for AgentQueryMode {
    fn default() -> Self {
        Self::All
    }
}

/// Form-level state for the service request panel.
pub struct ServiceFormState {
    pub devices_input: Entity<InputState>,
    pub timeout_input: Entity<InputState>,
    pub manual_imr_input: Entity<InputState>,
    pub requester_input: Entity<InputState>,
    pub args_input: Entity<InputState>,
    pub is_test: bool,
    pub selected_type_idx: usize,
    pub error_message: Option<String>,
}

/// Per-column filter input states for the prop topic table.
struct PropFilterInputs {
    global_uuid: Entity<InputState>,
    device: Entity<InputState>,
    imr: Entity<InputState>,
    imid: Entity<InputState>,
    value: Entity<InputState>,
    quality: Entity<InputState>,
    bcrid: Entity<InputState>,
    time: Entity<InputState>,
    time_calendar: Entity<CalendarState>,
    message_time: Entity<InputState>,
    message_time_calendar: Entity<CalendarState>,
    summary: Entity<InputState>,
}

impl PropFilterInputs {
    fn entity(&self, col: PropSortColumn) -> &Entity<InputState> {
        match col {
            PropSortColumn::GlobalUuid => &self.global_uuid,
            PropSortColumn::Device => &self.device,
            PropSortColumn::Imr => &self.imr,
            PropSortColumn::Imid => &self.imid,
            PropSortColumn::Value => &self.value,
            PropSortColumn::Quality => &self.quality,
            PropSortColumn::Bcrid => &self.bcrid,
            PropSortColumn::Time => &self.time,
            PropSortColumn::MessageTime => &self.message_time,
            PropSortColumn::Summary => &self.summary,
        }
    }

    fn all(&self) -> [&Entity<InputState>; 10] {
        [
            &self.global_uuid,
            &self.device,
            &self.imr,
            &self.imid,
            &self.value,
            &self.quality,
            &self.bcrid,
            &self.time,
            &self.message_time,
            &self.summary,
        ]
    }

    fn calendar(&self, col: PropSortColumn) -> Option<&Entity<CalendarState>> {
        match col {
            PropSortColumn::Time => Some(&self.time_calendar),
            PropSortColumn::MessageTime => Some(&self.message_time_calendar),
            _ => None,
        }
    }

    fn calendars(&self) -> [&Entity<CalendarState>; 2] {
        [&self.time_calendar, &self.message_time_calendar]
    }
}

/// Per-column filter input states for the event topic table.
struct EventFilterInputs {
    uuid: Entity<InputState>,
    device: Entity<InputState>,
    imr: Entity<InputState>,
    event_type: Entity<InputState>,
    level: Entity<InputState>,
    tags: Entity<InputState>,
    codes: Entity<InputState>,
    str_codes: Entity<InputState>,
    happened_time: Entity<InputState>,
    happened_time_calendar: Entity<CalendarState>,
    record_time: Entity<InputState>,
    record_time_calendar: Entity<CalendarState>,
    bcr_id: Entity<InputState>,
    context: Entity<InputState>,
    summary: Entity<InputState>,
}

impl EventFilterInputs {
    fn entity(&self, col: EventSortColumn) -> &Entity<InputState> {
        match col {
            EventSortColumn::Uuid => &self.uuid,
            EventSortColumn::Device => &self.device,
            EventSortColumn::Imr => &self.imr,
            EventSortColumn::EventType => &self.event_type,
            EventSortColumn::Level => &self.level,
            EventSortColumn::Tags => &self.tags,
            EventSortColumn::Codes => &self.codes,
            EventSortColumn::StrCodes => &self.str_codes,
            EventSortColumn::HappenedTime => &self.happened_time,
            EventSortColumn::RecordTime => &self.record_time,
            EventSortColumn::BcrId => &self.bcr_id,
            EventSortColumn::Context => &self.context,
            EventSortColumn::Summary => &self.summary,
        }
    }

    fn all(&self) -> [&Entity<InputState>; 13] {
        [
            &self.uuid,
            &self.device,
            &self.imr,
            &self.event_type,
            &self.level,
            &self.tags,
            &self.codes,
            &self.str_codes,
            &self.happened_time,
            &self.record_time,
            &self.bcr_id,
            &self.context,
            &self.summary,
        ]
    }

    fn calendar(&self, col: EventSortColumn) -> Option<&Entity<CalendarState>> {
        match col {
            EventSortColumn::HappenedTime => Some(&self.happened_time_calendar),
            EventSortColumn::RecordTime => Some(&self.record_time_calendar),
            _ => None,
        }
    }

    fn calendars(&self) -> [&Entity<CalendarState>; 2] {
        [&self.happened_time_calendar, &self.record_time_calendar]
    }
}

fn new_filter_input(window: &mut Window, cx: &mut Context<ConfigView>) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .clean_on_escape()
            .placeholder("过滤...")
    })
}

fn new_date_filter_calendar(
    window: &mut Window,
    cx: &mut Context<ConfigView>,
) -> Entity<CalendarState> {
    cx.new(|cx| CalendarState::new(window, cx))
}

fn date_filter_value(date: Date) -> String {
    date.start()
        .map(|date| date.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

fn new_table_cell_input(window: &mut Window, cx: &mut Context<ConfigView>) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .multi_line(true)
            .rows(1)
            .soft_wrap(false)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TableCellId {
    row_uid: u64,
    column_key: &'static str,
}

impl TableCellId {
    const fn new(row_uid: u64, column_key: &'static str) -> Self {
        Self {
            row_uid,
            column_key,
        }
    }
}

/// Configuration view component
pub struct ConfigView {
    /// App state entity
    app_state: Entity<DfcAppState>,
    /// Config state entity
    config_state: Entity<ConfigState>,
    /// Keys state entity
    keys_state: Entity<KeysState>,
    /// Search input state for filtering TopicAgentIds
    agent_search_state: Entity<InputState>,
    agent_query_mode: AgentQueryMode,
    agent_list_focus_handle: FocusHandle,
    topic_content_focus_handle: FocusHandle,
    /// Shared horizontal scroll handle for the selected agent topic tabs.
    agent_tabs_horizontal_scroll_handle: ScrollHandle,
    /// Prop topic table state (for `prop_data` topics)
    prop_table_state: Entity<PropTableState>,
    /// Per-column filter inputs for the prop topic table
    prop_filter_inputs: PropFilterInputs,
    /// Scroll handle for the visible body scrollbar in the prop table
    prop_table_scroll_handle: ScrollHandle,
    /// Shared horizontal scroll handle for the prop table header and body
    prop_table_horizontal_scroll_handle: ScrollHandle,
    /// Runtime-resizable prop table column widths
    prop_column_widths: TableColumnWidths<PROP_COLUMN_COUNT>,
    /// Event topic table state (for `thing_event` topics)
    event_table_state: Entity<EventTableState>,
    /// Per-column filter inputs for the event topic table
    event_filter_inputs: EventFilterInputs,
    /// Scroll handle for the visible body scrollbar in the event table
    event_table_scroll_handle: ScrollHandle,
    /// Shared horizontal scroll handle for the event table header and body
    event_table_horizontal_scroll_handle: ScrollHandle,
    /// Runtime-resizable event table column widths
    event_column_widths: TableColumnWidths<EVENT_COLUMN_COUNT>,
    /// Shared editor used when a table cell enters copy/select mode
    table_cell_input: Entity<InputState>,
    /// Currently active table cell in copy/select mode
    active_table_cell: Option<TableCellId>,
    /// Service topic state (for `thing_service` REQUEST/RESPONSE topic pair)
    service_table_state: Entity<ServiceTableState>,
    service_form: ServiceFormState,
    service_table_horizontal_scroll_handle: ScrollHandle,
    service_response_horizontal_scroll_handle: ScrollHandle,
    service_request_column_widths: TableColumnWidths<SERVICE_REQUEST_COLUMN_COUNT>,
    service_response_column_widths: TableColumnWidths<SERVICE_RESPONSE_COLUMN_COUNT>,
    active_column_resize: Option<ActiveColumnResize>,
    /// Service publish sender for the currently visible server/topic session.
    service_publish_tx: Option<Sender<ServicePublishRequest>>,
    /// Per-server topic runtimes keep their own table caches and background streams alive.
    server_topic_runtimes: BTreeMap<String, ServerTopicRuntime>,
    /// Skip pushing visible state back into the runtime cache for the next prop-state observe.
    suppress_prop_state_persist: bool,
    /// Skip pushing visible state back into the runtime cache for the next event-state observe.
    suppress_event_state_persist: bool,
    /// Skip pushing visible state back into the runtime cache for the next service-state observe.
    suppress_service_state_persist: bool,
    service_row_uid: Arc<AtomicU64>,
    /// Monotonic row id generator
    prop_row_uid: Arc<AtomicU64>,
    /// Monotonic event row id generator
    event_row_uid: Arc<AtomicU64>,
    /// Animated frame for the right-side switching/loading feedback.
    topic_feedback_frame: usize,
    _topic_feedback_task: Option<Task<()>>,
    last_selection_key: TopicSelectionKey,
    last_agent_tabs_signature: AgentTabsSignature,
    agent_tabs_scrollbar_visible: bool,
    switch_feedback_until: Option<Instant>,
    /// Subscriptions
    _subscriptions: Vec<Subscription>,
}

impl ConfigView {
    /// Create a new config view
    pub fn new(
        app_state: Entity<DfcAppState>,
        config_state: Entity<ConfigState>,
        keys_state: Entity<KeysState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();
        let agent_list_focus_handle = cx.focus_handle();
        let topic_content_focus_handle = cx.focus_handle();

        // Create agent search input
        let agent_search_state = cx.new(|cx| {
            let locale = cx.global::<DfcGlobalStore>().read(cx).locale().to_string();
            let placeholder = t!("config.search_agent_placeholder", locale = &locale).to_string();
            InputState::new(window, cx)
                .clean_on_escape()
                .placeholder(placeholder)
        });

        // Prop topic table state
        let prop_table_state = cx.new(|_| PropTableState::new());
        let prop_row_uid = Arc::new(AtomicU64::new(1));
        let event_table_state = cx.new(|_| EventTableState::new());
        let event_row_uid = Arc::new(AtomicU64::new(1));
        let service_table_state = cx.new(|_| ServiceTableState::new());
        let service_row_uid = Arc::new(AtomicU64::new(1));
        let table_cell_input = new_table_cell_input(window, cx);

        let devices_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .auto_grow(2, 6)
                .placeholder("请输入设备号, 每行一个 ...")
        });
        let timeout_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("超时时间(毫秒)");
            state.set_value("5000".to_string(), window, cx);
            state
        });
        let manual_imr_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("WindTurbine/SERVICE/..."));
        let requester_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("请求者");
            state.set_value("V8Test".to_string(), window, cx);
            state
        });
        let args_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .auto_grow(2, 6)
                .placeholder("请输入参数 (JSON 格式)")
        });

        let service_form = ServiceFormState {
            devices_input,
            timeout_input,
            manual_imr_input,
            requester_input,
            args_input,
            is_test: false,
            selected_type_idx: CUSTOM_TYPE_INDEX,
            error_message: None,
        };

        let prop_filter_inputs = PropFilterInputs {
            global_uuid: new_filter_input(window, cx),
            device: new_filter_input(window, cx),
            imr: new_filter_input(window, cx),
            imid: new_filter_input(window, cx),
            value: new_filter_input(window, cx),
            quality: new_filter_input(window, cx),
            bcrid: new_filter_input(window, cx),
            time: new_filter_input(window, cx),
            time_calendar: new_date_filter_calendar(window, cx),
            message_time: new_filter_input(window, cx),
            message_time_calendar: new_date_filter_calendar(window, cx),
            summary: new_filter_input(window, cx),
        };

        let event_filter_inputs = EventFilterInputs {
            uuid: new_filter_input(window, cx),
            device: new_filter_input(window, cx),
            imr: new_filter_input(window, cx),
            event_type: new_filter_input(window, cx),
            level: new_filter_input(window, cx),
            tags: new_filter_input(window, cx),
            codes: new_filter_input(window, cx),
            str_codes: new_filter_input(window, cx),
            happened_time: new_filter_input(window, cx),
            happened_time_calendar: new_date_filter_calendar(window, cx),
            record_time: new_filter_input(window, cx),
            record_time_calendar: new_date_filter_calendar(window, cx),
            bcr_id: new_filter_input(window, cx),
            context: new_filter_input(window, cx),
            summary: new_filter_input(window, cx),
        };

        for (col, entity) in [
            (
                PropSortColumn::GlobalUuid,
                prop_filter_inputs.global_uuid.clone(),
            ),
            (PropSortColumn::Device, prop_filter_inputs.device.clone()),
            (PropSortColumn::Imr, prop_filter_inputs.imr.clone()),
            (PropSortColumn::Imid, prop_filter_inputs.imid.clone()),
            (PropSortColumn::Value, prop_filter_inputs.value.clone()),
            (PropSortColumn::Quality, prop_filter_inputs.quality.clone()),
            (PropSortColumn::Bcrid, prop_filter_inputs.bcrid.clone()),
            (PropSortColumn::Time, prop_filter_inputs.time.clone()),
            (
                PropSortColumn::MessageTime,
                prop_filter_inputs.message_time.clone(),
            ),
            (PropSortColumn::Summary, prop_filter_inputs.summary.clone()),
        ] {
            subscriptions.push(cx.subscribe(&entity, move |this, state, event, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = state.read(cx).value().to_string();
                    this.prop_table_state.update(cx, |s, cx| {
                        s.set_filter(col, value);
                        cx.notify();
                    });
                }
            }));
        }

        for (calendar, input) in [
            (
                prop_filter_inputs.time_calendar.clone(),
                prop_filter_inputs.time.clone(),
            ),
            (
                prop_filter_inputs.message_time_calendar.clone(),
                prop_filter_inputs.message_time.clone(),
            ),
        ] {
            subscriptions.push(cx.subscribe_in(
                &calendar,
                window,
                move |_this, _state, event: &CalendarEvent, window, cx| match event {
                    CalendarEvent::Selected(date) => {
                        let value = date_filter_value(*date);
                        input.update(cx, |state, cx| {
                            state.set_value(value, window, cx);
                        });
                    }
                },
            ));
        }

        for (col, entity) in [
            (EventSortColumn::Uuid, event_filter_inputs.uuid.clone()),
            (EventSortColumn::Device, event_filter_inputs.device.clone()),
            (EventSortColumn::Imr, event_filter_inputs.imr.clone()),
            (
                EventSortColumn::EventType,
                event_filter_inputs.event_type.clone(),
            ),
            (EventSortColumn::Level, event_filter_inputs.level.clone()),
            (EventSortColumn::Tags, event_filter_inputs.tags.clone()),
            (EventSortColumn::Codes, event_filter_inputs.codes.clone()),
            (
                EventSortColumn::StrCodes,
                event_filter_inputs.str_codes.clone(),
            ),
            (
                EventSortColumn::HappenedTime,
                event_filter_inputs.happened_time.clone(),
            ),
            (
                EventSortColumn::RecordTime,
                event_filter_inputs.record_time.clone(),
            ),
            (EventSortColumn::BcrId, event_filter_inputs.bcr_id.clone()),
            (
                EventSortColumn::Context,
                event_filter_inputs.context.clone(),
            ),
            (
                EventSortColumn::Summary,
                event_filter_inputs.summary.clone(),
            ),
        ] {
            subscriptions.push(cx.subscribe(&entity, move |this, state, event, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = state.read(cx).value().to_string();
                    this.event_table_state.update(cx, |s, cx| {
                        s.set_filter(col, value);
                        cx.notify();
                    });
                }
            }));
        }

        for (calendar, input) in [
            (
                event_filter_inputs.happened_time_calendar.clone(),
                event_filter_inputs.happened_time.clone(),
            ),
            (
                event_filter_inputs.record_time_calendar.clone(),
                event_filter_inputs.record_time.clone(),
            ),
        ] {
            subscriptions.push(cx.subscribe_in(
                &calendar,
                window,
                move |_this, _state, event: &CalendarEvent, window, cx| match event {
                    CalendarEvent::Selected(date) => {
                        let value = date_filter_value(*date);
                        input.update(cx, |state, cx| {
                            state.set_value(value, window, cx);
                        });
                    }
                },
            ));
        }

        subscriptions.push(cx.observe(&prop_table_state, |this, _model, cx| {
            if this.suppress_prop_state_persist {
                this.suppress_prop_state_persist = false;
            } else {
                this.persist_visible_prop_state_for_current_server(cx);
            }
            cx.notify();
        }));
        subscriptions.push(cx.observe(&event_table_state, |this, _model, cx| {
            if this.suppress_event_state_persist {
                this.suppress_event_state_persist = false;
            } else {
                this.persist_visible_event_state_for_current_server(cx);
            }
            cx.notify();
        }));
        subscriptions.push(cx.observe(&service_table_state, |this, _model, cx| {
            if this.suppress_service_state_persist {
                this.suppress_service_state_persist = false;
            } else {
                this.persist_visible_service_state_for_current_server(cx);
            }
            cx.notify();
        }));
        subscriptions.push(cx.on_blur(
            &table_cell_input.read(cx).focus_handle(cx),
            window,
            |this, _window, cx| {
                if this.active_table_cell.take().is_some() {
                    cx.notify();
                }
            },
        ));

        // Subscribe to config state changes. We use observe_in (instead of observe) so
        // that the callback receives a Window — sync_topic_stream_with_selection needs
        // it to clear filter input fields on topic transitions.
        subscriptions.push(
            cx.observe_in(&config_state, window, |this, _model, window, cx| {
                let connected_server_ids =
                    this.config_state.read(cx).connected_server_ids().to_vec();
                let stale_server_ids: Vec<String> = this
                    .server_topic_runtimes
                    .keys()
                    .filter(|server_id| !connected_server_ids.iter().any(|id| id == *server_id))
                    .cloned()
                    .collect();
                for server_id in stale_server_ids {
                    this.stop_prop_streams_for_server(&server_id);
                    this.stop_event_streams_for_server(&server_id);
                    this.stop_service_streams_for_server(&server_id);
                    this.server_topic_runtimes.remove(&server_id);
                }

                let load_state = this.config_state.read(cx).load_state();
                if let Some(server_id) = this.current_server_id(cx) {
                    if matches!(
                        load_state,
                        ConfigLoadState::Loading | ConfigLoadState::Error(_)
                    ) {
                        this.stop_prop_streams_for_server(&server_id);
                        this.stop_event_streams_for_server(&server_id);
                        this.stop_service_streams_for_server(&server_id);
                        this.service_publish_tx = None;
                        cx.notify();
                        return;
                    }

                    this.prune_removed_topic_runtimes_for_server(&server_id, cx);
                }

                let query = this.agent_search_state.read(cx).value().trim().to_string();
                if !query.is_empty() {
                    let selected = this
                        .config_state
                        .read(cx)
                        .selected_agent_id()
                        .map(|s| s.to_string());
                    if let Some(selected) = selected {
                        if !this.agent_id_matches_query(&selected, &query) {
                            this.reset_agent_tabs_horizontal_scroll();
                            this.config_state.update(cx, |state, cx| {
                                state.select_agent(None, cx);
                            });
                        }
                    }
                }
                this.sync_topic_stream_with_selection(window, cx);
                cx.notify();
            }),
        );

        // Subscribe to agent search input changes for filtering and selection clearing
        subscriptions.push(cx.subscribe(&agent_search_state, |this, state, event, cx| {
            if matches!(event, InputEvent::Change | InputEvent::PressEnter { .. }) {
                let query = state.read(cx).value().trim().to_string();

                if !query.is_empty() {
                    let selected = this
                        .config_state
                        .read(cx)
                        .selected_agent_id()
                        .map(|s| s.to_string());
                    if let Some(selected) = selected {
                        if !this.agent_id_matches_query(&selected, &query) {
                            this.reset_agent_tabs_horizontal_scroll();
                            this.config_state.update(cx, |state, cx| {
                                state.select_agent(None, cx);
                            });
                        }
                    }
                }

                cx.notify();
            }
        }));
        subscriptions.push(cx.observe_window_bounds(window, |this, _window, cx| {
            this.update_agent_tabs_scrollbar_visibility(cx);
        }));

        Self {
            app_state,
            config_state,
            keys_state,
            agent_search_state,
            agent_query_mode: AgentQueryMode::default(),
            agent_list_focus_handle,
            topic_content_focus_handle,
            agent_tabs_horizontal_scroll_handle: ScrollHandle::default(),
            prop_table_state,
            prop_filter_inputs,
            prop_table_scroll_handle: ScrollHandle::default(),
            prop_table_horizontal_scroll_handle: ScrollHandle::default(),
            prop_column_widths: TableColumnWidths::new(PROP_DEFAULT_COLUMN_WIDTHS),
            event_table_state,
            event_filter_inputs,
            event_table_scroll_handle: ScrollHandle::default(),
            event_table_horizontal_scroll_handle: ScrollHandle::default(),
            event_column_widths: TableColumnWidths::new(EVENT_DEFAULT_COLUMN_WIDTHS),
            table_cell_input,
            active_table_cell: None,
            service_table_state,
            service_form,
            service_table_horizontal_scroll_handle: ScrollHandle::default(),
            service_response_horizontal_scroll_handle: ScrollHandle::default(),
            service_request_column_widths: TableColumnWidths::new(
                SERVICE_REQUEST_DEFAULT_COLUMN_WIDTHS,
            ),
            service_response_column_widths: TableColumnWidths::new(
                SERVICE_RESPONSE_DEFAULT_COLUMN_WIDTHS,
            ),
            active_column_resize: None,
            service_publish_tx: None,
            server_topic_runtimes: BTreeMap::new(),
            suppress_prop_state_persist: false,
            suppress_event_state_persist: false,
            suppress_service_state_persist: false,
            service_row_uid,
            prop_row_uid,
            event_row_uid,
            topic_feedback_frame: 0,
            _topic_feedback_task: None,
            last_selection_key: (None, None, None),
            last_agent_tabs_signature: (None, None, Vec::new()),
            agent_tabs_scrollbar_visible: false,
            switch_feedback_until: None,
            _subscriptions: subscriptions,
        }
    }

    /// Get the locale string
    fn locale(&self, cx: &App) -> String {
        cx.global::<DfcGlobalStore>().read(cx).locale().to_string()
    }

    fn activate_table_cell(
        &mut self,
        cell: TableCellId,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.table_cell_input.update(cx, |state, cx| {
            state.set_value(text.to_string(), window, cx);
        });
        self.active_table_cell = Some(cell);

        let input = self.table_cell_input.clone();
        window.defer(cx, move |window, cx| {
            let handle = input.read(cx).focus_handle(cx);
            window.focus(&handle);
            window.dispatch_action(Box::new(gpui_component::input::SelectAll), cx);
        });
        cx.notify();
    }

    fn agent_id_matches_query(&self, agent_id: &str, query: &str) -> bool {
        match self.agent_query_mode {
            AgentQueryMode::All => agent_id.contains(query),
            AgentQueryMode::Prefix => agent_id.starts_with(query),
            AgentQueryMode::Exact => agent_id == query,
        }
    }

    fn prop_column_width(&self, column: PropSortColumn) -> f32 {
        self.prop_column_widths.get(column as usize)
    }

    fn event_column_width(&self, column: EventSortColumn) -> f32 {
        self.event_column_widths.get(column as usize)
    }

    fn service_request_column_width(&self, column: ServiceRequestColumn) -> f32 {
        self.service_request_column_widths.get(column as usize)
    }

    fn service_response_column_width(&self, column: ServiceResponseColumn) -> f32 {
        self.service_response_column_widths.get(column as usize)
    }

    fn start_column_resize(
        &mut self,
        table: ResizableTableKind,
        column_ix: usize,
        start_mouse_x: f32,
        cx: &mut Context<Self>,
    ) {
        let start_width = match table {
            ResizableTableKind::Prop => self.prop_column_widths.get(column_ix),
            ResizableTableKind::Event => self.event_column_widths.get(column_ix),
            ResizableTableKind::ServiceRequest => self.service_request_column_widths.get(column_ix),
            ResizableTableKind::ServiceResponse => {
                self.service_response_column_widths.get(column_ix)
            }
        };

        self.active_column_resize = Some(ActiveColumnResize {
            token: ColumnResizeToken { table, column_ix },
            start_mouse_x,
            start_width,
        });
        cx.notify();
    }

    fn apply_column_resize(
        &mut self,
        table: ResizableTableKind,
        column_ix: usize,
        mouse_x: f32,
        cx: &mut Context<Self>,
    ) {
        let Some(active) = self.active_column_resize else {
            return;
        };

        if active.token.table != table || active.token.column_ix != column_ix {
            return;
        }

        let delta_x = mouse_x - active.start_mouse_x;
        match table {
            ResizableTableKind::Prop => {
                self.prop_column_widths
                    .resize_from_drag(column_ix, active.start_width, delta_x);
            }
            ResizableTableKind::Event => {
                self.event_column_widths
                    .resize_from_drag(column_ix, active.start_width, delta_x);
            }
            ResizableTableKind::ServiceRequest => {
                self.service_request_column_widths.resize_from_drag(
                    column_ix,
                    active.start_width,
                    delta_x,
                );
            }
            ResizableTableKind::ServiceResponse => {
                self.service_response_column_widths.resize_from_drag(
                    column_ix,
                    active.start_width,
                    delta_x,
                );
            }
        }
        cx.notify();
    }

    fn finish_column_resize(
        &mut self,
        table: ResizableTableKind,
        column_ix: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.active_column_resize.is_some_and(|active| {
            active.token.table == table && active.token.column_ix == column_ix
        }) {
            self.active_column_resize = None;
            cx.notify();
            true
        } else {
            false
        }
    }

    fn is_column_resizing(&self, table: ResizableTableKind, column_ix: usize) -> bool {
        self.active_column_resize.is_some_and(|active| {
            active.token.table == table && active.token.column_ix == column_ix
        })
    }

    fn render_column_resize_handle(
        &self,
        table: ResizableTableKind,
        column_ix: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let token = ColumnResizeToken { table, column_ix };
        let handle_id = match table {
            ResizableTableKind::Prop => ("prop-col-resize", column_ix),
            ResizableTableKind::Event => ("event-col-resize", column_ix),
            ResizableTableKind::ServiceRequest => ("svc-req-col-resize", column_ix),
            ResizableTableKind::ServiceResponse => ("svc-resp-col-resize", column_ix),
        };
        let line_color = if self.is_column_resizing(table, column_ix) {
            cx.theme().primary
        } else {
            cx.theme().border.opacity(0.8)
        };

        div()
            .id(handle_id)
            .absolute()
            .top_0()
            .right(px(-(TABLE_COLUMN_RESIZE_HANDLE_WIDTH / 2.0)))
            .h_full()
            .w(px(TABLE_COLUMN_RESIZE_HANDLE_WIDTH))
            .occlude()
            .cursor_col_resize()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.start_column_resize(table, column_ix, f32::from(event.position.x), cx);
                    cx.stop_propagation();
                }),
            )
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<ColumnResizeToken>, _, cx| {
                    let active = *event.drag(cx);
                    this.apply_column_resize(
                        active.table,
                        active.column_ix,
                        f32::from(event.event.position.x),
                        cx,
                    );
                    cx.stop_propagation();
                },
            ))
            .on_drag(token, |_, _, _, cx| {
                cx.stop_propagation();
                cx.new(|_| ColumnResizeGhost)
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if this.finish_column_resize(table, column_ix, cx) {
                        cx.stop_propagation();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if this.finish_column_resize(table, column_ix, cx) {
                        cx.stop_propagation();
                    }
                }),
            )
            .child(div().mx_auto().h_full().w(px(1.0)).bg(line_color))
    }

    fn clear_selected_agent_if_filtered_out(&self, cx: &mut Context<Self>) {
        let query = self.agent_search_state.read(cx).value().trim().to_string();
        if query.is_empty() {
            return;
        }

        let selected = self
            .config_state
            .read(cx)
            .selected_agent_id()
            .map(|s| s.to_string());
        if let Some(selected) = selected {
            if !self.agent_id_matches_query(&selected, &query) {
                self.reset_agent_tabs_horizontal_scroll();
                self.config_state.update(cx, |state, cx| {
                    state.select_agent(None, cx);
                });
            }
        }
    }

    fn reset_agent_tabs_horizontal_scroll(&self) -> bool {
        let mut offset = self.agent_tabs_horizontal_scroll_handle.offset();
        if offset.x == px(0.0) {
            return false;
        }

        offset.x = px(0.0);
        self.agent_tabs_horizontal_scroll_handle.set_offset(offset);
        tracing::debug!("reset agent topic tabs horizontal scroll offset");
        true
    }

    fn update_agent_tabs_scrollbar_visibility(&mut self, cx: &mut Context<Self>) -> bool {
        let is_visible = self.agent_tabs_horizontal_scroll_handle.max_offset().width > px(0.0);
        if self.agent_tabs_scrollbar_visible == is_visible {
            return false;
        }

        self.agent_tabs_scrollbar_visible = is_visible;
        tracing::debug!(
            visible = is_visible,
            max_offset_x = ?self.agent_tabs_horizontal_scroll_handle.max_offset().width,
            "updated agent topic tabs scrollbar visibility"
        );
        cx.notify();
        true
    }

    fn schedule_agent_tabs_scrollbar_visibility_refresh(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.entity();
        window.defer(cx, move |_window, cx| {
            let _ = entity.update(cx, |this, cx| {
                this.update_agent_tabs_scrollbar_visibility(cx);
            });
        });
    }

    pub fn handle_filter_shortcut(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.agent_list_focus_handle.contains_focused(window, cx) {
            return false;
        }

        let input = self.agent_search_state.clone();
        window.defer(cx, move |window, cx| {
            let handle = input.read(cx).focus_handle(cx);
            window.focus(&handle);
            window.dispatch_action(Box::new(gpui_component::input::SelectAll), cx);
        });

        true
    }

    fn current_server_id(&self, cx: &App) -> Option<String> {
        self.app_state
            .read(cx)
            .selected_server_id()
            .map(str::to_string)
    }

    fn current_selected_topic_path_raw(&self, cx: &App) -> Option<String> {
        let state = self.config_state.read(cx);
        match (state.selected_agent(), state.selected_topic_index()) {
            (Some(agent), Some(idx)) => agent.topics.get(idx as usize).map(|t| t.path.clone()),
            _ => None,
        }
    }

    fn current_runtime_mut(&mut self, server_id: &str) -> &mut ServerTopicRuntime {
        self.server_topic_runtimes
            .entry(server_id.to_string())
            .or_default()
    }

    fn prop_topic_runtime_mut(
        &mut self,
        server_id: &str,
        topic_path: &str,
    ) -> &mut PropTopicRuntime {
        let runtime = self.current_runtime_mut(server_id);
        runtime
            .prop_topics
            .entry(topic_path.to_string())
            .or_insert_with(|| {
                tracing::debug!(server_id, topic = %topic_path, "creating prop topic runtime");
                let mut topic_runtime = PropTopicRuntime::default();
                topic_runtime
                    .state
                    .reset_for_topic(Some(topic_path.to_string()));
                topic_runtime
            })
    }

    fn event_topic_runtime_mut(
        &mut self,
        server_id: &str,
        topic_path: &str,
    ) -> &mut EventTopicRuntime {
        let runtime = self.current_runtime_mut(server_id);
        runtime
            .event_topics
            .entry(topic_path.to_string())
            .or_insert_with(|| {
                tracing::debug!(server_id, topic = %topic_path, "creating event topic runtime");
                let mut topic_runtime = EventTopicRuntime::default();
                topic_runtime
                    .state
                    .reset_for_topic(Some(topic_path.to_string()));
                topic_runtime
            })
    }

    fn service_topic_runtime_mut(
        &mut self,
        server_id: &str,
        topic_path: &str,
    ) -> &mut ServiceTopicRuntime {
        let runtime = self.current_runtime_mut(server_id);
        runtime
            .service_topics
            .entry(topic_path.to_string())
            .or_insert_with(|| {
                tracing::debug!(server_id, topic = %topic_path, "creating service topic runtime");
                let mut topic_runtime = ServiceTopicRuntime::default();
                topic_runtime
                    .state
                    .reset_for_topic(Some(topic_path.to_string()));
                topic_runtime
            })
    }

    fn stop_prop_streams_for_server(&mut self, server_id: &str) {
        let Some(runtime) = self.server_topic_runtimes.get_mut(server_id) else {
            return;
        };
        for (topic_path, topic_runtime) in &mut runtime.prop_topics {
            if let Some(stop) = topic_runtime.stream_stop.take() {
                tracing::info!(server_id, topic = %topic_path, "stopping prop topic stream");
                let _ = stop.send(true);
            }
            if let Some(task) = topic_runtime.ingest_task.take() {
                drop(task);
            }
        }
    }

    fn stop_and_remove_prop_topic_runtime(&mut self, server_id: &str, topic_path: &str) {
        let Some(runtime) = self.server_topic_runtimes.get_mut(server_id) else {
            return;
        };
        let Some(mut topic_runtime) = runtime.prop_topics.remove(topic_path) else {
            return;
        };

        tracing::info!(server_id, topic = %topic_path, "pruning removed prop topic runtime");
        if let Some(stop) = topic_runtime.stream_stop.take() {
            let _ = stop.send(true);
        }
        if let Some(task) = topic_runtime.ingest_task.take() {
            drop(task);
        }
    }

    fn stop_event_streams_for_server(&mut self, server_id: &str) {
        let Some(runtime) = self.server_topic_runtimes.get_mut(server_id) else {
            return;
        };
        for (topic_path, topic_runtime) in &mut runtime.event_topics {
            if let Some(stop) = topic_runtime.stream_stop.take() {
                tracing::info!(server_id, topic = %topic_path, "stopping event topic stream");
                let _ = stop.send(true);
            }
            if let Some(task) = topic_runtime.ingest_task.take() {
                drop(task);
            }
        }
    }

    fn stop_and_remove_event_topic_runtime(&mut self, server_id: &str, topic_path: &str) {
        let Some(runtime) = self.server_topic_runtimes.get_mut(server_id) else {
            return;
        };
        let Some(mut topic_runtime) = runtime.event_topics.remove(topic_path) else {
            return;
        };

        tracing::info!(server_id, topic = %topic_path, "pruning removed event topic runtime");
        if let Some(stop) = topic_runtime.stream_stop.take() {
            let _ = stop.send(true);
        }
        if let Some(task) = topic_runtime.ingest_task.take() {
            drop(task);
        }
    }

    fn stop_service_streams_for_server(&mut self, server_id: &str) {
        let Some(runtime) = self.server_topic_runtimes.get_mut(server_id) else {
            return;
        };
        for (topic_path, topic_runtime) in &mut runtime.service_topics {
            if let Some(stop) = topic_runtime.stream_stop.take() {
                tracing::info!(server_id, topic = %topic_path, "stopping service topic stream");
                let _ = stop.send(true);
            }
            if let Some(task) = topic_runtime.ingest_task.take() {
                drop(task);
            }
            topic_runtime.publish_tx = None;
        }
    }

    fn stop_and_remove_service_topic_runtime(&mut self, server_id: &str, topic_path: &str) {
        let Some(runtime) = self.server_topic_runtimes.get_mut(server_id) else {
            return;
        };
        let Some(mut topic_runtime) = runtime.service_topics.remove(topic_path) else {
            return;
        };

        tracing::info!(server_id, topic = %topic_path, "pruning removed service topic runtime");
        if let Some(stop) = topic_runtime.stream_stop.take() {
            let _ = stop.send(true);
        }
        if let Some(task) = topic_runtime.ingest_task.take() {
            drop(task);
        }
        topic_runtime.publish_tx = None;
    }

    fn prune_removed_topic_runtimes_for_server(&mut self, server_id: &str, cx: &App) {
        let (valid_prop_topics, valid_event_topics, valid_service_topics) = {
            let config_state = self.config_state.read(cx);
            topic_paths_by_kind(config_state.topic_agents())
        };

        let Some(runtime) = self.server_topic_runtimes.get(server_id) else {
            return;
        };

        let stale_prop_topics: Vec<String> = runtime
            .prop_topics
            .keys()
            .filter(|topic_path| !valid_prop_topics.contains(*topic_path))
            .cloned()
            .collect();
        let stale_event_topics: Vec<String> = runtime
            .event_topics
            .keys()
            .filter(|topic_path| !valid_event_topics.contains(*topic_path))
            .cloned()
            .collect();
        let stale_service_topics: Vec<String> = runtime
            .service_topics
            .keys()
            .filter(|topic_path| !valid_service_topics.contains(*topic_path))
            .cloned()
            .collect();

        for topic_path in stale_prop_topics {
            self.stop_and_remove_prop_topic_runtime(server_id, &topic_path);
        }
        for topic_path in stale_event_topics {
            self.stop_and_remove_event_topic_runtime(server_id, &topic_path);
        }
        for topic_path in stale_service_topics {
            self.stop_and_remove_service_topic_runtime(server_id, &topic_path);
        }
    }

    fn persist_visible_prop_state_for_current_server(&mut self, cx: &mut Context<Self>) {
        let Some(server_id) = self.current_server_id(cx) else {
            return;
        };
        let Some(topic_path) = self.current_selected_topic_path_raw(cx) else {
            return;
        };
        if !Self::is_prop_topic_path(&topic_path) {
            return;
        }

        let snapshot = self.prop_table_state.read(cx).clone();
        let topic_runtime = self.prop_topic_runtime_mut(&server_id, &topic_path);
        topic_runtime.state = snapshot;
    }

    fn persist_visible_event_state_for_current_server(&mut self, cx: &mut Context<Self>) {
        let Some(server_id) = self.current_server_id(cx) else {
            return;
        };
        let Some(topic_path) = self.current_selected_topic_path_raw(cx) else {
            return;
        };
        if !Self::is_event_topic_path(&topic_path) {
            return;
        }

        let snapshot = self.event_table_state.read(cx).clone();
        let topic_runtime = self.event_topic_runtime_mut(&server_id, &topic_path);
        topic_runtime.state = snapshot;
    }

    fn persist_visible_service_state_for_current_server(&mut self, cx: &mut Context<Self>) {
        let Some(server_id) = self.current_server_id(cx) else {
            return;
        };
        let Some(topic_path) = self.current_selected_topic_path_raw(cx) else {
            return;
        };
        if !Self::is_service_topic_path(&topic_path) {
            return;
        }

        let snapshot = self.service_table_state.read(cx).clone();
        let topic_runtime = self.service_topic_runtime_mut(&server_id, &topic_path);
        topic_runtime.state = snapshot;
    }

    fn sync_prop_filter_inputs_from_visible_state(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let filters = self.prop_table_state.read(cx).filters().clone();
        for (column, entity) in [
            (
                PropSortColumn::GlobalUuid,
                self.prop_filter_inputs.global_uuid.clone(),
            ),
            (
                PropSortColumn::Device,
                self.prop_filter_inputs.device.clone(),
            ),
            (PropSortColumn::Imr, self.prop_filter_inputs.imr.clone()),
            (PropSortColumn::Imid, self.prop_filter_inputs.imid.clone()),
            (PropSortColumn::Value, self.prop_filter_inputs.value.clone()),
            (
                PropSortColumn::Quality,
                self.prop_filter_inputs.quality.clone(),
            ),
            (PropSortColumn::Bcrid, self.prop_filter_inputs.bcrid.clone()),
            (PropSortColumn::Time, self.prop_filter_inputs.time.clone()),
            (
                PropSortColumn::MessageTime,
                self.prop_filter_inputs.message_time.clone(),
            ),
            (
                PropSortColumn::Summary,
                self.prop_filter_inputs.summary.clone(),
            ),
        ] {
            let value = filters.get(column).to_string();
            entity.update(cx, |state, cx| {
                state.set_value(value, window, cx);
            });
        }
    }

    fn sync_event_filter_inputs_from_visible_state(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let filters = self.event_table_state.read(cx).filters().clone();
        for (column, entity) in [
            (EventSortColumn::Uuid, self.event_filter_inputs.uuid.clone()),
            (
                EventSortColumn::Device,
                self.event_filter_inputs.device.clone(),
            ),
            (EventSortColumn::Imr, self.event_filter_inputs.imr.clone()),
            (
                EventSortColumn::EventType,
                self.event_filter_inputs.event_type.clone(),
            ),
            (
                EventSortColumn::Level,
                self.event_filter_inputs.level.clone(),
            ),
            (EventSortColumn::Tags, self.event_filter_inputs.tags.clone()),
            (
                EventSortColumn::Codes,
                self.event_filter_inputs.codes.clone(),
            ),
            (
                EventSortColumn::StrCodes,
                self.event_filter_inputs.str_codes.clone(),
            ),
            (
                EventSortColumn::HappenedTime,
                self.event_filter_inputs.happened_time.clone(),
            ),
            (
                EventSortColumn::RecordTime,
                self.event_filter_inputs.record_time.clone(),
            ),
            (
                EventSortColumn::BcrId,
                self.event_filter_inputs.bcr_id.clone(),
            ),
            (
                EventSortColumn::Context,
                self.event_filter_inputs.context.clone(),
            ),
            (
                EventSortColumn::Summary,
                self.event_filter_inputs.summary.clone(),
            ),
        ] {
            let value = filters.get(column).to_string();
            entity.update(cx, |state, cx| {
                state.set_value(value, window, cx);
            });
        }
    }

    fn load_visible_prop_state_for_server(
        &mut self,
        server_id: &str,
        topic_path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let snapshot = self
            .server_topic_runtimes
            .get(server_id)
            .and_then(|runtime| runtime.prop_topics.get(topic_path))
            .map(|runtime| runtime.state.clone())
            .unwrap_or_else(|| {
                let mut snapshot = PropTableState::new();
                snapshot.reset_for_topic(Some(topic_path.to_string()));
                snapshot
            });
        self.replace_visible_prop_state(snapshot, cx);
        self.sync_prop_filter_inputs_from_visible_state(window, cx);
    }

    fn load_visible_event_state_for_server(
        &mut self,
        server_id: &str,
        topic_path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let snapshot = self
            .server_topic_runtimes
            .get(server_id)
            .and_then(|runtime| runtime.event_topics.get(topic_path))
            .map(|runtime| runtime.state.clone())
            .unwrap_or_else(|| {
                let mut snapshot = EventTableState::new();
                snapshot.reset_for_topic(Some(topic_path.to_string()));
                snapshot
            });
        self.replace_visible_event_state(snapshot, cx);
        self.sync_event_filter_inputs_from_visible_state(window, cx);
    }

    fn load_visible_service_state_for_server(
        &mut self,
        server_id: &str,
        topic_path: &str,
        cx: &mut Context<Self>,
    ) {
        let (snapshot, publish_tx) = self
            .server_topic_runtimes
            .get(server_id)
            .and_then(|runtime| runtime.service_topics.get(topic_path))
            .map(|runtime| (runtime.state.clone(), runtime.publish_tx.clone()))
            .unwrap_or_else(|| {
                let mut snapshot = ServiceTableState::new();
                snapshot.reset_for_topic(Some(topic_path.to_string()));
                (snapshot, None)
            });
        self.replace_visible_service_state(snapshot, cx);
        self.service_publish_tx = publish_tx;
    }

    fn replace_visible_prop_state(&mut self, snapshot: PropTableState, cx: &mut Context<Self>) {
        self.suppress_prop_state_persist = true;
        self.prop_table_state.update(cx, |state, cx| {
            *state = snapshot;
            cx.notify();
        });
    }

    fn replace_visible_event_state(&mut self, snapshot: EventTableState, cx: &mut Context<Self>) {
        self.suppress_event_state_persist = true;
        self.event_table_state.update(cx, |state, cx| {
            *state = snapshot;
            cx.notify();
        });
    }

    fn replace_visible_service_state(
        &mut self,
        snapshot: ServiceTableState,
        cx: &mut Context<Self>,
    ) {
        self.suppress_service_state_persist = true;
        self.service_table_state.update(cx, |state, cx| {
            *state = snapshot;
            cx.notify();
        });
    }

    fn is_prop_topic_path(topic_path: &str) -> bool {
        topic_path.contains("prop_data-BZ-")
    }

    fn is_event_topic_path(topic_path: &str) -> bool {
        // Service topic paths contain `thing_service-BZ` and a comma; treat them
        // as the service topic, not an event topic, so they don't get matched here.
        if Self::is_service_topic_path(topic_path) {
            return false;
        }
        topic_path.contains("thing_event-BZ")
            || topic_path.contains("/event/")
            || topic_path.contains("/events/")
    }

    fn is_service_topic_path(topic_path: &str) -> bool {
        topic_path.contains(',')
            && topic_path.contains("thing_service-BZ-REQUEST")
            && topic_path.contains("thing_service-BZ-RESPONSE")
    }

    fn current_selection_key(&self, cx: &App) -> TopicSelectionKey {
        let server_id = self
            .app_state
            .read(cx)
            .selected_server_id()
            .map(str::to_string);
        let state = self.config_state.read(cx);
        let agent_id = state.selected_agent_id().map(str::to_string);
        let topic_path = match (state.selected_agent(), state.selected_topic_index()) {
            (Some(agent), Some(idx)) => agent.topics.get(idx as usize).map(|t| t.path.clone()),
            _ => None,
        };

        (server_id, agent_id, topic_path)
    }

    fn current_agent_tabs_signature(&self, cx: &App) -> AgentTabsSignature {
        let server_id = self
            .app_state
            .read(cx)
            .selected_server_id()
            .map(str::to_string);
        let state = self.config_state.read(cx);
        let agent_id = state.selected_agent_id().map(str::to_string);
        let topic_paths = state
            .selected_agent()
            .map(|agent| {
                agent
                    .topics
                    .iter()
                    .map(|topic| topic.path.clone())
                    .collect()
            })
            .unwrap_or_default();

        (server_id, agent_id, topic_paths)
    }

    fn topic_feedback_kind_for_panel(
        &self,
        selected_topic_path: &str,
        active_topic_path: Option<&str>,
        item_count: usize,
        is_loading: bool,
        allow_loading_feedback: bool,
    ) -> Option<TopicFeedbackKind> {
        if active_topic_path != Some(selected_topic_path) {
            Some(TopicFeedbackKind::Switching)
        } else if self.is_topic_switch_feedback_active() && item_count == 0 {
            Some(TopicFeedbackKind::Switching)
        } else if allow_loading_feedback && is_loading && item_count == 0 {
            Some(TopicFeedbackKind::Loading)
        } else {
            None
        }
    }

    fn current_topic_feedback_kind(&self, cx: &App) -> Option<TopicFeedbackKind> {
        let (_, _, selected_topic_path) = self.current_selection_key(cx);

        match selected_topic_path.as_deref() {
            Some(topic_path) if Self::is_prop_topic_path(topic_path) => {
                let state = self.prop_table_state.read(cx);
                self.topic_feedback_kind_for_panel(
                    topic_path,
                    state.topic_path(),
                    state.rows_len(),
                    matches!(state.load_state(), PropTableLoadState::Loading),
                    true,
                )
            }
            Some(topic_path) if Self::is_event_topic_path(topic_path) => {
                let state = self.event_table_state.read(cx);
                self.topic_feedback_kind_for_panel(
                    topic_path,
                    state.topic_path(),
                    state.rows_len(),
                    matches!(state.load_state(), EventTableLoadState::Loading),
                    true,
                )
            }
            Some(topic_path) if Self::is_service_topic_path(topic_path) => {
                let state = self.service_table_state.read(cx);
                self.topic_feedback_kind_for_panel(
                    topic_path,
                    state.topic_path(),
                    state.requests_len() + state.responses_len(),
                    matches!(state.load_state(), ServiceTableLoadState::Loading),
                    false,
                )
            }
            _ => None,
        }
    }

    fn has_active_feedback(&self, cx: &App) -> bool {
        self.current_topic_feedback_kind(cx).is_some()
    }

    fn ensure_topic_feedback_task(&mut self, cx: &mut Context<Self>) {
        if self._topic_feedback_task.is_some() || !self.has_active_feedback(cx) {
            return;
        }

        let task = cx.spawn(async move |handle, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(TOPIC_FEEDBACK_TICK_MS))
                    .await;

                let Ok(still_active) = handle.update(cx, |this, cx| {
                    let now = Instant::now();
                    let mut should_notify = false;

                    if this
                        .switch_feedback_until
                        .is_some_and(|deadline| deadline <= now)
                    {
                        this.switch_feedback_until = None;
                        should_notify = true;
                    }

                    let still_active = this.has_active_feedback(cx);
                    if still_active {
                        this.topic_feedback_frame =
                            (this.topic_feedback_frame + 1) % TOPIC_FEEDBACK_FRAME_COUNT;
                        should_notify = true;
                    }

                    if should_notify {
                        cx.notify();
                    }

                    still_active
                }) else {
                    break;
                };

                if !still_active {
                    break;
                }
            }

            let _ = handle.update(cx, |this, _| {
                this._topic_feedback_task = None;
            });
        });

        self._topic_feedback_task = Some(task);
    }

    fn render_topic_feedback_if_needed(
        &self,
        selected_topic_path: &str,
        active_topic_path: Option<&str>,
        item_count: usize,
        is_loading: bool,
        allow_loading_feedback: bool,
        switching_subtitle: &str,
        loading_subtitle: &str,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let topic_label = topic_display_name(selected_topic_path);

        match self.topic_feedback_kind_for_panel(
            selected_topic_path,
            active_topic_path,
            item_count,
            is_loading,
            allow_loading_feedback,
        ) {
            Some(TopicFeedbackKind::Switching) => Some(
                self.render_topic_feedback(
                    TopicFeedbackKind::Switching,
                    format!("正在切换到 {topic_label}"),
                    switching_subtitle.to_string(),
                    cx,
                )
                .into_any_element(),
            ),
            Some(TopicFeedbackKind::Loading) => Some(
                self.render_topic_feedback(
                    TopicFeedbackKind::Loading,
                    format!("{topic_label} 等待数据"),
                    loading_subtitle.to_string(),
                    cx,
                )
                .into_any_element(),
            ),
            None => None,
        }
    }

    fn is_topic_switch_feedback_active(&self) -> bool {
        self.switch_feedback_until
            .is_some_and(|deadline| deadline > Instant::now())
    }

    fn render_topic_feedback(
        &self,
        kind: TopicFeedbackKind,
        title: String,
        subtitle: String,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let frame = self.topic_feedback_frame % TOPIC_FEEDBACK_FRAME_COUNT;
        let title = format!("{title}{}", ".".repeat(frame % 3 + 1));
        let accent = match kind {
            TopicFeedbackKind::Switching => cx.theme().primary,
            TopicFeedbackKind::Loading => cx.theme().accent,
        };
        let border = accent.opacity(if cx.theme().is_dark() { 0.75 } else { 0.45 });
        let badge_bg = accent.opacity(if cx.theme().is_dark() { 0.18 } else { 0.12 });
        let panel_bg = if cx.theme().is_dark() {
            cx.theme().secondary.opacity(0.35)
        } else {
            cx.theme().secondary.opacity(0.8)
        };
        let muted_fg = cx.theme().muted_foreground;
        let active_segment = frame % 3;
        let active_skeleton = frame % 4;

        let mut progress_segments = Vec::new();
        for idx in 0..3 {
            let opacity = if idx == active_segment {
                if cx.theme().is_dark() { 0.75 } else { 0.4 }
            } else if (idx + 1) % 3 == active_segment {
                if cx.theme().is_dark() { 0.4 } else { 0.22 }
            } else if cx.theme().is_dark() {
                0.18
            } else {
                0.1
            };

            progress_segments.push(
                div()
                    .flex_1()
                    .h(px(6.0))
                    .rounded_md()
                    .bg(accent.opacity(opacity))
                    .into_any_element(),
            );
        }

        let skeleton_widths = [1.0_f32, 0.86_f32, 0.72_f32, 0.58_f32];
        let mut skeleton_rows = Vec::new();
        for (idx, width_ratio) in skeleton_widths.into_iter().enumerate() {
            let opacity = if idx == active_skeleton {
                if cx.theme().is_dark() { 0.28 } else { 0.16 }
            } else if cx.theme().is_dark() {
                0.14
            } else {
                0.08
            };

            skeleton_rows.push(
                div()
                    .w_full()
                    .child(
                        div()
                            .h(px(11.0))
                            .w(px(360.0 * width_ratio))
                            .rounded_md()
                            .bg(accent.opacity(opacity)),
                    )
                    .into_any_element(),
            );
        }

        div()
            .flex_1()
            .h_0()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .p_6()
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(560.0))
                    .gap_4()
                    .p_5()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .bg(panel_bg)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .flex_none()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(badge_bg)
                                    .child(
                                        Label::new(match kind {
                                            TopicFeedbackKind::Switching => "切换中",
                                            TopicFeedbackKind::Loading => "加载中",
                                        })
                                        .text_sm()
                                        .text_color(accent),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        Label::new(title)
                                            .text_sm()
                                            .text_color(cx.theme().foreground),
                                    )
                                    .child(Label::new(subtitle).text_sm().text_color(muted_fg)),
                            ),
                    )
                    .child(h_flex().w_full().gap_2().children(progress_segments))
                    .child(v_flex().w_full().gap_2().children(skeleton_rows)),
            )
    }

    /// Returns `(request_topic, response_topic)` extracted from the comma-pair path.
    fn split_service_topic_path(topic_path: &str) -> Option<(String, String)> {
        let mut parts = topic_path.split(',').map(|s| s.trim().to_string());
        let first = parts.next()?;
        let second = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        let (req, resp) = if first.contains("thing_service-BZ-REQUEST") {
            (first, second)
        } else if second.contains("thing_service-BZ-REQUEST") {
            (second, first)
        } else {
            return None;
        };
        if !resp.contains("thing_service-BZ-RESPONSE") {
            return None;
        }
        Some((req, resp))
    }

    fn ensure_prop_stream_for_server(
        &mut self,
        server_id: &str,
        service_url: String,
        cfgid: String,
        topic_path: String,
        token: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let same_topic_running = self
            .server_topic_runtimes
            .get(server_id)
            .and_then(|runtime| runtime.prop_topics.get(&topic_path))
            .is_some_and(|runtime| runtime.stream_stop.is_some());
        if same_topic_running {
            tracing::debug!(server_id, topic = %topic_path, "reusing running prop topic stream");
            return;
        }

        let topic_runtime = self.prop_topic_runtime_mut(server_id, &topic_path);
        topic_runtime
            .state
            .mark_loading_for_topic(Some(topic_path.clone()));
        tracing::info!(server_id, topic = %topic_path, "starting prop topic stream");

        self.start_prop_stream(
            server_id.to_string(),
            service_url,
            cfgid,
            topic_path,
            token,
            cx,
        );
    }

    fn ensure_event_stream_for_server(
        &mut self,
        server_id: &str,
        service_url: String,
        topic_path: String,
        token: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let same_topic_running = self
            .server_topic_runtimes
            .get(server_id)
            .and_then(|runtime| runtime.event_topics.get(&topic_path))
            .is_some_and(|runtime| runtime.stream_stop.is_some());
        if same_topic_running {
            tracing::debug!(server_id, topic = %topic_path, "reusing running event topic stream");
            return;
        }

        let topic_runtime = self.event_topic_runtime_mut(server_id, &topic_path);
        topic_runtime
            .state
            .mark_loading_for_topic(Some(topic_path.clone()));
        tracing::info!(server_id, topic = %topic_path, "starting event topic stream");

        self.start_event_stream(server_id.to_string(), service_url, topic_path, token, cx);
    }

    fn ensure_service_stream_for_server(
        &mut self,
        server_id: &str,
        service_url: String,
        topic_path: String,
        request_topic: String,
        response_topic: String,
        token: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let same_topic_running = self
            .server_topic_runtimes
            .get(server_id)
            .and_then(|runtime| runtime.service_topics.get(&topic_path))
            .is_some_and(|runtime| runtime.stream_stop.is_some());
        if same_topic_running {
            tracing::debug!(
                server_id,
                topic = %topic_path,
                "reusing running service topic stream"
            );
            return;
        }

        let _ = self.service_topic_runtime_mut(server_id, &topic_path);
        tracing::info!(server_id, topic = %topic_path, "starting service topic stream");

        self.start_service_stream(
            server_id.to_string(),
            service_url,
            topic_path,
            request_topic,
            response_topic,
            token,
            cx,
        );
    }

    fn sync_topic_stream_with_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selection_key = self.current_selection_key(cx);
        let agent_tabs_signature = self.current_agent_tabs_signature(cx);
        let server_id = selection_key.0.clone();
        let selected_topic_path = selection_key.2.clone();

        if self.last_agent_tabs_signature != agent_tabs_signature {
            let reset = self.reset_agent_tabs_horizontal_scroll();
            self.agent_tabs_scrollbar_visible = false;
            tracing::debug!(
                agent_id = ?agent_tabs_signature.1,
                topic_count = agent_tabs_signature.2.len(),
                did_reset = reset,
                "agent topic tabs changed"
            );
            self.last_agent_tabs_signature = agent_tabs_signature;
            self.schedule_agent_tabs_scrollbar_visibility_refresh(window, cx);
        }

        if self.last_selection_key != selection_key {
            self.last_selection_key = selection_key.clone();
            self.switch_feedback_until = selected_topic_path
                .as_ref()
                .map(|_| Instant::now() + Duration::from_millis(TOPIC_SWITCH_FEEDBACK_MS));
            self.topic_feedback_frame = 0;
        }

        self.ensure_topic_feedback_task(cx);
        self.active_table_cell = None;
        let Some(server_id) = server_id else {
            self.service_publish_tx = None;
            return;
        };

        let token = cx
            .global::<DfcGlobalStore>()
            .read(cx)
            .selected_server()
            .and_then(|s| s.pulsar_token.clone())
            .filter(|t| !t.trim().is_empty());

        if let Some(topic_path) = selected_topic_path
            .clone()
            .filter(|path| Self::is_prop_topic_path(path))
        {
            let (service_url, cfgid) = {
                let config_state = self.config_state.read(cx);
                match find_topic_origin(config_state.configs(), &topic_path) {
                    Some(v) => v,
                    None => {
                        let topic_runtime = self.prop_topic_runtime_mut(&server_id, &topic_path);
                        topic_runtime
                            .state
                            .set_error("无法定位该 Topic 对应的 service_url/cfgid");
                        self.load_visible_prop_state_for_server(
                            &server_id,
                            &topic_path,
                            window,
                            cx,
                        );
                        self.service_publish_tx = None;
                        return;
                    }
                }
            };

            self.ensure_prop_stream_for_server(
                &server_id,
                service_url,
                cfgid,
                topic_path.clone(),
                token,
                cx,
            );
            self.load_visible_prop_state_for_server(&server_id, &topic_path, window, cx);
            self.service_publish_tx = None;
            return;
        }

        if let Some(topic_path) = selected_topic_path
            .clone()
            .filter(|path| Self::is_event_topic_path(path))
        {
            let service_url = {
                let config_state = self.config_state.read(cx);
                match find_topic_service_url(config_state.configs(), &topic_path) {
                    Some(service_url) => service_url,
                    None => {
                        let topic_runtime = self.event_topic_runtime_mut(&server_id, &topic_path);
                        topic_runtime
                            .state
                            .set_error("无法定位该 Topic 对应的 service_url");
                        self.load_visible_event_state_for_server(
                            &server_id,
                            &topic_path,
                            window,
                            cx,
                        );
                        self.service_publish_tx = None;
                        return;
                    }
                }
            };

            self.ensure_event_stream_for_server(
                &server_id,
                service_url,
                topic_path.clone(),
                token,
                cx,
            );
            self.load_visible_event_state_for_server(&server_id, &topic_path, window, cx);
            self.service_publish_tx = None;
            return;
        }

        if let Some(topic_path) = selected_topic_path
            .clone()
            .filter(|path| Self::is_service_topic_path(path))
        {
            let Some((request_topic, response_topic)) = Self::split_service_topic_path(&topic_path)
            else {
                let topic_runtime = self.service_topic_runtime_mut(&server_id, &topic_path);
                topic_runtime.state.set_error("无法解析 service topic 路径");
                self.load_visible_service_state_for_server(&server_id, &topic_path, cx);
                return;
            };

            let service_url = {
                let config_state = self.config_state.read(cx);
                match find_topic_service_url(config_state.configs(), &topic_path) {
                    Some(url) => url,
                    None => {
                        let topic_runtime = self.service_topic_runtime_mut(&server_id, &topic_path);
                        topic_runtime
                            .state
                            .set_error("无法定位该 Topic 对应的 service_url");
                        self.load_visible_service_state_for_server(&server_id, &topic_path, cx);
                        return;
                    }
                }
            };

            self.ensure_service_stream_for_server(
                &server_id,
                service_url,
                topic_path.clone(),
                request_topic,
                response_topic,
                token,
                cx,
            );
            self.load_visible_service_state_for_server(&server_id, &topic_path, cx);
            return;
        }

        self.service_publish_tx = None;
    }

    fn start_prop_stream(
        &mut self,
        server_id: String,
        service_url: String,
        cfgid: String,
        topic_path: String,
        token: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let (tx, rx): (Sender<PropStreamEvent>, Receiver<PropStreamEvent>) =
            crossbeam_channel::unbounded();
        let (stop_tx, stop_rx) = watch::channel(false);
        self.prop_topic_runtime_mut(&server_id, &topic_path)
            .stream_stop = Some(stop_tx);

        let redis = cx.global::<DfcGlobalStore>().services().redis().clone();
        let uid = self.prop_row_uid.clone();
        let stream_topic_path = topic_path.clone();
        let runtime_server_id = server_id.clone();
        let runtime_topic_path = topic_path.clone();

        spawn_named_in_tokio("prop-topic-stream", async move {
            run_prop_topic_stream(
                service_url,
                stream_topic_path,
                token,
                cfgid,
                redis,
                stop_rx,
                tx,
                uid,
            )
            .await;
        });

        let task = cx.spawn(async move |handle, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;

                let mut rows: Vec<PropRow> = Vec::new();
                let mut error: Option<String> = None;
                let mut stream_disconnected = false;

                loop {
                    match rx.try_recv() {
                        Ok(PropStreamEvent::Rows(mut batch)) => rows.append(&mut batch),
                        Ok(PropStreamEvent::Error(msg)) => error = Some(msg),
                        Err(crossbeam_channel::TryRecvError::Empty) => break,
                        Err(crossbeam_channel::TryRecvError::Disconnected) => {
                            stream_disconnected = true;
                            break;
                        }
                    }
                }

                if let Some(msg) = error {
                    let _ = handle.update(cx, |this, cx| {
                        let is_visible = this.current_server_id(cx).as_deref()
                            == Some(runtime_server_id.as_str())
                            && this.current_selected_topic_path_raw(cx).as_deref()
                                == Some(topic_path.as_str());
                        let mut snapshot = None;
                        if let Some(topic_runtime) = this
                            .server_topic_runtimes
                            .get_mut(&runtime_server_id)
                            .and_then(|runtime| runtime.prop_topics.get_mut(topic_path.as_str()))
                        {
                            topic_runtime.state.set_error(msg);
                            if is_visible {
                                snapshot = Some(topic_runtime.state.clone());
                            }
                        }

                        if is_visible {
                            let Some(snapshot) = snapshot else {
                                return;
                            };
                            this.replace_visible_prop_state(snapshot, cx);
                        }
                    });
                }

                if !rows.is_empty() {
                    let _ = handle.update(cx, |this, cx| {
                        let is_visible = this.current_server_id(cx).as_deref()
                            == Some(runtime_server_id.as_str())
                            && this.current_selected_topic_path_raw(cx).as_deref()
                                == Some(topic_path.as_str());
                        let mut snapshot = None;
                        if let Some(topic_runtime) = this
                            .server_topic_runtimes
                            .get_mut(&runtime_server_id)
                            .and_then(|runtime| runtime.prop_topics.get_mut(topic_path.as_str()))
                        {
                            topic_runtime.state.push_rows_front(rows);
                            if is_visible {
                                snapshot = Some(topic_runtime.state.clone());
                            }
                        }

                        if is_visible {
                            let Some(snapshot) = snapshot else {
                                return;
                            };
                            this.replace_visible_prop_state(snapshot, cx);
                        }
                    });
                }

                if stream_disconnected {
                    let _ = handle.update(cx, |this, cx| {
                        let is_visible = this.current_server_id(cx).as_deref()
                            == Some(runtime_server_id.as_str())
                            && this.current_selected_topic_path_raw(cx).as_deref()
                                == Some(topic_path.as_str());
                        let mut snapshot = None;
                        let mut unexpected_exit = false;

                        if let Some(topic_runtime) = this
                            .server_topic_runtimes
                            .get_mut(&runtime_server_id)
                            .and_then(|runtime| runtime.prop_topics.get_mut(topic_path.as_str()))
                        {
                            unexpected_exit = topic_runtime.stream_stop.take().is_some();
                            if is_visible {
                                snapshot = Some(topic_runtime.state.clone());
                            }
                        }

                        if is_visible {
                            if let Some(snapshot) = snapshot {
                                this.replace_visible_prop_state(snapshot, cx);
                            }
                        }

                        tracing::warn!(
                            server_id = %runtime_server_id,
                            topic = %topic_path,
                            unexpected_exit,
                            "prop topic background stream disconnected"
                        );
                    });
                    break;
                }
            }
        });

        self.prop_topic_runtime_mut(&server_id, &runtime_topic_path)
            .ingest_task = Some(task);
    }

    fn start_event_stream(
        &mut self,
        server_id: String,
        service_url: String,
        topic_path: String,
        token: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let (tx, rx): (Sender<EventStreamEvent>, Receiver<EventStreamEvent>) =
            crossbeam_channel::unbounded();
        let (stop_tx, stop_rx) = watch::channel(false);
        self.event_topic_runtime_mut(&server_id, &topic_path)
            .stream_stop = Some(stop_tx);

        let uid = self.event_row_uid.clone();
        let stream_topic_path = topic_path.clone();
        let runtime_server_id = server_id.clone();
        let runtime_topic_path = topic_path.clone();

        spawn_named_in_tokio("event-topic-stream", async move {
            run_event_topic_stream(service_url, stream_topic_path, token, stop_rx, tx, uid).await;
        });

        let task = cx.spawn(async move |handle, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;

                let mut rows: Vec<EventRow> = Vec::new();
                let mut error: Option<String> = None;
                let mut stream_disconnected = false;

                loop {
                    match rx.try_recv() {
                        Ok(EventStreamEvent::Rows(mut batch)) => rows.append(&mut batch),
                        Ok(EventStreamEvent::Error(msg)) => error = Some(msg),
                        Err(crossbeam_channel::TryRecvError::Empty) => break,
                        Err(crossbeam_channel::TryRecvError::Disconnected) => {
                            stream_disconnected = true;
                            break;
                        }
                    }
                }

                if let Some(msg) = error {
                    let _ = handle.update(cx, |this, cx| {
                        let is_visible = this.current_server_id(cx).as_deref()
                            == Some(runtime_server_id.as_str())
                            && this.current_selected_topic_path_raw(cx).as_deref()
                                == Some(topic_path.as_str());
                        let mut snapshot = None;
                        if let Some(topic_runtime) = this
                            .server_topic_runtimes
                            .get_mut(&runtime_server_id)
                            .and_then(|runtime| runtime.event_topics.get_mut(topic_path.as_str()))
                        {
                            topic_runtime.state.set_error(msg);
                            if is_visible {
                                snapshot = Some(topic_runtime.state.clone());
                            }
                        }

                        if is_visible {
                            let Some(snapshot) = snapshot else {
                                return;
                            };
                            this.replace_visible_event_state(snapshot, cx);
                        }
                    });
                }

                if !rows.is_empty() {
                    let _ = handle.update(cx, |this, cx| {
                        let is_visible = this.current_server_id(cx).as_deref()
                            == Some(runtime_server_id.as_str())
                            && this.current_selected_topic_path_raw(cx).as_deref()
                                == Some(topic_path.as_str());
                        let mut snapshot = None;
                        if let Some(topic_runtime) = this
                            .server_topic_runtimes
                            .get_mut(&runtime_server_id)
                            .and_then(|runtime| runtime.event_topics.get_mut(topic_path.as_str()))
                        {
                            topic_runtime.state.push_rows_front(rows);
                            if is_visible {
                                snapshot = Some(topic_runtime.state.clone());
                            }
                        }

                        if is_visible {
                            let Some(snapshot) = snapshot else {
                                return;
                            };
                            this.replace_visible_event_state(snapshot, cx);
                        }
                    });
                }

                if stream_disconnected {
                    let _ = handle.update(cx, |this, cx| {
                        let is_visible = this.current_server_id(cx).as_deref()
                            == Some(runtime_server_id.as_str())
                            && this.current_selected_topic_path_raw(cx).as_deref()
                                == Some(topic_path.as_str());
                        let mut snapshot = None;
                        let mut unexpected_exit = false;

                        if let Some(topic_runtime) = this
                            .server_topic_runtimes
                            .get_mut(&runtime_server_id)
                            .and_then(|runtime| runtime.event_topics.get_mut(topic_path.as_str()))
                        {
                            unexpected_exit = topic_runtime.stream_stop.take().is_some();
                            if is_visible {
                                snapshot = Some(topic_runtime.state.clone());
                            }
                        }

                        if is_visible {
                            if let Some(snapshot) = snapshot {
                                this.replace_visible_event_state(snapshot, cx);
                            }
                        }

                        tracing::warn!(
                            server_id = %runtime_server_id,
                            topic = %topic_path,
                            unexpected_exit,
                            "event topic background stream disconnected"
                        );
                    });
                    break;
                }
            }
        });

        self.event_topic_runtime_mut(&server_id, &runtime_topic_path)
            .ingest_task = Some(task);
    }

    fn start_service_stream(
        &mut self,
        server_id: String,
        service_url: String,
        topic_path: String,
        request_topic: String,
        response_topic: String,
        token: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let (event_tx, event_rx): (Sender<ServiceStreamEvent>, Receiver<ServiceStreamEvent>) =
            crossbeam_channel::unbounded();
        let (publish_tx, publish_rx): (
            Sender<ServicePublishRequest>,
            Receiver<ServicePublishRequest>,
        ) = crossbeam_channel::unbounded();
        let (stop_tx, stop_rx) = watch::channel(false);
        {
            let topic_runtime = self.service_topic_runtime_mut(&server_id, &topic_path);
            topic_runtime.stream_stop = Some(stop_tx);
            topic_runtime.publish_tx = Some(publish_tx.clone());
        }
        self.service_publish_tx = Some(publish_tx);

        let uid = self.service_row_uid.clone();
        let runtime_server_id = server_id.clone();
        let runtime_topic_path = topic_path.clone();

        spawn_named_in_tokio("service-topic-stream", async move {
            run_service_topic_stream(
                service_url,
                request_topic,
                response_topic,
                token,
                stop_rx,
                publish_rx,
                event_tx,
                uid,
            )
            .await;
        });

        let task = cx.spawn(async move |handle, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;

                let mut responses = Vec::new();
                let mut error: Option<String> = None;
                let mut stream_disconnected = false;

                loop {
                    match event_rx.try_recv() {
                        Ok(ServiceStreamEvent::Response(row)) => responses.push(row),
                        Ok(ServiceStreamEvent::Error(msg)) => error = Some(msg),
                        Err(crossbeam_channel::TryRecvError::Empty) => break,
                        Err(crossbeam_channel::TryRecvError::Disconnected) => {
                            stream_disconnected = true;
                            break;
                        }
                    }
                }

                if let Some(msg) = error {
                    let _ = handle.update(cx, |this, cx| {
                        let is_visible = this.current_server_id(cx).as_deref()
                            == Some(runtime_server_id.as_str())
                            && this.current_selected_topic_path_raw(cx).as_deref()
                                == Some(topic_path.as_str());
                        let mut snapshot = None;
                        if let Some(topic_runtime) = this
                            .server_topic_runtimes
                            .get_mut(&runtime_server_id)
                            .and_then(|runtime| runtime.service_topics.get_mut(topic_path.as_str()))
                        {
                            topic_runtime.state.set_error(msg);
                            if is_visible {
                                snapshot = Some(topic_runtime.state.clone());
                            }
                        }

                        if is_visible {
                            let Some(snapshot) = snapshot else {
                                return;
                            };
                            this.replace_visible_service_state(snapshot, cx);
                        }
                    });
                }

                if !responses.is_empty() {
                    let _ = handle.update(cx, |this, cx| {
                        let is_visible = this.current_server_id(cx).as_deref()
                            == Some(runtime_server_id.as_str())
                            && this.current_selected_topic_path_raw(cx).as_deref()
                                == Some(topic_path.as_str());
                        let mut snapshot = None;
                        if let Some(topic_runtime) = this
                            .server_topic_runtimes
                            .get_mut(&runtime_server_id)
                            .and_then(|runtime| runtime.service_topics.get_mut(topic_path.as_str()))
                        {
                            for row in responses {
                                topic_runtime.state.push_response_front(row);
                            }
                            if is_visible {
                                snapshot = Some(topic_runtime.state.clone());
                            }
                        }

                        if is_visible {
                            let Some(snapshot) = snapshot else {
                                return;
                            };
                            this.replace_visible_service_state(snapshot, cx);
                        }
                    });
                }

                if stream_disconnected {
                    let _ = handle.update(cx, |this, cx| {
                        let is_visible = this.current_server_id(cx).as_deref()
                            == Some(runtime_server_id.as_str())
                            && this.current_selected_topic_path_raw(cx).as_deref()
                                == Some(topic_path.as_str());
                        let mut snapshot = None;
                        let mut unexpected_exit = false;

                        if let Some(topic_runtime) = this
                            .server_topic_runtimes
                            .get_mut(&runtime_server_id)
                            .and_then(|runtime| runtime.service_topics.get_mut(topic_path.as_str()))
                        {
                            unexpected_exit = topic_runtime.stream_stop.take().is_some();
                            topic_runtime.publish_tx = None;
                            if is_visible {
                                snapshot = Some(topic_runtime.state.clone());
                            }
                        }

                        if is_visible {
                            this.service_publish_tx = None;
                            if let Some(snapshot) = snapshot {
                                this.replace_visible_service_state(snapshot, cx);
                            }
                        }

                        tracing::warn!(
                            server_id = %runtime_server_id,
                            topic = %topic_path,
                            unexpected_exit,
                            "service topic background stream disconnected"
                        );
                    });
                    break;
                }
            }
        });

        self.service_topic_runtime_mut(&server_id, &runtime_topic_path)
            .ingest_task = Some(task);
    }

    fn on_submit_service_request(&mut self, cx: &mut Context<Self>) {
        let devices_raw = self.service_form.devices_input.read(cx).value().to_string();
        let devices: Vec<String> = devices_raw
            .split('\n')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if devices.is_empty() {
            self.service_form.error_message = Some("请至少输入一个设备号".to_string());
            cx.notify();
            return;
        }

        let timeout_raw = self.service_form.timeout_input.read(cx).value().to_string();
        let timeout_ms = match timeout_raw.trim().parse::<u32>() {
            Ok(v) if v > 0 => v,
            _ => {
                self.service_form.error_message = Some("超时毫秒数必须是正整数".to_string());
                cx.notify();
                return;
            }
        };

        let preset_imr = REQUEST_TYPES
            .get(self.service_form.selected_type_idx)
            .map(|(_, imr)| imr.to_string())
            .unwrap_or_default();
        let manual_imr = self
            .service_form
            .manual_imr_input
            .read(cx)
            .value()
            .to_string();
        let imr = if !preset_imr.is_empty() {
            preset_imr
        } else {
            manual_imr.trim().to_string()
        };
        if imr.is_empty() {
            self.service_form.error_message =
                Some("请选择预设请求类型或填写自定义服务 IMR".to_string());
            cx.notify();
            return;
        }

        let requester = self
            .service_form
            .requester_input
            .read(cx)
            .value()
            .to_string()
            .trim()
            .to_string();
        let requester = if requester.is_empty() {
            "V8Test".to_string()
        } else {
            requester
        };

        let args_raw = self.service_form.args_input.read(cx).value().to_string();
        let args_trimmed = args_raw.trim();
        let parsed_args: std::collections::HashMap<String, crate::proto::iothub::AnyValue> =
            if args_trimmed.is_empty() {
                std::collections::HashMap::new()
            } else {
                match serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                    args_trimmed,
                ) {
                    Ok(map) => map
                        .iter()
                        .map(|(k, v)| (k.clone(), service_panel::json_value_to_any_value(v)))
                        .collect(),
                    Err(e) => {
                        self.service_form.error_message =
                            Some(format!("请求参数 JSON 解析失败: {e}"));
                        cx.notify();
                        return;
                    }
                }
            };

        let args_summary = if args_trimmed.is_empty() {
            String::new()
        } else {
            args_trimmed.to_string()
        };

        self.service_form.error_message = None;

        let is_test = self.service_form.is_test;
        let now_local = chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S%.3f")
            .to_string();

        let Some(publish_tx) = self.service_publish_tx.clone() else {
            self.service_form.error_message =
                Some("服务流尚未就绪,请先选中 service Topic".to_string());
            cx.notify();
            return;
        };

        for device in devices {
            let req_uuid = uuid::Uuid::new_v4().to_string();
            let record = crate::proto::iothub::SvrReqRecord {
                req_serial_uuid: req_uuid.clone(),
                req_date_time: Some(service_panel::now_clock_time()),
                time_out: timeout_ms,
                requester: requester.clone(),
                imr: imr.clone(),
                args: parsed_args.clone(),
                is_test_request: is_test,
            };

            let row = ServiceRequestRow {
                uid: self.service_row_uid.fetch_add(1, Ordering::Relaxed),
                device: device.clone(),
                imr: imr.clone(),
                request_time: now_local.clone(),
                timeout_ms,
                is_test,
                requester: requester.clone(),
                args_json: args_summary.clone(),
                uuid: req_uuid,
                response_time: String::new(),
                response_code_hex: String::new(),
                responser: String::new(),
                summary: String::new(),
            };

            let _ = self.service_table_state.update(cx, |state, cx| {
                state.push_request_front(row);
                cx.notify();
            });

            if let Err(e) = publish_tx.send(ServicePublishRequest {
                device: device.clone(),
                record,
            }) {
                self.service_form.error_message = Some(format!("发送请求队列失败 ({device}): {e}"));
                cx.notify();
                return;
            }
        }

        cx.notify();
    }

    fn on_clear_service_records(&mut self, cx: &mut Context<Self>) {
        let _ = self.service_table_state.update(cx, |state, cx| {
            state.clear_records();
            cx.notify();
        });
        self.service_form.error_message = None;
    }

    /// Render loading state
    fn render_loading(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let loading_text = t!("config.loading", locale = &locale).to_string();

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                h_flex()
                    .gap_2()
                    .child(Icon::new(IconName::Loader).size_5())
                    .child(Label::new(loading_text).text_color(cx.theme().muted_foreground)),
            )
    }

    /// Render error state
    fn render_error(&self, message: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);
        let error_title = t!("config.error", locale = &locale).to_string();

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Icon::new(IconName::CircleX)
                                    .size_5()
                                    .text_color(cx.theme().danger),
                            )
                            .child(Label::new(error_title).text_color(cx.theme().danger)),
                    )
                    .child(
                        Label::new(message.to_string())
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
    }

    /// Render config table row
    fn render_config_row(
        &self,
        index: usize,
        config: &ConfigItem,
        bg: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let group_id = config.group_id;
        let topic_count = config.details.len();
        let locale = self.locale(cx);
        let browse_keys_label = t!("keys.browse_keys", locale = &locale).to_string();
        let config_source = config.source.clone();

        // Browse Keys button
        let keys_state = self.keys_state.clone();
        let app_state = self.app_state.clone();
        let config_source_for_click = config_source.clone();

        let browse_btn = Button::new(("browse-keys", index))
            .ghost()
            .small()
            .icon(Icon::from(CustomIconName::DatabaseZap))
            .tooltip(browse_keys_label)
            .on_click(cx.listener(move |this, _, _, cx| {
                // Stop propagation to prevent row click
                cx.stop_propagation();

                // Get server info
                let server = this.app_state.read(cx).selected_server().cloned();
                let config_source = config_source_for_click.clone();

                if let Some(server) = server {
                    let server_id = server.id.clone();
                    // Add to connected servers
                    let server_info = ConnectedServerInfo {
                        server_id: server_id.clone(),
                        server_name: server.name.clone(),
                        config_source: Some(config_source.clone()),
                    };

                    this.keys_state.update(cx, |state, cx| {
                        state.add_connected_server(server_info, cx);
                        state.set_loading(cx);
                    });
                    let list_generation = this.keys_state.read(cx).list_generation();

                    // Load keys for this config pattern
                    let keys_state = this.keys_state.clone();
                    let store = cx.global::<DfcGlobalStore>().clone();

                    cx.spawn(async move |_, cx| {
                        let redis = store.services().redis();

                        // Use the config source as pattern or scan all keys
                        // For now, scan all keys with pattern *
                        match redis.scan_keys("*", 0, 100).await {
                            Ok((keys, cursor)) => {
                                tracing::info!("Loaded {} keys, cursor: {}", keys.len(), cursor);
                                let _ = keys_state.update(cx, |state, cx| {
                                    if state.active_server_id() == Some(server_id.as_str())
                                        && state.list_generation() == list_generation
                                    {
                                        state.set_keys(keys, cursor, cx);
                                    } else {
                                        tracing::info!(
                                            server_id,
                                            list_generation,
                                            cursor,
                                            "Ignoring stale key scan response after active server changed"
                                        );
                                    }
                                });
                            }
                            Err(e) => {
                                tracing::error!("Failed to scan keys: {}", e);
                                let _ = keys_state.update(cx, |state, cx| {
                                    if state.active_server_id() == Some(server_id.as_str())
                                        && state.list_generation() == list_generation
                                    {
                                        state.set_error(e.to_string(), cx);
                                    } else {
                                        tracing::info!(
                                            server_id,
                                            list_generation,
                                            error = %e,
                                            "Ignoring stale key scan error after active server changed"
                                        );
                                    }
                                });
                            }
                        }
                    })
                    .detach();
                }
            }));

        div()
            .id(("config-row", index))
            .w_full()
            .px_4()
            .py_2()
            .bg(bg)
            .cursor_pointer()
            .border_b_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .child(
                h_flex()
                    .w_full()
                    .gap_4()
                    .items_center()
                    // Group ID column
                    .child(
                        div()
                            .w(px(60.0))
                            .child(Label::new(format!("{}", config.group_id)).text_sm()),
                    )
                    // Service URL column
                    .child(
                        div().flex_1().overflow_hidden().child(
                            Label::new(config.service_url.clone())
                                .text_sm()
                                .text_ellipsis(),
                        ),
                    )
                    // Source column
                    .child(
                        div().w(px(250.0)).overflow_hidden().child(
                            Label::new(config.source.clone())
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .text_ellipsis(),
                        ),
                    )
                    // Topic count badge
                    .child(
                        div().w(px(80.0)).child(
                            Label::new(format!("{} topics", topic_count))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        ),
                    )
                    // Browse keys button
                    .child(browse_btn)
                    // Arrow icon
                    .child(
                        Icon::new(IconName::ChevronRight)
                            .size_4()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.config_state.update(cx, |state, cx| {
                    state.select_config(Some(group_id), cx);
                });
            }))
    }

    /// Render the config table header
    fn render_table_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);

        h_flex()
            .w_full()
            .px_4()
            .py_2()
            .bg(cx.theme().secondary)
            .border_b_1()
            .border_color(cx.theme().border)
            .gap_4()
            .child(
                div().w(px(60.0)).child(
                    Label::new(t!("config.group_id", locale = &locale).to_string())
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                ),
            )
            .child(
                div().flex_1().child(
                    Label::new(t!("config.service_url", locale = &locale).to_string())
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                ),
            )
            .child(
                div().w(px(250.0)).child(
                    Label::new(t!("config.source", locale = &locale).to_string())
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                ),
            )
            .child(
                div().w(px(80.0)).child(
                    Label::new(t!("config.topics", locale = &locale).to_string())
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                ),
            )
            .child(div().w(px(16.0))) // Spacer for arrow
    }

    /// Render the configuration list table
    fn render_config_table(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let config_state = self.config_state.read(cx);
        let configs: Vec<_> = config_state.configs().to_vec();

        // Alternate row colors
        let bg_even = if cx.theme().is_dark() {
            cx.theme().background.lighten(0.5)
        } else {
            cx.theme().background.darken(0.01)
        };
        let bg_odd = cx.theme().background;

        let mut rows = Vec::new();
        for (index, config) in configs.iter().enumerate() {
            let bg = if index % 2 == 0 { bg_even } else { bg_odd };
            rows.push(self.render_config_row(index, config, bg, cx));
        }

        v_flex()
            .size_full()
            .child(self.render_table_header(cx))
            .child(
                div()
                    .id("config-table-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .children(rows),
            )
    }

    /// Render topic tab item
    fn render_topic_tab(
        &self,
        index: i32,
        path: &str,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let tab_bg = if is_selected {
            cx.theme().accent
        } else {
            cx.theme().secondary
        };

        let text_color = if is_selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().muted_foreground
        };

        // Extract short name from path
        let short_name = path
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .chars()
            .take(20)
            .collect::<String>();

        div()
            .id(("topic-tab", index as usize))
            .px_3()
            .py_1()
            .bg(tab_bg)
            .cursor_pointer()
            .rounded_t_md()
            .border_1()
            .border_color(cx.theme().border)
            .when(!is_selected, |this| this.border_b_0())
            .child(Label::new(short_name).text_sm().text_color(text_color))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.config_state.update(cx, |state, cx| {
                    state.select_topic(Some(index), cx);
                });
            }))
    }

    /// Render topic content
    fn render_topic_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let config_state = self.config_state.read(cx);

        let content = if let Some(topic) = config_state.selected_topic() {
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Label::new("Path:")
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(Label::new(topic.path.clone()).text_sm()),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Label::new("Visibility:")
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            Label::new(if topic.visibility {
                                "Visible"
                            } else {
                                "Hidden"
                            })
                            .text_sm(),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Label::new("Index:")
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(Label::new(format!("{}", topic.index)).text_sm()),
                )
        } else {
            v_flex().child(Label::new("No topic selected").text_color(cx.theme().muted_foreground))
        };

        div()
            .p_4()
            .flex_1()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_b_md()
            .rounded_tr_md()
            .child(content)
    }

    /// Render a single agent item in the left list
    fn render_agent_item(
        &self,
        index: usize,
        agent_id: String,
        topic_count: usize,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let bg = if is_selected {
            cx.theme().list_active
        } else {
            cx.theme().background
        };

        let text_color = cx.theme().foreground;

        let hover_color = if is_selected {
            bg
        } else if cx.theme().is_dark() {
            cx.theme().secondary.lighten(0.04)
        } else {
            cx.theme().secondary.darken(0.02)
        };
        let border_color = if is_selected {
            cx.theme().list_active_border
        } else {
            cx.theme().border
        };
        let agent_id_for_click = agent_id.clone();
        let is_selected_for_click = is_selected;

        let count_color = if is_selected {
            cx.theme().foreground.opacity(0.72)
        } else {
            cx.theme().muted_foreground
        };

        div()
            .id(("agent-item", index))
            .w_full()
            .cursor_pointer()
            .when(is_selected, |this| this.px_1().py_px())
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .bg(bg)
                    .when(is_selected, |this| this.rounded_sm().border_1())
                    .when(!is_selected, |this| this.border_b_1())
                    .border_color(border_color)
                    .hover(|this| this.bg(hover_color))
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_2()
                            .child(
                                Label::new(agent_id.clone())
                                    .text_sm()
                                    .text_color(text_color)
                                    .text_ellipsis()
                                    .flex_1(),
                            )
                            .child(
                                Label::new(format!("{} topics", topic_count))
                                    .text_xs()
                                    .text_color(count_color),
                            ),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if is_selected_for_click {
                    return;
                }

                this.reset_agent_tabs_horizontal_scroll();
                this.config_state.update(cx, |state, cx| {
                    state.select_agent(Some(agent_id_for_click.clone()), cx);
                });
            }))
    }

    /// Render the left panel with TopicAgentId list
    fn render_agent_list(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.agent_search_state.read(cx).value().trim().to_string();

        // Collect agent data first to avoid borrow conflicts
        let agents_data: Vec<(usize, String, usize, bool)> = {
            let config_state = self.config_state.read(cx);
            let topic_agents = config_state.topic_agents();
            let selected_agent_id = config_state.selected_agent_id();

            topic_agents
                .iter()
                .filter(|agent| {
                    query.is_empty() || self.agent_id_matches_query(&agent.agent_id, &query)
                })
                .enumerate()
                .map(|(idx, agent)| {
                    let is_selected = selected_agent_id == Some(&agent.agent_id);
                    (idx, agent.agent_id.clone(), agent.topics.len(), is_selected)
                })
                .collect()
        };

        let mut agent_items: Vec<gpui::Stateful<gpui::Div>> = Vec::new();
        for (index, agent_id, topic_count, is_selected) in agents_data {
            agent_items.push(self.render_agent_item(index, agent_id, topic_count, is_selected, cx));
        }

        let border_color = cx.theme().border;
        let secondary_bg = cx.theme().secondary;

        let locale = self.locale(cx);
        let query_mode_all_label = t!("config.query_mode_all", locale = &locale).to_string();
        let query_mode_prefix_label = t!("config.query_mode_prefix", locale = &locale).to_string();
        let query_mode_exact_label = t!("config.query_mode_exact", locale = &locale).to_string();

        // Select icon based on query mode
        let icon = match self.agent_query_mode {
            AgentQueryMode::All => Icon::new(IconName::Asterisk), // * for all keys
            AgentQueryMode::Prefix => Icon::from(CustomIconName::ChevronUp), // ^ for prefix
            AgentQueryMode::Exact => Icon::from(CustomIconName::Equal), // = for exact match
        };
        let query_mode = self.agent_query_mode;
        let query_mode_dropdown = DropdownButton::new("agent-query-mode-dropdown")
            .button(
                Button::new("agent-query-mode-btn")
                    .ghost()
                    .px_2()
                    .icon(icon),
            )
            .dropdown_menu_with_anchor(Corner::TopLeft, move |menu, _, _| {
                let query_mode_all_label = query_mode_all_label.clone();
                let query_mode_prefix_label = query_mode_prefix_label.clone();
                let query_mode_exact_label = query_mode_exact_label.clone();

                menu.menu_element_with_check(
                    query_mode == AgentQueryMode::All,
                    Box::new(AgentQueryMode::All),
                    move |_, _cx| Label::new(query_mode_all_label.clone()).ml_2().text_xs(),
                )
                .menu_element_with_check(
                    query_mode == AgentQueryMode::Prefix,
                    Box::new(AgentQueryMode::Prefix),
                    move |_, _cx| Label::new(query_mode_prefix_label.clone()).ml_2().text_xs(),
                )
                .menu_element_with_check(
                    query_mode == AgentQueryMode::Exact,
                    Box::new(AgentQueryMode::Exact),
                    move |_, _cx| Label::new(query_mode_exact_label.clone()).ml_2().text_xs(),
                )
            });

        let search_btn = Button::new("agent-search-btn")
            .ghost()
            .icon(IconName::Search)
            .on_click(cx.listener(|this, _, _, cx| {
                this.clear_selected_agent_if_filtered_out(cx);
                cx.notify();
            }));

        let keyword_input = Input::new(&self.agent_search_state)
            .w_full()
            .flex_1()
            .px_0()
            .prefix(query_mode_dropdown)
            .suffix(search_btn)
            .cleanable(true);

        v_flex()
            .flex_none()
            .w(px(AGENT_LIST_WIDTH))
            .h_full()
            .bg(secondary_bg)
            .track_focus(&self.agent_list_focus_handle)
            // Search input
            .child(
                h_flex()
                    .w_full()
                    .h(px(PANEL_TOPBAR_HEIGHT))
                    .p_2()
                    .items_center()
                    .border_b_1()
                    .border_color(border_color)
                    .child(keyword_input),
            )
            // Agent list
            .child(
                div()
                    .id("agent-list-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .children(agent_items),
            )
    }

    /// Render the right panel with topic tabs for selected agent
    fn render_agent_topics(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale(cx);

        // Collect data first to avoid borrow conflicts
        let (agent_info, topic_paths, selected_topic_index): (
            Option<(String, usize)>,
            Vec<String>,
            Option<i32>,
        ) = {
            let config_state = self.config_state.read(cx);
            let selected_topic_idx = config_state.selected_topic_index();
            let selected_agent = config_state.selected_agent();

            match selected_agent {
                Some(agent) => {
                    let info = (agent.agent_id.clone(), agent.topics.len());
                    let topics: Vec<_> = agent.topics.iter().map(|t| t.path.clone()).collect();
                    (Some(info), topics, selected_topic_idx)
                }
                None => (None, Vec::new(), None),
            }
        };

        // No agent selected - show placeholder
        if agent_info.is_none() {
            let no_agent_text = t!("config.no_agent_selected", locale = &locale).to_string();
            let muted_fg = cx.theme().muted_foreground;
            return div()
                .flex_1()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(Label::new(no_agent_text).text_color(muted_fg))
                .into_any_element();
        }

        let (_agent_id, _topic_count) = agent_info.expect("checked above");

        let muted_fg = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let secondary_bg = cx.theme().secondary;
        let no_topic_selected = t!("config.no_topic_selected", locale = &locale).to_string();
        let show_agent_tabs_scrollbar = self.agent_tabs_scrollbar_visible;

        let selected_topic_index =
            selected_topic_index.filter(|idx| (*idx as usize) < topic_paths.len());
        let selected_topic_path = selected_topic_index
            .and_then(|idx| topic_paths.get(idx as usize))
            .cloned();
        let is_prop_topic = selected_topic_path
            .as_deref()
            .map(Self::is_prop_topic_path)
            .unwrap_or(false);
        let is_event_topic = selected_topic_path
            .as_deref()
            .map(Self::is_event_topic_path)
            .unwrap_or(false);
        let is_service_topic = selected_topic_path
            .as_deref()
            .map(Self::is_service_topic_path)
            .unwrap_or(false);

        // Build tab buttons
        let mut tabs = Vec::new();
        for (pos, path) in topic_paths.iter().enumerate() {
            let label = topic_display_name(path);
            let is_selected = selected_topic_index == Some(pos as i32);
            tabs.push(self.render_agent_topic_tab(pos as i32, label, path, is_selected, cx));
        }

        v_flex()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .h_full()
            .overflow_hidden()
            .track_focus(&self.topic_content_focus_handle)
            // Top bar spacer (align with left search bar)
            .child(
                h_flex()
                    .flex_none()
                    .w_full()
                    .h(px(PANEL_TOPBAR_HEIGHT))
                    .items_center()
                    .px_4()
                    .gap_2()
                    .bg(secondary_bg)
                    .border_b_1()
                    .border_color(border)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(0.0))
                            .h_full()
                            .child(
                                h_flex()
                                    .id("agent-tabs-scroll")
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .gap_2()
                                    .flex_nowrap()
                                    .items_center()
                                    .overflow_x_scroll()
                                    .track_scroll(&self.agent_tabs_horizontal_scroll_handle)
                                    .children(tabs),
                            )
                            .when(show_agent_tabs_scrollbar, |this| {
                                this.child(self.render_horizontal_scrollbar_row(
                                    &self.agent_tabs_horizontal_scroll_handle,
                                    cx,
                                ))
                            }),
                    ),
            )
            // Content area
            .child(
                v_flex()
                    .flex_1()
                    .h_0()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(match selected_topic_path.as_deref() {
                        Some(topic_path) if is_prop_topic => self
                            .render_prop_table(topic_path, window, cx)
                            .into_any_element(),
                        Some(topic_path) if is_event_topic => self
                            .render_event_table(topic_path, window, cx)
                            .into_any_element(),
                        Some(topic_path) if is_service_topic => self
                            .render_service_panel(topic_path, window, cx)
                            .into_any_element(),
                        Some(topic_path) => self
                            .render_unsupported_topic(topic_path, cx)
                            .into_any_element(),
                        None => div().flex_1().into_any_element(),
                    }),
            )
            // Bottom status bar
            .child(
                h_flex()
                    .flex_none()
                    .w_full()
                    .h(px(48.0))
                    .items_center()
                    .px_4()
                    .border_t_1()
                    .border_color(border)
                    .bg(secondary_bg)
                    .child(if is_prop_topic {
                        self.render_prop_pagination(cx).into_any_element()
                    } else if is_event_topic {
                        self.render_event_pagination(cx).into_any_element()
                    } else {
                        Label::new(no_topic_selected)
                            .text_color(muted_fg)
                            .into_any_element()
                    }),
            )
            .into_any_element()
    }

    fn render_unsupported_topic(
        &self,
        topic_path: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted_fg = cx.theme().muted_foreground;
        let border = cx.theme().border;

        v_flex()
            .flex_1()
            .p_4()
            .gap_2()
            .child(
                Label::new("当前仅实现 prop_data 和 thing_event Topic 的内容展示")
                    .text_color(muted_fg),
            )
            .child(
                div()
                    .border_1()
                    .border_color(border)
                    .rounded_md()
                    .p_3()
                    .child(
                        Label::new(topic_path.to_string())
                            .text_sm()
                            .text_color(muted_fg)
                            .text_ellipsis(),
                    ),
            )
    }

    fn render_prop_table(
        &self,
        selected_topic_path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let border = cx.theme().border;
        let header_bg = cx.theme().secondary;

        let (topic_path, load_state, total_rows) = {
            let state = self.prop_table_state.read(cx);
            (
                state.topic_path().map(|s| s.to_string()),
                state.load_state().clone(),
                state.rows_len(),
            )
        };
        if let Some(feedback) = self.render_topic_feedback_if_needed(
            selected_topic_path,
            topic_path.as_deref(),
            total_rows,
            matches!(&load_state, PropTableLoadState::Loading),
            true,
            "正在停止旧订阅并准备新 Topic",
            "正在建立订阅并等待首批消息",
            cx,
        ) {
            return feedback;
        }

        match &load_state {
            PropTableLoadState::Error(msg) => {
                return div()
                    .flex_1()
                    .h_0()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .p_4()
                    .child(Label::new(format!("加载失败: {msg}")).text_color(cx.theme().danger))
                    .into_any_element();
            }
            _ => {}
        }

        let page_rows = self.prop_table_state.read(cx).page_rows_owned();
        let table_width = self.prop_column_widths.total();

        // Build rows
        let mut rows = Vec::new();
        for (idx, row) in page_rows.iter().enumerate() {
            let bg = if idx % 2 == 0 {
                if cx.theme().is_dark() {
                    cx.theme().background.lighten(0.3)
                } else {
                    cx.theme().background.darken(0.01)
                }
            } else {
                cx.theme().background
            };

            rows.push(
                h_flex()
                    .id(("prop-row", row.uid as usize))
                    .w_full()
                    .bg(bg)
                    .border_b_1()
                    .border_color(border)
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "prop-global-uuid"),
                        self.prop_column_width(PropSortColumn::GlobalUuid),
                        &row.global_uuid,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "prop-device"),
                        self.prop_column_width(PropSortColumn::Device),
                        &row.device,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "prop-imr"),
                        self.prop_column_width(PropSortColumn::Imr),
                        &row.imr,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "prop-imid"),
                        self.prop_column_width(PropSortColumn::Imid),
                        &row.imid.to_string(),
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "prop-value"),
                        self.prop_column_width(PropSortColumn::Value),
                        &row.value,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "prop-quality"),
                        self.prop_column_width(PropSortColumn::Quality),
                        &row.quality.to_string(),
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "prop-bcrid"),
                        self.prop_column_width(PropSortColumn::Bcrid),
                        &row.bcrid,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "prop-time"),
                        self.prop_column_width(PropSortColumn::Time),
                        &row.time,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "prop-message-time"),
                        self.prop_column_width(PropSortColumn::MessageTime),
                        &row.message_time,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "prop-summary"),
                        self.prop_column_width(PropSortColumn::Summary),
                        &row.summary,
                        window,
                        cx,
                    )),
            );
        }

        // Horizontal scroll wrapper
        v_flex()
            .flex_1()
            .h_0()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .p_3()
            .child(
                v_flex()
                    .w_full()
                    .flex_1()
                    .h_0()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .overflow_hidden()
                    .child(
                        div()
                            .id("prop-table-header-x-scroll")
                            .w_full()
                            .flex_none()
                            .min_w(px(0.0))
                            .overflow_x_scroll()
                            .track_scroll(&self.prop_table_horizontal_scroll_handle)
                            .child(
                                h_flex()
                                    .min_w(px(table_width))
                                    .w(px(table_width))
                                    .bg(header_bg)
                                    .border_b_1()
                                    .border_color(border)
                                    .child(self.render_filterable_prop_header_cell(
                                        self.prop_column_width(PropSortColumn::GlobalUuid),
                                        "全局UUID",
                                        PropSortColumn::GlobalUuid,
                                        cx,
                                    ))
                                    .child(self.render_filterable_prop_header_cell(
                                        self.prop_column_width(PropSortColumn::Device),
                                        "设备号",
                                        PropSortColumn::Device,
                                        cx,
                                    ))
                                    .child(self.render_filterable_prop_header_cell(
                                        self.prop_column_width(PropSortColumn::Imr),
                                        "IMR",
                                        PropSortColumn::Imr,
                                        cx,
                                    ))
                                    .child(self.render_filterable_prop_header_cell(
                                        self.prop_column_width(PropSortColumn::Imid),
                                        "IMID",
                                        PropSortColumn::Imid,
                                        cx,
                                    ))
                                    .child(self.render_filterable_prop_header_cell(
                                        self.prop_column_width(PropSortColumn::Value),
                                        "值",
                                        PropSortColumn::Value,
                                        cx,
                                    ))
                                    .child(self.render_filterable_prop_header_cell(
                                        self.prop_column_width(PropSortColumn::Quality),
                                        "数据质量",
                                        PropSortColumn::Quality,
                                        cx,
                                    ))
                                    .child(self.render_filterable_prop_header_cell(
                                        self.prop_column_width(PropSortColumn::Bcrid),
                                        "BCRID",
                                        PropSortColumn::Bcrid,
                                        cx,
                                    ))
                                    .child(self.render_filterable_prop_header_cell(
                                        self.prop_column_width(PropSortColumn::Time),
                                        "数据时间",
                                        PropSortColumn::Time,
                                        cx,
                                    ))
                                    .child(self.render_filterable_prop_header_cell(
                                        self.prop_column_width(PropSortColumn::MessageTime),
                                        "报文时间",
                                        PropSortColumn::MessageTime,
                                        cx,
                                    ))
                                    .child(self.render_filterable_prop_header_cell(
                                        self.prop_column_width(PropSortColumn::Summary),
                                        "报文摘要",
                                        PropSortColumn::Summary,
                                        cx,
                                    )),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("prop-table-body")
                            .flex_1()
                            .h_0()
                            .min_w(px(0.0))
                            .min_h(px(0.0))
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex_1()
                                    .h_0()
                                    .min_w(px(0.0))
                                    .min_h(px(0.0))
                                    .relative()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .id("prop-table-body-x-scroll")
                                            .size_full()
                                            .min_w(px(0.0))
                                            .overflow_x_scroll()
                                            .track_scroll(&self.prop_table_horizontal_scroll_handle)
                                            .child(
                                                div()
                                                    .id("prop-table-y-scroll")
                                                    .min_w(px(table_width))
                                                    .w(px(table_width))
                                                    .h_full()
                                                    .min_h(px(0.0))
                                                    .track_scroll(&self.prop_table_scroll_handle)
                                                    .on_scroll_wheel(cx.listener(
                                                        Self::handle_prop_table_vertical_scroll,
                                                    ))
                                                    .children(rows),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .right_0()
                                            .bottom_0()
                                            .w(px(16.0))
                                            .on_scroll_wheel(
                                                cx.listener(
                                                    Self::handle_prop_table_vertical_scroll,
                                                ),
                                            )
                                            .child(
                                                Scrollbar::vertical(&self.prop_table_scroll_handle)
                                                    .scrollbar_show(ScrollbarShow::Always),
                                            ),
                                    ),
                            )
                            .child(self.render_horizontal_scrollbar_row(
                                &self.prop_table_horizontal_scroll_handle,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_event_table(
        &self,
        selected_topic_path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let border = cx.theme().border;
        let header_bg = cx.theme().secondary;
        let table_width = self.event_column_widths.total();

        let (topic_path, load_state, total_rows) = {
            let state = self.event_table_state.read(cx);
            (
                state.topic_path().map(|s| s.to_string()),
                state.load_state().clone(),
                state.rows_len(),
            )
        };
        if let Some(feedback) = self.render_topic_feedback_if_needed(
            selected_topic_path,
            topic_path.as_deref(),
            total_rows,
            matches!(&load_state, EventTableLoadState::Loading),
            true,
            "正在停止旧订阅并准备新 Topic",
            "正在建立订阅并等待首批消息",
            cx,
        ) {
            return feedback;
        }

        match &load_state {
            EventTableLoadState::Error(msg) => {
                return div()
                    .flex_1()
                    .h_0()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .p_4()
                    .child(Label::new(format!("加载失败: {msg}")).text_color(cx.theme().danger))
                    .into_any_element();
            }
            _ => {}
        }

        let page_rows = self.event_table_state.read(cx).page_rows_owned();

        let mut rows = Vec::new();
        for (idx, row) in page_rows.iter().enumerate() {
            let bg = if idx % 2 == 0 {
                if cx.theme().is_dark() {
                    cx.theme().background.lighten(0.3)
                } else {
                    cx.theme().background.darken(0.01)
                }
            } else {
                cx.theme().background
            };

            rows.push(
                h_flex()
                    .id(("event-row", row.uid as usize))
                    .w_full()
                    .bg(bg)
                    .border_b_1()
                    .border_color(border)
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-uuid"),
                        self.event_column_width(EventSortColumn::Uuid),
                        &row.uuid,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-device"),
                        self.event_column_width(EventSortColumn::Device),
                        &row.device,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-imr"),
                        self.event_column_width(EventSortColumn::Imr),
                        &row.imr,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-type"),
                        self.event_column_width(EventSortColumn::EventType),
                        &row.event_type,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-level"),
                        self.event_column_width(EventSortColumn::Level),
                        &row.level,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-tags"),
                        self.event_column_width(EventSortColumn::Tags),
                        &row.tags,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-codes"),
                        self.event_column_width(EventSortColumn::Codes),
                        &row.codes,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-str-codes"),
                        self.event_column_width(EventSortColumn::StrCodes),
                        &row.str_codes,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-happened-time"),
                        self.event_column_width(EventSortColumn::HappenedTime),
                        &row.happened_time,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-record-time"),
                        self.event_column_width(EventSortColumn::RecordTime),
                        &row.record_time,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-bcr-id"),
                        self.event_column_width(EventSortColumn::BcrId),
                        &row.bcr_id,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-context"),
                        self.event_column_width(EventSortColumn::Context),
                        &row.context,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "event-summary"),
                        self.event_column_width(EventSortColumn::Summary),
                        &row.summary,
                        window,
                        cx,
                    )),
            );
        }

        v_flex()
            .flex_1()
            .h_0()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .p_3()
            .child(
                v_flex()
                    .w_full()
                    .flex_1()
                    .h_0()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .overflow_hidden()
                    .child(
                        div()
                            .id("event-table-header-x-scroll")
                            .w_full()
                            .flex_none()
                            .min_w(px(0.0))
                            .overflow_x_scroll()
                            .track_scroll(&self.event_table_horizontal_scroll_handle)
                            .child(
                                h_flex()
                                    .min_w(px(table_width))
                                    .w(px(table_width))
                                    .bg(header_bg)
                                    .border_b_1()
                                    .border_color(border)
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::Uuid),
                                        "UUID",
                                        EventSortColumn::Uuid,
                                        cx,
                                    ))
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::Device),
                                        "设备",
                                        EventSortColumn::Device,
                                        cx,
                                    ))
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::Imr),
                                        "IMR",
                                        EventSortColumn::Imr,
                                        cx,
                                    ))
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::EventType),
                                        "事件类型",
                                        EventSortColumn::EventType,
                                        cx,
                                    ))
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::Level),
                                        "事件级别",
                                        EventSortColumn::Level,
                                        cx,
                                    ))
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::Tags),
                                        "标签",
                                        EventSortColumn::Tags,
                                        cx,
                                    ))
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::Codes),
                                        "事件码(数字)",
                                        EventSortColumn::Codes,
                                        cx,
                                    ))
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::StrCodes),
                                        "事件码(KKS)",
                                        EventSortColumn::StrCodes,
                                        cx,
                                    ))
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::HappenedTime),
                                        "发生时间",
                                        EventSortColumn::HappenedTime,
                                        cx,
                                    ))
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::RecordTime),
                                        "记录时间",
                                        EventSortColumn::RecordTime,
                                        cx,
                                    ))
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::BcrId),
                                        "BCRID",
                                        EventSortColumn::BcrId,
                                        cx,
                                    ))
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::Context),
                                        "事件上下文",
                                        EventSortColumn::Context,
                                        cx,
                                    ))
                                    .child(self.render_filterable_event_header_cell(
                                        self.event_column_width(EventSortColumn::Summary),
                                        "报文摘要",
                                        EventSortColumn::Summary,
                                        cx,
                                    )),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("event-table-body")
                            .flex_1()
                            .h_0()
                            .min_w(px(0.0))
                            .min_h(px(0.0))
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex_1()
                                    .h_0()
                                    .min_w(px(0.0))
                                    .min_h(px(0.0))
                                    .relative()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .id("event-table-body-x-scroll")
                                            .size_full()
                                            .min_w(px(0.0))
                                            .overflow_x_scroll()
                                            .track_scroll(
                                                &self.event_table_horizontal_scroll_handle,
                                            )
                                            .child(
                                                div()
                                                    .id("event-table-y-scroll")
                                                    .min_w(px(table_width))
                                                    .w(px(table_width))
                                                    .h_full()
                                                    .min_h(px(0.0))
                                                    .track_scroll(&self.event_table_scroll_handle)
                                                    .on_scroll_wheel(cx.listener(
                                                        Self::handle_event_table_vertical_scroll,
                                                    ))
                                                    .children(rows),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .right_0()
                                            .bottom_0()
                                            .w(px(16.0))
                                            .on_scroll_wheel(
                                                cx.listener(
                                                    Self::handle_event_table_vertical_scroll,
                                                ),
                                            )
                                            .child(
                                                Scrollbar::vertical(
                                                    &self.event_table_scroll_handle,
                                                )
                                                .scrollbar_show(ScrollbarShow::Always),
                                            ),
                                    ),
                            )
                            .child(self.render_horizontal_scrollbar_row(
                                &self.event_table_horizontal_scroll_handle,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_service_panel(
        &self,
        selected_topic_path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let border = cx.theme().border;
        let muted_fg = cx.theme().muted_foreground;
        let secondary_bg = cx.theme().secondary;

        let (topic_path, total_requests, total_responses, load_state) = {
            let state = self.service_table_state.read(cx);
            (
                state.topic_path().map(|s| s.to_string()),
                state.requests_len(),
                state.responses_len(),
                state.load_state().clone(),
            )
        };
        if let Some(feedback) = self.render_topic_feedback_if_needed(
            selected_topic_path,
            topic_path.as_deref(),
            total_requests + total_responses,
            matches!(&load_state, ServiceTableLoadState::Loading),
            false,
            "正在同步 service 请求/响应通道",
            "正在建立请求/响应监听",
            cx,
        ) {
            return feedback;
        }

        let (request_rows, response_rows) = {
            let state = self.service_table_state.read(cx);
            (state.req_page_rows_owned(), state.resp_page_rows_owned())
        };

        let error_banner = if let ServiceTableLoadState::Error(msg) = &load_state {
            Some(
                div()
                    .px_3()
                    .py_2()
                    .border_1()
                    .border_color(cx.theme().danger)
                    .rounded_md()
                    .child(
                        Label::new(format!("加载失败: {msg}"))
                            .text_sm()
                            .text_color(cx.theme().danger),
                    ),
            )
        } else {
            None
        };

        let form_error = self.service_form.error_message.as_ref().map(|msg| {
            div()
                .px_3()
                .py_2()
                .border_1()
                .border_color(cx.theme().danger)
                .rounded_md()
                .child(
                    Label::new(msg.clone())
                        .text_sm()
                        .text_color(cx.theme().danger),
                )
        });

        let mut radios = Vec::new();
        for (idx, (label, _imr)) in REQUEST_TYPES.iter().enumerate() {
            let selected = self.service_form.selected_type_idx == idx;
            radios.push(
                Radio::new(("svc-radio", idx))
                    .label((*label).to_string())
                    .checked(selected)
                    .on_click(cx.listener(move |this, _checked: &bool, _, cx| {
                        this.service_form.selected_type_idx = idx;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        let form = v_flex()
            .gap_3()
            .p_3()
            .border_1()
            .border_color(border)
            .rounded_md()
            .bg(secondary_bg)
            .child(self.render_service_form_row(
                "设备号",
                Input::new(&self.service_form.devices_input).into_any_element(),
                cx,
            ))
            .child(self.render_service_form_row(
                "超时毫秒数",
                Input::new(&self.service_form.timeout_input).into_any_element(),
                cx,
            ))
            .child(
                self.render_service_form_row(
                    "是否为测试",
                    Checkbox::new("svc-is-test")
                        .checked(self.service_form.is_test)
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.service_form.is_test = *checked;
                            cx.notify();
                        }))
                        .into_any_element(),
                    cx,
                ),
            )
            .child(
                self.render_service_form_row(
                    "请求类型",
                    h_flex()
                        .flex_wrap()
                        .gap_x_3()
                        .gap_y_2()
                        .children(radios)
                        .into_any_element(),
                    cx,
                ),
            )
            .child(self.render_service_form_row(
                "自定义服务IMR",
                Input::new(&self.service_form.manual_imr_input).into_any_element(),
                cx,
            ))
            .child(self.render_service_form_row(
                "请求者",
                Input::new(&self.service_form.requester_input).into_any_element(),
                cx,
            ))
            .child(self.render_service_form_row(
                "请求参数(JSON)",
                Input::new(&self.service_form.args_input).into_any_element(),
                cx,
            ))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("svc-submit")
                            .primary()
                            .label("发起请求")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.on_submit_service_request(cx);
                            })),
                    )
                    .child(
                        Button::new("svc-clear")
                            .danger()
                            .label("清除记录")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.on_clear_service_records(cx);
                            })),
                    ),
            );

        let request_table_width = self.service_request_column_widths.total();
        let response_table_width = self.service_response_column_widths.total();

        let mut request_body_rows = Vec::new();
        for (idx, row) in request_rows.iter().enumerate() {
            let bg = if idx % 2 == 0 {
                if cx.theme().is_dark() {
                    cx.theme().background.lighten(0.3)
                } else {
                    cx.theme().background.darken(0.01)
                }
            } else {
                cx.theme().background
            };
            let is_test_text = if row.is_test { "是" } else { "否" };
            request_body_rows.push(
                h_flex()
                    .id(("svc-req-row", row.uid as usize))
                    .w_full()
                    .bg(bg)
                    .border_b_1()
                    .border_color(border)
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-req-device"),
                        self.service_request_column_width(ServiceRequestColumn::Device),
                        &row.device,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-req-imr"),
                        self.service_request_column_width(ServiceRequestColumn::Imr),
                        &row.imr,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-req-request-time"),
                        self.service_request_column_width(ServiceRequestColumn::RequestTime),
                        &row.request_time,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-req-timeout-ms"),
                        self.service_request_column_width(ServiceRequestColumn::TimeoutMs),
                        &row.timeout_ms.to_string(),
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-req-is-test"),
                        self.service_request_column_width(ServiceRequestColumn::IsTest),
                        is_test_text,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-req-requester"),
                        self.service_request_column_width(ServiceRequestColumn::Requester),
                        &row.requester,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-req-args-json"),
                        self.service_request_column_width(ServiceRequestColumn::ArgsJson),
                        &row.args_json,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-req-uuid"),
                        self.service_request_column_width(ServiceRequestColumn::Uuid),
                        &row.uuid,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-req-response-time"),
                        self.service_request_column_width(ServiceRequestColumn::ResponseTime),
                        &row.response_time,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-req-response-code"),
                        self.service_request_column_width(ServiceRequestColumn::ResponseCode),
                        &row.response_code_hex,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-req-responser"),
                        self.service_request_column_width(ServiceRequestColumn::Responser),
                        &row.responser,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-req-summary"),
                        self.service_request_column_width(ServiceRequestColumn::Summary),
                        &row.summary,
                        window,
                        cx,
                    )),
            );
        }

        let request_table = v_flex()
            .w_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .rounded_md()
            .border_1()
            .border_color(border)
            .overflow_hidden()
            .child(
                h_flex()
                    .px_2()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .bg(secondary_bg)
                    .child(
                        Label::new("请求记录")
                            .text_sm()
                            .text_color(cx.theme().foreground),
                    )
                    .child(
                        Label::new(format!("(共 {total_requests} 条)"))
                            .text_sm()
                            .text_color(muted_fg)
                            .ml_2(),
                    ),
            )
            .child(
                div()
                    .id("svc-req-header-x-scroll")
                    .w_full()
                    .flex_none()
                    .min_w(px(0.0))
                    .overflow_x_scroll()
                    .track_scroll(&self.service_table_horizontal_scroll_handle)
                    .child(
                        h_flex()
                            .min_w(px(request_table_width))
                            .w(px(request_table_width))
                            .bg(cx.theme().secondary.opacity(0.6))
                            .border_b_1()
                            .border_color(border)
                            .child(self.render_static_header_cell(
                                self.service_request_column_width(ServiceRequestColumn::Device),
                                "设备号",
                                ResizableTableKind::ServiceRequest,
                                ServiceRequestColumn::Device as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_request_column_width(ServiceRequestColumn::Imr),
                                "IMR",
                                ResizableTableKind::ServiceRequest,
                                ServiceRequestColumn::Imr as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_request_column_width(
                                    ServiceRequestColumn::RequestTime,
                                ),
                                "请求时间",
                                ResizableTableKind::ServiceRequest,
                                ServiceRequestColumn::RequestTime as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_request_column_width(ServiceRequestColumn::TimeoutMs),
                                "超时(ms)",
                                ResizableTableKind::ServiceRequest,
                                ServiceRequestColumn::TimeoutMs as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_request_column_width(ServiceRequestColumn::IsTest),
                                "测试",
                                ResizableTableKind::ServiceRequest,
                                ServiceRequestColumn::IsTest as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_request_column_width(ServiceRequestColumn::Requester),
                                "请求者",
                                ResizableTableKind::ServiceRequest,
                                ServiceRequestColumn::Requester as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_request_column_width(ServiceRequestColumn::ArgsJson),
                                "其他参数",
                                ResizableTableKind::ServiceRequest,
                                ServiceRequestColumn::ArgsJson as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_request_column_width(ServiceRequestColumn::Uuid),
                                "UUID",
                                ResizableTableKind::ServiceRequest,
                                ServiceRequestColumn::Uuid as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_request_column_width(
                                    ServiceRequestColumn::ResponseTime,
                                ),
                                "响应时间",
                                ResizableTableKind::ServiceRequest,
                                ServiceRequestColumn::ResponseTime as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_request_column_width(
                                    ServiceRequestColumn::ResponseCode,
                                ),
                                "响应码(hex)",
                                ResizableTableKind::ServiceRequest,
                                ServiceRequestColumn::ResponseCode as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_request_column_width(ServiceRequestColumn::Responser),
                                "响应人",
                                ResizableTableKind::ServiceRequest,
                                ServiceRequestColumn::Responser as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_request_column_width(ServiceRequestColumn::Summary),
                                "报文摘要",
                                ResizableTableKind::ServiceRequest,
                                ServiceRequestColumn::Summary as usize,
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .id("svc-req-body-x-scroll")
                    .w_full()
                    .min_w(px(0.0))
                    .overflow_x_scroll()
                    .track_scroll(&self.service_table_horizontal_scroll_handle)
                    .child(
                        div()
                            .min_w(px(request_table_width))
                            .w(px(request_table_width))
                            .children(request_body_rows),
                    ),
            )
            .child(
                self.render_horizontal_scrollbar_row(
                    &self.service_table_horizontal_scroll_handle,
                    cx,
                ),
            );

        let mut response_body_rows = Vec::new();
        for (idx, row) in response_rows.iter().enumerate() {
            let bg = if idx % 2 == 0 {
                if cx.theme().is_dark() {
                    cx.theme().background.lighten(0.3)
                } else {
                    cx.theme().background.darken(0.01)
                }
            } else {
                cx.theme().background
            };
            response_body_rows.push(
                h_flex()
                    .id(("svc-resp-row", row.uid as usize))
                    .w_full()
                    .bg(bg)
                    .border_b_1()
                    .border_color(border)
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-resp-request-uuid"),
                        self.service_response_column_width(ServiceResponseColumn::RequestUuid),
                        &row.request_uuid,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-resp-response-uuid"),
                        self.service_response_column_width(ServiceResponseColumn::ResponseUuid),
                        &row.response_uuid,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-resp-response-time"),
                        self.service_response_column_width(ServiceResponseColumn::ResponseTime),
                        &row.response_time,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-resp-response-code"),
                        self.service_response_column_width(ServiceResponseColumn::ResponseCode),
                        &row.response_code_hex,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-resp-responser"),
                        self.service_response_column_width(ServiceResponseColumn::Responser),
                        &row.responser,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-resp-receive-time"),
                        self.service_response_column_width(ServiceResponseColumn::ReceiveTime),
                        &row.receive_time,
                        window,
                        cx,
                    ))
                    .child(self.render_prop_cell(
                        TableCellId::new(row.uid, "svc-resp-summary"),
                        self.service_response_column_width(ServiceResponseColumn::Summary),
                        &row.summary,
                        window,
                        cx,
                    )),
            );
        }

        let response_table = v_flex()
            .w_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .rounded_md()
            .border_1()
            .border_color(border)
            .overflow_hidden()
            .child(
                h_flex()
                    .px_2()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .bg(secondary_bg)
                    .child(
                        Label::new("响应记录")
                            .text_sm()
                            .text_color(cx.theme().foreground),
                    )
                    .child(
                        Label::new(format!("(共 {total_responses} 条)"))
                            .text_sm()
                            .text_color(muted_fg)
                            .ml_2(),
                    ),
            )
            .child(
                div()
                    .id("svc-resp-header-x-scroll")
                    .w_full()
                    .flex_none()
                    .min_w(px(0.0))
                    .overflow_x_scroll()
                    .track_scroll(&self.service_response_horizontal_scroll_handle)
                    .child(
                        h_flex()
                            .min_w(px(response_table_width))
                            .w(px(response_table_width))
                            .bg(cx.theme().secondary.opacity(0.6))
                            .border_b_1()
                            .border_color(border)
                            .child(self.render_static_header_cell(
                                self.service_response_column_width(
                                    ServiceResponseColumn::RequestUuid,
                                ),
                                "请求的UUID",
                                ResizableTableKind::ServiceResponse,
                                ServiceResponseColumn::RequestUuid as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_response_column_width(
                                    ServiceResponseColumn::ResponseUuid,
                                ),
                                "响应的UUID",
                                ResizableTableKind::ServiceResponse,
                                ServiceResponseColumn::ResponseUuid as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_response_column_width(
                                    ServiceResponseColumn::ResponseTime,
                                ),
                                "响应时间",
                                ResizableTableKind::ServiceResponse,
                                ServiceResponseColumn::ResponseTime as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_response_column_width(
                                    ServiceResponseColumn::ResponseCode,
                                ),
                                "响应码(hex)",
                                ResizableTableKind::ServiceResponse,
                                ServiceResponseColumn::ResponseCode as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_response_column_width(
                                    ServiceResponseColumn::Responser,
                                ),
                                "响应人",
                                ResizableTableKind::ServiceResponse,
                                ServiceResponseColumn::Responser as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_response_column_width(
                                    ServiceResponseColumn::ReceiveTime,
                                ),
                                "实际接收时间",
                                ResizableTableKind::ServiceResponse,
                                ServiceResponseColumn::ReceiveTime as usize,
                                cx,
                            ))
                            .child(self.render_static_header_cell(
                                self.service_response_column_width(ServiceResponseColumn::Summary),
                                "报文摘要",
                                ResizableTableKind::ServiceResponse,
                                ServiceResponseColumn::Summary as usize,
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .id("svc-resp-body-x-scroll")
                    .w_full()
                    .min_w(px(0.0))
                    .overflow_x_scroll()
                    .track_scroll(&self.service_response_horizontal_scroll_handle)
                    .child(
                        div()
                            .min_w(px(response_table_width))
                            .w(px(response_table_width))
                            .children(response_body_rows),
                    ),
            )
            .child(self.render_horizontal_scrollbar_row(
                &self.service_response_horizontal_scroll_handle,
                cx,
            ));

        v_flex()
            .flex_1()
            .h_0()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .p_3()
            .gap_3()
            .id("svc-panel-scroll")
            .overflow_y_scroll()
            .children(error_banner)
            .children(form_error)
            .child(form)
            .child(request_table)
            .child(response_table)
            .into_any_element()
    }

    fn render_service_form_row(
        &self,
        label: &str,
        content: gpui::AnyElement,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted_fg = cx.theme().muted_foreground;
        h_flex()
            .w_full()
            .gap_3()
            .items_start()
            .child(
                div()
                    .w(px(110.0))
                    .pt(px(6.0))
                    .child(Label::new(label.to_string()).text_sm().text_color(muted_fg)),
            )
            .child(div().flex_1().min_w(px(0.0)).child(content))
    }

    fn render_horizontal_scrollbar_row(
        &self,
        scroll_handle: &ScrollHandle,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .w_full()
            .h(px(16.0))
            .flex_none()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(Scrollbar::horizontal(scroll_handle).scrollbar_show(ScrollbarShow::Always))
    }

    fn handle_prop_table_vertical_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Self::handle_table_vertical_scroll(&self.prop_table_scroll_handle, event, window, cx);
    }

    fn handle_event_table_vertical_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Self::handle_table_vertical_scroll(&self.event_table_scroll_handle, event, window, cx);
    }

    fn handle_table_vertical_scroll(
        scroll_handle: &ScrollHandle,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = event.delta.pixel_delta(window.line_height());
        if delta.y.abs() < delta.x.abs() || delta.y == px(0.0) {
            return;
        }

        let mut offset = scroll_handle.offset();
        let max_y = scroll_handle.max_offset().height;
        let next_y = (offset.y + delta.y).clamp(-max_y, px(0.0));

        if next_y != offset.y {
            offset.y = next_y;
            scroll_handle.set_offset(offset);
            cx.notify();
        }
        cx.stop_propagation();
    }

    fn render_static_header_cell(
        &self,
        w: f32,
        text: &str,
        table: ResizableTableKind,
        column_ix: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .relative()
            .w(px(w))
            .px_2()
            .py_2()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                Label::new(text.to_string())
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .text_ellipsis(),
            )
            .child(self.render_column_resize_handle(table, column_ix, cx))
    }

    fn render_filterable_prop_header_cell(
        &self,
        w: f32,
        text: &str,
        column: PropSortColumn,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let fg = cx.theme().foreground;
        let primary = cx.theme().primary;
        let hover_bg = if cx.theme().is_dark() {
            cx.theme().secondary.lighten(0.04)
        } else {
            cx.theme().secondary.darken(0.02)
        };

        let (sort_icon, filter_active) = {
            let state = self.prop_table_state.read(cx);
            let icon = match state
                .sort()
                .filter(|s| s.column == column)
                .map(|s| s.direction)
            {
                Some(SortDirection::Asc) => IconName::ChevronUp,
                Some(SortDirection::Desc) => IconName::ChevronDown,
                None => IconName::ChevronsUpDown,
            };
            let active = !state.filters().get(column).is_empty();
            (icon, active)
        };

        let sort_active = !matches!(sort_icon, IconName::ChevronsUpDown);
        let sort_icon_color = if sort_active { fg } else { muted };
        let filter_icon_color = if filter_active { primary } else { muted };

        let column_id = column as usize;

        let input_entity = self.prop_filter_inputs.entity(column).clone();
        let calendar_entity = self.prop_filter_inputs.calendar(column).cloned();

        let trigger_button = Button::new(("prop-filter-trig", column_id))
            .ghost()
            .compact()
            .icon(Icon::from(CustomIconName::Filter).text_color(filter_icon_color));

        let popover = Popover::new(("prop-filter-pop", column_id))
            .anchor(Corner::TopRight)
            .mouse_button(MouseButton::Left)
            .trigger(trigger_button)
            .content(move |_state, _window, _cx| {
                let input_for_clear = input_entity.clone();
                let calendar_for_clear = calendar_entity.clone();
                let control = if let Some(calendar) = calendar_entity.as_ref() {
                    Calendar::new(calendar)
                        .number_of_months(1)
                        .small()
                        .border_0()
                        .rounded_none()
                        .p_0()
                        .into_any_element()
                } else {
                    Input::new(&input_entity)
                        .cleanable(true)
                        .small()
                        .into_any_element()
                };

                v_flex().gap_2().w(px(240.0)).p_2().child(control).child(
                    h_flex().justify_end().child(
                        Button::new(("prop-filter-clear", column_id))
                            .ghost()
                            .compact()
                            .small()
                            .label("清除")
                            .on_click(move |_, window, cx| {
                                input_for_clear.update(cx, |state, cx| {
                                    state.set_value("".to_string(), window, cx);
                                });
                                if let Some(calendar) = calendar_for_clear.as_ref() {
                                    calendar.update(cx, |state, cx| {
                                        state.set_date(Date::Single(None), window, cx);
                                    });
                                }
                            }),
                    ),
                )
            });

        div()
            .relative()
            .w(px(w))
            .h_full()
            .px_2()
            .py_2()
            .border_r_1()
            .border_color(border)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .id(("prop-header-sort", column_id))
                            .flex_1()
                            .min_w(px(0.0))
                            .cursor_pointer()
                            .hover(move |this| this.bg(hover_bg))
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        Label::new(text.to_string())
                                            .text_sm()
                                            .text_color(muted)
                                            .text_ellipsis(),
                                    )
                                    .child(
                                        Icon::new(sort_icon).size_3().text_color(sort_icon_color),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.prop_table_state.update(cx, |state, cx| {
                                    state.toggle_sort(column);
                                    cx.notify();
                                });
                            })),
                    )
                    .child(popover),
            )
            .child(self.render_column_resize_handle(ResizableTableKind::Prop, column_id, cx))
    }

    fn render_filterable_event_header_cell(
        &self,
        w: f32,
        text: &str,
        column: EventSortColumn,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let fg = cx.theme().foreground;
        let primary = cx.theme().primary;
        let hover_bg = if cx.theme().is_dark() {
            cx.theme().secondary.lighten(0.04)
        } else {
            cx.theme().secondary.darken(0.02)
        };

        let (sort_icon, filter_active) = {
            let state = self.event_table_state.read(cx);
            let icon = match state
                .sort()
                .filter(|s| s.column == column)
                .map(|s| s.direction)
            {
                Some(SortDirection::Asc) => IconName::ChevronUp,
                Some(SortDirection::Desc) => IconName::ChevronDown,
                None => IconName::ChevronsUpDown,
            };
            let active = !state.filters().get(column).is_empty();
            (icon, active)
        };

        let sort_active = !matches!(sort_icon, IconName::ChevronsUpDown);
        let sort_icon_color = if sort_active { fg } else { muted };
        let filter_icon_color = if filter_active { primary } else { muted };

        let column_id = column as usize;

        let input_entity = self.event_filter_inputs.entity(column).clone();
        let calendar_entity = self.event_filter_inputs.calendar(column).cloned();

        let trigger_button = Button::new(("event-filter-trig", column_id))
            .ghost()
            .compact()
            .icon(Icon::from(CustomIconName::Filter).text_color(filter_icon_color));

        let popover = Popover::new(("event-filter-pop", column_id))
            .anchor(Corner::TopRight)
            .mouse_button(MouseButton::Left)
            .trigger(trigger_button)
            .content(move |_state, _window, _cx| {
                let input_for_clear = input_entity.clone();
                let calendar_for_clear = calendar_entity.clone();
                let control = if let Some(calendar) = calendar_entity.as_ref() {
                    Calendar::new(calendar)
                        .number_of_months(1)
                        .small()
                        .border_0()
                        .rounded_none()
                        .p_0()
                        .into_any_element()
                } else {
                    Input::new(&input_entity)
                        .cleanable(true)
                        .small()
                        .into_any_element()
                };

                v_flex().gap_2().w(px(240.0)).p_2().child(control).child(
                    h_flex().justify_end().child(
                        Button::new(("event-filter-clear", column_id))
                            .ghost()
                            .compact()
                            .small()
                            .label("清除")
                            .on_click(move |_, window, cx| {
                                input_for_clear.update(cx, |state, cx| {
                                    state.set_value("".to_string(), window, cx);
                                });
                                if let Some(calendar) = calendar_for_clear.as_ref() {
                                    calendar.update(cx, |state, cx| {
                                        state.set_date(Date::Single(None), window, cx);
                                    });
                                }
                            }),
                    ),
                )
            });

        div()
            .relative()
            .w(px(w))
            .h_full()
            .px_2()
            .py_2()
            .border_r_1()
            .border_color(border)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .id(("event-header-sort", column_id))
                            .flex_1()
                            .min_w(px(0.0))
                            .cursor_pointer()
                            .hover(move |this| this.bg(hover_bg))
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        Label::new(text.to_string())
                                            .text_sm()
                                            .text_color(muted)
                                            .text_ellipsis(),
                                    )
                                    .child(
                                        Icon::new(sort_icon).size_3().text_color(sort_icon_color),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.event_table_state.update(cx, |state, cx| {
                                    state.toggle_sort(column);
                                    cx.notify();
                                });
                            })),
                    )
                    .child(popover),
            )
            .child(self.render_column_resize_handle(ResizableTableKind::Event, column_id, cx))
    }

    fn render_prop_cell(
        &self,
        cell: TableCellId,
        w: f32,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = text.to_string();
        let value = label.clone();
        let is_active = self.active_table_cell == Some(cell);

        div()
            .w(px(w))
            .h(px(36.0))
            .px_2()
            .py_2()
            .border_r_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .cursor_text()
            .when(is_active, |this| {
                this.child(
                    Input::new(&self.table_cell_input)
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .disabled(true)
                        .small()
                        .h_full()
                        .size_full(),
                )
            })
            .when(!is_active, |this| {
                this.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        this.activate_table_cell(cell, &value, window, cx);
                    }),
                )
                .child(Label::new(label).text_sm().text_ellipsis())
            })
    }

    fn render_prop_pagination(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (total_visible, total_all, start, end, pages, page_index, page_size, has_filters) = {
            let state = self.prop_table_state.read(cx);
            let total_visible = state.visible_len();
            let total_all = state.rows_len();
            let (start, end) = state.page_range();
            (
                total_visible,
                total_all,
                start,
                end,
                state.total_pages(),
                state.page_index(),
                state.page_size(),
                state.has_active_filters(),
            )
        };

        let display_start = if total_visible == 0 { 0 } else { start + 1 };
        let display_end = if total_visible == 0 { 0 } else { end };

        let info = if has_filters {
            format!(
                "显示第 {display_start} 到第 {display_end} 条记录, 符合过滤条件 {total_visible} 条 (共 {total_all} 条)"
            )
        } else {
            format!("显示第 {display_start} 到第 {display_end} 条记录，总共 {total_all} 条记录")
        };
        let page_label = format!("第 {} / {} 页", page_index + 1, pages);

        let current_size = PropPageSize::from_value(page_size);
        let dropdown = DropdownButton::new("prop-page-size-dropdown")
            .button(
                Button::new("prop-page-size-btn")
                    .ghost()
                    .compact()
                    .label(format!("{page_size}")),
            )
            .dropdown_menu_with_anchor(Corner::TopLeft, move |menu, _, _| {
                let menu = menu
                    .menu_element_with_check(
                        current_size == PropPageSize::S10,
                        Box::new(PropPageSize::S10),
                        move |_, _cx| Label::new("10").ml_2().text_xs(),
                    )
                    .menu_element_with_check(
                        current_size == PropPageSize::S20,
                        Box::new(PropPageSize::S20),
                        move |_, _cx| Label::new("20").ml_2().text_xs(),
                    )
                    .menu_element_with_check(
                        current_size == PropPageSize::S50,
                        Box::new(PropPageSize::S50),
                        move |_, _cx| Label::new("50").ml_2().text_xs(),
                    )
                    .menu_element_with_check(
                        current_size == PropPageSize::S100,
                        Box::new(PropPageSize::S100),
                        move |_, _cx| Label::new("100").ml_2().text_xs(),
                    );
                menu
            });

        let prev_disabled = page_index == 0;
        let next_disabled = page_index + 1 >= pages;

        let prev_btn = Button::new("prop-page-prev")
            .ghost()
            .icon(IconName::ChevronLeft)
            .disabled(prev_disabled)
            .on_click(cx.listener(|this, _, _, cx| {
                this.prop_table_state.update(cx, |state, cx| {
                    let current = state.page_index();
                    state.set_page_index(current.saturating_sub(1));
                    cx.notify();
                });
            }));

        let next_btn = Button::new("prop-page-next")
            .ghost()
            .icon(IconName::ChevronRight)
            .disabled(next_disabled)
            .on_click(cx.listener(|this, _, _, cx| {
                this.prop_table_state.update(cx, |state, cx| {
                    let current = state.page_index();
                    state.set_page_index(current + 1);
                    cx.notify();
                });
            }));

        h_flex()
            .w_full()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w(px(360.0))
                    .min_w(px(0.0))
                    .flex_shrink()
                    .truncate()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(info),
            )
            .child(div().flex_1())
            .child(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap_6()
                    .child(
                        h_flex()
                            .flex_none()
                            .items_center()
                            .gap_2()
                            .child(
                                Label::new("每页显示")
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(dropdown)
                            .child(
                                Label::new("条记录")
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .items_center()
                            .gap_2()
                            .child(prev_btn)
                            .child(
                                Label::new(page_label)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(next_btn),
                    ),
            )
    }

    fn render_event_pagination(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (total_visible, total_all, start, end, pages, page_index, page_size, has_filters) = {
            let state = self.event_table_state.read(cx);
            let total_visible = state.visible_len();
            let total_all = state.rows_len();
            let (start, end) = state.page_range();
            (
                total_visible,
                total_all,
                start,
                end,
                state.total_pages(),
                state.page_index(),
                state.page_size(),
                state.has_active_filters(),
            )
        };

        let display_start = if total_visible == 0 { 0 } else { start + 1 };
        let display_end = if total_visible == 0 { 0 } else { end };

        let info = if has_filters {
            format!(
                "显示第 {display_start} 到第 {display_end} 条记录, 符合过滤条件 {total_visible} 条 (共 {total_all} 条)"
            )
        } else {
            format!("显示第 {display_start} 到第 {display_end} 条记录，总共 {total_all} 条记录")
        };
        let page_label = format!("第 {} / {} 页", page_index + 1, pages);

        let current_size = PropPageSize::from_value(page_size);
        let dropdown = DropdownButton::new("event-page-size-dropdown")
            .button(
                Button::new("event-page-size-btn")
                    .ghost()
                    .compact()
                    .label(format!("{page_size}")),
            )
            .dropdown_menu_with_anchor(Corner::TopLeft, move |menu, _, _| {
                let menu = menu
                    .menu_element_with_check(
                        current_size == PropPageSize::S10,
                        Box::new(PropPageSize::S10),
                        move |_, _cx| Label::new("10").ml_2().text_xs(),
                    )
                    .menu_element_with_check(
                        current_size == PropPageSize::S20,
                        Box::new(PropPageSize::S20),
                        move |_, _cx| Label::new("20").ml_2().text_xs(),
                    )
                    .menu_element_with_check(
                        current_size == PropPageSize::S50,
                        Box::new(PropPageSize::S50),
                        move |_, _cx| Label::new("50").ml_2().text_xs(),
                    )
                    .menu_element_with_check(
                        current_size == PropPageSize::S100,
                        Box::new(PropPageSize::S100),
                        move |_, _cx| Label::new("100").ml_2().text_xs(),
                    );
                menu
            });

        let prev_disabled = page_index == 0;
        let next_disabled = page_index + 1 >= pages;

        let prev_btn = Button::new("event-page-prev")
            .ghost()
            .icon(IconName::ChevronLeft)
            .disabled(prev_disabled)
            .on_click(cx.listener(|this, _, _, cx| {
                this.event_table_state.update(cx, |state, cx| {
                    let current = state.page_index();
                    state.set_page_index(current.saturating_sub(1));
                    cx.notify();
                });
            }));

        let next_btn = Button::new("event-page-next")
            .ghost()
            .icon(IconName::ChevronRight)
            .disabled(next_disabled)
            .on_click(cx.listener(|this, _, _, cx| {
                this.event_table_state.update(cx, |state, cx| {
                    let current = state.page_index();
                    state.set_page_index(current + 1);
                    cx.notify();
                });
            }));

        h_flex()
            .w_full()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w(px(360.0))
                    .min_w(px(0.0))
                    .flex_shrink()
                    .truncate()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(info),
            )
            .child(div().flex_1())
            .child(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap_6()
                    .child(
                        h_flex()
                            .flex_none()
                            .items_center()
                            .gap_2()
                            .child(
                                Label::new("每页显示")
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(dropdown)
                            .child(
                                Label::new("条记录")
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .items_center()
                            .gap_2()
                            .child(prev_btn)
                            .child(
                                Label::new(page_label)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(next_btn),
                    ),
            )
    }

    /// Render a topic tab for the selected agent
    fn render_agent_topic_tab(
        &self,
        index: i32,
        label: String,
        topic_path: &str,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let tab_bg = if is_selected {
            cx.theme().primary.opacity(0.12)
        } else {
            cx.theme().background
        };
        let border_color = if is_selected {
            cx.theme().primary
        } else {
            cx.theme().border
        };
        let text_color = if is_selected {
            cx.theme().foreground
        } else {
            cx.theme().muted_foreground
        };
        let tooltip_label = topic_path.to_string();
        let topic_path_for_log = tooltip_label.clone();

        div()
            .id(("agent-topic-tab", index as usize))
            .px_4()
            .py_1()
            .bg(tab_bg)
            .cursor_pointer()
            .flex_none()
            .rounded_md()
            .border_2()
            .border_color(border_color)
            .child(Label::new(label).text_sm().text_color(text_color))
            .tooltip(move |window, cx| Tooltip::new(tooltip_label.clone()).build(window, cx))
            .on_click(cx.listener(move |this, _, _, cx| {
                tracing::debug!(
                    topic_index = index,
                    topic = %topic_path_for_log,
                    "selecting topic tab from agent topic bar"
                );
                this.config_state.update(cx, |state, cx| {
                    state.select_topic(Some(index), cx);
                });
            }))
    }

    /// Render content for selected agent's topic using pre-collected data
    fn render_agent_topic_content_from_data(
        &self,
        topic_data: Option<(i32, String, String)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted_fg = cx.theme().muted_foreground;
        let bg = cx.theme().background;
        let border = cx.theme().border;

        let content = if let Some((_idx, path, topic_type)) = topic_data {
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .gap_2()
                        .child(Label::new("Path:").text_sm().text_color(muted_fg))
                        .child(Label::new(path).text_sm()),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(Label::new("Type:").text_sm().text_color(muted_fg))
                        .child(Label::new(topic_type).text_sm()),
                )
        } else {
            v_flex().child(Label::new("No topic selected").text_color(muted_fg))
        };

        div()
            .p_4()
            .flex_1()
            .bg(bg)
            .border_1()
            .border_color(border)
            .rounded_b_md()
            .rounded_tr_md()
            .child(content)
    }

    /// Render the split panel layout (left: agent list, right: topic tabs)
    fn render_split_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .child(self.render_agent_list(window, cx))
            .child(div().flex_none().w(px(2.0)).h_full().bg(cx.theme().border))
            .child(self.render_agent_topics(window, cx))
    }

    /// Render topic tabs view
    fn render_topic_tabs(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let config_state = self.config_state.read(cx);
        let selected_topic_index = config_state.selected_topic_index().unwrap_or(0);

        let details = config_state
            .selected_config()
            .map(|c| c.details.clone())
            .unwrap_or_default();

        let config_info = config_state
            .selected_config()
            .map(|c| (c.service_url.clone(), c.source.clone()))
            .unwrap_or_default();

        // Build tab buttons
        let mut tabs = Vec::new();
        for detail in &details {
            let is_selected = detail.index == selected_topic_index;
            tabs.push(self.render_topic_tab(detail.index, &detail.path, is_selected, cx));
        }

        v_flex()
            .size_full()
            .p_4()
            .gap_2()
            // Config info header
            .child(
                v_flex()
                    .gap_1()
                    .child(Label::new(config_info.0).text_lg())
                    .child(
                        Label::new(config_info.1)
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            // Tab bar
            .child(
                h_flex()
                    .id("config-tabs-scroll")
                    .gap_1()
                    .flex_nowrap()
                    .overflow_x_scroll()
                    .children(tabs),
            )
            // Tab content
            .child(self.render_topic_content(cx))
    }

    /// Render the main content based on state
    fn render_content(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let load_state = self.config_state.read(cx).load_state().clone();
        let (has_configs, has_topic_agents) = {
            let config_state = self.config_state.read(cx);
            (
                !config_state.configs().is_empty(),
                !config_state.topic_agents().is_empty(),
            )
        };

        match load_state {
            ConfigLoadState::Loading => self.render_loading(cx).into_any_element(),
            ConfigLoadState::Error(msg) => self.render_error(&msg, cx).into_any_element(),
            ConfigLoadState::Loaded | ConfigLoadState::Idle => {
                if has_topic_agents {
                    return self.render_split_panel(window, cx).into_any_element();
                }

                let locale = self.locale(cx);
                let message = if has_configs {
                    t!("config.no_topic_agents", locale = &locale).to_string()
                } else {
                    t!("config.no_config", locale = &locale).to_string()
                };

                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Label::new(message).text_color(cx.theme().muted_foreground))
                    .into_any_element()
            }
        }
    }
}

fn find_topic_origin(configs: &[ConfigItem], topic_path: &str) -> Option<(String, String)> {
    for config in configs {
        for agent in &config.topic_agents {
            for topic in &agent.topics {
                if topic.path == topic_path {
                    let cfgid = extract_cfgid_from_source(&config.source)?;
                    let service_url = resolve_pulsar_service_url(configs, config, Some(&cfgid))?;
                    return Some((service_url, cfgid));
                }
            }
        }
    }
    None
}

fn find_topic_service_url(configs: &[ConfigItem], topic_path: &str) -> Option<String> {
    for config in configs {
        for agent in &config.topic_agents {
            for topic in &agent.topics {
                if topic.path == topic_path {
                    let cfgid = extract_cfgid_from_source(&config.source);
                    return resolve_pulsar_service_url(configs, config, cfgid.as_deref());
                }
            }
        }
    }
    None
}

fn resolve_pulsar_service_url(
    configs: &[ConfigItem],
    matched_config: &ConfigItem,
    cfgid: Option<&str>,
) -> Option<String> {
    normalize_pulsar_service_url(&matched_config.service_url).or_else(|| {
        cfgid.and_then(|cfgid| {
            configs
                .iter()
                .filter(|config| {
                    extract_cfgid_from_source(&config.source).as_deref() == Some(cfgid)
                })
                .find_map(|config| normalize_pulsar_service_url(&config.service_url))
        })
    })
}

fn extract_cfgid_from_source(source: &str) -> Option<String> {
    if !source.starts_with("CMC_") {
        return None;
    }

    if let (Some(start), Some(end)) = (source.find('{'), source.find('}')) {
        if start < end {
            let inner = &source[start + 1..end];
            if !inner.is_empty() {
                return Some(inner.to_string());
            }
        }
    }

    if let Some(sg_pos) = source.find("_sg") {
        let raw = &source["CMC_".len()..sg_pos];
        let trimmed = raw.trim_start_matches('{').trim_end_matches('}');
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    None
}

pub(super) fn decode_framed_iothub_message<T>(payload: &[u8]) -> Option<(String, T)>
where
    T: prost::Message + Default,
{
    fn try_with_summary_len_at<'a>(
        payload: &'a [u8],
        summary_len_index: usize,
    ) -> Option<(String, &'a [u8])> {
        if payload.len() <= summary_len_index {
            return None;
        }

        let summary_len = payload[summary_len_index] as usize;
        let prefix_len = summary_len_index + 1;
        let summary_end = prefix_len.saturating_add(summary_len);
        if summary_end > payload.len() {
            return None;
        }

        let summary = if summary_len == 0 {
            String::new()
        } else {
            String::from_utf8_lossy(&payload[prefix_len..summary_end]).to_string()
        };

        let proto = &payload[summary_end..];
        Some((summary, proto))
    }

    // Most common (DFC): payload[2] is summary length, summary starts at payload[3]
    if let Some((summary, proto)) = try_with_summary_len_at(payload, 2) {
        if let Ok(message) = T::decode(proto) {
            return Some((summary, message));
        }
    }
    // Some producers omit the 2-byte prefix: summary length at payload[0] or payload[1]
    if let Some((summary, proto)) = try_with_summary_len_at(payload, 0) {
        if let Ok(message) = T::decode(proto) {
            return Some((summary, message));
        }
    }
    if let Some((summary, proto)) = try_with_summary_len_at(payload, 1) {
        if let Ok(message) = T::decode(proto) {
            return Some((summary, message));
        }
    }

    // Fallback: no summary framing, payload is raw protobuf
    T::decode(payload)
        .ok()
        .map(|message| (String::new(), message))
}

fn decode_data_frame(payload: &[u8]) -> Option<(String, crate::proto::iothub::DataFrame)> {
    decode_framed_iothub_message(payload)
}

fn decode_event_record_list(
    payload: &[u8],
) -> Option<(String, crate::proto::iothub::EventRecordList)> {
    decode_framed_iothub_message(payload)
}

pub(super) fn format_clock_time(clock: Option<&crate::proto::iothub::ClockTime>) -> String {
    let Some(clock) = clock else {
        return String::new();
    };

    let secs = i64::from(clock.t);
    let Some(dt_utc) = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0) else {
        return String::new();
    };

    dt_utc
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string()
}

fn format_hi_clock_time(clock: Option<&crate::proto::iothub::HiClockTime>) -> String {
    let Some(clock) = clock else {
        return String::new();
    };

    let secs = i64::from(clock.t);
    let nanos = clock.nano.min(999_999_999);
    let Some(dt_utc) = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nanos) else {
        return String::new();
    };

    dt_utc
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string()
}

fn any_value_to_string(v: Option<&crate::proto::iothub::AnyValue>) -> String {
    let Some(v) = v else {
        return String::new();
    };

    use crate::proto::iothub::any_value::V;
    match v.v.as_ref() {
        Some(V::DoubleV(x)) => x.to_string(),
        Some(V::FloatV(x)) => x.to_string(),
        Some(V::Int32V(x)) => x.to_string(),
        Some(V::Uint32V(x)) => x.to_string(),
        Some(V::Uint64V(x)) => x.to_string(),
        Some(V::Sint32V(x)) => x.to_string(),
        Some(V::Sint64V(x)) => x.to_string(),
        Some(V::Fixed32V(x)) => x.to_string(),
        Some(V::Fixed64V(x)) => x.to_string(),
        Some(V::Sfixed32V(x)) => x.to_string(),
        Some(V::Sfixed64V(x)) => x.to_string(),
        Some(V::BoolV(x)) => x.to_string(),
        Some(V::StringV(s)) => s.clone(),
        Some(V::BytesV(b)) => format!("{} bytes", b.len()),
        Some(V::AnyV(a)) => format!("any({}) {} bytes", a.type_url, a.value.len()),
        Some(V::NullV(_)) => String::new(),
        Some(V::JsonV(s)) => s.clone(),
        Some(V::MsgPackV(b)) => format!("msgpack {} bytes", b.len()),
        None => String::new(),
    }
}

fn enum_value_to_string(v: &crate::proto::iothub::EnumValue) -> String {
    use crate::proto::iothub::enum_value::V;
    match v.v.as_ref() {
        Some(V::Uint64V(x)) => x.to_string(),
        Some(V::BoolV(x)) => x.to_string(),
        Some(V::StringV(s)) => s.clone(),
        None => String::new(),
    }
}

fn event_context_to_string(
    context: &std::collections::HashMap<String, crate::proto::iothub::AnyValue>,
) -> String {
    let mut entries: Vec<_> = context
        .iter()
        .map(|(key, value)| format!("{key}={}", any_value_to_string(Some(value))))
        .collect();
    entries.sort();
    entries.join(", ")
}

fn parse_prop_rows_from_payload(
    payload: &[u8],
    imid2imr: &std::collections::HashMap<(String, u32), String>,
    uid: &AtomicU64,
) -> (Vec<PropRow>, bool) {
    let Some((summary, df)) = decode_data_frame(payload) else {
        return (Vec::new(), false);
    };

    let mut out = Vec::new();
    for set in df.frame {
        let Some(header) = set.header.as_ref() else {
            continue;
        };

        let global_uuid = header.im_global_uuid.clone();
        let device = header.source_device.clone();
        let message_time = format_clock_time(header.t.as_ref());

        for record in &set.data {
            let (imid, imr) = match record.k.as_ref() {
                Some(crate::proto::iothub::data_record::K::Im2id(id)) => {
                    let key = (global_uuid.clone(), *id);
                    let imr = imid2imr
                        .get(&key)
                        .or_else(|| imid2imr.get(&(String::new(), *id)))
                        .cloned()
                        .unwrap_or_else(|| "Unknown Imr".to_string());
                    (i32::try_from(*id).unwrap_or(0), imr)
                }
                Some(crate::proto::iothub::data_record::K::Imr(imr_ref)) => {
                    (0, imr_ref.path.clone())
                }
                None => (0, "Unknown Imr".to_string()),
            };

            let time = format_clock_time(record.device_time.as_ref());

            out.push(PropRow {
                uid: uid.fetch_add(1, Ordering::Relaxed),
                global_uuid: global_uuid.clone(),
                device: device.clone(),
                imr,
                imid,
                value: any_value_to_string(record.v.as_ref()),
                quality: i32::try_from(record.q).unwrap_or(0),
                bcrid: record.bcr_uuid.clone(),
                time,
                message_time: message_time.clone(),
                summary: summary.clone(),
            });
        }
    }

    (out, true)
}

fn parse_event_rows_from_payload(payload: &[u8], uid: &AtomicU64) -> (Vec<EventRow>, bool) {
    let Some((summary, list)) = decode_event_record_list(payload) else {
        return (Vec::new(), false);
    };

    let mut out = Vec::new();
    for event in list.event_array {
        let codes: Vec<String> = event
            .code
            .iter()
            .filter_map(|code| match code.v.as_ref() {
                Some(crate::proto::iothub::enum_value::V::Uint64V(_)) => {
                    Some(enum_value_to_string(code))
                }
                _ => None,
            })
            .collect();
        let str_codes: Vec<String> = event
            .code
            .iter()
            .filter_map(|code| match code.v.as_ref() {
                Some(crate::proto::iothub::enum_value::V::StringV(_)) => {
                    Some(enum_value_to_string(code))
                }
                _ => None,
            })
            .collect();

        out.push(EventRow {
            uid: uid.fetch_add(1, Ordering::Relaxed),
            uuid: event.evt_uuid,
            device: event.src,
            imr: event.imr,
            event_type: event.r#type,
            level: event.level.to_string(),
            tags: event.tags.join(","),
            codes: codes.join(","),
            str_codes: str_codes.join(","),
            happened_time: format_hi_clock_time(event.happened_time.as_ref()),
            record_time: format_clock_time(event.record_time.as_ref()),
            bcr_id: event.bcr_uuid,
            context: event_context_to_string(&event.context),
            summary: summary.clone(),
        });
    }

    (out, true)
}

async fn run_prop_topic_stream(
    service_url: String,
    topic_path: String,
    token: Option<String>,
    cfgid: String,
    redis: Arc<crate::services::RedisRepo>,
    mut stop: watch::Receiver<bool>,
    tx: Sender<PropStreamEvent>,
    uid: Arc<AtomicU64>,
) {
    let imid2imr = match redis.fetch_imid2imr(&cfgid).await {
        Ok(map) => map,
        Err(e) => {
            tracing::warn!("Failed to load IMID->IMR mapping: {}", e);
            std::collections::HashMap::new()
        }
    };
    let service_urls = pulsar_service_url_candidates(&service_url);
    if service_urls.is_empty() {
        let _ = tx.send(PropStreamEvent::Error(format!(
            "无法解析 Pulsar service URL: {service_url}"
        )));
        return;
    }
    let subscription = format!("dfc-gui-prop-{}", uuid::Uuid::new_v4());

    let mut last_stats = Instant::now();
    let mut received_messages: u64 = 0;
    let mut decoded_messages: u64 = 0;
    let mut decode_failures: u64 = 0;
    let mut emitted_rows: u64 = 0;

    let mut connect_attempt: u64 = 0;
    let mut seek_done = false;

    while !*stop.borrow() {
        connect_attempt += 1;

        // Exponential backoff on reconnect (skip delay on first attempt)
        if connect_attempt > 1 {
            let backoff = Duration::from_secs((2u64.pow(connect_attempt.min(5) as u32)).min(30));
            tracing::info!(
                backoff_secs = backoff.as_secs(),
                attempt = connect_attempt,
                "reconnecting after backoff"
            );
            tokio::time::sleep(backoff).await;
            if *stop.borrow() {
                return;
            }
        }

        let (client, connected_service_url): (pulsar::Pulsar<_>, String) =
            match build_pulsar_client_with_fallbacks(&service_urls, token.as_deref()).await {
                Ok(client) => client,
                Err(e) => {
                    let _ = tx.send(PropStreamEvent::Error(format!("Pulsar 连接失败: {e}")));
                    continue;
                }
            };

        tracing::info!(
            topic = %topic_path,
            service_url = %connected_service_url,
            attempt = connect_attempt,
            "connected prop topic client"
        );

        // Diagnostic: list broker topics to verify target exists.
        if let Some(namespace) = topic_path.split("://").nth(1).and_then(|rest| {
            let mut parts = rest.splitn(3, '/');
            let tenant = parts.next()?;
            let ns = parts.next()?;
            Some(format!("{tenant}/{ns}"))
        }) {
            use pulsar::message::proto::command_get_topics_of_namespace::Mode;
            let mode = if topic_path.starts_with("persistent://") {
                Mode::Persistent
            } else {
                Mode::NonPersistent
            };
            match client
                .get_topics_of_namespace(namespace.clone(), mode)
                .await
            {
                Ok(topics) => {
                    let found = topics.iter().any(|t| *t == topic_path);
                    tracing::debug!(
                        namespace = %namespace,
                        total = topics.len(),
                        target = %topic_path,
                        found,
                        "Topic namespace listing"
                    );
                }
                Err(e) => {
                    tracing::warn!(namespace = %namespace, "Failed to list namespace topics: {}", e)
                }
            }
        }

        let options = pulsar::ConsumerOptions::default()
            .durable(false)
            .with_receiver_queue_size(1000);
        let consumer_name = format!("dfc-gui-prop-consumer-{}", uuid::Uuid::new_v4());

        tracing::info!(
            topic = %topic_path,
            subscription = %subscription,
            consumer_name = %consumer_name,
            attempt = connect_attempt,
            "connecting prop topic consumer"
        );

        let mut consumer: pulsar::Consumer<Vec<u8>, _> = match client
            .consumer()
            .with_topic(&topic_path)
            .with_subscription(subscription.clone())
            .with_subscription_type(pulsar::SubType::Shared)
            .with_consumer_name(consumer_name)
            .with_options(options)
            .build()
            .await
        {
            Ok(c) => {
                connect_attempt = 0;
                c
            }
            Err(e) => {
                let _ = tx.send(PropStreamEvent::Error(format!("创建 Consumer 失败: {e}")));
                continue;
            }
        };

        // Align with DFC default: seek to last 20 minutes for persistent topics when possible.
        // Note: Pulsar seek is only reliable for non-partitioned topics.
        if !seek_done && topic_path.starts_with("persistent://") {
            if let Ok(parts) = client.lookup_partitioned_topic(topic_path.clone()).await {
                if parts.len() == 1 {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    if now_ms > 0 {
                        let seek_ms = now_ms.saturating_sub(20 * 60 * 1000) as u64;
                        let _ = consumer
                            .seek(None, None, Some(seek_ms), client.clone())
                            .await;
                    }
                    seek_done = true;
                } else {
                    tracing::debug!(
                        topic = %topic_path,
                        partitions = parts.len(),
                        "prop topic is partitioned; skipping seek"
                    );
                    seek_done = true;
                }
            }
        }

        let mut heartbeat = tokio::time::interval(Duration::from_secs(10));

        loop {
            if *stop.borrow() {
                return;
            }

            tokio::select! {
                changed = stop.changed() => {
                    if changed.is_err() || *stop.borrow() {
                        return;
                    }
                }
                _ = heartbeat.tick() => {
                    tracing::debug!(
                        topic = %topic_path,
                        received_messages,
                        decoded_messages,
                        decode_failures,
                        emitted_rows,
                        consumer_received = consumer.messages_received(),
                        "prop topic consumer heartbeat"
                    );
                }
                msg = consumer.next() => {
                    match msg {
                        Some(Ok(message)) => {
                            received_messages += 1;

                            let data = message.deserialize();
                            let (rows, decoded) = parse_prop_rows_from_payload(&data, &imid2imr, &uid);
                            if decoded {
                                decoded_messages += 1;
                            } else {
                                decode_failures += 1;
                            }
                            if !rows.is_empty() {
                                emitted_rows += rows.len() as u64;
                                let _ = tx.send(PropStreamEvent::Rows(rows));
                            }

                            // Ack to avoid redelivery / memory build-up.
                            let _ = consumer.ack(&message).await;

                            if last_stats.elapsed() >= Duration::from_secs(10) {
                                tracing::info!(
                                    topic = %topic_path,
                                    received_messages,
                                    decoded_messages,
                                    decode_failures,
                                    emitted_rows,
                                    "prop topic stream stats"
                                );
                                last_stats = Instant::now();
                            }
                        }
                        Some(Err(e)) => {
                            let _ = tx.send(PropStreamEvent::Error(format!("读取消息失败: {e}")));
                            break;
                        }
                        None => {
                            let _ = tx.send(PropStreamEvent::Error("Consumer 数据流意外结束，正在重连…".to_string()));
                            break;
                        }
                    }
                }
            }
        }

        // Backoff before reconnecting.
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn run_event_topic_stream(
    service_url: String,
    topic_path: String,
    token: Option<String>,
    mut stop: watch::Receiver<bool>,
    tx: Sender<EventStreamEvent>,
    uid: Arc<AtomicU64>,
) {
    let service_urls = pulsar_service_url_candidates(&service_url);
    if service_urls.is_empty() {
        let _ = tx.send(EventStreamEvent::Error(format!(
            "无法解析 Pulsar service URL: {service_url}"
        )));
        return;
    }
    let subscription = format!("dfc-gui-event-{}", uuid::Uuid::new_v4());

    let mut last_stats = Instant::now();
    let mut received_messages: u64 = 0;
    let mut decoded_messages: u64 = 0;
    let mut decode_failures: u64 = 0;
    let mut emitted_rows: u64 = 0;

    let mut connect_attempt: u64 = 0;
    let mut seek_done = false;

    while !*stop.borrow() {
        connect_attempt += 1;

        if connect_attempt > 1 {
            let backoff = Duration::from_secs((2u64.pow(connect_attempt.min(5) as u32)).min(30));
            tracing::info!(
                backoff_secs = backoff.as_secs(),
                attempt = connect_attempt,
                "reconnecting after backoff"
            );
            tokio::time::sleep(backoff).await;
            if *stop.borrow() {
                return;
            }
        }

        let (client, connected_service_url): (pulsar::Pulsar<_>, String) =
            match build_pulsar_client_with_fallbacks(&service_urls, token.as_deref()).await {
                Ok(client) => client,
                Err(e) => {
                    let _ = tx.send(EventStreamEvent::Error(format!("Pulsar 连接失败: {e}")));
                    continue;
                }
            };

        tracing::info!(
            topic = %topic_path,
            service_url = %connected_service_url,
            attempt = connect_attempt,
            "connected event topic client"
        );

        if let Some(namespace) = topic_path.split("://").nth(1).and_then(|rest| {
            let mut parts = rest.splitn(3, '/');
            let tenant = parts.next()?;
            let ns = parts.next()?;
            Some(format!("{tenant}/{ns}"))
        }) {
            use pulsar::message::proto::command_get_topics_of_namespace::Mode;
            let mode = if topic_path.starts_with("persistent://") {
                Mode::Persistent
            } else {
                Mode::NonPersistent
            };
            match client
                .get_topics_of_namespace(namespace.clone(), mode)
                .await
            {
                Ok(topics) => {
                    let found = topics.iter().any(|t| *t == topic_path);
                    tracing::debug!(
                        namespace = %namespace,
                        total = topics.len(),
                        target = %topic_path,
                        found,
                        "Topic namespace listing"
                    );
                }
                Err(e) => {
                    tracing::warn!(namespace = %namespace, "Failed to list namespace topics: {}", e)
                }
            }
        }

        let options = pulsar::ConsumerOptions::default()
            .durable(false)
            .with_receiver_queue_size(1000);
        let consumer_name = format!("dfc-gui-event-consumer-{}", uuid::Uuid::new_v4());

        tracing::info!(
            topic = %topic_path,
            subscription = %subscription,
            consumer_name = %consumer_name,
            attempt = connect_attempt,
            "connecting event topic consumer"
        );

        let mut consumer: pulsar::Consumer<Vec<u8>, _> = match client
            .consumer()
            .with_topic(&topic_path)
            .with_subscription(subscription.clone())
            .with_subscription_type(pulsar::SubType::Shared)
            .with_consumer_name(consumer_name)
            .with_options(options)
            .build()
            .await
        {
            Ok(c) => {
                connect_attempt = 0;
                c
            }
            Err(e) => {
                let _ = tx.send(EventStreamEvent::Error(format!("创建 Consumer 失败: {e}")));
                continue;
            }
        };

        if !seek_done && topic_path.starts_with("persistent://") {
            if let Ok(parts) = client.lookup_partitioned_topic(topic_path.clone()).await {
                if parts.len() == 1 {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    if now_ms > 0 {
                        let seek_ms = now_ms.saturating_sub(20 * 60 * 1000) as u64;
                        let _ = consumer
                            .seek(None, None, Some(seek_ms), client.clone())
                            .await;
                    }
                    seek_done = true;
                } else {
                    tracing::debug!(
                        topic = %topic_path,
                        partitions = parts.len(),
                        "event topic is partitioned; skipping seek"
                    );
                    seek_done = true;
                }
            }
        }

        let mut heartbeat = tokio::time::interval(Duration::from_secs(10));

        loop {
            if *stop.borrow() {
                return;
            }

            tokio::select! {
                changed = stop.changed() => {
                    if changed.is_err() || *stop.borrow() {
                        return;
                    }
                }
                _ = heartbeat.tick() => {
                    tracing::debug!(
                        topic = %topic_path,
                        received_messages,
                        decoded_messages,
                        decode_failures,
                        emitted_rows,
                        consumer_received = consumer.messages_received(),
                        "event topic consumer heartbeat"
                    );
                }
                msg = consumer.next() => {
                    match msg {
                        Some(Ok(message)) => {
                            received_messages += 1;

                            let data = message.deserialize();
                            let (rows, decoded) = parse_event_rows_from_payload(&data, &uid);
                            if decoded {
                                decoded_messages += 1;
                            } else {
                                decode_failures += 1;
                            }
                            if !rows.is_empty() {
                                emitted_rows += rows.len() as u64;
                                let _ = tx.send(EventStreamEvent::Rows(rows));
                            }

                            let _ = consumer.ack(&message).await;

                            if last_stats.elapsed() >= Duration::from_secs(10) {
                                tracing::info!(
                                    topic = %topic_path,
                                    received_messages,
                                    decoded_messages,
                                    decode_failures,
                                    emitted_rows,
                                    "event topic stream stats"
                                );
                                last_stats = Instant::now();
                            }
                        }
                        Some(Err(e)) => {
                            let _ = tx.send(EventStreamEvent::Error(format!("读取消息失败: {e}")));
                            break;
                        }
                        None => {
                            let _ = tx.send(EventStreamEvent::Error("Consumer 数据流意外结束，正在重连…".to_string()));
                            break;
                        }
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TABLE_COLUMN_MIN_WIDTH, TableColumnWidths, find_topic_service_url,
        normalize_pulsar_service_url, parse_event_rows_from_payload, parse_prop_rows_from_payload,
        topic_display_name, topic_paths_by_kind,
    };
    use crate::connection::{ConfigItem, TopicAgentItem, TopicDetail};
    use crate::proto::iothub::{
        AnyValue, ClockTime, DataFrame, DataHeader, DataRecord, DataRecordSet, EnumValue,
        EventRecord, EventRecordList, HiClockTime, any_value, data_record, enum_value,
    };
    use prost::Message as _;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicU64;

    #[test]
    fn topic_display_name_prop_data_rules() {
        assert_eq!(
            topic_display_name(
                "persistent://goldwind/iothub/prop_data-BZ-GRID-realdev-Guarantee-626221420272574464"
            ),
            "GRID_Guarantee"
        );
        assert_eq!(
            topic_display_name(
                "non-persistent://goldwind/iothub/prop_data-BZ-FAST-realdev-Guarantee-626221420272574464"
            ),
            "FAST_Guarantee"
        );
        assert_eq!(
            topic_display_name(
                "persistent://goldwind/iothub/prop_data-BZ-GRID_SECTION-realdev-60-626221420272574464"
            ),
            "GRID_SECTION_60"
        );
        assert_eq!(
            topic_display_name(
                "persistent://goldwind/iothub/prop_data-BZ-GRID_SECTION-realdev-WindPower-626221420272574464"
            ),
            "GRID_SECTION_WindPower"
        );
    }

    #[test]
    fn topic_display_name_service_rule() {
        assert_eq!(
            topic_display_name(
                "persistent://goldwind/iothub/thing_service-BZ-RESPONSE-626221420272574464,persistent://goldwind/iothub/thing_service-BZ-REQUEST-626221420272574464"
            ),
            "service"
        );
    }

    #[test]
    fn topic_display_name_fallback_is_stable_and_not_topic_n() {
        assert_eq!(
            topic_display_name("persistent://goldwind/iothub/thing_event-BZ-626221420272574464"),
            "thing_event-BZ"
        );
    }

    #[test]
    fn normalize_pulsar_service_url_rejects_redis_key_fallback() {
        assert_eq!(
            normalize_pulsar_service_url("pulsar://CMC/{DCC0001}/sg.og.output.iothub"),
            None
        );
    }

    #[test]
    fn normalize_pulsar_service_url_preserves_multi_endpoint_input() {
        assert_eq!(
            normalize_pulsar_service_url("10.10.4.101:6650,10.10.4.102:6650"),
            Some("10.10.4.101:6650,10.10.4.102:6650".to_string())
        );
    }

    #[test]
    fn find_topic_service_url_falls_back_to_sibling_cfgid_config() {
        let topic_path = "persistent://goldwind/iothub/thing_event-BZ-650412879542296576";
        let configs = vec![
            ConfigItem {
                group_id: 1,
                service_url: "pulsar://CMC/{DCC0001}/sg.og.output.iothub".to_string(),
                source: "CMC_{DCC0001}_sg.og.output.iothub".to_string(),
                details: Vec::new(),
                topic_agents: vec![TopicAgentItem {
                    agent_id: "650412879542296576".to_string(),
                    topics: vec![TopicDetail {
                        index: 0,
                        path: topic_path.to_string(),
                        visibility: true,
                        topic_type: "event".to_string(),
                    }],
                    group_id: 1,
                }],
            },
            ConfigItem {
                group_id: 2,
                service_url: "10.10.4.101:6650,10.10.4.102:6650".to_string(),
                source: "CMC_{DCC0001}_sg.bus".to_string(),
                details: Vec::new(),
                topic_agents: Vec::new(),
            },
        ];

        assert_eq!(
            find_topic_service_url(&configs, topic_path),
            Some("10.10.4.101:6650,10.10.4.102:6650".to_string())
        );
    }

    #[test]
    fn topic_paths_by_kind_splits_supported_topics() {
        let agents = vec![TopicAgentItem {
            agent_id: "agent-1".to_string(),
            topics: vec![
                TopicDetail {
                    index: 0,
                    path: "persistent://goldwind/iothub/prop_data-BZ-GRID-realdev-Guarantee-1"
                        .to_string(),
                    visibility: true,
                    topic_type: "prop".to_string(),
                },
                TopicDetail {
                    index: 1,
                    path: "persistent://goldwind/iothub/thing_event-BZ-1".to_string(),
                    visibility: true,
                    topic_type: "event".to_string(),
                },
                TopicDetail {
                    index: 2,
                    path: "persistent://goldwind/iothub/thing_service-BZ-RESPONSE-1,persistent://goldwind/iothub/thing_service-BZ-REQUEST-1".to_string(),
                    visibility: true,
                    topic_type: "service".to_string(),
                },
            ],
            group_id: 1,
        }];

        let (prop_topics, event_topics, service_topics) = topic_paths_by_kind(agents.iter());

        assert_eq!(prop_topics.len(), 1);
        assert_eq!(event_topics.len(), 1);
        assert_eq!(service_topics.len(), 1);
    }

    #[test]
    fn parse_prop_rows_uses_dfc_data_frame_and_imid_mapping() {
        let frame = DataFrame {
            frame: vec![DataRecordSet {
                header: Some(DataHeader {
                    im_global_uuid: "705537041061273601".to_string(),
                    series_type: "Guarantee".to_string(),
                    window_size: 0,
                    source_device: "100852277".to_string(),
                    t: Some(ClockTime {
                        t: 1_711_111_112,
                        zone_info: 0,
                    }),
                    nano_second: 0,
                    extends_data: Default::default(),
                }),
                data: vec![DataRecord {
                    k: Some(data_record::K::Im2id(1)),
                    v: Some(AnyValue {
                        v: Some(any_value::V::BoolV(false)),
                    }),
                    q: 0,
                    bcr_uuid: "bcr-1".to_string(),
                    device_time: Some(ClockTime {
                        t: 1_711_111_111,
                        zone_info: 0,
                    }),
                }],
            }],
        };

        let mut proto = Vec::new();
        frame.encode(&mut proto).expect("encode test data frame");

        let summary = b"per";
        let mut payload = vec![0x20, 0x02, summary.len() as u8];
        payload.extend_from_slice(summary);
        payload.extend_from_slice(&proto);

        let uid = AtomicU64::new(1);
        let imid2imr = HashMap::from([(
            ("705537041061273601".to_string(), 1),
            "Turbine/WTUR/State/DataAvailable".to_string(),
        )]);
        let (rows, decoded) = parse_prop_rows_from_payload(&payload, &imid2imr, &uid);

        assert!(decoded);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].global_uuid, "705537041061273601");
        assert_eq!(rows[0].device, "100852277");
        assert_eq!(rows[0].imid, 1);
        assert_eq!(rows[0].imr, "Turbine/WTUR/State/DataAvailable");
        assert_eq!(rows[0].value, "false");
        assert_eq!(rows[0].quality, 0);
        assert_eq!(rows[0].bcrid, "bcr-1");
        assert_eq!(rows[0].summary, "per");
    }

    #[test]
    fn parse_event_rows_uses_dfc_event_record_list_frame() {
        let list = EventRecordList {
            event_array: vec![EventRecord {
                evt_uuid: "evt-1".to_string(),
                r#type: "状态变化".to_string(),
                tags: vec!["tag-a".to_string(), "tag-b".to_string()],
                src: "100852277".to_string(),
                im_global_uuid: "705537041061273601".to_string(),
                imr: "Turbine/WTUR/Event/TurbineFault".to_string(),
                happened_time: Some(HiClockTime {
                    t: 1_711_111_111,
                    nano: 123_000_000,
                    zone_info: 0,
                }),
                record_time: Some(ClockTime {
                    t: 1_711_111_112,
                    zone_info: 0,
                }),
                level: 2,
                code: vec![
                    EnumValue {
                        v: Some(enum_value::V::Uint64V(42)),
                    },
                    EnumValue {
                        v: Some(enum_value::V::StringV("KKS-A".to_string())),
                    },
                ],
                dict_name: "dict".to_string(),
                bcr_uuid: "bcr-1".to_string(),
                context: Default::default(),
            }],
        };

        let mut proto = Vec::new();
        list.encode(&mut proto).expect("encode test event list");

        let summary = b"per";
        let mut payload = vec![0x20, 0x02, summary.len() as u8];
        payload.extend_from_slice(summary);
        payload.extend_from_slice(&proto);

        let uid = AtomicU64::new(1);
        let (rows, decoded) = parse_event_rows_from_payload(&payload, &uid);

        assert!(decoded);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].uuid, "evt-1");
        assert_eq!(rows[0].device, "100852277");
        assert_eq!(rows[0].event_type, "状态变化");
        assert_eq!(rows[0].codes, "42");
        assert_eq!(rows[0].str_codes, "KKS-A");
        assert_eq!(rows[0].summary, "per");
    }

    #[test]
    fn table_column_widths_resize_updates_total_width() {
        let mut widths = TableColumnWidths::new([180.0, 120.0, 90.0]);
        widths.resize_from_drag(1, 120.0, 35.0);

        assert_eq!(widths.get(1), 155.0);
        assert_eq!(widths.total(), 425.0);
    }

    #[test]
    fn table_column_widths_resize_clamps_to_minimum() {
        let mut widths = TableColumnWidths::new([180.0, 120.0]);
        widths.resize_from_drag(0, 180.0, -500.0);

        assert_eq!(widths.get(0), TABLE_COLUMN_MIN_WIDTH);
    }
}

impl Render for ConfigView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .on_action(cx.listener(|this, mode: &AgentQueryMode, _window, cx| {
                this.agent_query_mode = *mode;
                this.clear_selected_agent_if_filtered_out(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, size: &PropPageSize, _window, cx| {
                let page_size = size.value();
                this.prop_table_state.update(cx, |state, cx| {
                    state.set_page_size(page_size);
                    cx.notify();
                });
                this.event_table_state.update(cx, |state, cx| {
                    state.set_page_size(page_size);
                    cx.notify();
                });
            }))
            .child(self.render_content(window, cx))
    }
}
