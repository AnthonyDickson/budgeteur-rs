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

use axum_server::Handle;
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
mod error;
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
pub use error::Error;
pub use logging::{LOG_BODY_LENGTH_LIMIT, logging_middleware};
pub use password::{PasswordHash, ValidatedPassword};
pub use routing::build_router;
pub use user::{User, UserID, get_user_by_id};

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
