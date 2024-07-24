use std::{net::SocketAddr, path::PathBuf, time::Duration};

use axum::{
    BoxError,
    extract::Host,
    handler::HandlerWithoutStateExt,
    http::{StatusCode, Uri},
    Json,
    response::{Html, Redirect},
    Router,
    routing::{get, put},
};
use axum_server::{Handle, tls_rustls::RustlsConfig};
use serde::{Deserialize, Serialize};
use tokio::signal;
use tracing_subscriber::{filter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

// TODO: Add route for creating user (email + password). Hash passwords with a salt which is stored alongside the hashed and salted password.
// TODO: Add route for login which issues a JWT on success (What happens on failure?).
// TODO: Add middleware that checks for valid JWT before providing access to protected routes.
#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .pretty()
                // Add an `INFO` filter to the stdout logging layer
                .with_filter(filter::LevelFilter::DEBUG)
        )
        .init();

    let ports = Ports {
        http: 7878,
        https: 3000,
    };

    // Spawn a second server to redirect http requests to this server
    tokio::spawn(redirect_http_to_https(ports));

    // configure certificate and private key used by https
    let config = RustlsConfig::from_pem_file(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("../self_signed_certs/cert.pem"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("../self_signed_certs/key.pem"),
    )
        .await
        .unwrap();

    let app = Router::new()
        .route("/", get(handler))
        .route("/json", get(test_json))
        .route("/hello", put(hello_json));

    let handle = Handle::new();
    // Spawn a task to gracefully shutdown server.
    tokio::spawn(graceful_shutdown(handle.clone()));

    // run https server
    let addr = SocketAddr::from(([127, 0, 0, 1], ports.https));
    tracing::debug!("HTTPS server listening on {}", addr);
    axum_server::bind_rustls(addr, config)
        .handle(handle)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn graceful_shutdown(handle: Handle) {
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

async fn redirect_http_to_https(ports: Ports) {
    fn make_https(host: String, uri: Uri, ports: Ports) -> Result<Uri, BoxError> {
        let mut parts = uri.into_parts();

        parts.scheme = Some(axum::http::uri::Scheme::HTTPS);

        if parts.path_and_query.is_none() {
            parts.path_and_query = Some("/".parse().unwrap());
        }

        let https_host = host.replace(&ports.http.to_string(), &ports.https.to_string());
        parts.authority = Some(https_host.parse()?);

        Ok(Uri::from_parts(parts)?)
    }

    let redirect = move |Host(host): Host, uri: Uri| async move {
        match make_https(host, uri, ports) {
            Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
            Err(error) => {
                tracing::warn!(%error, "failed to convert URI to HTTPS");
                Err(StatusCode::BAD_REQUEST)
            }
        }
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], ports.http));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::debug!("HTTPS redirect server listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, redirect.into_make_service())
        .await
        .unwrap();
}

#[derive(Clone, Copy)]
struct Ports {
    http: u16,
    https: u16,
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

async fn test_json() -> (StatusCode, Json<Foo>) {
    let foo = Foo {
        bar: "baz".to_string()
    };

    (StatusCode::OK, Json(foo))
}

#[derive(Serialize, Deserialize)]
struct Foo {
    bar: String,
}

async fn hello_json(Json(payload): Json<Name>) -> (StatusCode, Json<Greeting>) {
    let greeting = Greeting {
        text: format!("Hello, {}!", payload.name)
    };

    (StatusCode::CREATED, Json(greeting))
}

#[derive(Deserialize)]
struct Name {
    name: String,
}
#[derive(Serialize, Deserialize)]
struct Greeting {
    text: String,
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum::Router;
    use axum::routing::{get, put};
    use axum_test::TestServer;
    use serde_json::json;

    use crate::{Foo, Greeting, handler, hello_json, test_json};

    #[tokio::test]
    async fn test_root() {
        let app = Router::new()
            .route("/", get(handler));

        let server = TestServer::new(app)
            .expect("Could not create test server.");

        let response = server.get("/")
            .await;

        response.assert_status_ok();
    }

    #[tokio::test]
    async fn test_get_json() {
        let app = Router::new()
            .route("/json", get(test_json));

        let server = TestServer::new(app)
            .expect("Could not create test server.");

        let response = server.get("/json")
            .await;
        response.assert_status_ok();

        let response_json = response.json::<Foo>();
        assert_eq!(response_json.bar, "baz");
    }

    #[tokio::test]
    async fn test_post_json() {
        let app = Router::new()
            .route("/hello", put(hello_json));

        let server = TestServer::new(app)
            .expect("Could not create test server.");

        let response = server.put("/hello")
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