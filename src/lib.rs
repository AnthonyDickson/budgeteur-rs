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

use std::{net::SocketAddr, time::Duration};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_server::Handle;
use time::Date;
use tokio::signal;

mod account;
mod alert;
mod app_state;
mod auth;
mod csv_import;
mod dashboard;
mod dashboard_preferences;
mod database_id;
mod db;
mod endpoints;
mod filters;
mod forgot_password;
mod internal_server_error;
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
mod tags_page;
mod timezone;
mod transaction;
mod user;

pub use app_state::AppState;
pub use db::initialize as initialize_db;
pub use logging::{LOG_BODY_LENGTH_LIMIT, logging_middleware};
pub use password::{PasswordHash, ValidatedPassword};
pub use routing::build_router;
pub use user::{User, UserID, get_user_by_id};

use crate::{
    alert::AlertTemplate,
    internal_server_error::{InternalServerErrorPageTemplate, render_internal_server_error},
    not_found::get_404_not_found_response,
    shared_templates::render,
    tag::TagId,
};

/// An async task that waits for either the ctrl+c or terminate signal, whichever comes first, and
/// then signals the server to shut down gracefully.
///
/// `handle` is a handle to an Axum `Server`.
pub async fn graceful_shutdown(handle: Handle<SocketAddr>) {
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
    InvalidTag(Option<TagId>),

    /// An empty string was used to create a tag name.
    #[error("Tag name cannot be empty")]
    EmptyTagName,

    /// A date in the future was used to create a transaction.
    ///
    /// Transactions record events that have already happened, therefore future
    /// dates are not allowed.
    #[error("{0} is a date in the future, which is not allowed")]
    FutureDate(Date),

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

    /// An unhandled/unexpected SQL error.
    #[error("an unexpected SQL error occurred: {0}")]
    SqlError(rusqlite::Error),

    /// An error occurred while saving dashboard preferences.
    #[error("failed to save dashboard preferences")]
    DashboardPreferencesSaveError,

    /// An error occurred while getting the local timezone from a canonical timezone string.
    #[error("invalid timezone {0}")]
    InvalidTimezoneError(String),

    /// The specified account name already exists in the database.
    #[error("the account \"{0}\" already exists in the database")]
    DuplicateAccountName(String),

    /// An error occurred while serializing a struct as JSON
    #[error("could not serialize as JSON: {0}")]
    JSONSerializationError(String),

    /// Could not acquire the database lock
    #[error("could not acquire the database lock")]
    DatabaseLockError,

    /// Tried to delete a transaction that does not exist
    #[error("tried to delete a transaction that is not in the database")]
    DeleteMissingTransaction,

    /// Tried to update a transaction that does not exist
    #[error("tried to update a transaction that is not in the database")]
    UpdateMissingTransaction,

    /// Tried to delete an account that does not exist
    #[error("tried to delete an account that is not in the database")]
    DeleteMissingAccount,

    /// Tried to update an account that does not exist
    #[error("tried to update an account that is not in the database")]
    UpdateMissingAccount,

    /// Tried to update a tag that does not exist
    #[error("tried to update a tag that is not in the database")]
    UpdateMissingTag,

    /// Tried to delete a tag that does not exist
    #[error("tried to delete a tag that is not in the database")]
    DeleteMissingTag,

    /// Tried to update a rule that does not exist
    #[error("tried to update a rule that is not in the database")]
    UpdateMissingRule,

    /// Tried to delete a rule that does not exist
    #[error("tried to delete a rule that is not in the database")]
    DeleteMissingRule,
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
            Error::NotFound => get_404_not_found_response(),
            Error::DashboardPreferencesSaveError => {
                render_internal_server_error(InternalServerErrorPageTemplate {
                    description: "Save Failed",
                    fix: "Failed to save your preferences. Please try again.",
                })
            }
            Error::InvalidTimezoneError(timezone) => {
                render_internal_server_error(InternalServerErrorPageTemplate {
                    description: "Invalid Timezone Settings",
                    fix: &format!(
                        "Could not get local timezone \"{timezone}\". Check your server settings and \
                    ensure the timezone has been set to valid, canonical timezone string"
                    ),
                })
            }
            Error::DatabaseLockError => render_internal_server_error(Default::default()),
            // Any errors that are not handled above are not intended to be shown to the client.
            error => {
                tracing::error!("An unexpected error occurred: {}", error);
                render_internal_server_error(Default::default())
            }
        }
    }
}

