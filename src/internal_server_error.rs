//! Defines the templates and route handlers for the page to display for an internal server error.
use askama::Template;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;

use crate::{endpoints, shared_templates::render};

/// Get a response that will redirect the client to the internal server error 500 page.
///
/// **Note**: This redirect is intended to be served as a response to a POST request initiated by HTMX.
/// Route handlers using GET should use `axum::response::Redirect` to redirect via a response.
pub(crate) fn get_internal_server_error_redirect() -> Response {
    (
        HxRedirect(endpoints::INTERNAL_ERROR_VIEW.to_owned()),
        StatusCode::INTERNAL_SERVER_ERROR,
    )
        .into_response()
}

#[derive(Template)]
#[template(path = "views/internal_server_error_500.html")]
pub struct InternalServerErrorPageTemplate<'a> {
    pub description: &'a str,
    pub fix: &'a str,
}

impl Default for InternalServerErrorPageTemplate<'_> {
    fn default() -> Self {
        Self {
            description: "Sorry, something went wrong.",
            fix: "Try again later or check the server logs",
        }
    }
}

pub async fn get_internal_server_error_page() -> Response {
    render_internal_server_error(Default::default())
}

pub fn render_internal_server_error(template: InternalServerErrorPageTemplate) -> Response {
    render(StatusCode::INTERNAL_SERVER_ERROR, template)
}
