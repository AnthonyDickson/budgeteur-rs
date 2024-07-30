use axum::{response::IntoResponse, Json};
use serde_json::json;

use crate::auth::Claims;

pub async fn hello(claims: Claims) -> impl IntoResponse {
    Json(json!({
        "email": claims.email
    }))
}
