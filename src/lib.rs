//! [![github]](https://github.com/AnthonyDickson/budgeteur-rs)&ensp;
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//!
//! <br>
//!
//! Budgeteur is a web app for managing your budget and personal finances.
//!
//! This library provides a REST API that directly serves HTML pages.

#![warn(missing_docs)]

use std::time::Duration;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_server::Handle;
use tokio::signal;

mod alert;
mod app_state;
mod auth_cookie;
mod auth_middleware;
mod balances;
mod csv;
mod dashboard;
mod dashboard_preferences;
mod database_id;
pub mod db;
mod endpoints;
mod filters;
mod forgot_password;
mod import;
mod import_result;
mod log_in;
mod log_out;
mod logging;
mod navigation;
mod not_found;
mod pagination;
mod password;
mod register_user;
mod routing;
mod rule;
mod shared_templates;
mod tag;
mod timezone;
pub mod transaction;
mod transaction_tag;
pub mod user;

pub use app_state::AppState;
pub use logging::logging_middleware;
pub use password::{PasswordHash, ValidatedPassword};
pub use routing::build_router;

/// An async task that waits for either the ctrl+c or terminate signal, whichever comes first, and
/// then signals the server to shut down gracefully.
///
/// `handle` is a handle to an Axum `Server`.
pub async fn graceful_shutdown(handle: Handle) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::debug!("Received ctrl+c signal.");
            handle.graceful_shutdown(Some(Duration::from_secs(1)));
        },
        _ = terminate => {
            tracing::debug!("Received terminate signal.");
            handle.graceful_shutdown(Some(Duration::from_secs(1)));
        },
    }
}

/// The errors that may occur in the application.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    /// The user provided an invalid combination of password.
    #[error("invalid password")]
    InvalidCredentials,

    /// Either the user ID or expiry cookie is missing from the cookie jar in
    /// the request.
    #[error("no cookies in the cookie jar :(")]
    CookieMissing,

    /// There was an error parsing the date in the cookie or creating the new
    /// expiry date time.
    ///
    /// Callers should pass in the original error as a string and the date
    /// string that caused the error.
    #[error("could not format expiry cookie date-time string \"{1}\": {0}")]
    InvalidDateFormat(String, String),

    /// The user provided a password that is too easy to guess.
    #[error("password is too weak: {0}")]
    TooWeak(String),

    /// An unexpected error occurred with the underlying hashing library.
    ///
    /// The error string should only be logged for debugging on the server.
    /// When communicating with the application client this error should be
    /// replaced with a general error type indicating an internal server error.
    #[error("hashing failed: {0}")]
    HashingError(String),

    /// The tag ID used to create a transaction did not match a valid tag.
    #[error("the tag ID does not refer to a valid tag")]
    InvalidTag,

    /// An empty string was used to create a tag name.
    #[error("Tag name cannot be empty")]
    EmptyTagName,

    /// A date in the future was used to create a transaction.
    ///
    /// Transactions record events that have already happened, therefore future
    /// dates are not allowed.
    #[error("transaction dates must not be later than the current date")]
    FutureDate,

    /// The specified import ID already exists in the database.
    ///
    /// When importing transactions from a CSV file, an import ID is used to
    /// uniquely identify each transaction. Rejecting duplicate import IDs
    /// avoids importing the same transaction multiple times, which is likely
    /// to happen if the user tries to import CSV files that overlap in time.
    #[error("the import ID already exists in the database")]
    DuplicateImportId,

    /// The multipart form could not be parsed as a list of CSV files.
    #[error("Could not parse multipart form: {0}")]
    MultipartError(String),

    /// The multipart form did not contain a CSV file.
    #[error("File is not a CSV")]
    NotCSV,

    /// The CSV had issues that prevented it from being parsed.
    #[error("Could not parse the CSV file: {0}")]
    InvalidCSV(String),

    /// The requested resource was not found.
    ///
    /// For HTTP request handlers, the client should check that the parameters
    /// (e.g., ID) are correct and that the resource has been created.
    ///
    /// Internally, this error may occur when a query returns no rows.
    #[error("the requested resource could not be found")]
    NotFound,

    /// An unexpected error occurred when hashing a password or parsing a password hash.
    #[error("an unexpected error occurred: {0}")]
    InternalError(String),

    /// An unhandled/unexpected SQL error.
    #[error("an error occurred while creating the user: {0}")]
    SqlError(rusqlite::Error),

    /// An error occurred while saving dashboard preferences.
    #[error("failed to save dashboard preferences")]
    DashboardPreferencesSaveError,

    /// An error occurred while calculating dashboard summaries.
    #[error("failed to calculate dashboard summaries")]
    DashboardCalculationError,

    /// An error occurred while getting the local timezone from a canonical timezone string.
    #[error("invalid timezone {0}")]
    InvalidTimezoneError(String),
}

impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        match value {
            // Code 2067 occurs when a UNIQUE constraint failed.
            rusqlite::Error::SqliteFailure(sql_error, Some(ref desc))
                if sql_error.extended_code == 2067 && desc.ends_with("transaction.import_id") =>
            {
                Error::DuplicateImportId
            }
            rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
            error => {
                tracing::error!("an unhandled SQL error occurred: {}", error);
                Error::SqlError(error)
            }
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::NotFound => (StatusCode::NOT_FOUND, "Resource not found"),
            Error::InternalError(err) => {
                tracing::error!("An unexpected error occurred: {}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
            Error::DashboardPreferencesSaveError => {
                tracing::error!("Failed to save dashboard preferences");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to save your preferences. Please try again.",
                )
            }
            Error::DashboardCalculationError => {
                tracing::error!("Failed to calculate dashboard summaries");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update dashboard summaries. Please try again.",
                )
            }
            // Any errors that are not handled above are not intended to be shown to the client.
            error => {
                tracing::error!("An unexpected error occurred: {}", error);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        }
        .into_response()
    }
}
