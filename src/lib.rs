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
    Json,
};
use axum_server::Handle;
use models::{CategoryError, TransactionError};
use serde_json::json;
use thiserror::Error;
use tokio::signal;

use auth::AuthError;
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
#[derive(Debug, Error)]
enum AppError {
    /// The requested resource was not found. The client should check that the parameters (e.g., ID) are correct and that the resource has been created.
    #[error("the requested resource could not be found")]
    NotFound,

    /// An error occurred while operating on a category.
    #[error("category error")]
    CategoryError(CategoryError),

    /// An error occurred while operating on a transaction.
    #[error("transaction error")]
    TransactionError(TransactionError),

    /// The user is not authenticated/authorized to access the given resource.
    #[error("auth error")]
    AuthError(AuthError),
}

impl From<AuthError> for AppError {
    fn from(value: AuthError) -> Self {
        AppError::AuthError(value)
    }
}

impl From<CategoryError> for AppError {
    fn from(e: CategoryError) -> Self {
        tracing::error!("{e:?}");

        AppError::CategoryError(e)
    }
}

impl From<TransactionError> for AppError {
    fn from(value: TransactionError) -> Self {
        AppError::TransactionError(value)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::CategoryError(e) => return e.into_response(),
            AppError::TransactionError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal server error: {e:?}"),
            ),

            AppError::AuthError(e) => (StatusCode::UNAUTHORIZED, format!("Auth error: {e:?}")),
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                "The requested resource could not be found.".to_string(),
            ),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
