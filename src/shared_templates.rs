/*! Askama HTML templates that are shared between views. */

use askama::Template;

use crate::transaction::Transaction;

use crate::endpoints;

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

/// Renders a log-in form with client-side and server-side validation.
#[derive(Template)]
#[template(path = "partials/log_in/form.html")]
pub struct LogInFormTemplate<'a> {
    pub email_input: EmailInputTemplate<'a>,
    pub password_input: PasswordInputTemplate<'a>,
    pub log_in_route: &'a str,
    pub forgot_password_route: &'a str,
    pub register_route: &'a str,
}

impl Default for LogInFormTemplate<'_> {
    fn default() -> Self {
        Self {
            email_input: Default::default(),
            password_input: Default::default(),
            log_in_route: endpoints::LOG_IN_API,
            forgot_password_route: endpoints::FORGOT_PASSWORD_VIEW,
            register_route: endpoints::REGISTER_VIEW,
        }
    }
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

/// Renders the form for creating a category.
#[derive(Template)]
#[template(path = "partials/new_category_form.html")]
pub struct NewCategoryFormTemplate<'a> {
    pub category_route: &'a str,
    pub error_message: &'a str,
}
