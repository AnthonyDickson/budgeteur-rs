use askama_axum::Template;
use axum::response::{IntoResponse, Response};

#[derive(Template)]
#[template(path = "views/forgot_password.html")]
struct ForgotPasswordPageTemplate;

/// Renders a page describing how the user's password can be reset.
pub async fn get_forgot_password_page() -> Response {
    ForgotPasswordPageTemplate.into_response()
}
