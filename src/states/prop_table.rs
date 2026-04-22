//! Prop topic table state
//!
//! Holds parsed rows from iothub `prop_data` topics and UI pagination state.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use crate::helpers::cmp_u64ish;
use hashlink::LinkedHashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum PropSortColumn {
    GlobalUuid = 0,
    Device,
    Imr,
    Imid,
    Value,
    Quality,
    Bcrid,
    Time,
    MessageTime,
    Summary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropSort {
    pub column: PropSortColumn,
    pub direction: SortDirection,
}

/// Per-column substring filters for the prop table (case-insensitive contains).
#[derive(Clone, Debug, Default)]
pub struct PropFilters {
    pub global_uuid: String,
    pub device: String,
    pub imr: String,
    pub imid: String,
    pub value: String,
    pub quality: String,
    pub bcrid: String,
    pub time: String,
    pub message_time: String,
    pub summary: String,
}

impl PropFilters {
    pub fn is_empty(&self) -> bool {
        self.global_uuid.is_empty()
            && self.device.is_empty()
            && self.imr.is_empty()
            && self.imid.is_empty()
            && self.value.is_empty()
            && self.quality.is_empty()
            && self.bcrid.is_empty()
            && self.time.is_empty()
            && self.message_time.is_empty()
            && self.summary.is_empty()
    }

    pub fn get(&self, col: PropSortColumn) -> &str {
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

    pub fn set(&mut self, col: PropSortColumn, value: String) {
        match col {
            PropSortColumn::GlobalUuid => self.global_uuid = value,
            PropSortColumn::Device => self.device = value,
            PropSortColumn::Imr => self.imr = value,
            PropSortColumn::Imid => self.imid = value,
            PropSortColumn::Value => self.value = value,
            PropSortColumn::Quality => self.quality = value,
            PropSortColumn::Bcrid => self.bcrid = value,
            PropSortColumn::Time => self.time = value,
            PropSortColumn::MessageTime => self.message_time = value,
            PropSortColumn::Summary => self.summary = value,
        }
    }

    /// Pre-lowercase non-empty needles for the hot path. Returns one slot per column.
    fn lowered_needles(&self) -> [Option<String>; 10] {
        [
            opt_lower(&self.global_uuid),
            opt_lower(&self.device),
            opt_lower(&self.imr),
            opt_lower(&self.imid),
            opt_lower(&self.value),
            opt_lower(&self.quality),
            opt_lower(&self.bcrid),
            opt_lower(&self.time),
            opt_lower(&self.message_time),
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

/// A single row in the property data table (aligned with DFC PropHistory columns).
#[derive(Clone, Debug)]
pub struct PropRow {
    pub uid: u64,
    pub global_uuid: String,
    pub device: String,
    pub imr: String,
    pub imid: i32,
    pub value: String,
    pub quality: i32,
    pub bcrid: String,
    pub time: String,
    pub message_time: String,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PropPointKey {
    global_uuid: String,
    device: String,
    imr: String,
    imid: i32,
}

impl From<&PropRow> for PropPointKey {
    fn from(row: &PropRow) -> Self {
        Self {
            global_uuid: row.global_uuid.clone(),
            device: row.device.clone(),
            imr: row.imr.clone(),
            imid: row.imid,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum PropTableLoadState {
    #[default]
    Idle,
    Loading,
    Ready,
    Error(Arc<str>),
}

/// State for rendering prop topic data as a paginated table.
#[derive(Clone)]
pub struct PropTableState {
    topic_path: Option<String>,
    rows: LinkedHashMap<PropPointKey, PropRow>,
    load_state: PropTableLoadState,
    page_size: usize,
    page_index: usize,
    max_rows: usize,
    sort: Option<PropSort>,
    filters: PropFilters,
    /// Keys into `rows` after filtering and sorting, in display order.
    visible_keys: Vec<PropPointKey>,
}

impl PropTableState {
    pub fn new() -> Self {
        Self {
            topic_path: None,
            rows: LinkedHashMap::new(),
            load_state: PropTableLoadState::Idle,
            page_size: 20,
            page_index: 0,
            max_rows: 20 * 10_000,
            sort: None,
            filters: PropFilters::default(),
            visible_keys: Vec::new(),
        }
    }

    pub fn topic_path(&self) -> Option<&str> {
        self.topic_path.as_deref()
    }

    pub fn load_state(&self) -> &PropTableLoadState {
        &self.load_state
    }

    pub fn rows_len(&self) -> usize {
        self.rows.len()
    }

    pub fn visible_len(&self) -> usize {
        if self.is_identity_view() {
            self.rows.len()
        } else {
            self.visible_keys.len()
        }
    }

    pub fn page_size(&self) -> usize {
        self.page_size
    }

    pub fn page_index(&self) -> usize {
        self.page_index
    }

    pub fn sort(&self) -> Option<PropSort> {
        self.sort
    }

    pub fn filters(&self) -> &PropFilters {
        &self.filters
    }

    pub fn has_active_filters(&self) -> bool {
        !self.filters.is_empty()
    }

    fn is_identity_view(&self) -> bool {
        self.sort.is_none() && self.filters.is_empty()
    }

    pub fn toggle_sort(&mut self, column: PropSortColumn) {
        self.sort = match self.sort {
            None => Some(PropSort {
                column,
                direction: SortDirection::Asc,
            }),
            Some(current) if current.column != column => Some(PropSort {
                column,
                direction: SortDirection::Asc,
            }),
            Some(current) => match current.direction {
                SortDirection::Asc => Some(PropSort {
                    column,
                    direction: SortDirection::Desc,
                }),
                SortDirection::Desc => None,
            },
        };

        self.page_index = 0;
        self.rebuild_visible_indices();
    }

    pub fn set_filter(&mut self, column: PropSortColumn, value: String) {
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
        self.filters = PropFilters::default();
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
        self.filters = PropFilters::default();
        self.visible_keys.clear();
        self.load_state = if self.topic_path.is_some() {
            PropTableLoadState::Loading
        } else {
            PropTableLoadState::Idle
        };
    }

    pub fn mark_loading_for_topic(&mut self, topic_path: Option<String>) {
        self.topic_path = topic_path;
        self.load_state = if self.topic_path.is_some() {
            PropTableLoadState::Loading
        } else {
            PropTableLoadState::Idle
        };
    }

    /// Clear cached rows before a reconnect-driven reload while preserving the
    /// current topic binding and user-visible filters/sort settings.
    pub fn prepare_for_reload(&mut self) {
        self.rows.clear();
        self.page_index = 0;
        self.visible_keys.clear();
        self.load_state = if self.topic_path.is_some() {
            PropTableLoadState::Loading
        } else {
            PropTableLoadState::Idle
        };
    }

    pub fn set_error(&mut self, message: impl Into<Arc<str>>) {
        self.load_state = PropTableLoadState::Error(message.into());
    }

    pub fn mark_ready(&mut self) {
        if self.topic_path.is_some() {
            self.load_state = PropTableLoadState::Ready;
        }
    }

    pub fn push_rows_front(&mut self, mut batch: Vec<PropRow>) {
        if batch.is_empty() {
            return;
        }

        let mut replaced_rows = 0usize;
        let mut incoming_positions: HashMap<PropPointKey, usize> =
            HashMap::with_capacity(batch.len());
        let mut replacement_batch = Vec::with_capacity(batch.len());
        for row in batch.drain(..) {
            let key = PropPointKey::from(&row);
            if let Some(&incoming_idx) = incoming_positions.get(&key) {
                replacement_batch[incoming_idx] = row;
                continue;
            }

            incoming_positions.insert(key, replacement_batch.len());
            replacement_batch.push(row);
        }

        if replacement_batch.is_empty() {
            self.mark_ready();
            return;
        }

        // Keep per-message ordering stable: the first element in `batch` remains
        // before later ones after pushing to the front.
        for row in replacement_batch.into_iter().rev() {
            let key = PropPointKey::from(&row);
            if self.rows.replace(key.clone(), row).is_some() {
                replaced_rows += 1;
            }
            let _ = self.rows.to_front(&key);
        }

        while self.rows.len() > self.max_rows {
            let _ = self.rows.pop_back();
        }

        if replaced_rows > 0 {
            tracing::debug!(
                replaced_rows,
                "replaced existing prop rows with latest point values"
            );
        }

        self.rebuild_visible_indices();

        // If user is on a later page, keep their position bounded.
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

    pub fn page_rows_owned(&self) -> Vec<PropRow> {
        let (start, end) = self.page_range();
        let count = end.saturating_sub(start);
        if count == 0 {
            return Vec::new();
        }

        if self.is_identity_view() {
            return self
                .rows
                .values()
                .skip(start)
                .take(count)
                .cloned()
                .collect();
        }

        self.visible_keys
            .iter()
            .skip(start)
            .take(count)
            .filter_map(|key| self.rows.get(key).cloned())
            .collect()
    }

    fn rebuild_visible_indices(&mut self) {
        self.visible_keys.clear();

        // Identity view (no sort, no filters) bypasses visible_keys entirely
        // — page_rows_owned reads from `rows` directly.
        if self.is_identity_view() {
            return;
        }

        if self.filters.is_empty() {
            self.visible_keys.extend(self.rows.keys().cloned());
        } else {
            let needles = self.filters.lowered_needles();
            for (key, row) in &self.rows {
                if row_matches_lowered(row, &needles) {
                    self.visible_keys.push(key.clone());
                }
            }
        }

        let Some(sort) = self.sort else {
            return;
        };

        let direction = sort.direction;
        let column = sort.column;

        self.visible_keys.sort_by(|ka, kb| {
            let a = match self.rows.get(ka) {
                Some(v) => v,
                None => return Ordering::Equal,
            };
            let b = match self.rows.get(kb) {
                Some(v) => v,
                None => return Ordering::Equal,
            };

            let ord = compare_prop_rows(a, b, column);
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

    fn prop_row(uid: u64, message_time: &str) -> PropRow {
        PropRow {
            uid,
            global_uuid: "705537041061273601".to_string(),
            device: "100852277".to_string(),
            imr: "Turbine/WTUR/State/DataAvailable".to_string(),
            imid: 1,
            value: "false".to_string(),
            quality: 0,
            bcrid: String::new(),
            time: "2026-04-03 11:04:40.000".to_string(),
            message_time: message_time.to_string(),
            summary: "per".to_string(),
        }
    }

    #[test]
    fn push_rows_front_keeps_only_latest_row_per_point() {
        let mut state = PropTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));

        state.push_rows_front(vec![
            prop_row(1, "2026-04-14 11:33:03.000"),
            prop_row(2, "2026-04-14 11:33:06.000"),
        ]);

        assert_eq!(state.rows_len(), 1);
        assert_eq!(
            state.page_rows_owned()[0].message_time,
            "2026-04-14 11:33:06.000"
        );

        let mut changed = prop_row(3, "2026-04-14 11:33:09.000");
        changed.value = "true".to_string();
        state.push_rows_front(vec![changed]);

        assert_eq!(state.rows_len(), 1);
        assert_eq!(state.page_rows_owned()[0].value, "true");
        assert_eq!(
            state.page_rows_owned()[0].message_time,
            "2026-04-14 11:33:09.000"
        );
    }

    #[test]
    fn reset_for_topic_clears_latest_point_rows() {
        let mut state = PropTableState::new();
        state.reset_for_topic(Some("persistent://topic-a".to_string()));
        state.push_rows_front(vec![prop_row(1, "2026-04-14 11:33:03.000")]);

        state.reset_for_topic(Some("persistent://topic-b".to_string()));
        state.push_rows_front(vec![prop_row(2, "2026-04-14 11:33:06.000")]);

        assert_eq!(state.rows_len(), 1);
        assert_eq!(
            state.page_rows_owned()[0].message_time,
            "2026-04-14 11:33:06.000"
        );
    }

    #[test]
    fn mark_loading_for_topic_preserves_existing_rows_and_filters() {
        let mut state = PropTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));
        state.push_rows_front(vec![prop_row(1, "2026-04-14 11:33:03.000")]);
        state.set_filter(PropSortColumn::Value, "false".to_string());

        state.mark_loading_for_topic(Some("persistent://topic".to_string()));

        assert!(matches!(state.load_state(), PropTableLoadState::Loading));
        assert_eq!(state.rows_len(), 1);
        assert_eq!(state.filters().value, "false");
    }

    #[test]
    fn prepare_for_reload_clears_rows_but_keeps_filters() {
        let mut state = PropTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));
        let row = prop_row(1, "2026-04-14 11:33:03.000");
        state.push_rows_front(vec![row.clone()]);
        state.set_filter(PropSortColumn::Value, "false".to_string());

        state.prepare_for_reload();

        assert!(matches!(state.load_state(), PropTableLoadState::Loading));
        assert_eq!(state.topic_path(), Some("persistent://topic"));
        assert_eq!(state.rows_len(), 0);
        assert_eq!(state.filters().value, "false");

        state.push_rows_front(vec![row]);
        assert_eq!(state.rows_len(), 1);
    }

    #[test]
    fn trimming_old_rows_keeps_latest_point_row() {
        let mut state = PropTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));
        state.max_rows = 1;

        let first = prop_row(1, "2026-04-14 11:33:03.000");
        let mut second = prop_row(2, "2026-04-14 11:33:06.000");
        second.value = "true".to_string();

        state.push_rows_front(vec![first.clone()]);
        state.push_rows_front(vec![second]);
        state.push_rows_front(vec![first]);

        assert_eq!(state.rows_len(), 1);
        assert_eq!(
            state.page_rows_owned()[0].message_time,
            "2026-04-14 11:33:03.000"
        );
    }

    #[test]
    fn push_rows_front_retains_distinct_points() {
        let mut state = PropTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));

        let first = prop_row(1, "2026-04-14 11:33:03.000");
        let mut second = prop_row(2, "2026-04-14 11:33:06.000");
        second.imid = 2;
        second.imr = "Turbine/WTUR/State/Other".to_string();

        state.push_rows_front(vec![first, second]);

        assert_eq!(state.rows_len(), 2);
    }

    #[test]
    fn time_filters_match_whole_day_prefix() {
        let mut state = PropTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));

        let mut first = prop_row(1, "2026-04-14 11:33:03.000");
        first.time = "2026-04-14 00:00:01.000".to_string();
        let mut second = prop_row(2, "2026-04-15 11:33:03.000");
        second.time = "2026-04-15 00:00:01.000".to_string();
        second.imid = 2;
        second.imr = "Turbine/WTUR/State/Other".to_string();

        state.push_rows_front(vec![first, second]);
        state.set_filter(PropSortColumn::Time, "2026-04-14".to_string());

        let rows = state.page_rows_owned();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].time, "2026-04-14 00:00:01.000");
    }
}

