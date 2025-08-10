//! Implements a SQLite backed transaction store.
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, Row, params_from_iter, types::Value};

use crate::{
    Error,
    db::{CreateTable, MapRow},
    models::{DatabaseID, Transaction, TransactionBuilder},
    stores::{
        TransactionStore,
        transaction::{SortOrder, TransactionQuery},
    },
};

/// Stores transactions in a SQLite database.
///
/// Note that because a transaction depends on the [User](crate::models::User) and
/// [Category](crate::models::Category) models, these models must be set up in the database.
///
///
#[derive(Debug, Clone)]
pub struct SQLiteTransactionStore {
    connection: Arc<Mutex<Connection>>,
}

impl SQLiteTransactionStore {
    /// Create a new store for the SQLite `connection`.
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }
}

impl TransactionStore for SQLiteTransactionStore {
    /// Create a new transaction in the database.
    ///
    /// Dates must be no later than today.
    ///
    /// # Errors
    /// This function will return a:
    /// - [Error::InvalidCategory] if `category_id` does not refer to a valid category,
    /// - [Error::SqlError] if there is some other SQL error,
    /// - or [Error::Unspecified] if there was an unexpected error.
    fn create(&mut self, amount: f64) -> Result<Transaction, Error> {
        let transaction = Transaction::build(amount);

        self.create_from_builder(transaction)
    }

    /// Create a new transaction in the database.
    ///
    /// Dates must be no later than today.
    ///
    /// # Errors
    /// This function will return a:
    /// - [Error::InvalidCategory] if `category_id` does not refer to a valid category,
    /// - [Error::SqlError] if there is some other SQL error,
    /// - or [Error::Unspecified] if there was an unexpected error.
    fn create_from_builder(&mut self, builder: TransactionBuilder) -> Result<Transaction, Error> {
        let connection = self.connection.lock().unwrap();

        let transaction = connection
            .prepare(
                "INSERT INTO \"transaction\" (amount, date, description, category_id, import_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 RETURNING id, amount, date, description, category_id, import_id",
            )?
            .query_row(
                (
                    builder.amount,
                    builder.date,
                    builder.description,
                    builder.category_id,
                    builder.import_id,
                ),
                Self::map_row,
            )
            .map_err(|error| match error {
                // Code 787 occurs when a FOREIGN KEY constraint failed.
                // The client tried to add a transaction for a non-existent category.
                rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                    Error::InvalidCategory
                }
                // Handle duplicate import_id constraint violation
                rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 2067 => {
                    Error::DuplicateImportId
                }
                error => error.into(),
            })?;

