/*! Askama HTML templates that are shared between views. */

use askama::Template;

#[derive(Template, Default)]
#[template(path = "partials/register/inputs/email.html")]
pub struct EmailInputTemplate<'a> {
    pub value: &'a str,
    pub error_message: &'a str,
}

#[derive(Template, Default)]
#[template(path = "partials/register/inputs/password.html")]
pub struct PasswordInputTemplate<'a> {
    pub value: &'a str,
    pub min_length: usize,
    pub error_message: &'a str,
}

#[derive(Template)]
#[template(path = "components/nav_link.html")]
pub struct Link<'a> {
    pub url: &'a str,
    pub title: &'a str,
    pub is_current: bool,
}

#[derive(Template)]
#[template(path = "partials/navbar.html")]
pub struct NavbarTemplate<'a> {
    pub links: Vec<Link<'a>>,
}
