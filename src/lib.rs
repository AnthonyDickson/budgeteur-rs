use std::{env, env::VarError, time::Duration};

use axum::{
    http::StatusCode,
    response::Html,
    routing::{get, post, put},
    Json, Router,
};
use axum_server::Handle;
use serde::{Deserialize, Serialize};
use tokio::signal;
use tracing;

pub use config::AppConfig;

pub mod auth;
mod config;
mod db;
mod services;

/// Return a router with all the app's routes.
pub fn build_router() -> Router<AppConfig> {
    Router::new()
        .route("/", get(handler))
        .route("/json", get(test_json))
        .route("/hello", put(hello_json))
        .route("/signin", post(auth::sign_in))
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
        Err(e) if e == VarError::NotPresent => {
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

pub async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

pub async fn test_json() -> (StatusCode, Json<Foo>) {
    let foo = Foo {
        bar: "baz".to_string(),
    };

    (StatusCode::OK, Json(foo))
}

#[derive(Serialize, Deserialize)]
pub struct Foo {
    bar: String,
}

pub async fn hello_json(Json(payload): Json<Name>) -> (StatusCode, Json<Greeting>) {
    let greeting = Greeting {
        text: format!("Hello, {}!", payload.name),
    };

    (StatusCode::CREATED, Json(greeting))
}

#[derive(Deserialize)]
pub struct Name {
    name: String,
}

#[derive(Serialize, Deserialize)]
pub struct Greeting {
    text: String,
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum::routing::{get, put};
    use axum::Router;
    use axum_test::TestServer;
    use serde_json::json;

    use crate::{handler, hello_json, test_json, Foo, Greeting};

    #[tokio::test]
    async fn test_root() {
        let app = Router::new().route("/", get(handler));

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server.get("/").await;

        response.assert_status_ok();
    }

    #[tokio::test]
    async fn test_get_json() {
        let app = Router::new().route("/json", get(test_json));

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server.get("/json").await;
        response.assert_status_ok();

        let response_json = response.json::<Foo>();
        assert_eq!(response_json.bar, "baz");
    }

    #[tokio::test]
    async fn test_post_json() {
        let app = Router::new().route("/hello", put(hello_json));

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server
            .put("/hello")
            .content_type(&"application/json")
            .json(&json!({
                "name": "World",
            }))
            .await;
        response.assert_status(StatusCode::CREATED);

        let response_json = response.json::<Greeting>();
        assert_eq!(response_json.text, "Hello, World!");
    }
}
