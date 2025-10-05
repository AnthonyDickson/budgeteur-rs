//! Transaction management for the budgeting application.
//!
//! This module contains everything related to transactions:
//! - The `Transaction` model and `TransactionBuilder` for creating transactions
//! - Database functions for storing, querying, and managing transactions
//! - View handlers for transaction-related web pages

use std::{
    ops::RangeInclusive,
    sync::{Arc, Mutex},
};

use askama::Template;
use axum::{
    Json,
    extract::{FromRef, Path, Query, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use axum_extra::extract::Form;
use axum_htmx::HxRedirect;
use rusqlite::{Connection, Row, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime, UtcOffset};

use crate::{
    AppState, Error,
    database_id::DatabaseID,
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
    pagination::{PaginationConfig, PaginationIndicator, create_pagination_indicators},
    shared_templates::render,
    state::TransactionState,
    tag::{Tag, get_all_tags},
    transaction_tag::{get_transaction_tags, set_transaction_tags},
};

// ============================================================================
// MODELS
// ============================================================================

/// An expense or income, i.e. an event where money was either spent or earned.
///
/// To create a new `Transaction`, use [Transaction::build].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    /// The ID of the transaction.
    pub id: DatabaseID,
    /// The amount of money spent or earned in this transaction.
    pub amount: f64,
    /// When the transaction happened.
    pub date: Date,
    /// A text description of what the transaction was for.
    pub description: String,
    /// The ID of the import that this transaction belongs to.
    pub import_id: Option<i64>,
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
/// ```rust
/// use time::{macros::date, UtcOffset};
///
/// use budgeteur_rs::transaction::Transaction;
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
}

impl TransactionBuilder {
    /// Set the import ID for the transaction.
    pub fn import_id(mut self, import_id: Option<i64>) -> Self {
        self.import_id = import_id;
        self
    }

    /// Build the final [Transaction] instance.
    ///
    /// `local_timezone` is used to check that `.date` is not a future date.
    ///
    /// # Errors
    /// This function will return an error if `date` is a date in the future.
    pub fn finalize(self, id: DatabaseID, local_timezone: UtcOffset) -> Result<Transaction, Error> {
        if self.date > OffsetDateTime::now_utc().to_offset(local_timezone).date() {
            return Err(Error::FutureDate);
        }

        Ok(Transaction {
            id,
            amount: self.amount,
            date: self.date,
            description: self.description,
            import_id: self.import_id,
        })
    }
}

// ============================================================================
// TEMPLATES
// ============================================================================

/// Renders a transaction with its tags as a table row.
#[derive(Template)]
#[template(path = "partials/dashboard/transaction_with_tags.html")]
pub struct TransactionTableRow {
    /// The transaction to display.
    pub transaction: Transaction,
    /// The tags associated with this transaction.
    pub tags: Vec<Tag>,
    /// An optional error message if tags failed to load.
    pub tag_error: Option<String>,
}

// ============================================================================
// ROUTE HANDLERS
// ============================================================================

/// The form data for creating a transaction.
#[derive(Debug, Deserialize)]
pub struct TransactionForm {
    /// The value of the transaction in dollars.
    pub amount: f64,
    /// The date when the transaction ocurred.
    pub date: Date,
    /// Text detailing the transaction.
    pub description: String,
    /// The IDs of tags to associate with this transaction.
    #[serde(default)]
    pub tag_ids: Vec<i64>,
}

/// A route handler for creating a new transaction, redirects to transactions view on success.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_transaction_endpoint(
    State(state): State<TransactionState>,
    Form(data): Form<TransactionForm>,
) -> impl IntoResponse {
    let transaction = Transaction::build(data.amount, data.date, data.description);

    let connection = state.db_connection.lock().unwrap();
    let created_transaction = match create_transaction(transaction, &connection) {
        Ok(transaction) => transaction,
        Err(e) => return e.into_response(),
    };

    if !data.tag_ids.is_empty()
        && let Err(e) = set_transaction_tags(created_transaction.id, &data.tag_ids, &connection)
    {
        tracing::error!(
            "Failed to assign tags to transaction {}: {e}",
            created_transaction.id
        );
        return e.into_response();
    }

    (
        HxRedirect(endpoints::TRANSACTIONS_VIEW.to_owned()),
        StatusCode::SEE_OTHER,
    )
        .into_response()
}

