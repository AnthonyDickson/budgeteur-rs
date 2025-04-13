//! This file defines the type `Transaction`, the core type of the budgeting part of the
//! application.

use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};

use crate::{
    Error,
    models::{DatabaseID, UserID},
};

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
    // TODO: Add import ID: Option<u64>
    // TODO: Make all fields pub and remove the accessor methods.
}

impl Transaction {
    /// Create a new transaction without checking invariants such as a valid date.
    ///
    /// This function is intended to be used when loading data from a trusted source such as the
    /// application databases/stores which validate data on insertion. You **should not** use this
    /// function with unvalidated data.
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
    pub fn date(mut self, date: Date) -> Result<Self, Error> {
        if date > OffsetDateTime::now_utc().date() {
            return Err(Error::FutureDate);
        }

        self.date = date;
        Ok(self)
    }

    /// Set the description for the transaction.
    pub fn description(mut self, description: &str) -> Self {
        self.description = description.to_owned();
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

    use crate::models::{TransactionBuilder, UserID};

    use super::{Error, Transaction};

    #[test]
    fn new_fails_on_future_date() {
        let tomorrow = OffsetDateTime::now_utc()
            .date()
            .checked_add(Duration::days(1))
            .unwrap();
        let user_id = UserID::new(42);

        let result = TransactionBuilder::new(123.45, user_id).date(tomorrow);

        assert_eq!(result, Err(Error::FutureDate));
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
            .description(&description)
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
