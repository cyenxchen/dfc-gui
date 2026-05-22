//! Event topic table state
//!
//! Holds parsed rows from iothub `thing_event` topics and UI pagination state.

use std::sync::{Arc, Mutex};

use super::prop_table::SortDirection;
use crate::helpers::{cmp_u64ish, split_filter_values};
use rusqlite::types::Value;
use rusqlite::{Connection, params, params_from_iter};
use rust_i18n::t;

const EVENT_TABLE_MAX_ROWS: usize = 1_000_000;

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
    Error(EventTableError),
}

#[derive(Clone, Debug)]
pub enum EventTableError {
    Message(Arc<str>),
    StoreUnavailable,
    StoreInit { detail: Arc<str> },
    StoreInsert { detail: Arc<str> },
    StoreClear { detail: Arc<str> },
    StoreCountAll { detail: Arc<str> },
    StoreCountFiltered { detail: Arc<str> },
}

impl EventTableError {
    pub fn localized_message(&self, locale: &str) -> String {
        match self {
            Self::Message(message) => message.to_string(),
            Self::StoreUnavailable => t!("events.event_store_unavailable", locale = locale).into(),
            Self::StoreInit { detail } => {
                format_event_store_error("events.event_store_init_failed", locale, detail)
            }
            Self::StoreInsert { detail } => {
                format_event_store_error("events.event_store_insert_failed", locale, detail)
            }
            Self::StoreClear { detail } => {
                format_event_store_error("events.event_store_clear_failed", locale, detail)
            }
            Self::StoreCountAll { detail } => {
                format_event_store_error("events.event_store_count_failed", locale, detail)
            }
            Self::StoreCountFiltered { detail } => {
                format_event_store_error("events.event_store_filter_count_failed", locale, detail)
            }
        }
    }
}

#[derive(Clone)]
pub struct EventTableState {
    topic_path: Option<String>,
    store: Option<EventTableStore>,
    load_state: EventTableLoadState,
    page_size: usize,
    page_index: usize,
    sort: Option<EventSort>,
    filters: EventFilters,
    total_rows: usize,
    visible_rows: usize,
}

impl EventTableState {
    pub fn new() -> Self {
        let (store, load_state) = match EventTableStore::new() {
            Ok(store) => (Some(store), EventTableLoadState::Idle),
            Err(e) => {
                tracing::error!("failed to create event table store: {e}");
                (
                    None,
                    EventTableLoadState::Error(EventTableError::StoreInit {
                        detail: Arc::from(e.to_string()),
                    }),
                )
            }
        };

        Self {
            topic_path: None,
            store,
            load_state,
            page_size: 20,
            page_index: 0,
            sort: None,
            filters: EventFilters::default(),
            total_rows: 0,
            visible_rows: 0,
        }
    }

    pub fn topic_path(&self) -> Option<&str> {
        self.topic_path.as_deref()
    }

    pub fn load_state(&self) -> &EventTableLoadState {
        &self.load_state
    }

    pub fn rows_len(&self) -> usize {
        self.total_rows
    }

    pub fn visible_len(&self) -> usize {
        self.visible_rows
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
    }

    pub fn set_filter(&mut self, column: EventSortColumn, value: String) {
        if self.filters.get(column) == value {
            return;
        }
        let value_count = if event_column_allows_multi_filter(column) {
            split_filter_values(&value).len()
        } else {
            0
        };
        if value_count > 1 {
            tracing::debug!(
                ?column,
                value_count,
                "applying event table multi-value column filter"
            );
        }
        self.filters.set(column, value);
        self.page_index = 0;
        self.refresh_counts();
    }

    pub fn clear_filters(&mut self) {
        if self.filters.is_empty() {
            return;
        }
        self.filters = EventFilters::default();
        self.page_index = 0;
        self.refresh_counts();
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
        if !self.clear_store() {
            return;
        }
        self.page_index = 0;
        self.sort = None;
        self.filters = EventFilters::default();
        self.load_state = if self.topic_path.is_some() {
            EventTableLoadState::Loading
        } else {
            EventTableLoadState::Idle
        };
        self.refresh_counts();
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
        if !self.clear_store() {
            return;
        }
        self.page_index = 0;
        self.load_state = if self.topic_path.is_some() {
            EventTableLoadState::Loading
        } else {
            EventTableLoadState::Idle
        };
        self.refresh_counts();
    }

