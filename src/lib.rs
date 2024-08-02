use std::{env, env::VarError, time::Duration};

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum_server::Handle;
use serde_json::json;
use tokio::signal;

pub use config::AppConfig;

use crate::{
    auth::{hash_password, Credentials},
    db::insert_user,
};

pub mod auth;
mod config;
pub mod db;
mod services;

/// Return a router with all the app's routes.
pub fn build_router() -> Router<AppConfig> {
    // TODO: Have each module build routes and compose the routes here.
    Router::new()
        .route("/", get(|| async { StatusCode::IM_A_TEAPOT }))
        .route("/user", post(create_user))
        .route("/sign_in", post(auth::sign_in))
        .route("/protected", get(services::hello))
}

/// Get a port number from the environment variable `env_key` if set, otherwise return `default_port`.
///
/// # Panics
/// This function may panic if the environment variable `env_key` is not valid unicode.
///
/// This function may panic if the environment variable `env_key` cannot be parsed as an integer.
///
/// ```rust,should_panic
/// use std::env;
/// use backrooms_rs::parse_port_or_default;
///
/// unsafe { env::set_var("FOO", "123s"); }
/// // This will panic!
/// let port = parse_port_or_default("FOO", 1234);
/// ```
///
/// # Examples
///
/// ```
/// use std::env;
/// use backrooms_rs::parse_port_or_default;
///
/// assert_eq!(parse_port_or_default("FOO", 1234), 1234);
///
/// unsafe { env::set_var("FOO", "4321"); }
/// assert_eq!(parse_port_or_default("FOO", 1234), 4321);
/// # unsafe { env::remove_var("FOO"); }
/// ```
pub fn parse_port_or_default(env_key: &str, default_port: u16) -> u16 {
    let port_string = match env::var(env_key) {
        Ok(string) => string,
        Err(VarError::NotPresent) => {
            tracing::debug!(
                "The environment variable '{}' was not set, using the default port {}.",
                env_key,
                default_port
            );
            return default_port;
        }
        Err(e) => {
            tracing::error!(
                "An error occurred retrieving the environment variable '{}': {}",
                env_key,
                e
            );
            panic!();
        }
    };

    match port_string.parse() {
        Ok(port_number) => port_number,
        Err(e) => {
            tracing::error!("An error occurred parsing the port number '{}' from the environment variable '{}': {}", port_string, env_key, e);
            panic!();
        }
    }
}

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

async fn create_user(
    State(state): State<AppConfig>,
    Json(user_data): Json<Credentials>,
) -> Response {
    let connection_lock = state.db_connection();
    let connection = match connection_lock.lock() {
        Ok(connection_mutex) => connection_mutex,
        Err(e) => {
            tracing::error!("{e:#?}");

            return AppError::InternalError.into_response();
        }
    };

    let password_hash = match hash_password(&user_data.password) {
        Ok(password_hash) => password_hash,
        Err(e) => {
            tracing::error!("Error hashing password: {e:?}");
            return AppError::InternalError.into_response();
        }
    };

    match insert_user(&user_data.email, &password_hash, &connection) {
        Ok(user) => (StatusCode::CREATED, Json(user)).into_response(),
        Err(e) => AppError::UserCreation(format!("Could not create user: {e:?}")).into_response(),
    }
}

enum AppError {
    InternalError,
    UserCreation(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::InternalError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            AppError::UserCreation(description) => (StatusCode::OK, description),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::{http::StatusCode, routing::post, Router};
    use axum_test::TestServer;
    use rusqlite::Connection;
    use serde_json::json;

    use crate::auth::verify_password;
    use crate::{
        create_user,
        db::{initialize, User},
        AppConfig,
    };

    fn get_test_app_config() -> AppConfig {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppConfig::new(db_connection, "42".to_string())
    }

    #[tokio::test]
    async fn test_create_user() {
        let app = Router::new()
            .route("/user", post(create_user))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let email = "test@test.com";
        let password = "hunter2";

        let response = server
            .post("/user")
            .content_type("application/json")
            .json(&json!({
                "email": email,
                "password": password
            }))
            .await;

        response.assert_status(StatusCode::CREATED);

        let user = response.json::<User>();
        assert_eq!(user.email(), email);
        assert!(verify_password(password, user.password()).unwrap());
    }
}
