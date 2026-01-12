/*! Askama HTML templates that are shared between views. */

use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use crate::endpoints;

#[derive(Template, Default)]
#[template(path = "partials/register/inputs/password.html")]
pub struct PasswordInputTemplate<'a> {
    pub value: &'a str,
    pub min_length: u8,
    pub error_message: &'a str,
}

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
