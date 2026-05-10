//! Dashboard module
//!
//! Provides an overview page showing financial summaries and charts.
//! Includes functionality for filtering data by tags.

mod aggregation;
mod cards;
mod charts;
pub(crate) mod handlers;
pub(crate) mod json;
mod tables;
pub(crate) mod transaction;

pub use handlers::{get_dashboard_page, update_excluded_tags};
pub use json::get_dashboard_json;
