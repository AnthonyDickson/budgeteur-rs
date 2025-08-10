//! Functions for managing transactions in the database.

use std::ops::RangeInclusive;

use rusqlite::{Connection, Row, params_from_iter, types::Value};
use time::Date;

use crate::{
    Error,
    models::{DatabaseID, Transaction, TransactionBuilder},
};

/// Create a new transaction in the database from a builder.
///
/// Dates must be no later than today.
///
/// # Errors
/// This function will return a:
/// - [Error::InvalidCategory] if `category_id` does not refer to a valid category,
/// - [Error::SqlError] if there is some other SQL error,
/// - or [Error::InternalError] if there was an unexpected error.
pub fn create_transaction(
    builder: TransactionBuilder,
    connection: &Connection,
) -> Result<Transaction, Error> {
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
            map_transaction_row,
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
/// Ignores transactions with import IDs that already exist in the database.
///
/// # Errors
/// Returns an [Error::SqlError] if there is an unexpected SQL error.
pub fn import_transactions(
    builders: Vec<TransactionBuilder>,
    connection: &Connection,
) -> Result<Vec<Transaction>, Error> {
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
            map_transaction_row,
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

/// Retrieve a transaction from the database by its `id`.
///
/// # Errors
/// This function will return a:
/// - [Error::NotFound] if `id` does not refer to a valid transaction,
/// - or [Error::SqlError] there is some other SQL error.
pub fn get_transaction(id: DatabaseID, connection: &Connection) -> Result<Transaction, Error> {
    let transaction = connection
        .prepare("SELECT id, amount, date, description, category_id, import_id FROM \"transaction\" WHERE id = :id")?
        .query_row(&[(":id", &id)], map_transaction_row)?;

    Ok(transaction)
}

/// Defines how transactions should be fetched from [query_transactions].
#[derive(Default)]
pub struct TransactionQuery {
    /// Include transactions within `date_range` (inclusive).
    pub date_range: Option<RangeInclusive<Date>>,
    /// Selects up to the first N (`limit`) transactions.
    pub limit: Option<u64>,
    /// Ignore the first N transactions. Only has an effect if `limit` is not `None`.
    pub offset: u64,
    /// Orders transactions by date in the order `sort_date`. None returns transactions in the
    /// order they are stored.
    pub sort_date: Option<SortOrder>,
}

/// The order to sort transactions in a [TransactionQuery].
pub enum SortOrder {
    /// Sort in order of increasing value.
    // TODO: Remove #[allow(dead_code)] once Ascending is used
    #[allow(dead_code)]
    Ascending,
    /// Sort in order of decreasing value.
    Descending,
}

/// Query for transactions in the database.
///
/// # Errors
/// This function will return a [Error::SqlError] there is a SQL error.
pub fn query_transactions(
    filter: TransactionQuery,
    connection: &Connection,
) -> Result<Vec<Transaction>, Error> {
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
        Some(SortOrder::Descending) => query_string_parts.push("ORDER BY date DESC".to_string()),
        None => {}
    }

    if let Some(limit) = filter.limit {
        query_string_parts.push(format!("LIMIT {limit} OFFSET {}", filter.offset));
    }

    let query_string = query_string_parts.join(" ");
    let params = params_from_iter(query_parameters.iter());

    connection
        .prepare(&query_string)?
        .query_map(params, map_transaction_row)?
        .map(|maybe_transaction| maybe_transaction.map_err(Error::SqlError))
        .collect()
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

/// Map a database row to a Transaction.
fn map_transaction_row(row: &Row) -> Result<Transaction, rusqlite::Error> {
    let id = row.get(0)?;
    let amount = row.get(1)?;
    let date = row.get(2)?;
    let description = row.get(3)?;
    let category_id = row.get(4)?;
    let import_id = row.get(5)?;

    let transaction =
        Transaction::new_unchecked(id, amount, date, description, category_id, import_id);
    Ok(transaction)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use rusqlite::Connection;
    use time::{Duration, OffsetDateTime};

    use crate::{
        Error,
        db::initialize,
        models::{Transaction, TransactionBuilder},
    };

    use super::*;

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn create_succeeds() {
        let conn = get_test_connection();
        let amount = 12.3;

        let result = create_transaction(Transaction::build(amount), &conn);

        assert!(result.is_ok());
        let transaction = result.unwrap();
        assert_eq!(transaction.amount(), amount);
    }

    #[test]
    fn create_fails_on_invalid_category_id() {
        let conn = get_test_connection();

        let transaction = create_transaction(Transaction::build(PI).category(Some(999)), &conn);

        assert_eq!(transaction, Err(Error::InvalidCategory));
    }

    #[test]
    fn create_fails_on_duplicate_import_id() {
        let conn = get_test_connection();
        let import_id = Some(123456789);
        create_transaction(Transaction::build(123.45).import_id(import_id), &conn)
            .expect("Could not create transaction");

        let duplicate_transaction =
            create_transaction(Transaction::build(123.45).import_id(import_id), &conn);

        assert_eq!(duplicate_transaction, Err(Error::DuplicateImportId));
    }

    #[test]
    fn import_multiple() {
        let conn = get_test_connection();
        let want = vec![
            Transaction::build(123.45).import_id(Some(123456789)),
            Transaction::build(678.90).import_id(Some(101112131)),
        ];

        let imported_transactions =
            import_transactions(want.clone(), &conn).expect("Could not create transaction");

        assert_eq!(
            want.len(),
            imported_transactions.len(),
            "want {} transactions, got {}",
            want.len(),
            imported_transactions.len()
        );

        want.into_iter()
            .zip(imported_transactions.iter())
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
        let conn = get_test_connection();
        let import_id = Some(123456789);
        let want = create_transaction(Transaction::build(123.45).import_id(import_id), &conn)
            .expect("Could not create transaction");

        let duplicate_transactions =
            import_transactions(vec![Transaction::build(123.45).import_id(import_id)], &conn)
                .expect("Could not import transactions");

        // The import should return 0 transactions since the import_id already exists
        assert_eq!(
            duplicate_transactions.len(),
            0,
            "import should ignore transactions with duplicate import IDs: want 0 transactions, got {}",
            duplicate_transactions.len()
        );

        // Verify that only the original transaction exists in the database
        let all_transactions = query_transactions(TransactionQuery::default(), &conn)
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
        let conn = get_test_connection();
        let want = vec![
            Transaction::build(123.45)
                .import_id(Some(123456789))
                .description("Tom's Hardware"),
        ];

        let imported_transactions =
            import_transactions(want.clone(), &conn).expect("Could not create transaction");

        assert_eq!(
            want.len(),
            imported_transactions.len(),
            "want {} transactions, got {}",
            want.len(),
            imported_transactions.len()
        );

        want.into_iter()
            .zip(imported_transactions.iter())
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
        let conn = get_test_connection();
        let transaction = create_transaction(Transaction::build(PI), &conn).unwrap();

        let selected_transaction = get_transaction(transaction.id(), &conn);

        assert_eq!(Ok(transaction), selected_transaction);
    }

    #[test]
    fn get_transaction_fails_on_invalid_id() {
        let conn = get_test_connection();
        let transaction = create_transaction(Transaction::build(123.0), &conn).unwrap();

        let maybe_transaction = get_transaction(transaction.id() + 654, &conn);

        assert_eq!(maybe_transaction, Err(Error::NotFound));
    }

    #[test]
    fn get_transactions_by_date_range() {
        let conn = get_test_connection();

        let end_date = OffsetDateTime::now_utc()
            .date()
            .checked_sub(Duration::weeks(1))
            .unwrap();
        let start_date = end_date.checked_sub(Duration::weeks(1)).unwrap();

        let want = [
            create_transaction(
                TransactionBuilder::new(12.3).date(start_date).unwrap(),
                &conn,
            )
            .unwrap(),
            create_transaction(
                TransactionBuilder::new(23.4)
                    .date(start_date.checked_add(Duration::days(3)).unwrap())
                    .unwrap(),
                &conn,
            )
            .unwrap(),
            create_transaction(TransactionBuilder::new(34.5).date(end_date).unwrap(), &conn)
                .unwrap(),
        ];

        // The below transactions should NOT be returned by the query.
        let cases = [
            start_date.checked_sub(Duration::days(1)).unwrap(),
            end_date.checked_add(Duration::days(1)).unwrap(),
        ];

        for date in cases {
            create_transaction(TransactionBuilder::new(999.99).date(date).unwrap(), &conn).unwrap();
        }

        let got = query_transactions(
            TransactionQuery {
                date_range: Some(start_date..=end_date),
                ..Default::default()
            },
            &conn,
        )
        .unwrap();

        assert_eq!(got, want, "got transactions {:?}, want {:?}", got, want);
    }

    #[test]
    fn get_transactions_with_limit() {
        let conn = get_test_connection();

        let today = OffsetDateTime::now_utc().date();

        for i in 1..=10 {
            let transaction_builder = TransactionBuilder::new(i as f64)
                .date(today.checked_sub(Duration::days(i)).unwrap())
                .unwrap()
                .description(&format!("transaction #{i}"));

            create_transaction(transaction_builder, &conn).unwrap();
        }

        let got = query_transactions(
            TransactionQuery {
                limit: Some(5),
                ..Default::default()
            },
            &conn,
        )
        .unwrap();

        assert_eq!(got.len(), 5, "got {} transactions, want 5", got.len());
    }

    #[test]
    fn get_transactions_with_offset() {
        let conn = get_test_connection();
        let offset = 10;
        let limit = 5;
        let mut want = Vec::new();
        for i in 1..20 {
            let transaction = create_transaction(Transaction::build(i as f64), &conn)
                .expect("Could not create transaction");

            if i > offset && i <= offset + limit {
                want.push(transaction);
            }
        }

        let got = query_transactions(
            TransactionQuery {
                offset,
                limit: Some(limit),
                ..Default::default()
            },
            &conn,
        )
        .expect("Could not query transactions");

        assert_eq!(want, got);
    }

    #[test]
    fn get_transactions_descending_date() {
        let conn = get_test_connection();

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

            let transaction = create_transaction(transaction_builder, &conn).unwrap();
            want.push(transaction);
        }

        want.sort_by(|a, b| b.date().cmp(a.date()));

        let got = query_transactions(
            TransactionQuery {
                sort_date: Some(SortOrder::Descending),
                ..Default::default()
            },
            &conn,
        )
        .unwrap();

        assert_eq!(
            got, want,
            "got transactions that were not sorted in descending order."
        );
    }

    #[test]
    fn get_count() {
        let conn = get_test_connection();
        let want_count = 20;
        for i in 1..=want_count {
            create_transaction(Transaction::build(i as f64), &conn)
                .expect("Could not create transaction");
        }

        let got_count = count_transactions(&conn).expect("Could not get count");

        assert_eq!(want_count, got_count);
    }
}
