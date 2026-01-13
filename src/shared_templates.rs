/*! Askama HTML templates that are shared between views. */

use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use crate::endpoints;

#[inline]
pub fn render(status_code: StatusCode, template: impl Template) -> Response {
    match template.render() {
        Ok(body) => (status_code, Html(body)).into_response(),
        Err(error) => {
            tracing::error!("Could not render template: {error}");
            (StatusCode::SEE_OTHER, endpoints::INTERNAL_ERROR_VIEW).into_response()
        }
    }
}
