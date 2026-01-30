//! DataState - Aggregation Data State (Curve, 1min, 10min)

use crate::domain::aggregation::{OneMinData, TenMinData};
use crate::domain::curve::CurveData;

/// State for data aggregation
#[derive(Debug, Clone, Default)]
pub struct DataState {
    /// Power curve data
    pub curve_data: Vec<CurveData>,
    /// One minute aggregation data
    pub one_min_data: Vec<OneMinData>,
    /// Ten minute aggregation data
    pub ten_min_data: Vec<TenMinData>,
    /// Loading states
    pub curve_loading: bool,
    pub one_min_loading: bool,
    pub ten_min_loading: bool,
}

impl DataState {
    /// Update curve data
    pub fn update_curve_data(&mut self, data: Vec<CurveData>) {
        self.curve_data = data;
        self.curve_loading = false;
    }

    /// Update one minute data
    pub fn update_one_min_data(&mut self, data: Vec<OneMinData>) {
        self.one_min_data = data;
        self.one_min_loading = false;
    }

    /// Update ten minute data
    pub fn update_ten_min_data(&mut self, data: Vec<TenMinData>) {
        self.ten_min_data = data;
        self.ten_min_loading = false;
    }
}