/// A route handler for getting a transaction by its database ID.
///
/// This function will return the status code 404 if the requested resource does not exist (e.g., not created yet).
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_transaction_endpoint(
    State(state): State<TransactionState>,
    Path(transaction_id): Path<DatabaseID>,
) -> impl IntoResponse {
    let connection = state.db_connection.lock().unwrap();
    get_transaction(transaction_id, &connection)
        .map(|transaction| (StatusCode::OK, Json(transaction)))
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
            "INSERT INTO \"transaction\" (amount, date, description, import_id)
             VALUES (?1, ?2, ?3, ?4)
             RETURNING id, amount, date, description, import_id",
        )?
        .query_row(
            (
                builder.amount,
                builder.date,
                builder.description,
                builder.import_id,
            ),
            map_transaction_row,
        )
        .map_err(|error| match error {
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
        "INSERT INTO \"transaction\" (amount, date, description, import_id)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(import_id) DO NOTHING
         RETURNING id, amount, date, description, import_id",
    )?;

    for builder in builders {
        // Try to insert and get the result
        let transaction_result = stmt.query_row(
            (
                builder.amount,
                builder.date,
                builder.description,
                builder.import_id,
            ),
            map_transaction_row,
        );

        // Only collect successfully inserted transactions (not conflicts)
        if let Ok(transaction) = transaction_result {
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
        .prepare(
            "SELECT id, amount, date, description, import_id FROM \"transaction\" WHERE id = :id",
        )?
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
    let mut query_string_parts =
        vec!["SELECT id, amount, date, description, import_id FROM \"transaction\"".to_string()];
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
        .map(|transaction_result| transaction_result.map_err(Error::SqlError))
        .collect()
}

/// Get transactions with pagination and sorting by date.
///
/// # Arguments
/// * `limit` - Maximum number of transactions to return
/// * `offset` - Number of transactions to skip
/// * `sort_order` - Sort direction for date field
/// * `connection` - Database connection reference
///
/// # Errors
/// Returns [Error::SqlError] if:
/// - Database connection fails
/// - SQL query preparation or execution fails
/// - Transaction row mapping fails
pub fn get_transactions_paginated(
    limit: u64,
    offset: u64,
    sort_order: SortOrder,
    connection: &Connection,
) -> Result<Vec<Transaction>, Error> {
    let order_clause = match sort_order {
        SortOrder::Ascending => "ORDER BY date ASC",
        SortOrder::Descending => "ORDER BY date DESC",
    };

    let query = format!(
        "SELECT id, amount, date, description, import_id FROM \"transaction\" {} LIMIT {} OFFSET {}",
        order_clause, limit, offset
    );

    connection
        .prepare(&query)?
        .query_map([], map_transaction_row)?
        .map(|transaction_result| transaction_result.map_err(Error::SqlError))
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
    connection.execute(
        "CREATE TABLE IF NOT EXISTS \"transaction\" (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                amount REAL NOT NULL,
                date TEXT NOT NULL,
                description TEXT NOT NULL,
                import_id INTEGER UNIQUE
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
    let import_id = row.get(4)?;

    Ok(Transaction {
        id,
        amount,
        date,
        description,
        import_id,
    })
}

// ============================================================================
// VIEW HANDLERS
// ============================================================================

/// Renders the new transaction page.
#[derive(Template)]
#[template(path = "views/new_transaction.html")]
struct NewTransactionTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    create_transaction_route: &'a str,
    max_date: Date,
    available_tags: Vec<Tag>,
}

/// The state needed for the new transaction page.
#[derive(Debug, Clone)]
pub struct NewTransactionPageState {
    /// The local timezone as a UTC offset.
    pub local_timezone: UtcOffset,
    /// The database connection for accessing tags.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for NewTransactionPageState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            local_timezone: state.local_timezone,
            db_connection: state.db_connection.clone(),
        }
    }
}

/// Renders the page for creating a transaction.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_new_transaction_page(State(state): State<NewTransactionPageState>) -> Response {
    let nav_bar = get_nav_bar(endpoints::NEW_TRANSACTION_VIEW);

    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    let available_tags = match get_all_tags(&connection) {
        Ok(tags) => tags,
        Err(error) => {
            tracing::error!("Failed to retrieve tags for new transaction page: {error}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load tags").into_response();
        }
    };

    render(
        StatusCode::OK,
        NewTransactionTemplate {
            nav_bar,
            create_transaction_route: endpoints::TRANSACTIONS_API,
            max_date: time::OffsetDateTime::now_utc()
                .to_offset(state.local_timezone)
                .date(),
            available_tags,
        },
    )
}

