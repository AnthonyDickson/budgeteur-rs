/*! This module defines and implements the data structures, response handlers and functions for authenticating a user and handling cookie auth. */

use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub mod cookie;
pub mod log_in;
pub mod middleware;

/// Errors that can occur when authenticating a user.
#[derive(Debug, PartialEq)]
pub enum AuthError {
    /// The user provided an invalid combination of email and password.
    InvalidCredentials,
    /// An unexpected error occurred when hashing a password or parsing a password hash.
    InternalError,
    /// Either the user ID or expiry cookie is missing from the cookie jar in the request.
    CookieMissing,
    /// There was an error parsing the date in the cookie or creating the new expiry date time.
    DateError,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response<Body> {
        let (status, error_message) = match self {
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid credentials"),
            AuthError::CookieMissing => (StatusCode::UNAUTHORIZED, "Invalid cookie state"),
            AuthError::DateError => (StatusCode::INTERNAL_SERVER_ERROR, "Invalid date format"),
            AuthError::InternalError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