        Ok(transaction)
    }

    /// Import many transactions from a CSV file.
    ///
    /// Ignores transactions with import IDs that already exist in the store.
    ///
    /// # Errors
    /// Returns an [Error::SqlError] if there is an unexpected SQL error.
    fn import(&mut self, builders: Vec<TransactionBuilder>) -> Result<Vec<Transaction>, Error> {
        let connection = self.connection.lock().unwrap();

        let tx = connection.unchecked_transaction()?;
        let mut imported_transactions = Vec::new();

        // Prepare the insert statement once for reuse
        let mut stmt = tx.prepare(
            "INSERT INTO \"transaction\" (amount, date, description, category_id, import_id)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(import_id) DO NOTHING
         RETURNING id, amount, date, description, category_id, import_id",
        )?;

        for builder in builders {
            // Try to insert and get the result
            let maybe_transaction = stmt.query_row(
                (
                    builder.amount,
                    builder.date,
                    builder.description,
                    builder.category_id,
                    builder.import_id,
                ),
                Self::map_row,
            );

            // Only collect successfully inserted transactions (not conflicts)
            if let Ok(transaction) = maybe_transaction {
                imported_transactions.push(transaction);
            }
        }

        drop(stmt);

        tx.commit()?;
        Ok(imported_transactions)
    }

    /// Retrieve a transaction in the database by its `id`.
    ///
    /// # Errors
    /// This function will return a:
    /// - [Error::NotFound] if `id` does not refer to a valid transaction,
    /// - or [Error::SqlError] there is some other SQL error.
    fn get(&self, id: DatabaseID) -> Result<Transaction, Error> {
        let transaction = self.connection.lock().unwrap()
                .prepare("SELECT id, amount, date, description, category_id, import_id FROM \"transaction\" WHERE id = :id")?
                .query_row(&[(":id", &id)], Self::map_row)?;

        Ok(transaction)
    }

    /// Query for transactions in the database.
    ///
    /// # Errors
    /// This function will return a [Error::SqlError] there is a SQL error.
    fn get_query(&self, filter: TransactionQuery) -> Result<Vec<Transaction>, Error> {
        let mut query_string_parts = vec![
            "SELECT id, amount, date, description, category_id, import_id FROM \"transaction\""
                .to_string(),
        ];
        let mut where_clause_parts = vec![];
        let mut query_parameters = vec![];

        if let Some(date_range) = filter.date_range {
            where_clause_parts.push(format!(
                "date BETWEEN ?{} AND ?{}",
                query_parameters.len() + 1,
                query_parameters.len() + 2,
            ));
            query_parameters.push(Value::Text(date_range.start().to_string()));
            query_parameters.push(Value::Text(date_range.end().to_string()));
        }

        if !where_clause_parts.is_empty() {
            query_string_parts.push(String::from("WHERE ") + &where_clause_parts.join(" AND "));
        }

        match filter.sort_date {
            Some(SortOrder::Ascending) => query_string_parts.push("ORDER BY date ASC".to_string()),
            Some(SortOrder::Descending) => {
                query_string_parts.push("ORDER BY date DESC".to_string())
            }
            None => {}
        }

        if let Some(limit) = filter.limit {
            query_string_parts.push(format!("LIMIT {limit} OFFSET {}", filter.offset));
        }

        let query_string = query_string_parts.join(" ");
        let params = params_from_iter(query_parameters.iter());

        self.connection
            .lock()
            .unwrap()
            .prepare(&query_string)?
            .query_map(params, Self::map_row)?
            .map(|maybe_category| maybe_category.map_err(Error::SqlError))
            .collect()
    }

    /// Get the total number of transactions in the database.
    ///
    /// # Errors
    /// This function will return a [Error::SqlError] there is some SQL error.
    fn count(&self) -> Result<usize, Error> {
        self.connection
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(id) FROM \"transaction\";", [], |row| {
                row.get(0)
            })
            .map_err(|error| error.into())
    }
}

impl CreateTable for SQLiteTransactionStore {
    fn create_table(connection: &Connection) -> Result<(), rusqlite::Error> {
        connection
                .execute(
                    "CREATE TABLE IF NOT EXISTS \"transaction\" (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            amount REAL NOT NULL,
                            date TEXT NOT NULL,
                            description TEXT NOT NULL,
                            category_id INTEGER,
                            import_id INTEGER UNIQUE,
                            FOREIGN KEY(category_id) REFERENCES category(id) ON UPDATE CASCADE ON DELETE SET NULL
                            )",
                    (),
                )?;

        // Ensure the sequence starts at 1
        connection.execute(
            "INSERT OR IGNORE INTO sqlite_sequence (name, seq) VALUES ('transaction', 0)",
            (),
        )?;

        Ok(())
    }
}

impl MapRow for SQLiteTransactionStore {
    type ReturnType = Transaction;

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self::ReturnType, rusqlite::Error> {
        let id = row.get(offset)?;
        let amount = row.get(offset + 1)?;
        let date = row.get(offset + 2)?;
        let description = row.get(offset + 3)?;
        let category_id = row.get(offset + 4)?;
        let import_id = row.get(offset + 5)?;

        let transaction =
            Transaction::new_unchecked(id, amount, date, description, category_id, import_id);

        Ok(transaction)
    }
}

#[cfg(test)]
mod sqlite_transaction_store_tests {
    use std::f64::consts::PI;

    use rusqlite::Connection;
    use time::{Duration, OffsetDateTime};

    use crate::{
        models::{Transaction, TransactionBuilder},
        stores::{
            sqlite::{SQLAppState, create_app_state},
            transaction::{SortOrder, TransactionQuery},
        },
    };

    use super::{Error, TransactionStore};

    fn get_app_state() -> SQLAppState {
        let conn = Connection::open_in_memory().unwrap();
        create_app_state(conn, "stneaoetse", Default::default()).unwrap()
    }

    #[test]
    fn create_succeeds() {
        let mut state = get_app_state();
        let amount = 12.3;

        let result = state.transaction_store.create(amount);

        assert!(result.is_ok());
        let transaction = result.unwrap();
        assert_eq!(transaction.amount(), amount);
    }

