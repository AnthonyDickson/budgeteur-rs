//! Dashboard module
//!
//! Provides an overview page showing financial summaries and charts.
//! Includes functionality for filtering data by tags.

mod aggregation;
mod cards;
mod charts;
mod handlers;
mod tables;
mod transaction;

pub use handlers::{get_dashboard_page, update_excluded_tags};