fn row_matches_lowered(row: &PropRow, needles: &[Option<String>; 10]) -> bool {
    if let Some(n) = &needles[0] {
        if !matches_lowered(&row.global_uuid, n) {
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
        if !matches_lowered(&row.imid.to_string(), n) {
            return false;
        }
    }
    if let Some(n) = &needles[4] {
        if !matches_lowered(&row.value, n) {
            return false;
        }
    }
    if let Some(n) = &needles[5] {
        if !matches_lowered(&row.quality.to_string(), n) {
            return false;
        }
    }
    if let Some(n) = &needles[6] {
        if !matches_lowered(&row.bcrid, n) {
            return false;
        }
    }
    if let Some(n) = &needles[7] {
        if !matches_day_filter(&row.time, n) {
            return false;
        }
    }
    if let Some(n) = &needles[8] {
        if !matches_day_filter(&row.message_time, n) {
            return false;
        }
    }
    if let Some(n) = &needles[9] {
        if !matches_lowered(&row.summary, n) {
            return false;
        }
    }
    true
}

fn matches_day_filter(timestamp: &str, day: &str) -> bool {
    timestamp.starts_with(day)
}

impl Default for PropTableState {
    fn default() -> Self {
        Self::new()
    }
}

fn compare_prop_rows(a: &PropRow, b: &PropRow, column: PropSortColumn) -> Ordering {
    match column {
        PropSortColumn::GlobalUuid => cmp_u64ish(&a.global_uuid, &b.global_uuid),
        PropSortColumn::Device => cmp_u64ish(&a.device, &b.device),
        PropSortColumn::Imr => a.imr.cmp(&b.imr),
        PropSortColumn::Imid => a.imid.cmp(&b.imid),
        PropSortColumn::Value => a.value.cmp(&b.value),
        PropSortColumn::Quality => a.quality.cmp(&b.quality),
        PropSortColumn::Bcrid => a.bcrid.cmp(&b.bcrid),
        PropSortColumn::Time => a.time.cmp(&b.time),
        PropSortColumn::MessageTime => a.message_time.cmp(&b.message_time),
        PropSortColumn::Summary => a.summary.cmp(&b.summary),
    }
}
