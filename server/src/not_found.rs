use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use crate::html::error_view;

pub struct NotFoundError;

impl NotFoundError {
    pub fn into_html(self) -> Html<String> {
        Html(
            error_view(
                "Page Not Found",
                "404",
                "Sorry, we can't find that page.",
                "You'll find lots to explore on the home page.",
            )
            .into_string(),
        )
    }
}

impl IntoResponse for NotFoundError {
    fn into_response(self) -> Response {
        (StatusCode::NOT_FOUND, self.into_html()).into_response()
    }
}

pub async fn get_404_not_found() -> Response {
    NotFoundError.into_response()
}
