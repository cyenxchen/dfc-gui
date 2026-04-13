//! Event topic table state
//!
//! Holds parsed rows from iothub `thing_event` topics and UI pagination state.

use std::collections::VecDeque;
use std::sync::Arc;

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

pub struct EventTableState {
    topic_path: Option<String>,
    rows: VecDeque<EventRow>,
    load_state: EventTableLoadState,
    page_size: usize,
    page_index: usize,
    max_rows: usize,
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

    pub fn page_rows_owned(&self) -> Vec<EventRow> {
        let (start, end) = self.page_range();
        if start == end {
            return Vec::new();
        }

        self.rows
            .iter()
            .skip(start)
            .take(end.saturating_sub(start))
            .cloned()
            .collect()
    }
}

impl Default for EventTableState {
    fn default() -> Self {
        Self::new()
    }
}
