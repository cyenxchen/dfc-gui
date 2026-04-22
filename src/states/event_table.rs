//! Event topic table state
//!
//! Holds parsed rows from iothub `thing_event` topics and UI pagination state.

use std::cmp::Ordering;
use std::collections::VecDeque;
use std::sync::Arc;

use super::prop_table::SortDirection;
use crate::helpers::cmp_u64ish;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum EventSortColumn {
    Uuid = 0,
    Device,
    Imr,
    EventType,
    Level,
    Tags,
    Codes,
    StrCodes,
    HappenedTime,
    RecordTime,
    BcrId,
    Context,
    Summary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventSort {
    pub column: EventSortColumn,
    pub direction: SortDirection,
}

/// Per-column substring filters for the event table (case-insensitive contains).
#[derive(Clone, Debug, Default)]
pub struct EventFilters {
    pub uuid: String,
    pub device: String,
    pub imr: String,
    pub event_type: String,
    pub level: String,
    pub tags: String,
    pub codes: String,
    pub str_codes: String,
    pub happened_time: String,
    pub record_time: String,
    pub bcr_id: String,
    pub context: String,
    pub summary: String,
}

impl EventFilters {
    pub fn is_empty(&self) -> bool {
        self.uuid.is_empty()
            && self.device.is_empty()
            && self.imr.is_empty()
            && self.event_type.is_empty()
            && self.level.is_empty()
            && self.tags.is_empty()
            && self.codes.is_empty()
            && self.str_codes.is_empty()
            && self.happened_time.is_empty()
            && self.record_time.is_empty()
            && self.bcr_id.is_empty()
            && self.context.is_empty()
            && self.summary.is_empty()
    }

    pub fn get(&self, col: EventSortColumn) -> &str {
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

    pub fn set(&mut self, col: EventSortColumn, value: String) {
        match col {
            EventSortColumn::Uuid => self.uuid = value,
            EventSortColumn::Device => self.device = value,
            EventSortColumn::Imr => self.imr = value,
            EventSortColumn::EventType => self.event_type = value,
            EventSortColumn::Level => self.level = value,
            EventSortColumn::Tags => self.tags = value,
            EventSortColumn::Codes => self.codes = value,
            EventSortColumn::StrCodes => self.str_codes = value,
            EventSortColumn::HappenedTime => self.happened_time = value,
            EventSortColumn::RecordTime => self.record_time = value,
            EventSortColumn::BcrId => self.bcr_id = value,
            EventSortColumn::Context => self.context = value,
            EventSortColumn::Summary => self.summary = value,
        }
    }

    /// Pre-lowercase non-empty needles for the hot path. Returns one slot per column.
    fn lowered_needles(&self) -> [Option<String>; 13] {
        [
            opt_lower(&self.uuid),
            opt_lower(&self.device),
            opt_lower(&self.imr),
            opt_lower(&self.event_type),
            opt_lower(&self.level),
            opt_lower(&self.tags),
            opt_lower(&self.codes),
            opt_lower(&self.str_codes),
            opt_lower(&self.happened_time),
            opt_lower(&self.record_time),
            opt_lower(&self.bcr_id),
            opt_lower(&self.context),
            opt_lower(&self.summary),
        ]
    }
}

fn opt_lower(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_lowercase())
    }
}

fn matches_lowered(haystack: &str, lowered_needle: &str) -> bool {
    haystack.to_lowercase().contains(lowered_needle)
}

#[derive(Clone, Debug)]
pub struct EventRow {
    pub uid: u64,
    pub uuid: String,
    pub device: String,
    pub imr: String,
    pub event_type: String,
    pub level: String,
    pub tags: String,
    pub codes: String,
    pub str_codes: String,
    pub happened_time: String,
    pub record_time: String,
    pub bcr_id: String,
    pub context: String,
    pub summary: String,
}

#[derive(Clone, Debug, Default)]
pub enum EventTableLoadState {
    #[default]
    Idle,
    Loading,
    Ready,
    Error(Arc<str>),
}