    #[test]
    fn create_fails_on_invalid_category_id() {
        let mut state = get_app_state();

        let transaction = state
            .transaction_store
            .create_from_builder(Transaction::build(PI).category(Some(999)));

        assert_eq!(transaction, Err(Error::InvalidCategory));
    }

    #[test]
    fn create_fails_on_duplicate_import_id() {
        let state = get_app_state();
        let mut store = state.transaction_store;
        let import_id = Some(123456789);
        store
            .create_from_builder(Transaction::build(123.45).import_id(import_id))
            .expect("Could not create transaction");

        let duplicate_transaction =
            store.create_from_builder(Transaction::build(123.45).import_id(import_id));

        assert_eq!(duplicate_transaction, Err(Error::DuplicateImportId));
    }

    #[test]
    fn import_multiple() {
        let state = get_app_state();
        let mut store = state.transaction_store;
        let want = vec![
            Transaction::build(123.45).import_id(Some(123456789)),
            Transaction::build(678.90).import_id(Some(101112131)),
        ];

        let duplicate_transactions = store
            .import(want.clone())
            .expect("Could not create transaction");

        assert_eq!(
            want.len(),
            duplicate_transactions.len(),
            "want {} transactions, got {}",
            want.len(),
            duplicate_transactions.len()
        );

        want.into_iter()
            .zip(duplicate_transactions.iter())
            .for_each(|(want, got)| {
                let want = want.finalise(got.id());
                let error_message = format!("want transaction {want:?}, got {got:?}");
                assert_eq!(want.amount(), got.amount(), "{error_message}");
                assert_eq!(want.date(), got.date(), "{error_message}");
                assert_eq!(want.description(), got.description(), "{error_message}");
                assert_eq!(want.category_id(), got.category_id(), "{error_message}");
                assert_eq!(want.import_id(), got.import_id(), "{error_message}");
            });
    }

    #[test]
    fn import_ignores_duplicate_import_id() {
        let state = get_app_state();
        let mut store = state.transaction_store;
        let import_id = Some(123456789);
        let want = store
            .create_from_builder(Transaction::build(123.45).import_id(import_id))
            .expect("Could not create transaction");

        let duplicate_transactions = store
            .import(vec![Transaction::build(123.45).import_id(import_id)])
            .expect("Could not import transactions");

        // The import should return 0 transactions since the import_id already exists
        assert_eq!(
            duplicate_transactions.len(),
            0,
            "import should ignore transactions with duplicate import IDs: want 0 transactions, got {}",
            duplicate_transactions.len()
        );

        // Verify that only the original transaction exists in the database
        let all_transactions = store
            .get_query(TransactionQuery::default())
            .expect("Could not query transactions");

        assert_eq!(
            all_transactions.len(),
            1,
            "Expected exactly 1 transaction in database after duplicate import attempt, got {}",
            all_transactions.len()
        );

        // Verify the original transaction is unchanged
        let stored_transaction = &all_transactions[0];
        assert_eq!(stored_transaction.amount(), want.amount());
        assert_eq!(stored_transaction.date(), want.date());
        assert_eq!(stored_transaction.description(), want.description());
        assert_eq!(stored_transaction.category_id(), want.category_id());
        assert_eq!(stored_transaction.import_id(), want.import_id());
    }

    #[tokio::test]
    async fn import_escapes_single_quotes() {
        let state = get_app_state();
        let mut store = state.transaction_store;
        let want = vec![
            Transaction::build(123.45)
                .import_id(Some(123456789))
                .description("Tom's Hardware"),
        ];

        let duplicate_transactions = store
            .import(want.clone())
            .expect("Could not create transaction");

        assert_eq!(
            want.len(),
            duplicate_transactions.len(),
            "want {} transactions, got {}",
            want.len(),
            duplicate_transactions.len()
        );

        want.into_iter()
            .zip(duplicate_transactions.iter())
            .for_each(|(want, got)| {
                let want = want.finalise(got.id());
                let error_message = format!("want transaction {want:?}, got {got:?}");
                assert_eq!(want.amount(), got.amount(), "{error_message}");
                assert_eq!(want.date(), got.date(), "{error_message}");
                assert_eq!(want.description(), got.description(), "{error_message}");
                assert_eq!(want.category_id(), got.category_id(), "{error_message}");
                assert_eq!(want.import_id(), got.import_id(), "{error_message}");
            });
    }

