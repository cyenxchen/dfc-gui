//! Column Definition
//!
//! Defines table columns with their properties and cell renderers.

use gpui::{AnyElement, SharedString};

/// Column definition for the DataTable
pub struct Column<R> {
    /// Column identifier
    pub id: SharedString,
    /// Column header label
    pub label: SharedString,
    /// Column width (in pixels, or flexible)
    pub width: ColumnWidth,
    /// Whether the column is sortable
    pub sortable: bool,
    /// Cell renderer function
    pub render: Box<dyn Fn(&R) -> AnyElement + Send + Sync>,
}

/// Column width specification
#[derive(Debug, Clone, Copy)]
pub enum ColumnWidth {
    /// Fixed width in pixels
    Fixed(f32),
    /// Flexible width with optional min/max
    Flex { min: Option<f32>, max: Option<f32> },
    /// Percentage of available space
    Percent(f32),
}

impl Default for ColumnWidth {
    fn default() -> Self {
        ColumnWidth::Flex { min: None, max: None }
    }
}

impl<R: 'static> Column<R> {
    /// Create a new column
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        render: impl Fn(&R) -> AnyElement + Send + Sync + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            width: ColumnWidth::default(),
            sortable: false,
            render: Box::new(render),
        }
    }

    /// Set fixed width
    pub fn fixed_width(mut self, width: f32) -> Self {
        self.width = ColumnWidth::Fixed(width);
        self
    }

    /// Set flexible width with optional constraints
    pub fn flex_width(mut self, min: Option<f32>, max: Option<f32>) -> Self {
        self.width = ColumnWidth::Flex { min, max };
        self
    }

    /// Set percentage width
    pub fn percent_width(mut self, percent: f32) -> Self {
        self.width = ColumnWidth::Percent(percent);
        self
    }

    /// Make the column sortable
    pub fn sortable(mut self) -> Self {
        self.sortable = true;
        self
    }

    /// Render a cell
    pub fn render_cell(&self, row: &R) -> AnyElement {
        (self.render)(row)
    }
}
