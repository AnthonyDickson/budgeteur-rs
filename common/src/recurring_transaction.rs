use chrono::NaiveDate;
use thiserror::Error;

use crate::{DatabaseID, Transaction};

#[derive(Debug, Error)]
#[error("{0} is not a valid frequency code")]
pub struct FrequencyError(i64);

/// How often a recurring transaction happens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Frequency {
    Daily,
    Weekly,
    Fortnightly,
    /// A calendar month of variable length.
    Monthly,
    /// A calendar quarter (Jan-Mar, Apr-Jun, Jul-Sep, Oct-Dec).
    Quarterly,
    Yearly,
}

impl TryFrom<i64> for Frequency {
    type Error = FrequencyError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Frequency::Daily),
            1 => Ok(Frequency::Weekly),
            2 => Ok(Frequency::Fortnightly),
            3 => Ok(Frequency::Monthly),
            4 => Ok(Frequency::Quarterly),
            5 => Ok(Frequency::Yearly),
            _ => Err(FrequencyError(value)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid recurring transaction: {0}")]
pub struct RecurringTransactionError(pub String);

/// A transaction (income or expense) that repeats on a regular basis (e.g., wages, phone bill).
///
/// This object must be attached to an existing transaction and cannot exist independently.
///
/// New instances should be created through `RecurringTransaction::insert(...)`.
#[derive(Debug, PartialEq)]
pub struct RecurringTransaction {
    transaction_id: DatabaseID,
    end_date: Option<NaiveDate>,
    frequency: Frequency,
}

impl RecurringTransaction {
    /// Create a new `RecurringTransaction` that indicates `transaction` occurs more than once on a regular schedule.
    ///
    /// An `end_date` of `None` is interpreted as `transaction` recurring indefinitely.
    ///
    /// Note that this function does not insert the object into the application database.
    /// Consider using the `insert` trait function on a `NewRecurringTransaction` to insert and create the recurring transaction at the same time.
    ///
    /// # Errors
    ///
    /// This function will return an error if `end_date` is a date before or equal to `transaction.date()`.
    pub fn new(
        transaction: &Transaction,
        end_date: Option<NaiveDate>,
        frequency: Frequency,
    ) -> Result<Self, RecurringTransactionError> {
        match end_date {
            Some(date) if date <= *transaction.date() => Err(RecurringTransactionError(format!(
                "the end date {date} is before the transaction date (i.e. the start date) {}",
                transaction.date()
            ))),
            Some(_) | None => Ok(Self {
                transaction_id: transaction.id(),
                end_date,
                frequency,
            }),
        }
    }

    /// Create a new `RecurringTransaction` without validating `end_date`.
    ///
    /// The caller should ensure that `end_date` is greater than the date of the transaction that `transaction_id` refers to.
    ///
    /// This function has `_unchecked` in the name but is not `unsafe`, because if the `end_date` invariant is violated it will cause incorrect behaviour but not affect memory safety.
    pub fn new_unchecked(
        transaction_id: DatabaseID,
        end_date: Option<NaiveDate>,
        frequency: Frequency,
    ) -> Self {
        Self {
            transaction_id,
            end_date,
            frequency,
        }
    }

    pub fn transaction_id(&self) -> DatabaseID {
        self.transaction_id
    }

    pub fn end_date(&self) -> Option<&NaiveDate> {
        self.end_date.as_ref()
    }

    pub fn frequency(&self) -> Frequency {
        self.frequency
    }
}

pub struct NewRecurringTransaction {
    transaction_id: DatabaseID,
    end_date: Option<NaiveDate>,
    frequency: Frequency,
}

impl NewRecurringTransaction {
    /// Create a `NewRecurringTransaction` that indicates `transaction` occurs more than once on a regular schedule.
    ///
    /// An `end_date` of `None` is interpreted as `transaction` recurring indefinitely.
    ///
    /// Note that this function does not insert the object into the application database.
    ///
    /// # Errors
    ///
    /// This function will return an error if `end_date` is a date before or equal to `transaction.date()`.
    pub fn new(
        transaction: &Transaction,
        end_date: Option<NaiveDate>,
        frequency: Frequency,
    ) -> Result<Self, RecurringTransactionError> {
        match end_date {
            Some(date) if date <= *transaction.date() => Err(RecurringTransactionError(format!(
                "the end date {date} is before the transaction date (i.e. the start date) {}",
                transaction.date()
            ))),
            Some(_) | None => Ok(Self {
                transaction_id: transaction.id(),
                end_date,
                frequency,
            }),
        }
    }

    pub fn transaction_id(&self) -> i64 {
        self.transaction_id
    }

    pub fn end_date(&self) -> Option<&NaiveDate> {
        self.end_date.as_ref()
    }

    pub fn frequency(&self) -> Frequency {
        self.frequency
    }
}

#[cfg(test)]
mod recurring_transaction_tests {
    use std::f64::consts::PI;

    use chrono::{Days, NaiveDate};

    use crate::{
        recurring_transaction::RecurringTransactionError, Frequency, RecurringTransaction,
        Transaction, UserID,
    };

    fn create_transaction() -> Transaction {
        Transaction::new(
            1,
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            2,
            UserID::new(3),
        )
        .unwrap()
    }

    #[test]
    fn new_recurring_transaction_succeeds_on_future_date() {
        let transaction = create_transaction();

        let new_recurring_transaction = RecurringTransaction::new(
            &transaction,
            transaction.date().checked_add_days(Days::new(1)),
            Frequency::Weekly,
        );

        assert!(new_recurring_transaction.is_ok());
    }

    #[test]
    fn new_recurring_transaction_fails_on_same_date() {
        let transaction = create_transaction();

        let new_recurring_transaction =
            RecurringTransaction::new(&transaction, Some(*transaction.date()), Frequency::Weekly);

        assert!(matches!(
            new_recurring_transaction,
            Err(RecurringTransactionError(_))
        ));
    }

    #[test]
    fn new_recurring_transaction_fails_on_past_date() {
        let transaction = create_transaction();

        let new_recurring_transaction = RecurringTransaction::new(
            &transaction,
            transaction.date().checked_sub_days(Days::new(1)),
            Frequency::Weekly,
        );

        assert!(matches!(
            new_recurring_transaction,
            Err(RecurringTransactionError(_))
        ));
    }
}

#[cfg(test)]
mod new_recurring_transaction_tests {
    use std::f64::consts::PI;

    use chrono::{Days, NaiveDate};

    use crate::{
        recurring_transaction::RecurringTransactionError, Frequency, NewRecurringTransaction,
        Transaction, UserID,
    };

    fn create_transaction() -> Transaction {
        Transaction::new(
            1,
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            2,
            UserID::new(3),
        )
        .unwrap()
    }

    #[test]
    fn new_recurring_transaction_succeeds_on_future_date() {
        let transaction = create_transaction();

        let new_recurring_transaction = NewRecurringTransaction::new(
            &transaction,
            transaction.date().checked_add_days(Days::new(1)),
            Frequency::Weekly,
        );

        assert!(new_recurring_transaction.is_ok());
    }

    #[test]
    fn new_recurring_transaction_fails_on_same_date() {
        let transaction = create_transaction();

        let new_recurring_transaction = NewRecurringTransaction::new(
            &transaction,
            Some(*transaction.date()),
            Frequency::Weekly,
        );

        assert!(matches!(
            new_recurring_transaction,
            Err(RecurringTransactionError(_))
        ));
    }

    #[test]
    fn new_recurring_transaction_fails_on_past_date() {
        let transaction = create_transaction();

        let new_recurring_transaction = NewRecurringTransaction::new(
            &transaction,
            transaction.date().checked_sub_days(Days::new(1)),
            Frequency::Weekly,
        );

        assert!(matches!(
            new_recurring_transaction,
            Err(RecurringTransactionError(_))
        ));
    }
}
