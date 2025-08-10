//! This file defines the type `Transaction`, the core type of the budgeting part of the
//! application.

use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};

use crate::{Error, models::DatabaseID};

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
    import_id: Option<i64>,
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
        import_id: Option<i64>,
    ) -> Self {
        Self {
            id,
            amount,
            date,
            description,
            category_id,
            import_id,
        }
    }

    /// Create a new transaction.
    ///
    /// Shortcut for [TransactionBuilder::new] for discoverability.
    pub fn build(amount: f64) -> TransactionBuilder {
        TransactionBuilder::new(amount)
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

    /// The ID of the import that this transaction belongs to.
    pub fn import_id(&self) -> Option<i64> {
        self.import_id
    }
}

/// A builder for creating [Transaction] instances.
///
/// This builder allows you to construct transactions step by step, providing
/// sensible defaults for optional fields. Once all required fields are set,
/// call `finalise()` to create the actual [Transaction].
///
/// # Examples
///
/// ```rust
/// use time::macros::date;
///
/// use budgeteur_rs::models::Transaction;
///
/// // Simple transaction with just an amount
/// let transaction = Transaction::build(150.00)
///     .finalise(1);
///
/// // Transaction with full details
/// let transaction = Transaction::build(-45.99)
///     .date(date!(2025-01-15))
///     .unwrap()
///     .description("Coffee shop purchase")
///     .category(Some(5))
///     .import_id(Some(987654321))
///     .finalise(2);
/// ```
#[derive(Debug, PartialEq, Clone)]
pub struct TransactionBuilder {
    /// The monetary amount of the transaction.
    ///
    /// Positive values represent income/credits, negative values represent
    /// expenses/debits. This follows standard accounting conventions where
    /// money flowing into your account is positive.
    ///
    /// # Examples
    /// - `150.00` - Salary deposit
    /// - `-45.99` - Coffee shop purchase
    /// - `-1200.00` - Rent payment
    pub amount: f64,

    /// The date when the transaction occurred.
    ///
    /// Defaults to today's date if not specified. The date must not be in the
    /// future - transactions cannot be dated later than the current date.
    ///
    /// This represents the actual transaction date (when money moved), not
    /// when it was recorded in your system.
    pub date: Date,

    /// A human-readable description of the transaction.
    ///
    /// Defaults to "Transaction" if not specified. This field is used to help
    /// identify and categorize transactions. For imported transactions, this
    /// typically comes from the bank's description field.
    ///
    /// # Examples
    /// - `"Salary - January 2025"`
    /// - `"Starbucks #1234 - Downtown"`
    /// - `"TRANSFER TO A R DICKSON - 01"`
    /// - `"POS W/D LOBSTER SEAFOO-19:47"`
    pub description: String,

    /// Optional reference to a category for organizing transactions.
    ///
    /// If `Some(id)`, the transaction will be associated with the category
    /// having that database ID. If `None`, the transaction remains uncategorized.
    ///
    /// Categories help with budgeting and expense tracking by grouping similar
    /// transactions together (e.g., "Food & Dining", "Transportation", "Utilities").
    ///
    /// # Database Constraint
    /// If specified, the category ID must exist in the categories table,
    /// otherwise transaction creation will fail with [Error::InvalidCategory].
    pub category_id: Option<DatabaseID>,

    /// Optional unique identifier for imported transactions.
    ///
    /// This field is used to prevent duplicate imports when processing CSV files
    /// from banks. Each imported transaction gets a unique hash based on its
    /// content (date, amount, description, etc.).
    ///
    /// - `Some(id)` - Transaction was imported from a CSV file
    /// - `None` - Transaction was created manually by the user
    ///
    /// # Duplicate Prevention
    /// The database enforces uniqueness on this field. Attempting to import
    /// a transaction with a duplicate `import_id` will fail gracefully, allowing
    /// the same CSV file to be imported multiple times safely.
    ///
    /// # Implementation Note
    /// The import ID is typically generated using [crate::csv::create_import_id]
    /// which creates a hash from the raw CSV line content.
    pub import_id: Option<i64>,
}

impl TransactionBuilder {
    /// Create a new transaction.
    ///
    /// Finalize the builder with [TransactionBuilder::finalise].
    pub fn new(amount: f64) -> Self {
        Self {
            amount,
            date: OffsetDateTime::now_utc().date(),
            description: String::new(),
            category_id: None,
            import_id: None,
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
            import_id: self.import_id,
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

    /// Set the import ID for the transaction.
    pub fn import_id(mut self, import_id: Option<i64>) -> Self {
        self.import_id = import_id;
        self
    }
}

#[cfg(test)]
mod transaction_builder_tests {
    use std::f64::consts::PI;

    use time::{Duration, OffsetDateTime};

    use crate::models::TransactionBuilder;

    use super::{Error, Transaction};

    #[test]
    fn new_fails_on_future_date() {
        let tomorrow = OffsetDateTime::now_utc()
            .date()
            .checked_add(Duration::days(1))
            .unwrap();

        let result = TransactionBuilder::new(123.45).date(tomorrow);

        assert_eq!(result, Err(Error::FutureDate));
    }

    #[test]
    fn new_succeeds_on_today() {
        let today = OffsetDateTime::now_utc().date();

        let transaction_buider = TransactionBuilder::new(123.45).date(today);

        assert!(transaction_buider.is_ok());

        let transaction = transaction_buider.unwrap().finalise(1);
        assert_eq!(transaction.date(), &today);
    }

    #[test]
    fn new_succeeds_on_past_date() {
        let yesterday = OffsetDateTime::now_utc()
            .date()
            .checked_sub(Duration::days(1))
            .unwrap();

        let result = TransactionBuilder::new(123.45).date(yesterday);

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
        let import_id = Some(123456789);

        let transaction = Transaction::build(amount)
            .category(category_id)
            .description(&description)
            .date(date)
            .unwrap()
            .import_id(import_id)
            .finalise(id);

        assert_eq!(transaction.id(), id);
        assert_eq!(transaction.amount(), amount);
        assert_eq!(transaction.date(), &date);
        assert_eq!(transaction.description(), description);
        assert_eq!(transaction.category_id(), category_id);
        assert_eq!(transaction.import_id, import_id);
    }
}
