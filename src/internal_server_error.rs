//! Defines the templates and route handlers for the page to display for an internal server error.
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use crate::html::error_view;

pub struct InternalServerError<'a> {
    pub description: &'a str,
    pub fix: &'a str,
}

impl Default for InternalServerError<'_> {
    fn default() -> Self {
        Self {
            description: "Sorry, something went wrong.",
            fix: "Try again later or check the server logs",
        }
    }
}

impl InternalServerError<'_> {
    pub fn into_html(self) -> Html<String> {
        Html(error_view("Internal Server Error", "500", self.description, self.fix).into_string())
    }
}

impl IntoResponse for InternalServerError<'_> {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.into_html()).into_response()
    }
}

pub async fn get_internal_server_error_page() -> Response {
    InternalServerError::default().into_response()
}
