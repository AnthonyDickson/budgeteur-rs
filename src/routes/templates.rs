/*! Askama HTML templates that are shared between views. */

use askama::Template;

use crate::models::Transaction;

use super::endpoints;

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

/// Renders a transaction as a 5 column table row.
#[derive(Template)]
#[template(path = "partials/dashboard/transaction.html")]
pub struct TransactionRow {
    pub transaction: Transaction,
}

#[derive(Template)]
#[template(path = "partials/register/form.html")]
pub struct RegisterFormTemplate<'a> {
    pub log_in_route: &'a str,
    pub create_user_route: &'a str,
    pub email_input: EmailInputTemplate<'a>,
    pub password_input: PasswordInputTemplate<'a>,
    pub confirm_password_input: ConfirmPasswordInputTemplate<'a>,
}

impl Default for RegisterFormTemplate<'_> {
    fn default() -> Self {
        Self {
            log_in_route: endpoints::LOG_IN_VIEW,
            create_user_route: endpoints::USERS,
            email_input: EmailInputTemplate::default(),
            password_input: PasswordInputTemplate::default(),
            confirm_password_input: ConfirmPasswordInputTemplate::default(),
        }
    }
}

#[derive(Template, Default)]
#[template(path = "partials/register/inputs/confirm_password.html")]
pub struct ConfirmPasswordInputTemplate<'a> {
    pub error_message: &'a str,
}