impl Error {
    fn into_alert_response(self) -> Response {
        match self {
            Error::InvalidTimezoneError(timezone) => render(
                StatusCode::INTERNAL_SERVER_ERROR,
                AlertTemplate::error(
                    "Invalid Timezone Settings",
                    &format!(
                        "Could not get local timezone \"{timezone}\". Check your server settings and \
                    ensure the timezone has been set to valid, canonical timezone string"
                    ),
                ),
            ),
            Error::FutureDate(date) => render(
                StatusCode::BAD_REQUEST,
                AlertTemplate::error(
                    "Invalid transaction date",
                    &format!(
                        "{date} is a date in the future, which is not allowed. Change the date to"
                    ),
                ),
            ),
            Error::InvalidTag(tag_id) => render(
                StatusCode::BAD_REQUEST,
                AlertTemplate::error(
                    "Invalid tag ID",
                    &format!("Could not find a tag with the ID {tag_id:?}"),
                ),
            ),
            Error::UpdateMissingTransaction => render(
                StatusCode::NOT_FOUND,
                AlertTemplate::error(
                    "Could not update transaction",
                    "The transaction could not be found.",
                ),
            ),
            Error::DeleteMissingTransaction => render(
                StatusCode::NOT_FOUND,
                AlertTemplate::error(
                    "Could not delete transaction",
                    "The transaction could not be found. \
                    Try refreshing the page to see if the transaction has already been deleted.",
                ),
            ),
            Error::UpdateMissingAccount => render(
                StatusCode::NOT_FOUND,
                AlertTemplate::error(
                    "Could not update account",
                    "The account could not be found.",
                ),
            ),
            Error::DeleteMissingAccount => render(
                StatusCode::NOT_FOUND,
                AlertTemplate::error(
                    "Could not delete account",
                    "The account could not be found. \
                    Try refreshing the page to see if the account has already been deleted.",
                ),
            ),
            Error::UpdateMissingTag => render(
                StatusCode::NOT_FOUND,
                AlertTemplate::error("Could not update tag", "The tag could not be found."),
            ),
            Error::DeleteMissingTag => render(
                StatusCode::NOT_FOUND,
                AlertTemplate::error(
                    "Could not delete tag",
                    "The tag could not be found. \
                    Try refreshing the page to see if the tag has already been deleted.",
                ),
            ),
            Error::UpdateMissingRule => render(
                StatusCode::NOT_FOUND,
                AlertTemplate::error("Could not update rule", "The rule could not be found."),
            ),
            Error::DeleteMissingRule => render(
                StatusCode::NOT_FOUND,
                AlertTemplate::error(
                    "Could not delete rule",
                    "The rule could not be found. \
                    Try refreshing the page to see if the rule has already been deleted.",
                ),
            ),
            Error::DuplicateAccountName(name) => render(
                StatusCode::BAD_REQUEST,
                AlertTemplate::error(
                    "Duplicate Account Name",
                    &format!(
                        "The account {name} already exists in the database. \
                        Choose a different account name, or edit or delete the existing account.",
                    ),
                ),
            ),
            _ => render(
                StatusCode::INTERNAL_SERVER_ERROR,
                AlertTemplate::error(
                    "Something went wrong",
                    "An unexpected error occurred, check the server logs for more details.",
                ),
            ),
        }
    }
}
