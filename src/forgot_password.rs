use askama::Template;
use axum::{http::StatusCode, response::Response};

use crate::shared_templates::render;

#[derive(Template)]
#[template(path = "views/forgot_password.html")]
struct ForgotPasswordPageTemplate;

/// Renders a page describing how the user's password can be reset.
pub async fn get_forgot_password_page() -> Response {
    render(StatusCode::OK, ForgotPasswordPageTemplate)
}
