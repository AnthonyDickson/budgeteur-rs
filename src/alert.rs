//! Alert system for displaying success and error messages to users.
//!
//! This module provides a unified way to display alert messages across the application
//! with proper styling and dismissal functionality.
//!
//! ## Displaying Alerts
//!
//! To display an alert, just add the attribute `hx-swap="none"` and respond with
//! an [AlertTemplate]. The base template already has a `div` with the ID
//! `alert-container` to swap with HTMX.
//!
//! If you want to display an error alert for a 4XX-5XX status code, add the
//! attribute `hx-target-error="#alert-container"` to the element that is making
//! the HTMX request.
//! **Note**: This assumes that *all* errors are handled by responding with an
//! alert, otherwise the alert container element will be simply replaced with
//! the response body.

use askama::Template;

/// Alert message types for styling
#[derive(Debug, Clone)]
pub enum AlertType {
    Success,
    Error,
}

/// Renders alert messages with appropriate styling
#[derive(Template)]
#[template(path = "partials/alert.html")]
pub struct AlertTemplate<'a> {
    pub alert_type: AlertType,
    pub message: &'a str,
    pub details: &'a str,
}

impl<'a> AlertTemplate<'a> {
    /// Create a new success alert
    pub fn success(message: &'a str, details: &'a str) -> Self {
        Self {
            alert_type: AlertType::Success,
            message,
            details,
        }
    }

    /// Create a new error alert
    pub fn error(message: &'a str, details: &'a str) -> Self {
        Self {
            alert_type: AlertType::Error,
            message,
            details,
        }
    }

    /// Create a new error alert without details
    pub fn error_simple(message: &'a str) -> Self {
        Self::error(message, "")
    }
}