#[derive(Clone)]
pub struct EventTableState {
    topic_path: Option<String>,
    rows: VecDeque<EventRow>,
    load_state: EventTableLoadState,
    page_size: usize,
    page_index: usize,
    max_rows: usize,
    sort: Option<EventSort>,
    filters: EventFilters,
    visible_indices: Vec<usize>,
}

impl EventTableState {
    pub fn new() -> Self {
        Self {
            topic_path: None,
            rows: VecDeque::new(),
            load_state: EventTableLoadState::Idle,
            page_size: 20,
            page_index: 0,
            max_rows: 20 * 10_000,
            sort: None,
            filters: EventFilters::default(),
            visible_indices: Vec::new(),
        }
    }

    pub fn topic_path(&self) -> Option<&str> {
        self.topic_path.as_deref()
    }

    pub fn load_state(&self) -> &EventTableLoadState {
        &self.load_state
    }

    pub fn rows_len(&self) -> usize {
        self.rows.len()
    }

    pub fn visible_len(&self) -> usize {
        if self.is_identity_view() {
            self.rows.len()
        } else {
            self.visible_indices.len()
        }
    }

    pub fn page_size(&self) -> usize {
        self.page_size
    }

    pub fn page_index(&self) -> usize {
        self.page_index
    }

    pub fn sort(&self) -> Option<EventSort> {
        self.sort
    }

    pub fn filters(&self) -> &EventFilters {
        &self.filters
    }

    pub fn has_active_filters(&self) -> bool {
        !self.filters.is_empty()
    }

    fn is_identity_view(&self) -> bool {
        self.sort.is_none() && self.filters.is_empty()
    }

    pub fn toggle_sort(&mut self, column: EventSortColumn) {
        self.sort = match self.sort {
            None => Some(EventSort {
                column,
                direction: SortDirection::Asc,
            }),
            Some(current) if current.column != column => Some(EventSort {
                column,
                direction: SortDirection::Asc,
            }),
            Some(current) => match current.direction {
                SortDirection::Asc => Some(EventSort {
                    column,
                    direction: SortDirection::Desc,
                }),
                SortDirection::Desc => None,
            },
        };

        self.page_index = 0;
        self.rebuild_visible_indices();
    }

    pub fn set_filter(&mut self, column: EventSortColumn, value: String) {
        if self.filters.get(column) == value {
            return;
        }
        self.filters.set(column, value);
        self.page_index = 0;
        self.rebuild_visible_indices();
    }

    pub fn clear_filters(&mut self) {
        if self.filters.is_empty() {
            return;
        }
        self.filters = EventFilters::default();
        self.page_index = 0;
        self.rebuild_visible_indices();
    }

    pub fn total_pages(&self) -> usize {
        if self.page_size == 0 {
            return 1;
        }
        let total = self.visible_len();
        let pages = (total + self.page_size - 1) / self.page_size;
        pages.max(1)
    }

    pub fn set_page_size(&mut self, size: usize) {
        self.page_size = size.max(1);
        self.page_index = 0;
    }

    pub fn set_page_index(&mut self, index: usize) {
        self.page_index = index.min(self.total_pages().saturating_sub(1));
    }

    pub fn reset_for_topic(&mut self, topic_path: Option<String>) {
        self.topic_path = topic_path;
        self.rows.clear();
        self.page_index = 0;
        self.sort = None;
        self.filters = EventFilters::default();
        self.visible_indices.clear();
        self.load_state = if self.topic_path.is_some() {
            EventTableLoadState::Loading
        } else {
            EventTableLoadState::Idle
        };
    }

    pub fn mark_loading_for_topic(&mut self, topic_path: Option<String>) {
        self.topic_path = topic_path;
        self.load_state = if self.topic_path.is_some() {
            EventTableLoadState::Loading
        } else {
            EventTableLoadState::Idle
        };
    }

    /// Clear cached rows before a reconnect-driven reload while preserving the
    /// current topic binding and user-visible filters/sort settings.
    pub fn prepare_for_reload(&mut self) {
        self.rows.clear();
        self.page_index = 0;
        self.visible_indices.clear();
        self.load_state = if self.topic_path.is_some() {
            EventTableLoadState::Loading
        } else {
            EventTableLoadState::Idle
        };
    }

