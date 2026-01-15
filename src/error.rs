//! Defines the app level error type and conversions to rendered HTML pages and alerts.
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use time::Date;

use crate::{
    alert::Alert, internal_server_error::InternalServerError, not_found::NotFoundError, tag::TagId,
};

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
            Error::NotFound => NotFoundError.into_response(),
            Error::DashboardPreferencesSaveError => InternalServerError {
                description: "Save Failed",
                fix: "Failed to save your preferences. Please try again.",
            }
            .into_response(),
            Error::InvalidTimezoneError(timezone) => InternalServerError {
                description: "Invalid Timezone Settings",
                fix: &format!(
                    "Could not get local timezone \"{timezone}\". Check your server settings and \
                    ensure the timezone has been set to valid, canonical timezone string"
                ),
            }
            .into_response(),
            Error::DatabaseLockError => InternalServerError::default().into_response(),
            // Any errors that are not handled above are not intended to be shown to the client.
            error => {
                tracing::error!("An unexpected error occurred: {}", error);
                InternalServerError::default().into_response()
            }
        }
    }
}

impl Error {
    /// Convert the error into an HTTP response with an HTML alert.
    pub fn into_alert_response(self) -> Response {
        let (status_code, alert) = match self {
            Error::InvalidTimezoneError(timezone) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Alert::Error {
                    message: "Invalid Timezone Settings".to_owned(),
                    details: format!(
                        "Could not get local timezone \"{timezone}\". Check your server settings and \
                    ensure the timezone has been set to valid, canonical timezone string"
                    ),
                },
            ),
            Error::FutureDate(date) => (
                StatusCode::BAD_REQUEST,
                Alert::Error {
                    message: "Invalid transaction date".to_owned(),
                    details: format!(
                        "{date} is a date in the future, which is not allowed. Change the date to"
                    ),
                },
            ),
            Error::InvalidTag(tag_id) => (
                StatusCode::BAD_REQUEST,
                Alert::Error {
                    message: "Invalid tag ID".to_owned(),
                    details: format!("Could not find a tag with the ID {tag_id:?}"),
                },
            ),
            Error::UpdateMissingTransaction => (
                StatusCode::NOT_FOUND,
                Alert::Error {
                    message: "Could not update transaction".to_owned(),
                    details: "The transaction could not be found.".to_owned(),
                },
            ),
            Error::DeleteMissingTransaction => (
                StatusCode::NOT_FOUND,
                Alert::Error {
                    message: "Could not delete transaction".to_owned(),
                    details: "The transaction could not be found. \
                    Try refreshing the page to see if the transaction has already been deleted."
                        .to_owned(),
                },
            ),
            Error::UpdateMissingAccount => (
                StatusCode::NOT_FOUND,
                Alert::Error {
                    message: "Could not update account".to_owned(),
                    details: "The account could not be found.".to_owned(),
                },
            ),
            Error::DeleteMissingAccount => (
                StatusCode::NOT_FOUND,
                Alert::Error {
                    message: "Could not delete account".to_owned(),
                    details: "The account could not be found. \
                    Try refreshing the page to see if the account has already been deleted."
                        .to_owned(),
                },
            ),
            Error::UpdateMissingTag => (
                StatusCode::NOT_FOUND,
                Alert::Error {
                    message: "Could not update tag".to_owned(),
                    details: "The tag could not be found.".to_owned(),
                },
            ),
            Error::DeleteMissingTag => (
                StatusCode::NOT_FOUND,
                Alert::Error {
                    message: "Could not delete tag".to_owned(),
                    details: "The tag could not be found. \
                    Try refreshing the page to see if the tag has already been deleted."
                        .to_owned(),
                },
            ),
            Error::UpdateMissingRule => (
                StatusCode::NOT_FOUND,
                Alert::Error {
                    message: "Could not update rule".to_owned(),
                    details: "The rule could not be found.".to_owned(),
                },
            ),
            Error::DeleteMissingRule => (
                StatusCode::NOT_FOUND,
                Alert::Error {
                    message: "Could not delete rule".to_owned(),
                    details: "The rule could not be found. \
                    Try refreshing the page to see if the rule has already been deleted."
                        .to_owned(),
                },
            ),
            Error::DuplicateAccountName(name) => (
                StatusCode::BAD_REQUEST,
                Alert::Error {
                    message: "Duplicate Account Name".to_owned(),
                    details: format!(
                        "The account {name} already exists in the database. \
                        Choose a different account name, or edit or delete the existing account.",
                    ),
                },
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Alert::Error {
                    message: "Something went wrong".to_owned(),
                    details:
                        "An unexpected error occurred, check the server logs for more details."
                            .to_owned(),
                },
            ),
        };

        (status_code, alert.into_html()).into_response()
    }
}
