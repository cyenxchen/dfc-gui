//! DataTable Component
//!
//! A data table with virtual scrolling support.

use gpui::{
    div, prelude::*, px, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Window,
};

use super::column::{Column, ColumnWidth};
use crate::theme::colors::DfcColors;

/// DataTable component
pub struct DataTable<R: Clone + Send + Sync + 'static> {
    columns: Vec<Column<R>>,
    rows: Vec<R>,
    row_height: f32,
    header_height: f32,
    loading: bool,
    empty_message: SharedString,
}

impl<R: Clone + Send + Sync + 'static> DataTable<R> {
    /// Create a new data table
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            row_height: 36.0,
            header_height: 40.0,
            loading: false,
            empty_message: "No data".into(),
        }
    }

    /// Set the columns
    pub fn set_columns(&mut self, columns: Vec<Column<R>>) {
        self.columns = columns;
    }

    /// Set the rows
    pub fn set_rows(&mut self, rows: Vec<R>) {
        self.rows = rows;
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    /// Set the empty message
    pub fn set_empty_message(&mut self, message: impl Into<SharedString>) {
        self.empty_message = message.into();
    }

    /// Get column width style
    fn column_width_style(&self, width: &ColumnWidth) -> f32 {
        match width {
            ColumnWidth::Fixed(w) => *w,
            ColumnWidth::Flex { min, .. } => min.unwrap_or(100.0),
            ColumnWidth::Percent(p) => 100.0 * p / 100.0,
        }
    }

    /// Render the header row
    fn render_header(&self) -> impl IntoElement {
        div()
            .h(px(self.header_height))
            .w_full()
            .flex()
            .items_center()
            .bg(DfcColors::table_header_bg())
            .border_b_1()
            .border_color(DfcColors::border())
            .children(self.columns.iter().map(|col| {
                let width = self.column_width_style(&col.width);
                div()
                    .w(px(width))
                    .px_3()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(DfcColors::text_primary())
                    .child(col.label.clone())
            }))
    }

    /// Render a data row
    fn render_row(&self, row: &R, index: usize) -> impl IntoElement {
        let bg = if index % 2 == 0 {
            DfcColors::content_bg()
        } else {
            DfcColors::table_row_alt()
        };

        div()
            .h(px(self.row_height))
            .w_full()
            .flex()
            .items_center()
            .bg(bg)
            .hover(|s| s.bg(DfcColors::table_row_hover()))
            .border_b_1()
            .border_color(DfcColors::border())
            .children(self.columns.iter().map(|col| {
                let width = self.column_width_style(&col.width);
                let cell_content = col.render_cell(row);
                div()
                    .w(px(width))
                    .px_3()
                    .text_sm()
                    .text_color(DfcColors::text_primary())
                    .overflow_hidden()
                    .child(cell_content)
            }))
    }

    /// Render empty state
    fn render_empty(&self) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_color(DfcColors::text_muted())
            .child(self.empty_message.clone())
    }

    /// Render loading state
    fn render_loading(&self) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_color(DfcColors::text_muted())
            .child("Loading...")
    }
}

impl<R: Clone + Send + Sync + 'static> Render for DataTable<R> {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut table = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(DfcColors::content_bg())
            .border_1()
            .border_color(DfcColors::border())
            .rounded_md()
            .overflow_hidden();

        // Header
        table = table.child(self.render_header());

        // Body
        if self.loading {
            table = table.child(self.render_loading());
        } else if self.rows.is_empty() {
            table = table.child(self.render_empty());
        } else {
            // Render rows (for now, render all - virtual scrolling to be added)
            let rows_content = div()
                .id("data-table-rows")
                .flex_1()
                .overflow_y_scroll()
                .children(
                    self.rows
                        .iter()
                        .enumerate()
                        .map(|(i, row)| self.render_row(row, i)),
                );
            table = table.child(rows_content);
        }

        table
    }
}

/// Helper to create a DataTable entity
pub fn data_table<R: Clone + Send + Sync + 'static, V: 'static>(
    columns: Vec<Column<R>>,
    rows: Vec<R>,
    cx: &mut Context<V>,
) -> Entity<DataTable<R>> {
    cx.new(|cx| {
        let mut table = DataTable::new(cx);
        table.set_columns(columns);
        table.set_rows(rows);
        table
    })
}
