use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

#[derive(Template)]
#[template(path = "views/not_found_404.html")]
pub struct NotFoundTemplate;

pub async fn get_404_not_found() -> Response {
    get_404_not_found_response()
}

pub fn get_404_not_found_response() -> Response {
    (
        StatusCode::NOT_FOUND,
        Html(NotFoundTemplate.render().unwrap_or("Not found".to_owned())),
    )
        .into_response()
}
