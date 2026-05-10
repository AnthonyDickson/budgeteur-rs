//! Server health check endpoint — returns a minimal JSON response so clients and
//! orchestrators can verify the server is running without hitting auth-guarded routes.

use axum::{Json, response::IntoResponse};
use serde::Serialize;

/// The health-check response body.
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
}

/// Return `{"status": "ok"}` to signal the server is alive.
pub async fn get_health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".into(),
    })
}

#[cfg(test)]
mod health_tests {
    use axum::response::IntoResponse;

    use super::get_health;

    #[tokio::test]
    async fn returns_200_with_ok_status() {
        let response = get_health().await.into_response();
        assert_eq!(response.status(), 200);
    }
}
