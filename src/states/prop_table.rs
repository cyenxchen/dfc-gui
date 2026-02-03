//! Prop topic table state
//!
//! Holds parsed rows from iothub `prop_data` topics and UI pagination state.

use std::collections::VecDeque;
use std::sync::Arc;

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

    pub fn page_rows(&self) -> impl Iterator<Item = &PropRow> {
        let (start, end) = self.page_range();
        self.rows.iter().skip(start).take(end.saturating_sub(start))
    }
}

impl Default for PropTableState {
    fn default() -> Self {
        Self::new()
    }
}
