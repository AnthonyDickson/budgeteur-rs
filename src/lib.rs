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
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_server::Handle;
use tokio::signal;

pub use routes::build_router;
pub use state::AppState;

pub mod auth;
pub mod db;
pub mod models;
pub mod routes;
pub mod state;
pub mod stores;

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
    /// The email used to create the user is already in use.
    ///
    /// The client should try again with a different email address.
    #[error("the email is already in use")]
    DuplicateEmail,

    /// The user provided an invalid combination of email and password.
    #[error("invalid email or password")]
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

    /// The user ID used to create a category does not refer to a valid user.
    #[error("the user ID does not refer to a valid user.")]
    InvalidUser,

    /// The category ID used to create a transaction did not match a valid category.
    #[error("the category ID does not refer to a valid category")]
    InvalidCategory,

    /// An empty string was used to create a category name.
    #[error("an empty string is not a valid category name")]
    EmptyCategoryName,

    /// A date in the future was used to create a transaction.
    ///
    /// Transactions record events that have already happened, therefore future
    /// dates are not allowed.
    #[error("transaction dates must not be later than the current date")]
    FutureDate,

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
}

impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        match value {
            // Code 2067 occurs when a UNIQUE constraint failed.
            rusqlite::Error::SqliteFailure(sql_error, Some(ref desc))
                if sql_error.extended_code == 2067 && desc.contains("email") =>
            {
                Error::DuplicateEmail
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
            Error::EmptyCategoryName => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "category name cannot be emtpy.",
            ),
            Error::NotFound => (StatusCode::NOT_FOUND, "Resource not found"),
            Error::InternalError(err) => {
                tracing::error!("An unexpected error occurred: {}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
            // Any errors that are not handled above are not intended to be shown to the client.
            error => {
                println!("An unexpected error occurred: {}", error);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        }
        .into_response()
    }
}

/// Log the request and response for each request.
///
/// Both the request and response are logged at the `info` level.
/// If the response body is longer than [LOG_BODY_LENGTH_LIMIT] bytes, it is
/// truncated and logged at the `debug` level.
pub async fn logging_middleware(request: Request, next: Next) -> Response {
    // TODO: redact passwords from logs
    let (headers, body_text) = extract_header_and_body_text_from_request(request).await;
    log_request(&headers, &body_text);

    let request = Request::from_parts(headers, body_text.into());
    let response = next.run(request).await;

    let (headers, body_text) = extract_header_and_body_text_from_response(response).await;
    log_response(&headers, &body_text);

    Response::from_parts(headers, body_text.into())
}

async fn extract_header_and_body_text_from_request(
    request: Request,
) -> (axum::http::request::Parts, String) {
    let (headers, body) = request.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();

    (headers, String::from_utf8_lossy(&body_bytes).to_string())
}

async fn extract_header_and_body_text_from_response(
    response: Response,
) -> (axum::http::response::Parts, String) {
    let (headers, body) = response.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();

    (headers, String::from_utf8_lossy(&body_bytes).to_string())
}

const LOG_BODY_LENGTH_LIMIT: usize = 64;

fn log_request(headers: &axum::http::request::Parts, body: &str) {
    if body.len() > LOG_BODY_LENGTH_LIMIT {
        tracing::info!(
            "Sending request: {headers:#?}\nbody: {:}...",
            &body[..LOG_BODY_LENGTH_LIMIT]
        );
        tracing::debug!("Sending request: {headers:#?}\nbody: {body:?}");
    } else {
        tracing::info!("Sending request: {headers:#?}\nbody: {body:?}");
    }
}

fn log_response(headers: &axum::http::response::Parts, body: &str) {
    if body.len() > LOG_BODY_LENGTH_LIMIT {
        tracing::info!(
            "Sending response: {headers:#?}\nbody: {:}...",
            &body[..LOG_BODY_LENGTH_LIMIT]
        );
        tracing::debug!("Sending response: {headers:#?}\nbody: {body:?}");
    } else {
        tracing::info!("Sending response: {headers:#?}\nbody: {body:?}");
    }
}