    pub fn set_error(&mut self, message: impl Into<Arc<str>>) {
        self.load_state = EventTableLoadState::Error(EventTableError::Message(message.into()));
    }

    pub fn mark_ready(&mut self) {
        if self.topic_path.is_some() {
            self.load_state = EventTableLoadState::Ready;
        }
    }

    pub fn push_rows_front(&mut self, batch: Vec<EventRow>) {
        if batch.is_empty() {
            return;
        }

        let Some(store) = &self.store else {
            tracing::error!("event rows arrived before local store was available");
            self.load_state = EventTableLoadState::Error(EventTableError::StoreUnavailable);
            return;
        };
        if let Err(e) = store.insert_rows(&batch) {
            tracing::error!(error = %e, "failed to insert event rows into local store");
            self.load_state = EventTableLoadState::Error(EventTableError::StoreInsert {
                detail: Arc::from(e.to_string()),
            });
            return;
        }

        self.refresh_counts();

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

        let Some(store) = &self.store else {
            return Vec::new();
        };
        match store.query_page(&self.filters, self.sort, start, count) {
            Ok(rows) => rows,
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "failed to query event rows from local store"
                );
                Vec::new()
            }
        }
    }

    fn clear_store(&mut self) -> bool {
        if self.store.is_none() {
            match EventTableStore::new() {
                Ok(store) => self.store = Some(store),
                Err(e) => {
                    tracing::error!("failed to recreate event table store: {e}");
                    self.load_state = EventTableLoadState::Error(EventTableError::StoreInit {
                        detail: Arc::from(e.to_string()),
                    });
                    return false;
                }
            }
        }

        if let Some(store) = &self.store {
            if let Err(e) = store.clear() {
                tracing::error!(error = %e, "failed to clear event table local store");
                self.load_state = EventTableLoadState::Error(EventTableError::StoreClear {
                    detail: Arc::from(e.to_string()),
                });
                return false;
            }
        }
        self.total_rows = 0;
        self.visible_rows = 0;
        true
    }

    fn refresh_counts(&mut self) {
        let Some(store) = &self.store else {
            self.total_rows = 0;
            self.visible_rows = 0;
            return;
        };

        let total_rows = match store.count_all() {
            Ok(count) => count,
            Err(e) => {
                tracing::error!(error = %e, "failed to count event rows");
                self.load_state = EventTableLoadState::Error(EventTableError::StoreCountAll {
                    detail: Arc::from(e.to_string()),
                });
                return;
            }
        };
        let visible_rows = match store.count_filtered(&self.filters) {
            Ok(count) => count,
            Err(e) => {
                tracing::error!(error = %e, "failed to count filtered event rows");
                self.load_state = EventTableLoadState::Error(EventTableError::StoreCountFiltered {
                    detail: Arc::from(e.to_string()),
                });
                return;
            }
        };

        self.total_rows = total_rows;
        self.visible_rows = visible_rows;
    }
}

fn format_event_store_error(key: &str, locale: &str, detail: &str) -> String {
    let title = t!(key, locale = locale).to_string();
    format!("{title}: {detail}")
}

#[derive(Clone)]
struct EventTableStore {
    inner: Arc<Mutex<EventTableStoreInner>>,
    max_rows: usize,
}

struct EventTableStoreInner {
    conn: Connection,
}

impl EventTableStore {
    fn new() -> rusqlite::Result<Self> {
        Self::new_with_max_rows(EVENT_TABLE_MAX_ROWS)
    }