    pub fn set_error(&mut self, message: impl Into<Arc<str>>) {
        self.load_state = EventTableLoadState::Error(message.into());
    }

    pub fn mark_ready(&mut self) {
        if self.topic_path.is_some() {
            self.load_state = EventTableLoadState::Ready;
        }
    }

    pub fn push_rows_front(&mut self, mut batch: Vec<EventRow>) {
        if batch.is_empty() {
            return;
        }

        while let Some(row) = batch.pop() {
            self.rows.push_front(row);
        }

        while self.rows.len() > self.max_rows {
            self.rows.pop_back();
        }

        self.rebuild_visible_indices();

        self.page_index = self.page_index.min(self.total_pages().saturating_sub(1));
        self.mark_ready();
    }

    pub fn page_range(&self) -> (usize, usize) {
        let total = self.visible_len();
        if total == 0 || self.page_size == 0 {
            return (0, 0);
        }

        let start = self.page_index * self.page_size;
        if start >= total {
            return (0, 0);
        }
        let end = (start + self.page_size).min(total);
        (start, end)
    }

    pub fn page_rows_owned(&self) -> Vec<EventRow> {
        let (start, end) = self.page_range();
        let count = end.saturating_sub(start);
        if count == 0 {
            return Vec::new();
        }

        if self.is_identity_view() {
            return self.rows.iter().skip(start).take(count).cloned().collect();
        }

        self.visible_indices
            .iter()
            .skip(start)
            .take(count)
            .filter_map(|&idx| self.rows.get(idx).cloned())
            .collect()
    }

    fn rebuild_visible_indices(&mut self) {
        self.visible_indices.clear();

        // Identity view (no sort, no filters) bypasses visible_indices entirely
        // — page_rows_owned reads from `rows` directly.
        if self.is_identity_view() {
            return;
        }

        if self.filters.is_empty() {
            self.visible_indices.extend(0..self.rows.len());
        } else {
            let needles = self.filters.lowered_needles();
            for (i, row) in self.rows.iter().enumerate() {
                if row_matches_lowered(row, &needles) {
                    self.visible_indices.push(i);
                }
            }
        }

        let Some(sort) = self.sort else {
            return;
        };

        let direction = sort.direction;
        let column = sort.column;

        self.visible_indices.sort_by(|&ia, &ib| {
            let a = match self.rows.get(ia) {
                Some(v) => v,
                None => return Ordering::Equal,
            };
            let b = match self.rows.get(ib) {
                Some(v) => v,
                None => return Ordering::Equal,
            };

            let ord = compare_event_rows(a, b, column);
            let ord = match direction {
                SortDirection::Asc => ord,
                SortDirection::Desc => ord.reverse(),
            };

            // Deterministic tie-breaker: newer (larger uid) first.
            if ord == Ordering::Equal {
                b.uid.cmp(&a.uid)
            } else {
                ord
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_row(uid: u64, happened_time: &str) -> EventRow {
        EventRow {
            uid,
            uuid: format!("uuid-{uid}"),
            device: "100852277".to_string(),
            imr: "Turbine/EVENT/Grid".to_string(),
            event_type: "event".to_string(),
            level: "info".to_string(),
            tags: String::new(),
            codes: String::new(),
            str_codes: String::new(),
            happened_time: happened_time.to_string(),
            record_time: "2026-04-14 11:33:03.000".to_string(),
            bcr_id: String::new(),
            context: String::new(),
            summary: String::new(),
        }
    }

    #[test]
    fn time_filters_match_whole_day_prefix() {
        let mut state = EventTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));

        state.push_rows_front(vec![
            event_row(1, "2026-04-14 00:00:01.000"),
            event_row(2, "2026-04-15 00:00:01.000"),
        ]);
        state.set_filter(EventSortColumn::HappenedTime, "2026-04-14".to_string());

        let rows = state.page_rows_owned();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].happened_time, "2026-04-14 00:00:01.000");
    }

