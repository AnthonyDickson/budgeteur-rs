//! This file defines the type `Transaction`, the core type of the budgeting part of the
//! application.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{Date, OffsetDateTime};

use crate::models::{DatabaseID, UserID};

/// Errors that can occur during the creation or retrieval of a transaction.
#[derive(Debug, Error, PartialEq)]
pub enum TransactionError {
    /// A date in the future was used to create a transaction.
    ///
    /// Transactions record events that have already happened, therefore future dates are disallowed.
    #[error("transaction dates must not be later than the current date")]
    FutureDate,

    /// The category ID used to create a transaction did not match a valid category.
    #[error("the category ID does not refer to a valid category")]
    InvalidCategory,

    /// The user ID used to create a transaction did not match a valid user.
    #[error("the user ID does not refer to a valid user")]
    InvalidUser,

    /// There was no transaction in the database that matched the given details.
    #[error("a transaction with the given details could not be found")]
    NotFound,

    /// There was an unexpected and unhandled SQL error.
    #[error("an unexpected error occurred: {0}")]
    SqlError(rusqlite::Error),

    /// There was an unexpected and unhandled error.
    #[error("an unexpected error occurred: {0}")]
    Unspecified(String),
}

impl From<rusqlite::Error> for TransactionError {
    fn from(value: rusqlite::Error) -> Self {
        match value {
            rusqlite::Error::QueryReturnedNoRows => TransactionError::NotFound,
            value => {
                tracing::error!("an unhandled SQL error occurred: {}", value);
                TransactionError::SqlError(value)
            }
        }
    }
}

/// An expense or income, i.e. an event where money was either spent or earned.
///
/// To create a new `Transaction`, use [Transaction::build].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    id: DatabaseID,
    amount: f64,
    date: Date,
    description: String,
    category_id: Option<DatabaseID>,
    user_id: UserID,
}

impl Transaction {
    /// Create a new transaction without checking invariants such as a valid date.
    ///
    /// This function is intended to be used when loading data from a trusted source such as the
    /// application databases/stores which validate data on insertion. You **should not** use this
    /// function with unvaldated data.
    ///
    /// This function has `_unchecked` in the name but is not `unsafe`, because if an invalid date
    /// is provided it may cause incorrect behaviour but will not affect memory safety.
    pub fn new_unchecked(
        id: DatabaseID,
        amount: f64,
        date: Date,
        description: String,
        category_id: Option<DatabaseID>,
        user_id: UserID,
    ) -> Self {
        Self {
            id,
            amount,
            date,
            description,
            category_id,
            user_id,
        }
    }

    /// Create a new transaction.
    ///
    /// Shortcut for [TransactionBuilder::new] for discoverability.
    pub fn build(amount: f64, user_id: UserID) -> TransactionBuilder {
        TransactionBuilder::new(amount, user_id)
    }

    /// The ID of the transaction.
    pub fn id(&self) -> DatabaseID {
        self.id
    }

    /// The amount of money spent or earned in this transaction.
    pub fn amount(&self) -> f64 {
        self.amount
    }

    /// When the transaction happened.
    pub fn date(&self) -> &Date {
        &self.date
    }

    /// A text description of what the transaction was for.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// A user-defined category that describes the type of the transaction.
    pub fn category_id(&self) -> Option<DatabaseID> {
        self.category_id
    }

    /// The ID of the user that created this transaction.
    pub fn user_id(&self) -> UserID {
        self.user_id
    }
}

/// Builder for creating a new [Transaction].
///
/// The function for finalizing the builder is [TransactionBuilder::finalise].
#[derive(Debug, Clone, PartialEq)]
pub struct TransactionBuilder {
    amount: f64,
    date: Date,
    description: String,
    category_id: Option<DatabaseID>,
    user_id: UserID,
}

impl TransactionBuilder {
    /// Create a new transaction.
    ///
    /// Finalize the builder with [TransactionBuilder::finalise].
    pub fn new(amount: f64, user_id: UserID) -> Self {
        Self {
            amount,
            date: OffsetDateTime::now_utc().date(),
            description: String::new(),
            category_id: None,
            user_id,
        }
    }

    /// Build the final [Transaction] instance.
    pub fn finalise(self, id: DatabaseID) -> Transaction {
        Transaction {
            id,
            amount: self.amount,
            date: self.date,
            description: self.description,
            category_id: self.category_id,
            user_id: self.user_id,
        }
    }

    /// Set the date for the transaction.
    ///
    /// # Errors
    /// This function will return an error if `date` is a date in the future.
    pub fn date(mut self, date: Date) -> Result<Self, TransactionError> {
        if date > OffsetDateTime::now_utc().date() {
            return Err(TransactionError::FutureDate);
        }

        self.date = date;
        Ok(self)
    }

    /// Set the description for the transaction.
    pub fn description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    /// Set the category for the transaction.
    pub fn category(mut self, category_id: Option<DatabaseID>) -> Self {
        self.category_id = category_id;
        self
    }
}

#[cfg(test)]
mod transaction_builder_tests {
    use std::f64::consts::PI;

    use time::{Duration, OffsetDateTime};

    use crate::models::TransactionBuilder;
    use crate::models::UserID;

    use super::Transaction;
    use super::TransactionError;

    #[test]
    fn new_fails_on_future_date() {
        let tomorrow = OffsetDateTime::now_utc()
            .date()
            .checked_add(Duration::days(1))
            .unwrap();
        let user_id = UserID::new(42);

        let result = TransactionBuilder::new(123.45, user_id).date(tomorrow);

        assert_eq!(result, Err(TransactionError::FutureDate));
    }

    #[test]
    fn new_succeeds_on_today() {
        let user_id = UserID::new(42);
        let today = OffsetDateTime::now_utc().date();

        let transaction_buider = TransactionBuilder::new(123.45, user_id).date(today);

        assert!(transaction_buider.is_ok());

        let transaction = transaction_buider.unwrap().finalise(1);
        assert_eq!(transaction.date(), &today);
    }

    #[test]
    fn new_succeeds_on_past_date() {
        let user_id = UserID::new(42);

        let yesterday = OffsetDateTime::now_utc()
            .date()
            .checked_sub(Duration::days(1))
            .unwrap();

        let result = TransactionBuilder::new(123.45, user_id).date(yesterday);

        assert!(result.is_ok());

        let transaction = result.unwrap().finalise(1);
        assert_eq!(transaction.date(), &yesterday);
    }

    #[test]
    fn insert_transaction_succeeds() {
        let id = 123;
        let amount = PI;
        let date = OffsetDateTime::now_utc().date();
        let description = "Rust Pie".to_string();
        let category_id = Some(42);
        let user_id = UserID::new(321);

        let transaction = Transaction::build(amount, user_id)
            .category(category_id)
            .description(description.clone())
            .date(date)
            .unwrap()
            .finalise(id);

        assert_eq!(transaction.id(), id);
        assert_eq!(transaction.amount(), amount);
        assert_eq!(transaction.date(), &date);
        assert_eq!(transaction.description(), description);
        assert_eq!(transaction.category_id(), category_id);
        assert_eq!(transaction.user_id(), user_id);
    }
}