    fn new_with_max_rows(max_rows: usize) -> rusqlite::Result<Self> {
        // Empty path creates a private temporary database, so long-running event
        // streams are not retained in a Rust Vec or SQLite's pure in-memory DB.
        let conn = Connection::open("")?;
        conn.create_collation("u64ish", cmp_u64ish)?;
        conn.execute_batch(
            r#"
            CREATE TABLE event_rows (
                uid INTEGER PRIMARY KEY,
                batch_uid INTEGER NOT NULL,
                uuid TEXT NOT NULL,
                device TEXT NOT NULL,
                imr TEXT NOT NULL,
                event_type TEXT NOT NULL,
                level TEXT NOT NULL,
                tags TEXT NOT NULL,
                codes TEXT NOT NULL,
                str_codes TEXT NOT NULL,
                happened_time TEXT NOT NULL,
                record_time TEXT NOT NULL,
                bcr_id TEXT NOT NULL,
                context TEXT NOT NULL,
                summary TEXT NOT NULL
            );
            CREATE INDEX event_rows_default_order_idx ON event_rows(batch_uid DESC, uid ASC);
            CREATE INDEX event_rows_device_idx ON event_rows(device);
            CREATE INDEX event_rows_imr_idx ON event_rows(imr);
            CREATE INDEX event_rows_happened_time_idx ON event_rows(happened_time);
            CREATE INDEX event_rows_record_time_idx ON event_rows(record_time);
            "#,
        )?;
        Ok(Self {
            inner: Arc::new(Mutex::new(EventTableStoreInner { conn })),
            max_rows,
        })
    }

    fn clear(&self) -> rusqlite::Result<()> {
        let inner = self.lock_inner()?;
        inner.conn.execute("DELETE FROM event_rows", [])?;
        Ok(())
    }

    fn insert_rows(&self, rows: &[EventRow]) -> rusqlite::Result<()> {
        let mut inner = self.lock_inner()?;
        let tx = inner.conn.transaction()?;
        let mut inserted_rows = 0usize;
        let batch_uid = rows
            .iter()
            .filter_map(|row| i64::try_from(row.uid).ok())
            .max()
            .unwrap_or(0);
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR REPLACE INTO event_rows (
                    uid, batch_uid, uuid, device, imr, event_type, level, tags, codes,
                    str_codes, happened_time, record_time, bcr_id, context, summary
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                "#,
            )?;
            for row in rows {
                let uid = match i64::try_from(row.uid) {
                    Ok(uid) => uid,
                    Err(_) => {
                        tracing::warn!(
                            uid = row.uid,
                            "skipping event row because uid exceeds sqlite integer range"
                        );
                        continue;
                    }
                };
                inserted_rows += stmt.execute(params![
                    uid,
                    batch_uid,
                    &row.uuid,
                    &row.device,
                    &row.imr,
                    &row.event_type,
                    &row.level,
                    &row.tags,
                    &row.codes,
                    &row.str_codes,
                    &row.happened_time,
                    &row.record_time,
                    &row.bcr_id,
                    &row.context,
                    &row.summary,
                ])?;
            }
        }
        let deleted_rows = trim_event_rows_to_limit(&tx, self.max_rows)?;
        tx.commit()?;
        if deleted_rows > 0 {
            tracing::debug!(
                inserted_rows,
                deleted_rows,
                max_rows = self.max_rows,
                "trimmed event table local store after insert"
            );
        }
        Ok(())
    }

    fn count_all(&self) -> rusqlite::Result<usize> {
        let inner = self.lock_inner()?;
        let count = inner
            .conn
            .query_row("SELECT COUNT(*) FROM event_rows", [], |row| {
                row.get::<_, i64>(0)
            })?;
        Ok(count.max(0) as usize)
    }

    fn count_filtered(&self, filters: &EventFilters) -> rusqlite::Result<usize> {
        let inner = self.lock_inner()?;
        let mut sql = String::from("SELECT COUNT(*) FROM event_rows");
        let params = append_filter_clause(filters, &mut sql);
        let count = inner
            .conn
            .query_row(&sql, params_from_iter(params.iter()), |row| {
                row.get::<_, i64>(0)
            })?;
        Ok(count.max(0) as usize)
    }

    fn query_page(
        &self,
        filters: &EventFilters,
        sort: Option<EventSort>,
        start: usize,
        count: usize,
    ) -> rusqlite::Result<Vec<EventRow>> {
        let inner = self.lock_inner()?;
        let mut sql = String::from(
            r#"
            SELECT uid, uuid, device, imr, event_type, level, tags, codes,
                   str_codes, happened_time, record_time, bcr_id, context, summary
            FROM event_rows
            "#,
        );
        let mut params = append_filter_clause(filters, &mut sql);
        sql.push_str(" ORDER BY ");
        sql.push_str(sort_order_clause(sort));
        sql.push_str(" LIMIT ? OFFSET ?");
        params.push(Value::Integer(count.min(i64::MAX as usize) as i64));
        params.push(Value::Integer(start.min(i64::MAX as usize) as i64));

        let mut stmt = inner.conn.prepare(&sql)?;
        let mapped = stmt.query_map(params_from_iter(params.iter()), |row| {
            Ok(EventRow {
                uid: row.get::<_, i64>(0)?.max(0) as u64,
                uuid: row.get(1)?,
                device: row.get(2)?,
                imr: row.get(3)?,
                event_type: row.get(4)?,
                level: row.get(5)?,
                tags: row.get(6)?,
                codes: row.get(7)?,
                str_codes: row.get(8)?,
                happened_time: row.get(9)?,
                record_time: row.get(10)?,
                bcr_id: row.get(11)?,
                context: row.get(12)?,
                summary: row.get(13)?,
            })
        })?;

        let mut rows = Vec::new();
        for row in mapped {
            rows.push(row?);
        }
        Ok(rows)
    }

    fn lock_inner(&self) -> rusqlite::Result<std::sync::MutexGuard<'_, EventTableStoreInner>> {
        self.inner.lock().map_err(|_| rusqlite::Error::InvalidQuery)
    }
}

