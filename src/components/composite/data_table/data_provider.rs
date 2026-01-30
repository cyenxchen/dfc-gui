//! DataProvider Trait
//!
//! Abstraction for data sources to support both in-memory and lazy-loaded data.

use std::sync::Arc;

/// Trait for providing data to the DataTable
pub trait DataProvider: Send + Sync + 'static {
    type Row: Clone + Send + Sync + 'static;

    /// Get the total number of rows
    fn len(&self) -> usize;

    /// Check if empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a row by index
    fn row(&self, index: usize) -> Option<Self::Row>;

    /// Get multiple rows in a range
    fn rows(&self, range: std::ops::Range<usize>) -> Vec<Self::Row> {
        range.filter_map(|i| self.row(i)).collect()
    }
}

/// Simple in-memory data provider
pub struct VecDataProvider<R> {
    rows: Arc<Vec<R>>,
}

impl<R: Clone + Send + Sync + 'static> VecDataProvider<R> {
    /// Create a new VecDataProvider
    pub fn new(rows: Vec<R>) -> Self {
        Self {
            rows: Arc::new(rows),
        }
    }

    /// Create from a shared reference
    pub fn from_arc(rows: Arc<Vec<R>>) -> Self {
        Self { rows }
    }

    /// Get all rows
    pub fn all(&self) -> &[R] {
        &self.rows
    }
}

impl<R: Clone + Send + Sync + 'static> DataProvider for VecDataProvider<R> {
    type Row = R;

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn row(&self, index: usize) -> Option<Self::Row> {
        self.rows.get(index).cloned()
    }
}

/// Paged data provider for lazy loading
pub trait PagedDataProvider: Send + Sync + 'static {
    type Row: Clone + Send + Sync + 'static;

    /// Get the total number of rows (may be estimated)
    fn len(&self) -> usize;

    /// Check if empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a cached row by index (returns None if not yet loaded)
    fn cached_row(&self, index: usize) -> Option<Self::Row>;

    /// Request a range of rows to be loaded
    fn request_range(&self, range: std::ops::Range<usize>);

    /// Check if a range is loaded
    fn is_range_loaded(&self, range: std::ops::Range<usize>) -> bool;
}
