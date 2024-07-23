use axum::{Json, response::Html, Router, routing::get};
use axum::http::StatusCode;
use axum::routing::put;
use serde::{Deserialize, Serialize};
use tokio::signal;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(handler))
        .route("/json", get(test_json))
        .route("/hello", put(hello_json));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
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
        _ = ctrl_c => {},
        _ = terminate => {},
    }
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
            .await
            .json::<Foo>();

        assert_eq!(response.bar, "baz");
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
            .await
            .json::<Greeting>();

        assert_eq!(response.text, "Hello, World!");
    }
}