/// Render an overview of the user's transactions.
pub async fn get_transactions_page(
    State(state): State<TransactionsViewState>,
    Query(query_params): Query<Pagination>,
) -> Response {
    let nav_bar = get_nav_bar(endpoints::TRANSACTIONS_VIEW);

    let current_page = query_params
        .page
        .unwrap_or(state.pagination_config.default_page);
    let per_page = query_params
        .per_page
        .unwrap_or(state.pagination_config.default_page_size);

    let limit = per_page;
    let offset = (current_page - 1) * per_page;
    let connection = state.db_connection.lock().unwrap();
    let page_count = match count_transactions(&connection) {
        Ok(transaction_count) => (transaction_count as f64 / per_page as f64).ceil() as u64,
        Err(error) => return error.into_response(),
    };

    let transactions =
        get_transactions_paginated(limit, offset, SortOrder::Descending, &connection);
    let transactions = match transactions {
        Ok(transactions) => transactions,
        Err(error) => return error.into_response(),
    };

    let transactions = transactions
        .into_iter()
        .map(|transaction| {
            let (tags, tag_error) = match get_transaction_tags(transaction.id, &connection) {
                Ok(tags) => (tags, None),
                Err(error) => {
                    tracing::error!(
                        "Failed to get tags for transaction {}: {error}",
                        transaction.id
                    );
                    (Vec::new(), Some("Failed to load tags".to_string()))
                }
            };
            TransactionTableRow {
                transaction,
                tags,
                tag_error,
            }
        })
        .collect();

    let max_pages = state.pagination_config.max_pages;
    let pagination_indicators = create_pagination_indicators(current_page, page_count, max_pages);

    render(
        StatusCode::OK,
        TransactionsTemplate {
            nav_bar,
            transactions,
            create_transaction_route: Uri::from_static(endpoints::NEW_TRANSACTION_VIEW),
            import_transaction_route: Uri::from_static(endpoints::IMPORT_VIEW),
            transactions_page_route: Uri::from_static(endpoints::TRANSACTIONS_VIEW),
            pagination: &pagination_indicators,
            per_page,
        },
    )
}

/// The state needed for the transactions page.
#[derive(Debug, Clone)]
pub struct TransactionsViewState {
    /// The database connection for managing transactions.
    pub db_connection: Arc<Mutex<Connection>>,
    /// Configuration for pagination controls.
    pub pagination_config: PaginationConfig,
}

impl FromRef<AppState> for TransactionsViewState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
            pagination_config: state.pagination_config.clone(),
        }
    }
}

