//! Defines the core data models and database queries for transactions.

use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};
use time::Date;

// TODO: Remove once `TransactionBuilder::finalize` is removed
#[cfg(test)]
use time::{OffsetDateTime, UtcOffset};

use crate::{Error, database_id::DatabaseId, tag::TagId};

// ============================================================================
// MODELS
// ============================================================================

/// An expense or income, i.e. an event where money was either spent or earned.
///
/// To create a new `Transaction`, use [Transaction::build].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    /// The ID of the transaction.
    pub id: DatabaseId,
    /// The amount of money spent or earned in this transaction.
    pub amount: f64,
    /// When the transaction happened.
    pub date: Date,
    /// A text description of what the transaction was for.
    pub description: String,
    /// The ID of the import that this transaction belongs to.
    pub import_id: Option<i64>,
    /// The ID of the category the transaction belongs to.
    pub tag_id: Option<TagId>,
}

impl Transaction {
    /// Create a new transaction.
    ///
    /// Shortcut for [TransactionBuilder] for discoverability.
    pub fn build(amount: f64, date: Date, description: String) -> TransactionBuilder {
        TransactionBuilder {
            amount,
            date,
            description,
            import_id: None,
            tag_id: None,
        }
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
/// ```ignore
/// use time::{macros::date, UtcOffset};
///
/// use crate::transaction::Transaction;
///
/// // Transaction with full details
/// let transaction = Transaction::build(
///         -45.99,
///         date!(2025-01-15),
///         "Coffee shop purchase".to_owned()
///     )
///     .import_id(Some(987654321))
///     .finalize(2, UtcOffset::UTC)
///     .unwrap();
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

    /// The category of the transaction, e.g. "Groceries", "Transport", "Rent".
    pub tag_id: Option<TagId>,
}

impl TransactionBuilder {
    /// Set the import ID for the transaction.
    pub fn import_id(mut self, import_id: Option<i64>) -> Self {
        self.import_id = import_id;
        self
    }

    /// Set the tag id for the transaction.
    pub fn tag_id(mut self, tag_id: Option<TagId>) -> Self {
        self.tag_id = tag_id;
        self
    }

    /// Build the final [Transaction] instance.
    ///
    /// `local_timezone` is used to check that `.date` is not a future date.
    ///
    /// # Errors
    /// This function will return an error if `date` is a date in the future.
    // TODO: Move this functionality to `transaction::create_transaction`?
    #[cfg(test)]
    pub fn finalize(self, id: DatabaseId, local_timezone: UtcOffset) -> Result<Transaction, Error> {
        if self.date > OffsetDateTime::now_utc().to_offset(local_timezone).date() {
            return Err(Error::FutureDate);
        }

        Ok(Transaction {
            id,
            amount: self.amount,
            date: self.date,
            description: self.description,
            import_id: self.import_id,
            tag_id: self.tag_id,
        })
    }
}

// ============================================================================
// DATABASE FUNCTIONS
// ============================================================================

/// Create a new transaction in the database from a builder.
///
/// Dates must be no later than today.
///
/// # Errors
/// This function will return a:
/// - [Error::SqlError] if there is some other SQL error,
/// - or [Error::InternalError] if there was an unexpected error.
pub fn create_transaction(
    builder: TransactionBuilder,
    connection: &Connection,
) -> Result<Transaction, Error> {
    let transaction = connection
        .prepare(
            "INSERT INTO \"transaction\" (amount, date, description, import_id, tag_id)
             VALUES (?1, ?2, ?3, ?4, ?5)
             RETURNING id, amount, date, description, import_id, tag_id",
        )?
        .query_row(
            (
                builder.amount,
                builder.date,
                builder.description,
                builder.import_id,
                builder.tag_id,
            ),
            map_transaction_row,
        )
        .map_err(|error| match error {
            // TODO: Check how this handles tag_id constraint violation and write test
            // Handle duplicate import_id constraint violation
            rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 2067 => {
                Error::DuplicateImportId
            }
            error => error.into(),
        })?;

    Ok(transaction)
}

/// Get the total number of transactions in the database.
///
/// # Errors
/// This function will return a [Error::SqlError] there is some SQL error.
pub fn count_transactions(connection: &Connection) -> Result<usize, Error> {
    connection
        .query_row("SELECT COUNT(id) FROM \"transaction\";", [], |row| {
            row.get(0)
        })
        .map_err(|error| error.into())
}

/// Create the transaction table in the database.
///
/// # Errors
/// Returns an error if the table cannot be created or if there is an SQL error.
pub fn create_transaction_table(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS \"transaction\" (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                amount REAL NOT NULL,
                date TEXT NOT NULL,
                description TEXT NOT NULL,
                import_id INTEGER UNIQUE,
                tag_id INTEGER,
                FOREIGN KEY(tag_id) REFERENCES tag(id) ON UPDATE CASCADE ON DELETE SET NULL
                )",
        (),
    )?;

    // Ensure the sequence starts at 1
    connection.execute(
        "INSERT OR IGNORE INTO sqlite_sequence (name, seq) VALUES ('transaction', 0)",
        (),
    )?;

    // Add composite index used by dashboard page.
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_transaction_date_tag ON \"transaction\"(date, tag_id);",
        (),
    )?;

    Ok(())
}

