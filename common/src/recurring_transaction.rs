use chrono::NaiveDate;
use thiserror::Error;

use crate::{DatabaseID, Transaction};

#[derive(Debug, Error)]
#[error("{0} is not a valid frequency code")]
pub struct FrequencyError(String);

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
            _ => Err(FrequencyError(value.to_string())),
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

    pub fn end_date(&self) -> &Option<NaiveDate> {
        &self.end_date
    }

    pub fn frequency(&self) -> Frequency {
        self.frequency
    }
}
