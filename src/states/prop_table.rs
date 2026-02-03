//! Prop topic table state
//!
//! Holds parsed rows from iothub `prop_data` topics and UI pagination state.

use std::collections::VecDeque;
use std::sync::Arc;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropSortColumn {
    GlobalUuid,
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
    sorted_indices: Vec<usize>,
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
            sorted_indices: Vec::new(),
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

    pub fn page_size(&self) -> usize {
        self.page_size
    }

    pub fn page_index(&self) -> usize {
        self.page_index
    }

    pub fn sort(&self) -> Option<PropSort> {
        self.sort
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
        self.rebuild_sorted_indices();
    }

    pub fn total_pages(&self) -> usize {
        if self.page_size == 0 {
            return 1;
        }
        let total = self.rows.len();
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
        self.sorted_indices.clear();
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

        if self.sort.is_some() {
            self.rebuild_sorted_indices();
        }

        // If user is on a later page, keep their position bounded.
        self.page_index = self.page_index.min(self.total_pages().saturating_sub(1));
        self.mark_ready();
    }

    pub fn page_range(&self) -> (usize, usize) {
        let total = self.rows.len();
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
        if start == end {
            return Vec::new();
        }

        if self.sort.is_none() {
            return self
                .rows
                .iter()
                .skip(start)
                .take(end.saturating_sub(start))
                .cloned()
                .collect();
        }

        let indices = &self.sorted_indices;
        if indices.len() != self.rows.len() {
            // Should not happen (we rebuild on mutations), but fallback safely.
            return self
                .rows
                .iter()
                .skip(start)
                .take(end.saturating_sub(start))
                .cloned()
                .collect();
        }

        indices
            .iter()
            .skip(start)
            .take(end.saturating_sub(start))
            .filter_map(|&idx| self.rows.get(idx).cloned())
            .collect()
    }

    fn rebuild_sorted_indices(&mut self) {
        self.sorted_indices.clear();

        let Some(sort) = self.sort else {
            return;
        };

        self.sorted_indices.extend(0..self.rows.len());

        let direction = sort.direction;
        let column = sort.column;

        self.sorted_indices.sort_by(|&ia, &ib| {
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

fn cmp_u64ish(a: &str, b: &str) -> Ordering {
    let pa = a.trim().parse::<u64>();
    let pb = b.trim().parse::<u64>();
    match (pa, pb) {
        (Ok(va), Ok(vb)) => va.cmp(&vb),
        _ => a.cmp(b),
    }
}