/// Map a database row to a Transaction.
pub fn map_transaction_row(row: &Row) -> Result<Transaction, rusqlite::Error> {
    let id = row.get(0)?;
    let amount = row.get(1)?;
    let date = row.get(2)?;
    let description = row.get(3)?;
    let import_id = row.get(4)?;
    let tag_id = row.get(5)?;

    Ok(Transaction {
        id,
        amount,
        date,
        description,
        import_id,
        tag_id,
    })
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod transaction_builder_tests {
    use std::f64::consts::PI;

    use time::{Duration, OffsetDateTime, UtcOffset};

    use super::{Error, Transaction, TransactionBuilder};

    #[test]
    fn finalize_fails_on_future_date() {
        let tomorrow = OffsetDateTime::now_utc()
            .date()
            .checked_add(Duration::days(1))
            .unwrap();

        let result = TransactionBuilder {
            amount: 123.45,
            date: tomorrow,
            description: "".to_owned(),
            import_id: None,
            tag_id: None,
        }
        .finalize(123, UtcOffset::UTC);

        assert_eq!(result, Err(Error::FutureDate));
    }

    #[test]
    fn finalize_succeeds_on_today() {
        let today = OffsetDateTime::now_utc().date();

        let transaction = TransactionBuilder {
            amount: 123.45,
            date: today,
            description: "".to_owned(),
            import_id: None,
            tag_id: None,
        }
        .finalize(123, UtcOffset::UTC);

        match transaction {
            Ok(Transaction { date: got_date, .. }) => assert_eq!(today, got_date),
            Err(error) => panic!("Got unexpected error {error}"),
        }
    }

    #[test]
    fn finalize_succeeds_on_past_date() {
        let yesterday = OffsetDateTime::now_utc()
            .date()
            .checked_sub(Duration::days(1))
            .unwrap();

        let result = TransactionBuilder {
            amount: 123.45,
            date: yesterday,
            description: "".to_owned(),
            import_id: None,
            tag_id: None,
        }
        .finalize(123, UtcOffset::UTC);

        match result {
            Ok(Transaction { date: got_date, .. }) => assert_eq!(yesterday, got_date),
            Err(error) => panic!("Got unexpected error {error}"),
        }
    }

    #[test]
    fn insert_transaction_succeeds() {
        let id = 123;
        let amount = PI;
        let date = OffsetDateTime::now_utc().date();
        let description = "Rust Pie".to_string();
        let import_id = Some(123456789);

        let result = Transaction::build(amount, date, description.clone())
            .import_id(import_id)
            .finalize(id, UtcOffset::UTC);

        match result {
            Ok(transaction) => {
                assert_eq!(transaction.id, id);
                assert_eq!(transaction.amount, amount);
                assert_eq!(transaction.date, date);
                assert_eq!(transaction.description, description);
                assert_eq!(transaction.import_id, import_id);
            }
            Err(error) => panic!("Unexpected error: {error}"),
        }
    }
}

#[cfg(test)]
mod database_tests {
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        Error,
        db::initialize,
        transaction::{Transaction, count_transactions, create_transaction},
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn create_succeeds() {
        let conn = get_test_connection();
        let amount = 12.3;

        let result = create_transaction(
            Transaction::build(amount, date!(2025 - 10 - 05), "".to_owned()),
            &conn,
        );

        match result {
            Ok(transaction) => assert_eq!(transaction.amount, amount),
            Err(error) => panic!("Unexpected error: {error}"),
        }
    }

    #[test]
    fn create_fails_on_duplicate_import_id() {
        let conn = get_test_connection();
        let import_id = Some(123456789);
        let today = date!(2025 - 10 - 04);
        create_transaction(
            Transaction::build(123.45, today, "".to_owned()).import_id(import_id),
            &conn,
        )
        .expect("Could not create transaction");

        let duplicate_transaction = create_transaction(
            Transaction::build(123.45, today, "".to_owned()).import_id(import_id),
            &conn,
        );

        assert_eq!(duplicate_transaction, Err(Error::DuplicateImportId));
    }

    #[test]
    fn get_count() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);
        let want_count = 20;
        for i in 1..=want_count {
            create_transaction(Transaction::build(i as f64, today, "".to_owned()), &conn)
                .expect("Could not create transaction");
        }

        let got_count = count_transactions(&conn).expect("Could not get count");

        assert_eq!(want_count, got_count);
    }
}