/// Controls paginations of transactions table.
#[derive(Deserialize)]
pub struct Pagination {
    /// The page number to display. Starts from 1.
    pub page: Option<u64>,
    /// The maximum number of transactions to display per page.
    pub per_page: Option<u64>,
}

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/transactions.html")]
struct TransactionsTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    /// The user's transactions for this week, as Askama templates.
    transactions: Vec<TransactionTableRow>,
    /// The route for creating a new transaction for the current user.
    create_transaction_route: Uri,
    /// The route for importing transactions from CSV files.
    import_transaction_route: Uri,
    /// The route to the transactions (current) page.
    transactions_page_route: Uri,
    pagination: &'a [PaginationIndicator],
    per_page: u64,
    // HACK: ^ Use reference for current page since (de)referencing doesn't work
    // in asakama template as expected.
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
    use std::f64::consts::PI;

    use rusqlite::Connection;
    use time::{Duration, OffsetDateTime, macros::date};

    use crate::{
        Error,
        db::initialize,
        transaction::{
            SortOrder, Transaction, count_transactions, create_transaction, get_transaction,
            get_transactions_paginated, import_transactions, map_transaction_row,
        },
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
    fn import_multiple() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 04);
        let want = vec![
            Transaction::build(123.45, today, "".to_owned()).import_id(Some(123456789)),
            Transaction::build(678.90, today, "".to_owned()).import_id(Some(101112131)),
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

        for (want, got) in want.iter().zip(imported_transactions) {
            assert_eq!(want.amount, got.amount);
            assert_eq!(want.date, got.date);
            assert_eq!(want.description, got.description);
            assert_eq!(want.import_id, got.import_id);
        }
    }

    #[test]
    fn import_ignores_duplicate_import_id() {
        let conn = get_test_connection();
        let import_id = Some(123456789);
        let today = date!(2025 - 10 - 04);
        let want = create_transaction(
            Transaction::build(123.45, today, "".to_owned()).import_id(import_id),
            &conn,
        )
        .expect("Could not create transaction");

        let duplicate_transactions = import_transactions(
            vec![Transaction::build(123.45, today, "".to_owned()).import_id(import_id)],
            &conn,
        )
        .expect("Could not import transactions");

        // The import should return 0 transactions since the import_id already exists
        assert_eq!(
            duplicate_transactions.len(),
            0,
            "import should ignore transactions with duplicate import IDs: want 0 transactions, got {}",
            duplicate_transactions.len()
        );

        // Verify that only the original transaction exists in the database
        let all_transactions = conn
            .prepare("SELECT id, amount, date, description, import_id FROM \"transaction\"")
            .unwrap()
            .query_map([], map_transaction_row)
            .unwrap()
            .map(|transaction_result| transaction_result.map_err(Error::SqlError))
            .collect::<Result<Vec<Transaction>, Error>>()
            .expect("Could not query transactions");

        assert_eq!(
            all_transactions.len(),
            1,
            "Expected exactly 1 transaction in database after duplicate import attempt, got {}",
            all_transactions.len()
        );

        // Verify the original transaction is unchanged
        let stored_transaction = &all_transactions[0];
        assert_eq!(stored_transaction.amount, want.amount);
        assert_eq!(stored_transaction.date, want.date);
        assert_eq!(stored_transaction.description, want.description);
        assert_eq!(stored_transaction.import_id, want.import_id);
    }

    #[tokio::test]
    async fn import_escapes_single_quotes() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);
        let want = vec![
            Transaction::build(123.45, today, "Tom's Hardware".to_owned())
                .import_id(Some(123456789)),
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
            .zip(imported_transactions)
            .for_each(|(want, got)| {
                assert_eq!(want.amount, got.amount);
                assert_eq!(want.date, got.date);
                assert_eq!(want.description, got.description);
                assert_eq!(want.import_id, got.import_id);
            });
    }

    #[test]
    fn get_transaction_by_id_succeeds() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);
        let transaction =
            create_transaction(Transaction::build(PI, today, "".to_owned()), &conn).unwrap();

        let selected_transaction = get_transaction(transaction.id, &conn);

        assert_eq!(Ok(transaction), selected_transaction);
    }

    #[test]
    fn get_transaction_fails_on_invalid_id() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);
        let transaction =
            create_transaction(Transaction::build(123.0, today, "".to_owned()), &conn).unwrap();

        let transaction_result = get_transaction(transaction.id + 654, &conn);

        assert_eq!(transaction_result, Err(Error::NotFound));
    }

    #[test]
    fn get_transactions_with_limit() {
        let conn = get_test_connection();

        let today = OffsetDateTime::now_utc().date();

        for i in 1..=10 {
            let transaction_builder = Transaction::build(
                i as f64,
                today - Duration::days(i),
                format!("transaction #{i}"),
            );

            create_transaction(transaction_builder, &conn).unwrap();
        }

        let got = get_transactions_paginated(5, 0, SortOrder::Ascending, &conn).unwrap();

        assert_eq!(got.len(), 5, "got {} transactions, want 5", got.len());
    }

    #[test]
    fn get_transactions_with_offset() {
        let conn = get_test_connection();
        let offset = 10;
        let limit = 5;
        let today = date!(2025 - 10 - 05);
        let mut want = Vec::new();
        for i in 1..20 {
            let transaction =
                create_transaction(Transaction::build(i as f64, today, "".to_owned()), &conn)
                    .expect("Could not create transaction");

            if i > offset && i <= offset + limit {
                want.push(transaction);
            }
        }

        let got = get_transactions_paginated(limit, offset, SortOrder::Ascending, &conn)
            .expect("Could not query transactions");

        assert_eq!(want, got);
    }

    #[test]
    fn get_transactions_descending_date() {
        let conn = get_test_connection();
        let start_date = OffsetDateTime::now_utc().date() - Duration::weeks(2);
        let mut want = vec![];
        for i in 1..=3 {
            let transaction_builder = Transaction::build(
                i as f64,
                start_date - Duration::days(i),
                format!("transaction #{i}"),
            );

            let transaction = create_transaction(transaction_builder, &conn).unwrap();
            want.push(transaction);
        }
        want.sort_by(|a, b| b.date.cmp(&a.date));

        let got = conn
            .prepare("SELECT id, amount, date, description, import_id FROM \"transaction\" ORDER BY date DESC").unwrap()
            .query_map([], map_transaction_row).unwrap()
            .map(|transaction_result| transaction_result.map_err(Error::SqlError))
            .collect::<Result<Vec<Transaction>, Error>>()
        .unwrap();

        assert_eq!(
            got, want,
            "got transactions that were not sorted in descending order."
        );
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

#[cfg(test)]
mod view_tests {
    use std::sync::{Arc, Mutex};

    use askama::Template;
    use axum::{
        body::Body,
        extract::{Query, State},
        http::StatusCode,
        response::Response,
    };
    use rusqlite::Connection;
    use scraper::{ElementRef, Html, Selector, selectable::Selectable};
    use time::{OffsetDateTime, UtcOffset, macros::date};

    use crate::{
        db::initialize,
        endpoints,
        pagination::{PaginationConfig, PaginationIndicator},
        tag::{TagName, create_tag},
        transaction::TransactionTableRow,
        transaction_tag::set_transaction_tags,
    };

    use super::{
        NewTransactionPageState, Pagination, Transaction, TransactionsViewState,
        create_transaction, get_new_transaction_page, get_transactions_page,
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[tokio::test]
    async fn new_transaction_returns_form() {
        let conn = get_test_connection();
        let state = NewTransactionPageState {
            local_timezone: UtcOffset::UTC,
            db_connection: Arc::new(Mutex::new(conn)),
        };
        let response = get_new_transaction_page(State(state)).await;

        assert_status_ok(&response);
        assert_html_content_type(&response);
        let document = parse_html(response).await;
        assert_valid_html(&document);
        assert_correct_form(&document);
    }

    #[track_caller]
    fn assert_status_ok(response: &Response<Body>) {
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[track_caller]
    fn assert_html_content_type(response: &Response<Body>) {
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "text/html; charset=utf-8"
        );
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors: {:?}",
            html.errors
        );
    }

    #[track_caller]
    fn assert_correct_form(document: &Html) {
        let form_selector = scraper::Selector::parse("form").unwrap();
        let forms = document.select(&form_selector).collect::<Vec<_>>();
        assert_eq!(forms.len(), 1, "want 1 form, got {}", forms.len());

        let form = forms.first().unwrap();
        let hx_post = form.value().attr("hx-post");
        assert_eq!(
            hx_post,
            Some(endpoints::TRANSACTIONS_API),
            "want form with attribute hx-post=\"{}\", got {:?}",
            endpoints::TRANSACTIONS_API,
            hx_post
        );

        assert_correct_inputs(form);
        assert_has_submit_button(form);
    }

    #[track_caller]
    fn assert_correct_inputs(form: &ElementRef) {
        let expected_input_types = vec![
            ("amount", "number"),
            ("date", "date"),
            ("description", "text"),
        ];

        for (name, element_type) in expected_input_types {
            let selector_string = format!("input[type={element_type}]");
            let input_selector = scraper::Selector::parse(&selector_string).unwrap();
            let inputs = form.select(&input_selector).collect::<Vec<_>>();
            assert_eq!(
                inputs.len(),
                1,
                "want 1 {element_type} input, got {}",
                inputs.len()
            );

            let input = inputs.first().unwrap();

            let input_name = input.value().attr("name");
            assert_eq!(
                input_name,
                Some(name),
                "want {element_type} with name=\"{name}\", got {input_name:?}"
            );

            match input_name {
                Some("amount") => {
                    assert_required(input);
                    assert_amount_min_and_step(input);
                }
                Some("date") => {
                    assert_required(input);
                    assert_max_date(input);
                    assert_value(input, &OffsetDateTime::now_utc().date().to_string());
                }
                _ => {}
            }
        }
    }

    #[track_caller]
    fn assert_value(input: &ElementRef, expected_value: &str) {
        let value = input.value().attr("value");
        assert_eq!(
            value,
            Some(expected_value),
            "want input with value=\"{expected_value}\", got {value:?}"
        );
    }

    #[track_caller]
    fn assert_required(input: &ElementRef) {
        let required = input.value().attr("required");
        let input_name = input.value().attr("name").unwrap();
        assert!(
            required.is_some(),
            "want {input_name} input to be required, got {required:?}"
        );
    }

    #[track_caller]
    fn assert_max_date(input: &ElementRef) {
        let today = OffsetDateTime::now_utc().date();
        let max_date = input.value().attr("max");

        assert_eq!(
            Some(today.to_string().as_str()),
            max_date,
            "the date for a new transaction should be limited to the current date {today}, but got {max_date:?}"
        );
    }

    #[track_caller]
    fn assert_amount_min_and_step(input: &ElementRef) {
        let min_value = input
            .value()
            .attr("min")
            .expect("amount input should have the attribute 'min'");
        let min_value: i64 = min_value
            .parse()
            .expect("the attribute 'min' for the amount input should be an integer");
        assert_eq!(
            0, min_value,
            "the amount for a new transaction should be limited to a minimum of 0, but got {min_value}"
        );

        let step = input
            .value()
            .attr("step")
            .expect("amount input should have the attribute 'step'");
        let step: f64 = step
            .parse()
            .expect("the attribute 'step' for the amount input should be a float");
        assert_eq!(
            0.01, step,
            "the amount for a new transaction should increment in steps of 0.01, but got {step}"
        );
    }

    #[track_caller]
    fn assert_has_submit_button(form: &ElementRef) {
        let button_selector = scraper::Selector::parse("button").unwrap();
        let buttons = form.select(&button_selector).collect::<Vec<_>>();
        assert_eq!(buttons.len(), 1, "want 1 button, got {}", buttons.len());
        let button_type = buttons.first().unwrap().value().attr("type");
        assert_eq!(
            button_type,
            Some("submit"),
            "want button with type=\"submit\", got {button_type:?}"
        );
    }

    #[tokio::test]
    async fn transactions_page_displays_paged_data() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);

        // Create 30 transactions in the database
        for i in 1..=30 {
            create_transaction(Transaction::build(i as f64, today, "".to_owned()), &conn).unwrap();
        }

        let state = TransactionsViewState {
            db_connection: Arc::new(Mutex::new(conn)),
            pagination_config: PaginationConfig {
                max_pages: 5,
                ..Default::default()
            },
        };
        let per_page = 3;
        let page = 5;
        let want_transactions = [
            Transaction {
                id: 13,
                amount: 1.0,
                date: today,
                description: "".to_owned(),
                import_id: None,
            },
            Transaction {
                id: 14,
                amount: 1.0,
                date: today,
                description: "".to_owned(),
                import_id: None,
            },
            Transaction {
                id: 15,
                amount: 1.0,
                date: today,
                description: "".to_owned(),
                import_id: None,
            },
        ];
        let want_indicators = [
            PaginationIndicator::BackButton(4),
            PaginationIndicator::Page(1),
            PaginationIndicator::Ellipsis,
            PaginationIndicator::Page(3),
            PaginationIndicator::Page(4),
            PaginationIndicator::CurrPage(5),
            PaginationIndicator::Page(6),
            PaginationIndicator::Page(7),
            PaginationIndicator::Ellipsis,
            PaginationIndicator::Page(10),
            PaginationIndicator::NextButton(6),
        ];

        let response = get_transactions_page(
            State(state),
            Query(Pagination {
                page: Some(page),
                per_page: Some(per_page),
            }),
        )
        .await;

        let html = parse_html(response).await;
        assert_valid_html(&html);
        let table = must_get_table(&html);
        assert_table_has_transactions(table, &want_transactions);
        let pagination = must_get_pagination_indicator(&html);
        assert_correct_pagination_indicators(pagination, per_page, &want_indicators);
    }

    #[track_caller]
    fn must_get_table(html: &Html) -> ElementRef<'_> {
        html.select(&Selector::parse("table").unwrap())
            .next()
            .expect("No table found")
    }

    #[track_caller]
    fn assert_table_has_transactions(table: ElementRef, transactions: &[Transaction]) {
        let row_selector = Selector::parse("tbody tr").unwrap();
        let table_rows: Vec<ElementRef<'_>> = table.select(&row_selector).collect();

        assert_eq!(
            table_rows.len(),
            transactions.len(),
            "want table with {} rows, got {}",
            transactions.len(),
            table_rows.len()
        );

        let th_selector = Selector::parse("th").unwrap();
        for (i, (row, want)) in table_rows.iter().zip(transactions).enumerate() {
            let th = row
                .select(&th_selector)
                .next()
                .unwrap_or_else(|| panic!("Could not find th element in table row {i}"));

            let id_str = th.text().collect::<String>();
            let got_id: i64 = id_str.trim().parse().unwrap_or_else(|_| {
                panic!("Could not parse ID {id_str} on table row {i} as integer")
            });

            assert_eq!(
                got_id, want.id,
                "Want transaction with ID {}, got {got_id}",
                want.id
            );
        }
    }

    #[track_caller]
    fn must_get_pagination_indicator(html: &Html) -> ElementRef<'_> {
        html.select(&Selector::parse("nav.pagination > ul.pagination").unwrap())
            .next()
            .expect("No pagination indicator found")
    }

    #[track_caller]
    fn assert_correct_pagination_indicators(
        pagination_indicator: ElementRef,
        want_per_page: u64,
        want_indicators: &[PaginationIndicator],
    ) {
        let li_selector = Selector::parse("li").unwrap();
        let list_items: Vec<ElementRef> = pagination_indicator.select(&li_selector).collect();
        let list_len = list_items.len();
        let want_len = want_indicators.len();
        assert_eq!(list_len, want_len, "got {list_len} pages, want {want_len}");

        let link_selector = Selector::parse("a").unwrap();

        for (i, (list_item, want_indicator)) in list_items.iter().zip(want_indicators).enumerate() {
            match *want_indicator {
                PaginationIndicator::CurrPage(want_page) => {
                    assert!(
                        list_item.select(&link_selector).next().is_none(),
                        "The current page indicator should not contain a link"
                    );

                    let paragraph_selector =
                        Selector::parse("p").expect("Could not create selector 'p'");
                    let paragraph = list_item
                        .select(&paragraph_selector)
                        .next()
                        .expect("Current page indicator should have a paragraph element ('<p>')");

                    assert_eq!(paragraph.attr("aria-current"), Some("page"));

                    let text = {
                        let text = paragraph.text().collect::<String>();
                        text.trim().to_owned()
                    };

                    let got_page_number: u64 = text.parse().unwrap_or_else(|_| {
                        panic!(
                            "Could not parse \"{text}\" as a u64 for list item {i} in {}",
                            list_item.html()
                        )
                    });

                    assert_eq!(
                        want_page,
                        got_page_number,
                        "want page number {want_page}, got {got_page_number} for list item {i} in {}",
                        pagination_indicator.html()
                    );
                }
                PaginationIndicator::Page(want_page) => {
                    let link = list_item.select(&link_selector).next().unwrap_or_else(|| {
                        panic!("Could not get link (<a> tag) for list item {i}")
                    });
                    let link_text = {
                        let text = link.text().collect::<String>();
                        text.trim().to_owned()
                    };
                    let got_page_number = link_text.parse::<u64>().unwrap_or_else(|_| {
                        panic!(
                            "Could not parse page number {link_text} for page {want_page} as usize"
                        )
                    });

                    assert_eq!(
                        want_page,
                        got_page_number,
                        "want page number {want_page}, got {got_page_number} for list item {i} in {}",
                        pagination_indicator.html()
                    );

                    let link_target = link.attr("href").unwrap_or_else(|| {
                        panic!("Link for page {want_page} did not have href element")
                    });
                    let want_target = format!(
                        "{}?page={want_page}&per_page={want_per_page}",
                        endpoints::TRANSACTIONS_VIEW
                    );
                    assert_eq!(
                        want_target, link_target,
                        "Got incorrect page link for page {want_page}"
                    );
                }
                PaginationIndicator::Ellipsis => {
                    assert!(
                        list_item.select(&link_selector).next().is_none(),
                        "Item {i} should not contain a link tag (<a>) in {}",
                        pagination_indicator.html()
                    );
                    let got_text = list_item.text().collect::<String>();
                    let got_text = got_text.trim();
                    assert_eq!(got_text, "...");
                }
                PaginationIndicator::NextButton(want_page) => {
                    let link = list_item.select(&link_selector).next().unwrap_or_else(|| {
                        panic!("Could not get link (<a> tag) for list item {i}")
                    });
                    let link_text = {
                        let text = link.text().collect::<String>();
                        text.trim().to_owned()
                    };
                    assert_eq!(
                        "Next", link_text,
                        "want link text \"Next\", got \"{link_text}\""
                    );

                    let role = link
                        .attr("role")
                        .expect("The next button did not have \"role\" attribute.");
                    assert_eq!(
                        role, "button",
                        "The next page anchor tag should be marked as a button."
                    );

                    let link_target = link
                        .attr("href")
                        .expect("Link for next button did not have href element");
                    let want_target = format!(
                        "{}?page={want_page}&per_page={want_per_page}",
                        endpoints::TRANSACTIONS_VIEW
                    );
                    assert_eq!(
                        want_target, link_target,
                        "Got link to {link_target} for next button, want {want_page}"
                    );
                }
                PaginationIndicator::BackButton(want_page) => {
                    let link = list_item.select(&link_selector).next().unwrap_or_else(|| {
                        panic!("Could not get link (<a> tag) for list item {i}")
                    });
                    let link_text = {
                        let text = link.text().collect::<String>();
                        text.trim().to_owned()
                    };
                    assert_eq!(
                        "Back", link_text,
                        "want link text \"Back\", got \"{link_text}\""
                    );

                    let role = link
                        .attr("role")
                        .expect("The back button did not have \"role\" attribute.");
                    assert_eq!(
                        role, "button",
                        "The back button's anchor tag should be marked as a button."
                    );

                    let link_target = link
                        .attr("href")
                        .expect("Link for back button did not have href element");
                    let want_target = format!(
                        "{}?page={want_page}&per_page={want_per_page}",
                        endpoints::TRANSACTIONS_VIEW
                    );
                    assert_eq!(
                        want_target, link_target,
                        "Got link to {link_target} for back button, want {want_page}"
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn transactions_page_displays_tags_column() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);

        // Create test tags
        let tag1 = create_tag(TagName::new_unchecked("Groceries"), &conn).unwrap();
        let tag2 = create_tag(TagName::new_unchecked("Food"), &conn).unwrap();

        // Create transactions
        let transaction1 = create_transaction(
            Transaction::build(50.0, today, "Store purchase".to_owned()),
            &conn,
        )
        .unwrap();
        let transaction2 = create_transaction(
            Transaction::build(25.0, today, "Restaurant".to_owned()),
            &conn,
        )
        .unwrap();
        let _transaction3 = create_transaction(
            Transaction::build(100.0, today, "No tags transaction".to_owned()),
            &conn,
        )
        .unwrap();

        // Assign tags to transactions
        set_transaction_tags(transaction1.id, &[tag1.id, tag2.id], &conn).unwrap();
        set_transaction_tags(transaction2.id, &[tag1.id], &conn).unwrap();
        // transaction3 gets no tags

        let state = TransactionsViewState {
            db_connection: Arc::new(Mutex::new(conn)),
            pagination_config: PaginationConfig::default(),
        };

        let response = get_transactions_page(
            State(state),
            Query(Pagination {
                page: Some(1),
                per_page: Some(10),
            }),
        )
        .await;

        let html = parse_html(response).await;
        assert_valid_html(&html);

        // Check that Tags column header exists
        let headers = html
            .select(&Selector::parse("thead th").unwrap())
            .collect::<Vec<_>>();
        let header_texts: Vec<String> = headers
            .iter()
            .map(|h| h.text().collect::<String>().trim().to_string())
            .collect();
        assert!(
            header_texts.contains(&"Tags".to_string()),
            "Tags column header should exist. Found headers: {:?}",
            header_texts
        );

        // Check table rows for tag content
        let table_rows = html
            .select(&Selector::parse("tbody tr").unwrap())
            .collect::<Vec<_>>();
        assert_eq!(table_rows.len(), 3, "Should have 3 transaction rows");

        // Check that each row has 5 columns (ID, Amount, Date, Description, Tags)
        for (i, row) in table_rows.iter().enumerate() {
            let cells = row
                .select(&Selector::parse("th, td").unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                cells.len(),
                5,
                "Row {} should have 5 columns (ID, Amount, Date, Description, Tags)",
                i
            );

            // The last cell should be the Tags column
            let tags_cell = &cells[4];
            let tags_cell_html = tags_cell.html();

            // Check if this row should have tags or not
            if tags_cell_html.contains("-") && !tags_cell_html.contains("bg-blue-100") {
                // This is the "no tags" case showing "-"
                assert!(
                    tags_cell_html.contains("text-gray-400"),
                    "Empty tags should be displayed with gray text"
                );
            } else {
                // Should contain tag badges
                assert!(
                    tags_cell_html.contains("bg-blue-100"),
                    "Tag should have blue background styling"
                );
            }
        }
    }

    #[test]
    fn transaction_table_row_displays_tag_error() {
        let transaction = Transaction {
            id: 1,
            amount: 50.0,
            date: date!(2025 - 10 - 05),
            description: "Test transaction".to_owned(),
            import_id: None,
        };

        let row_with_error = TransactionTableRow {
            transaction: transaction.clone(),
            tags: vec![],
            tag_error: Some("Failed to load tags".to_string()),
        };

        let rendered = row_with_error.render().unwrap();
        assert!(
            rendered.contains("Error: Failed to load tags"),
            "Should display error message when tag_error is present"
        );
        assert!(
            rendered.contains("text-red-600"),
            "Error should be displayed in red"
        );

        let row_without_error = TransactionTableRow {
            transaction,
            tags: vec![],
            tag_error: None,
        };

        let rendered = row_without_error.render().unwrap();
        assert!(
            !rendered.contains("Error:"),
            "Should not display error when tag_error is None"
        );
        assert!(
            rendered.contains("text-gray-400"),
            "Should display gray dash when no tags and no error"
        );
    }

    async fn parse_html(response: Response) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX)
            .await
            .expect("Could not get response body");
        let text = String::from_utf8_lossy(&body).to_string();

        Html::parse_document(&text)
    }
}