    #[test]
    fn get_transaction_by_id_succeeds() {
        let state = get_app_state();
        let mut store = state.transaction_store;
        let transaction = store.create(PI).unwrap();

        let selected_transaction = store.get(transaction.id());

        assert_eq!(Ok(transaction), selected_transaction);
    }

    #[test]
    fn get_transaction_fails_on_invalid_id() {
        let state = get_app_state();
        let mut store = state.transaction_store;
        let transaction = store.create(123.0).unwrap();

        let maybe_transaction = store.get(transaction.id() + 654);

        assert_eq!(maybe_transaction, Err(Error::NotFound));
    }

    #[test]
    fn get_transactions_by_date_range() {
        let state = get_app_state();
        let mut store = state.transaction_store;

        let end_date = OffsetDateTime::now_utc()
            .date()
            .checked_sub(Duration::weeks(1))
            .unwrap();
        let start_date = end_date.checked_sub(Duration::weeks(1)).unwrap();

        let want = [
            store
                .create_from_builder(TransactionBuilder::new(12.3).date(start_date).unwrap())
                .unwrap(),
            store
                .create_from_builder(
                    TransactionBuilder::new(23.4)
                        .date(start_date.checked_add(Duration::days(3)).unwrap())
                        .unwrap(),
                )
                .unwrap(),
            store
                .create_from_builder(TransactionBuilder::new(34.5).date(end_date).unwrap())
                .unwrap(),
        ];

        // The below transactions should NOT be returned by the query.
        let cases = [
            start_date.checked_sub(Duration::days(1)).unwrap(),
            end_date.checked_add(Duration::days(1)).unwrap(),
        ];

        for date in cases {
            store
                .create_from_builder(TransactionBuilder::new(999.99).date(date).unwrap())
                .unwrap();
        }

        let got = store
            .get_query(TransactionQuery {
                date_range: Some(start_date..=end_date),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(got, want, "got transactions {:?}, want {:?}", got, want);
    }

    #[test]
    fn get_transactions_with_limit() {
        let mut state = get_app_state();

        let today = OffsetDateTime::now_utc().date();

        for i in 1..=10 {
            let transaction_builder = TransactionBuilder::new(i as f64)
                .date(today.checked_sub(Duration::days(i)).unwrap())
                .unwrap()
                .description(&format!("transaction #{i}"));

            state
                .transaction_store
                .create_from_builder(transaction_builder)
                .unwrap();
        }

        let got = state
            .transaction_store
            .get_query(TransactionQuery {
                limit: Some(5),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(got.len(), 5, "got {} transactions, want 5", got.len());
    }

    #[test]
    fn get_transactions_with_offset() {
        let mut state = get_app_state();
        let offset = 10;
        let limit = 5;
        let mut want = Vec::new();
        for i in 1..20 {
            let transaction = state
                .transaction_store
                .create(i as f64)
                .expect("Could not create transaction");

            if i > offset && i <= offset + limit {
                want.push(transaction);
            }
        }

        let got = state
            .transaction_store
            .get_query(TransactionQuery {
                offset,
                limit: Some(limit),
                ..Default::default()
            })
            .expect("Could not query store");

        assert_eq!(want, got);
    }

    #[test]
    fn get_transactions_descending_date() {
        let mut state = get_app_state();

        let mut want = vec![];
        let start_date = OffsetDateTime::now_utc()
            .date()
            .checked_sub(Duration::weeks(2))
            .unwrap();

        for i in 1..=3 {
            let transaction_builder = TransactionBuilder::new(i as f64)
                .date(start_date.checked_add(Duration::days(i)).unwrap())
                .unwrap()
                .description(&format!("transaction #{i}"));

            let transaction = state
                .transaction_store
                .create_from_builder(transaction_builder)
                .unwrap();

            want.push(transaction);
        }

        want.sort_by(|a, b| b.date().cmp(a.date()));

        let got = state
            .transaction_store
            .get_query(TransactionQuery {
                sort_date: Some(SortOrder::Descending),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(
            got, want,
            "got transactions that were not sorted in descending order."
        );
    }

    #[test]
    fn get_count() {
        let mut state = get_app_state();
        let want_count = 20;
        for i in 1..=want_count {
            state
                .transaction_store
                .create(i as f64)
                .expect("Could not create transaction");
        }

        let got_count = state
            .transaction_store
            .count()
            .expect("Could not get count");

        assert_eq!(want_count, got_count);
    }
}
