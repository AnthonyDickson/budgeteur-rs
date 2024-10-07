/*! Askama HTML templates that are shared between views. */

use askama::Template;

use crate::models::Transaction;

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