#[cfg(test)]
mod route_handler_tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        body::Body,
        extract::{Path, State},
        http::{Response, StatusCode},
        response::IntoResponse,
    };
    use axum_extra::extract::Form;
    use axum_htmx::HX_REDIRECT;
    use time::{OffsetDateTime, macros::date};

    use crate::{
        db::initialize,
        state::TransactionState,
        transaction::{Transaction, create_transaction as create_transaction_db, get_transaction},
        transaction_tag::get_transaction_tags,
    };
    use rusqlite::Connection;

    use super::{TransactionForm, create_transaction_endpoint, get_transaction_endpoint};

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[tokio::test]
    async fn can_create_transaction() {
        let conn = get_test_connection();
        let state = TransactionState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let form = TransactionForm {
            description: "test transaction".to_string(),
            amount: 12.3,
            date: OffsetDateTime::now_utc().date(),
            tag_ids: vec![],
        };

        let response = create_transaction_endpoint(State(state.clone()), Form(form))
            .await
            .into_response();

        assert_redirects_to_transactions_view(response);

        // Verify the transaction was actually created by getting it by ID
        // We know the first transaction will have ID 1
        let connection = state.db_connection.lock().unwrap();
        let transaction = get_transaction(1, &connection).unwrap();
        assert_eq!(transaction.amount, 12.3);
        assert_eq!(transaction.description, "test transaction");
    }

    #[tokio::test]
    async fn can_create_transaction_with_tags() {
        let conn = get_test_connection();

        // Create test tags
        let tag1 =
            crate::tag::create_tag(crate::tag::TagName::new_unchecked("Groceries"), &conn).unwrap();
        let tag2 =
            crate::tag::create_tag(crate::tag::TagName::new_unchecked("Food"), &conn).unwrap();

        let state = TransactionState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let form = TransactionForm {
            description: "test transaction with tags".to_string(),
            amount: 25.50,
            date: OffsetDateTime::now_utc().date(),
            tag_ids: vec![tag1.id, tag2.id],
        };

        let response = create_transaction_endpoint(State(state.clone()), Form(form))
            .await
            .into_response();

        assert_redirects_to_transactions_view(response);

        // Verify the transaction was created with tags
        let connection = state.db_connection.lock().unwrap();
        let transaction = get_transaction(1, &connection).unwrap();
        let tags = get_transaction_tags(transaction.id, &connection).unwrap();

        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&tag1));
        assert!(tags.contains(&tag2));
    }

    #[tokio::test]
    async fn can_get_transaction() {
        let conn = get_test_connection();
        let state = TransactionState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        // Create a transaction first
        let transaction = {
            let connection = state.db_connection.lock().unwrap();
            create_transaction_db(
                Transaction::build(13.34, date!(2025 - 10 - 05), "foobar".to_owned()),
                &connection,
            )
            .unwrap()
        };

        let response = get_transaction_endpoint(State(state), Path(transaction.id))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let json_response = extract_from_json(response).await;
        assert_eq!(json_response, transaction);
    }

    async fn extract_from_json(response: Response<Body>) -> Transaction {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        serde_json::from_slice(&body).unwrap()
    }

    #[track_caller]
    fn assert_redirects_to_transactions_view(response: Response<Body>) {
        let location = response
            .headers()
            .get(HX_REDIRECT)
            .expect("expected response to have the header hx-redirect");
        assert_eq!(
            location, "/transactions",
            "got redirect to {location:?}, want redirect to /transactions"
        );
    }
}