fn trim_event_rows_to_limit(
    tx: &rusqlite::Transaction<'_>,
    max_rows: usize,
) -> rusqlite::Result<usize> {
    let offset = max_rows.min(i64::MAX as usize) as i64;
    tx.execute(
        r#"
        DELETE FROM event_rows
        WHERE uid <= COALESCE(
            (SELECT uid FROM event_rows ORDER BY uid DESC LIMIT 1 OFFSET ?1),
            -1
        )
        "#,
        [offset],
    )
}

fn append_filter_clause(filters: &EventFilters, sql: &mut String) -> Vec<Value> {
    let mut clauses = Vec::new();
    let mut params = Vec::new();

    push_contains_filter("uuid", &filters.uuid, &mut clauses, &mut params);
    push_contains_filter("device", &filters.device, &mut clauses, &mut params);
    push_contains_filter("imr", &filters.imr, &mut clauses, &mut params);
    push_contains_filter("event_type", &filters.event_type, &mut clauses, &mut params);
    push_contains_filter("level", &filters.level, &mut clauses, &mut params);
    push_contains_filter("tags", &filters.tags, &mut clauses, &mut params);
    push_contains_filter("codes", &filters.codes, &mut clauses, &mut params);
    push_contains_filter("str_codes", &filters.str_codes, &mut clauses, &mut params);
    push_prefix_filter(
        "happened_time",
        &filters.happened_time,
        &mut clauses,
        &mut params,
    );
    push_prefix_filter(
        "record_time",
        &filters.record_time,
        &mut clauses,
        &mut params,
    );
    push_contains_filter("bcr_id", &filters.bcr_id, &mut clauses, &mut params);
    push_contains_filter("context", &filters.context, &mut clauses, &mut params);
    push_contains_filter("summary", &filters.summary, &mut clauses, &mut params);

    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }

    params
}

fn event_column_allows_multi_filter(column: EventSortColumn) -> bool {
    !matches!(
        column,
        EventSortColumn::HappenedTime | EventSortColumn::RecordTime
    )
}

fn push_contains_filter(
    column: &'static str,
    value: &str,
    clauses: &mut Vec<String>,
    params: &mut Vec<Value>,
) {
    let values = split_filter_values(value);
    if values.is_empty() {
        return;
    }
    clauses.push(or_clause(values.len(), || {
        format!("instr(lower({column}), ?) > 0")
    }));
    params.extend(
        values
            .into_iter()
            .map(|value| Value::Text(value.to_lowercase())),
    );
}

