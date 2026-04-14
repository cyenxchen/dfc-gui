//! Prop topic table state
//!
//! Holds parsed rows from iothub `prop_data` topics and UI pagination state.

use std::cmp::Ordering;
use std::collections::VecDeque;
use std::sync::Arc;

use crate::helpers::cmp_u64ish;

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

#[derive(Clone, Debug, Default)]
pub enum PropTableLoadState {
    #[default]
    Idle,
    Loading,
    Ready,
    Error(Arc<str>),
}

/// State for rendering prop topic data as a paginated table.
pub struct PropTableState {
    topic_path: Option<String>,
    rows: VecDeque<PropRow>,
    load_state: PropTableLoadState,
    page_size: usize,
    page_index: usize,
    max_rows: usize,
    sort: Option<PropSort>,
    filters: PropFilters,
    /// Indices into `rows` after filtering and sorting, in display order.
    visible_indices: Vec<usize>,
}

impl PropTableState {
    pub fn new() -> Self {
        Self {
            topic_path: None,
            rows: VecDeque::new(),
            load_state: PropTableLoadState::Idle,
            page_size: 20,
            page_index: 0,
            max_rows: 20 * 10_000,
            sort: None,
            filters: PropFilters::default(),
            visible_indices: Vec::new(),
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
            self.visible_indices.len()
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
        self.visible_indices.clear();
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

        // Keep per-message ordering stable: newest overall should appear first.
        // We push in reverse so the first element in `batch` ends up before later ones.
        while let Some(row) = batch.pop() {
            self.rows.push_front(row);
        }

        while self.rows.len() > self.max_rows {
            self.rows.pop_back();
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
        if !matches_lowered(&row.time, n) {
            return false;
        }
    }
    if let Some(n) = &needles[8] {
        if !matches_lowered(&row.message_time, n) {
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

