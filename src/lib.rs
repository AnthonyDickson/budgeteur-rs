use std::time::Duration;

use askama::Template;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::response::Html;
use axum_server::Handle;
use serde_json::json;
use tokio::signal;

use auth::AuthError;
pub use config::AppState;
pub use routes::build_router;

use crate::db::DbError;

mod auth;
mod config;
pub mod db;
pub mod models;
mod routes;

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

enum AppError {
    /// The requested resource was not found. The client should check that the parameters (e.g., ID) are correct and that the resource has been created.
    NotFound,
    /// An error occurred while accessing the application's database. This may be due to a database constraint being violated (e.g., foreign keys).
    DatabaseError(DbError),
    /// The user is not authenticated/authorized to access the given resource.
    AuthError(AuthError),
}

impl From<AuthError> for AppError {
    fn from(value: AuthError) -> Self {
        AppError::AuthError(value)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::DatabaseError(e) => (
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

impl From<DbError> for AppError {
    fn from(e: DbError) -> Self {
        tracing::error!("{e:?}");

        AppError::DatabaseError(e)
    }
}

const INTERNAL_SERVER_ERROR_HTML: &str = "
    <!doctype html>
    <html lang=\"en\">
        <head>
            <title>500 Internal Server Error</title>
        </head>
        <body>
        <h1>Internal Server Error</h1>
        <p>The server was unable to complete your request. Please try again later.</p>
        </body>
    </html>
";

/// Newtype wrapper for `askama::Template`.
/// Implements `axum::response::IntoResponse` to reduce repetitive boilerplate code for handling rendering and its errors.
///
/// # Examples
/// ```no_run
/// use askama::Template;
/// use axum::{Extension, response::{IntoResponse, Response}};
/// #
/// # use backrooms_rs::{HtmlTemplate, models::UserID};
///
/// #[derive(Template)]
/// #[template(source = "<p>Hello, User #{{ user_id }}!</p>", ext = "html")]
/// struct HelloUserTemplate {
///     user_id: UserID,
/// }
///
/// async fn get_dashboard_page(Extension(user_id): Extension<UserID>) -> Response {
///     HtmlTemplate(HelloUserTemplate { user_id }).into_response()
/// }
/// ```
pub struct HtmlTemplate<T>(pub T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => (StatusCode::OK, Html(html)).into_response(),
            Err(err) => {
                tracing::error!("Failed to render template. Error: {err}");

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    INTERNAL_SERVER_ERROR_HTML,
                )
                    .into_response()
            }
        }
    }
}