fn push_prefix_filter(
    column: &'static str,
    value: &str,
    clauses: &mut Vec<String>,
    params: &mut Vec<Value>,
) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    clauses.push(format!("substr({column}, 1, length(?)) = ?"));
    params.push(Value::Text(value.to_string()));
    params.push(Value::Text(value.to_string()));
}

fn or_clause(count: usize, mut clause: impl FnMut() -> String) -> String {
    if count == 1 {
        return clause();
    }
    let clauses = (0..count).map(|_| clause()).collect::<Vec<_>>();
    format!("({})", clauses.join(" OR "))
}

fn sort_order_clause(sort: Option<EventSort>) -> &'static str {
    let Some(sort) = sort else {
        return "batch_uid DESC, uid ASC";
    };

    match (sort.column, sort.direction) {
        (EventSortColumn::Uuid, SortDirection::Asc) => "uuid COLLATE u64ish ASC, uid DESC",
        (EventSortColumn::Uuid, SortDirection::Desc) => "uuid COLLATE u64ish DESC, uid DESC",
        (EventSortColumn::Device, SortDirection::Asc) => "device COLLATE u64ish ASC, uid DESC",
        (EventSortColumn::Device, SortDirection::Desc) => "device COLLATE u64ish DESC, uid DESC",
        (EventSortColumn::Imr, SortDirection::Asc) => "imr ASC, uid DESC",
        (EventSortColumn::Imr, SortDirection::Desc) => "imr DESC, uid DESC",
        (EventSortColumn::EventType, SortDirection::Asc) => "event_type ASC, uid DESC",
        (EventSortColumn::EventType, SortDirection::Desc) => "event_type DESC, uid DESC",
        (EventSortColumn::Level, SortDirection::Asc) => "level ASC, uid DESC",
        (EventSortColumn::Level, SortDirection::Desc) => "level DESC, uid DESC",
        (EventSortColumn::Tags, SortDirection::Asc) => "tags ASC, uid DESC",
        (EventSortColumn::Tags, SortDirection::Desc) => "tags DESC, uid DESC",
        (EventSortColumn::Codes, SortDirection::Asc) => "codes ASC, uid DESC",
        (EventSortColumn::Codes, SortDirection::Desc) => "codes DESC, uid DESC",
        (EventSortColumn::StrCodes, SortDirection::Asc) => "str_codes ASC, uid DESC",
        (EventSortColumn::StrCodes, SortDirection::Desc) => "str_codes DESC, uid DESC",
        (EventSortColumn::HappenedTime, SortDirection::Asc) => "happened_time ASC, uid DESC",
        (EventSortColumn::HappenedTime, SortDirection::Desc) => "happened_time DESC, uid DESC",
        (EventSortColumn::RecordTime, SortDirection::Asc) => "record_time ASC, uid DESC",
        (EventSortColumn::RecordTime, SortDirection::Desc) => "record_time DESC, uid DESC",
        (EventSortColumn::BcrId, SortDirection::Asc) => "bcr_id ASC, uid DESC",
        (EventSortColumn::BcrId, SortDirection::Desc) => "bcr_id DESC, uid DESC",
        (EventSortColumn::Context, SortDirection::Asc) => "context ASC, uid DESC",
        (EventSortColumn::Context, SortDirection::Desc) => "context DESC, uid DESC",
        (EventSortColumn::Summary, SortDirection::Asc) => "summary ASC, uid DESC",
        (EventSortColumn::Summary, SortDirection::Desc) => "summary DESC, uid DESC",
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
    fn numeric_device_sort_matches_previous_u64ish_order() {
        let mut state = EventTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));

        let mut device_100 = event_row(1, "2026-04-14 00:00:01.000");
        device_100.device = "100".to_string();
        let mut device_9 = event_row(2, "2026-04-14 00:00:02.000");
        device_9.device = "9".to_string();

        state.push_rows_front(vec![device_100, device_9]);
        state.toggle_sort(EventSortColumn::Device);

        let rows = state.page_rows_owned();
        assert_eq!(
            rows.iter()
                .map(|row| row.device.as_str())
                .collect::<Vec<_>>(),
            vec!["9", "100"]
        );
    }

    #[test]
    fn numeric_uuid_sort_matches_previous_u64ish_order() {
        let mut state = EventTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));

        let mut uuid_100 = event_row(1, "2026-04-14 00:00:01.000");
        uuid_100.uuid = "100".to_string();
        let mut uuid_9 = event_row(2, "2026-04-14 00:00:02.000");
        uuid_9.uuid = "9".to_string();

        state.push_rows_front(vec![uuid_100, uuid_9]);
        state.toggle_sort(EventSortColumn::Uuid);

        let rows = state.page_rows_owned();
        assert_eq!(
            rows.iter().map(|row| row.uuid.as_str()).collect::<Vec<_>>(),
            vec!["9", "100"]
        );
    }

    #[test]
    fn event_store_errors_are_localized() {
        let unavailable = EventTableError::StoreUnavailable;
        assert_eq!(
            unavailable.localized_message("en"),
            "Event table local store is unavailable"
        );
        assert_eq!(unavailable.localized_message("zh"), "事件表本地存储不可用");

        let insert = EventTableError::StoreInsert {
            detail: std::sync::Arc::from("disk full"),
        };
        assert_eq!(
            insert.localized_message("en"),
            "Failed to write to event table local store: disk full"
        );
    }

    #[test]
    fn event_store_retention_keeps_latest_rows() {
        let store = EventTableStore::new_with_max_rows(3).expect("create event table store");
        let first_batch = (1..=2)
            .map(|uid| event_row(uid, "2026-04-14 00:00:01.000"))
            .collect::<Vec<_>>();
        let second_batch = (3..=5)
            .map(|uid| event_row(uid, "2026-04-14 00:00:02.000"))
            .collect::<Vec<_>>();

        store
            .insert_rows(&first_batch)
            .expect("insert first event batch");
        store
            .insert_rows(&second_batch)
            .expect("insert second event batch");

        assert_eq!(store.count_all().expect("count event rows"), 3);
        let page = store
            .query_page(&EventFilters::default(), None, 0, 10)
            .expect("query event rows");
        assert_eq!(
            page.iter().map(|row| row.uid).collect::<Vec<_>>(),
            vec![3, 4, 5]
        );
    }

    #[test]
    fn default_view_preserves_batch_order() {
        let mut state = EventTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));

        state.push_rows_front(vec![
            event_row(1, "2026-04-14 00:00:01.000"),
            event_row(2, "2026-04-14 00:00:02.000"),
        ]);
        state.push_rows_front(vec![
            event_row(3, "2026-04-14 00:00:03.000"),
            event_row(4, "2026-04-14 00:00:04.000"),
        ]);

        let rows = state.page_rows_owned();
        assert_eq!(
            rows.iter().map(|row| row.uid).collect::<Vec<_>>(),
            vec![3, 4, 1, 2]
        );
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
    fn time_filter_keeps_single_value_semantics() {
        let mut state = EventTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));

        state.push_rows_front(vec![
            event_row(1, "2026-04-14 00:00:01.000"),
            event_row(2, "2026-04-15 00:00:01.000"),
        ]);
        state.set_filter(
            EventSortColumn::HappenedTime,
            "2026-04-14,2026-04-15".to_string(),
        );

        assert!(state.page_rows_owned().is_empty());
    }

    #[test]
    fn column_filter_matches_any_comma_separated_value() {
        let mut state = EventTableState::new();
        state.reset_for_topic(Some("persistent://topic".to_string()));

        let mut first = event_row(1, "2026-04-14 00:00:01.000");
        first.device = "100".to_string();
        let mut second = event_row(2, "2026-04-14 00:00:02.000");
        second.device = "200".to_string();
        let mut third = event_row(3, "2026-04-14 00:00:03.000");
        third.device = "300".to_string();

        state.push_rows_front(vec![first, second, third]);
        state.set_filter(EventSortColumn::Device, "100, 300".to_string());

        let rows = state.page_rows_owned();
        assert_eq!(
            rows.iter()
                .map(|row| row.device.as_str())
                .collect::<Vec<_>>(),
            vec!["100", "300"]
        );
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

impl Default for EventTableState {
    fn default() -> Self {
        Self::new()
    }
}