    #[test]
    fn mark_loading_for_topic_preserves_existing_rows_and_filters() {
        let mut state = EventTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));
        state.push_rows_front(vec![event_row(1, "2026-04-14 00:00:01.000")]);
        state.set_filter(EventSortColumn::Device, "dev".to_string());

        state.mark_loading_for_topic(Some("persistent://topic".to_string()));

        assert!(matches!(state.load_state(), EventTableLoadState::Loading));
        assert_eq!(state.rows_len(), 1);
        assert_eq!(state.filters().device, "dev");
    }

    #[test]
    fn prepare_for_reload_clears_rows_but_keeps_filters() {
        let mut state = EventTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));
        state.push_rows_front(vec![event_row(1, "2026-04-14 00:00:01.000")]);
        state.set_filter(EventSortColumn::Device, "dev".to_string());

        state.prepare_for_reload();

        assert!(matches!(state.load_state(), EventTableLoadState::Loading));
        assert_eq!(state.topic_path(), Some("persistent://topic"));
        assert_eq!(state.rows_len(), 0);
        assert_eq!(state.filters().device, "dev");
    }
}

fn row_matches_lowered(row: &EventRow, needles: &[Option<String>; 13]) -> bool {
    if let Some(n) = &needles[0] {
        if !matches_lowered(&row.uuid, n) {
            return false;
        }
    }
    if let Some(n) = &needles[1] {
        if !matches_lowered(&row.device, n) {
            return false;
        }
    }
    if let Some(n) = &needles[2] {
        if !matches_lowered(&row.imr, n) {
            return false;
        }
    }
    if let Some(n) = &needles[3] {
        if !matches_lowered(&row.event_type, n) {
            return false;
        }
    }
    if let Some(n) = &needles[4] {
        if !matches_lowered(&row.level, n) {
            return false;
        }
    }
    if let Some(n) = &needles[5] {
        if !matches_lowered(&row.tags, n) {
            return false;
        }
    }
    if let Some(n) = &needles[6] {
        if !matches_lowered(&row.codes, n) {
            return false;
        }
    }
    if let Some(n) = &needles[7] {
        if !matches_lowered(&row.str_codes, n) {
            return false;
        }
    }
    if let Some(n) = &needles[8] {
        if !matches_day_filter(&row.happened_time, n) {
            return false;
        }
    }
    if let Some(n) = &needles[9] {
        if !matches_day_filter(&row.record_time, n) {
            return false;
        }
    }
    if let Some(n) = &needles[10] {
        if !matches_lowered(&row.bcr_id, n) {
            return false;
        }
    }
    if let Some(n) = &needles[11] {
        if !matches_lowered(&row.context, n) {
            return false;
        }
    }
    if let Some(n) = &needles[12] {
        if !matches_lowered(&row.summary, n) {
            return false;
        }
    }
    true
}

fn matches_day_filter(timestamp: &str, day: &str) -> bool {
    timestamp.starts_with(day)
}

impl Default for EventTableState {
    fn default() -> Self {
        Self::new()
    }
}

fn compare_event_rows(a: &EventRow, b: &EventRow, column: EventSortColumn) -> Ordering {
    match column {
        EventSortColumn::Uuid => cmp_u64ish(&a.uuid, &b.uuid),
        EventSortColumn::Device => cmp_u64ish(&a.device, &b.device),
        EventSortColumn::Imr => a.imr.cmp(&b.imr),
        EventSortColumn::EventType => a.event_type.cmp(&b.event_type),
        EventSortColumn::Level => a.level.cmp(&b.level),
        EventSortColumn::Tags => a.tags.cmp(&b.tags),
        EventSortColumn::Codes => a.codes.cmp(&b.codes),
        EventSortColumn::StrCodes => a.str_codes.cmp(&b.str_codes),
        EventSortColumn::HappenedTime => a.happened_time.cmp(&b.happened_time),
        EventSortColumn::RecordTime => a.record_time.cmp(&b.record_time),
        EventSortColumn::BcrId => a.bcr_id.cmp(&b.bcr_id),
        EventSortColumn::Context => a.context.cmp(&b.context),
        EventSortColumn::Summary => a.summary.cmp(&b.summary),
    }
}
