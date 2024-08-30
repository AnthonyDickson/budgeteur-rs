use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::{DatabaseID, UserID};

/// An expense or income, i.e. an event where money was either spent or earned.
///
/// New instances should be created through `Transaction::insert(...)`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    id: DatabaseID,
    amount: f64,
    date: NaiveDate,
    description: String,
    category_id: DatabaseID,
    user_id: UserID,
}

impl Transaction {
    /// Create a new `Transaction` and validate `date`.
    ///
    /// Note that this function does not insert the object into the application database.
    /// Consider using the `insert` trait function on a `NewTransaction` to insert and create a transaction at the same time.
    ///
    /// # Errors
    ///
    /// This function will return an error if `date` is a future date.
    pub fn new(
        id: DatabaseID,
        amount: f64,
        date: NaiveDate,
        description: String,
        category_id: DatabaseID,
        user_id: UserID,
    ) -> Result<Self, NewTransactionError> {
        match date <= Utc::now().date_naive() {
            true => Ok(Self {
                id,
                amount,
                date,
                description,
                category_id,
                user_id,
            }),
            false => Err(NewTransactionError(date)),
        }
    }

    /// Create a new `Transaction` without validating `date`.
    ///
    /// The caller should ensure that `date` is less than or equal to today (server time).
    ///
    /// This function has `_unchecked` in the name but is not `unsafe`, because if the `date` invariant is violated it will cause incorrect behaviour but not affect memory safety.
    pub fn new_unchecked(
        id: DatabaseID,
        amount: f64,
        date: NaiveDate,
        description: String,
        category_id: DatabaseID,
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

    pub fn id(&self) -> DatabaseID {
        self.id
    }

    pub fn amount(&self) -> f64 {
        self.amount
    }

    pub fn date(&self) -> &NaiveDate {
        &self.date
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn category_id(&self) -> DatabaseID {
        self.category_id
    }

    pub fn user_id(&self) -> UserID {
        self.user_id
    }
}

#[derive(Debug, Error)]
#[error("{0} is not a valid date for a transaction")]
pub struct NewTransactionError(NaiveDate);

#[derive(Debug, Deserialize, Serialize)]
pub struct NewTransaction {
    amount: f64,
    date: NaiveDate,
    description: String,
    category_id: DatabaseID,
    user_id: UserID,
}

impl NewTransaction {
    /// Create a `NewTransaction` and validate `date`.
    ///
    /// # Errors
    ///
    /// This function will return an error if `date` is after today (server time).
    pub fn new(
        amount: f64,
        date: NaiveDate,
        description: String,
        category_id: DatabaseID,
        user_id: UserID,
    ) -> Result<Self, NewTransactionError> {
        match date <= Utc::now().date_naive() {
            true => Ok(Self {
                amount,
                date,
                description,
                category_id,
                user_id,
            }),
            false => Err(NewTransactionError(date)),
        }
    }

    pub fn amount(&self) -> f64 {
        self.amount
    }

    pub fn date(&self) -> NaiveDate {
        self.date
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn category_id(&self) -> i64 {
        self.category_id
    }

    pub fn user_id(&self) -> UserID {
        self.user_id
    }
}

#[cfg(test)]
mod recurring_transaction_tests {
    use chrono::{Days, Utc};

    use crate::model::{transaction::NewTransactionError, UserID};

    use super::Transaction;

    #[test]
    fn new_fails_on_future_date() {
        let new_transaction = Transaction::new(
            1,
            123.45,
            Utc::now()
                .date_naive()
                .checked_add_days(Days::new(1))
                .unwrap(),
            "".to_string(),
            1,
            UserID::new(2),
        );

        assert!(matches!(new_transaction, Err(NewTransactionError(_))))
    }

    #[test]
    fn new_succeeds_on_today() {
        let new_transaction = Transaction::new(
            1,
            123.45,
            Utc::now().date_naive(),
            "".to_string(),
            1,
            UserID::new(2),
        );

        assert!(new_transaction.is_ok())
    }

    #[test]
    fn new_succeeds_on_past_date() {
        let new_transaction = Transaction::new(
            1,
            123.45,
            Utc::now()
                .date_naive()
                .checked_sub_days(Days::new(1))
                .unwrap(),
            "".to_string(),
            1,
            UserID::new(2),
        );

        assert!(new_transaction.is_ok())
    }
}

#[cfg(test)]
mod new_recurring_transaction_tests {
    use chrono::{Days, Utc};

    use crate::model::{transaction::NewTransactionError, UserID};

    use super::NewTransaction;

    #[test]
    fn new_fails_on_future_date() {
        let new_transaction = NewTransaction::new(
            123.45,
            Utc::now()
                .date_naive()
                .checked_add_days(Days::new(1))
                .unwrap(),
            "".to_string(),
            1,
            UserID::new(2),
        );

        assert!(matches!(new_transaction, Err(NewTransactionError(_))))
    }

    #[test]
    fn new_succeeds_on_today() {
        let new_transaction = NewTransaction::new(
            123.45,
            Utc::now().date_naive(),
            "".to_string(),
            1,
            UserID::new(2),
        );

        assert!(new_transaction.is_ok())
    }

    #[test]
    fn new_succeeds_on_past_date() {
        let new_transaction = NewTransaction::new(
            123.45,
            Utc::now()
                .date_naive()
                .checked_sub_days(Days::new(1))
                .unwrap(),
            "".to_string(),
            1,
            UserID::new(2),
        );

        assert!(new_transaction.is_ok())
    }
}
