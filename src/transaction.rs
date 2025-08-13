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
use askama_axum::Template as AxumTemplate;
use axum::{
    Form, Json,
    extract::{FromRef, Path, Query, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;
use rusqlite::{Connection, Row, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};

use crate::{
    AppState, Error,
    category::{Category, get_all_categories},
    database_id::DatabaseID,
    pagination::{PaginationConfig, PaginationIndicator, create_pagination_indicators},
    state::{NewTransactionState, TransactionState},
    {
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
        shared_templates::TransactionRow,
    },
};

// ============================================================================
// MODELS
// ============================================================================

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
}

/// A summary of transaction amounts over a period, with income, expenses, and net income.
#[derive(Debug, Clone, PartialEq)]
pub struct TransactionSummary {
    /// Total positive transaction amounts (income)
    pub income: f64,
    /// Total negative transaction amounts (expenses, as absolute value)
    pub expenses: f64,
    /// Net income (income - expenses)
    pub net_income: f64,
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
/// use budgeteur_rs::transaction::Transaction;
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
    /// The ID of the category to assign the transaction to.
    ///
    /// Zero should be interpreted as `None`.
    pub category_id: DatabaseID,
}

/// A route handler for creating a new transaction, returns [TransactionRow] as a [Response] on success.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_transaction_endpoint(
    State(state): State<TransactionState>,
    Form(data): Form<TransactionForm>,
) -> impl IntoResponse {
    // HACK: Zero is used as a sentinel value for None. Currently, options do not work with empty
    // form values. For example, the URL encoded form "num=" will return an error.
    let category = match data.category_id {
        0 => None,
        id => Some(id),
    };

    let transaction = Transaction::build(data.amount)
        .description(&data.description)
        .category(category)
        .date(data.date);

    let transaction = match transaction {
        Ok(transaction) => transaction,
        Err(e) => return e.into_response(),
    };

    let connection = state.db_connection.lock().unwrap();
    match create_transaction(transaction, &connection) {
        Ok(_) => {}
        Err(e) => return e.into_response(),
    }

    (
        HxRedirect(Uri::from_static(endpoints::TRANSACTIONS_VIEW)),
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


/// Get a summary of transactions (income, expenses, net income) within a date range.
///
/// # Arguments
/// * `date_range` - The inclusive date range to summarize transactions for
/// * `connection` - Database connection reference
///
/// # Errors
/// Returns [Error::SqlError] if:
/// - Database connection fails
/// - SQL query preparation or execution fails
pub fn get_transaction_summary(
    date_range: RangeInclusive<Date>,
    connection: &Connection,
) -> Result<TransactionSummary, Error> {
    let mut stmt = connection.prepare(
        "SELECT 
            COALESCE(SUM(CASE WHEN amount > 0 THEN amount ELSE 0 END), 0) as income,
            COALESCE(SUM(CASE WHEN amount < 0 THEN -amount ELSE 0 END), 0) as expenses
        FROM \"transaction\" 
        WHERE date BETWEEN ?1 AND ?2"
    )?;

    let (income, expenses): (f64, f64) = stmt.query_row(
        [&date_range.start().to_string(), &date_range.end().to_string()],
        |row| {
            Ok((row.get(0)?, row.get(1)?))
        }
    )?;

    Ok(TransactionSummary {
        income,
        expenses,
        net_income: income - expenses,
    })
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
        "SELECT id, amount, date, description, category_id, import_id FROM \"transaction\" {} LIMIT {} OFFSET {}",
        order_clause, limit, offset
    );

    connection
        .prepare(&query)?
        .query_map([], map_transaction_row)?
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

// ============================================================================
// VIEW HANDLERS
// ============================================================================

/// Renders the new transaction page.
#[derive(Template)]
#[template(path = "views/new_transaction.html")]
struct NewTransactionTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    create_transaction_route: &'a str,
    new_category_route: &'a str,
    categories: Vec<Category>,
    max_date: Date,
}

/// Renders the page for creating a transaction.
pub async fn get_new_transaction_page(State(state): State<NewTransactionState>) -> Response {
    let categories = match get_all_categories(
        &state
            .db_connection
            .lock()
            .expect("Could not acquire database lock"),
    ) {
        Ok(categories) => categories,
        Err(error) => {
            tracing::error!(
                "Failed to retrieve categories for new transaction page: {}",
                error
            );
            return error.into_response();
        }
    };

    let nav_bar = get_nav_bar(endpoints::NEW_TRANSACTION_VIEW);

    NewTransactionTemplate {
        nav_bar,
        create_transaction_route: endpoints::TRANSACTIONS_API,
        new_category_route: endpoints::NEW_CATEGORY_VIEW,
        categories,
        max_date: time::OffsetDateTime::now_utc().date(),
    }
    .into_response()
}

/// Render an overview of the user's transactions.
pub async fn get_transactions_page(
    State(state): State<TransactionsViewState>,
    Query(query_params): Query<Pagination>,
) -> Response {
    let nav_bar = get_nav_bar(endpoints::TRANSACTIONS_VIEW);

    let curr_page = query_params
        .page
        .unwrap_or(state.pagination_config.default_page);
    let per_page = query_params
        .per_page
        .unwrap_or(state.pagination_config.default_page_size);

    let limit = per_page;
    let offset = (curr_page - 1) * per_page;
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
        .map(|transaction| TransactionRow { transaction })
        .collect();

    let max_pages = state.pagination_config.max_pages;
    let pagination_indicators = create_pagination_indicators(curr_page, page_count, max_pages);

    TransactionsTemplate {
        nav_bar,
        transactions,
        create_transaction_route: Uri::from_static(endpoints::NEW_TRANSACTION_VIEW),
        import_transaction_route: Uri::from_static(endpoints::IMPORT_VIEW),
        transactions_page_route: Uri::from_static(endpoints::TRANSACTIONS_VIEW),
        pagination: &pagination_indicators,
        per_page,
    }
    .into_response()
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
#[derive(AxumTemplate)]
#[template(path = "views/transactions.html")]
struct TransactionsTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    /// The user's transactions for this week, as Askama templates.
    transactions: Vec<TransactionRow>,
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

    use time::{Duration, OffsetDateTime};

    use super::{Error, Transaction, TransactionBuilder};

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

#[cfg(test)]
mod database_tests {
    use std::f64::consts::PI;

    use rusqlite::Connection;
    use time::{Duration, OffsetDateTime};

    use crate::{Error, db::initialize};

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
        let all_transactions = conn
            .prepare(
                "SELECT id, amount, date, description, category_id, import_id FROM \"transaction\"",
            )
            .unwrap()
            .query_map([], map_transaction_row)
            .unwrap()
            .map(|maybe_transaction| maybe_transaction.map_err(Error::SqlError))
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

        let got = get_transactions_paginated(5, 0, SortOrder::Ascending, &conn).unwrap();

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

        let got = get_transactions_paginated(limit, offset, SortOrder::Ascending, &conn)
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

        let got = conn
            .prepare("SELECT id, amount, date, description, category_id, import_id FROM \"transaction\" ORDER BY date DESC").unwrap()
            .query_map([], map_transaction_row).unwrap()
            .map(|maybe_transaction| maybe_transaction.map_err(Error::SqlError))
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
        let want_count = 20;
        for i in 1..=want_count {
            create_transaction(Transaction::build(i as f64), &conn)
                .expect("Could not create transaction");
        }

        let got_count = count_transactions(&conn).expect("Could not get count");

        assert_eq!(want_count, got_count);
    }
}

#[cfg(test)]
mod view_tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use axum::{
        body::Body,
        extract::{Query, State},
        http::StatusCode,
        response::Response,
    };
    use rusqlite::Connection;
    use scraper::{ElementRef, Html, Selector, selectable::Selectable};
    use time::OffsetDateTime;

    use crate::{
        category::{Category, CategoryName, create_category, create_category_table},
        db::initialize,
        endpoints,
        pagination::PaginationConfig,
        state::NewTransactionState,
    };

    use super::*;

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[tokio::test]
    async fn new_transaction_returns_form() {
        let connection =
            Connection::open_in_memory().expect("Could not create in-memory SQLite database");
        create_category_table(&connection).expect("Could not create category table");
        let mut categories = vec![
            create_category(CategoryName::new_unchecked("foo"), &connection)
                .expect("Could not create test category"),
            create_category(CategoryName::new_unchecked("bar"), &connection)
                .expect("Could not create test category"),
        ];
        // This category should be auto-generated by the view.
        categories.push(Category {
            id: 0,
            name: CategoryName::new_unchecked("None"),
        });
        let app_state = NewTransactionState {
            db_connection: Arc::new(Mutex::new(connection)),
        };

        let response = get_new_transaction_page(State(app_state)).await;

        assert_status_ok(&response);
        assert_html_content_type(&response);
        let document = parse_html(response).await;
        assert_valid_html(&document);
        assert_correct_form(&document, categories);
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
    fn assert_correct_form(document: &Html, categories: Vec<Category>) {
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
        assert_correct_select_and_options(form, categories);
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
    fn assert_correct_select_and_options(form: &ElementRef, categories: Vec<Category>) {
        let select_selector = scraper::Selector::parse("select").unwrap();
        let selects = form.select(&select_selector).collect::<Vec<_>>();
        assert_eq!(selects.len(), 1, "want 1 select tag, got {}", selects.len());
        let select_tag = selects.first().unwrap();
        let select_name = select_tag.value().attr("name");
        assert_eq!(
            select_name,
            Some("category_id"),
            "want select with name=\"category_id\", got {select_name:?}"
        );

        let select_option_selector = scraper::Selector::parse("option").unwrap();
        let options = select_tag
            .select(&select_option_selector)
            .collect::<Vec<_>>();

        assert_eq!(
            categories.len(),
            options.len(),
            "want {} options, got {}",
            categories.len(),
            options.len()
        );
        let mut category_names = HashMap::new();
        for category in categories {
            category_names.insert(category.id, category.name.clone());
        }

        for option in options {
            let option_value = option.value().attr("value");
            let option_text = option.text().collect::<String>();
            let category_id = option_value
                .unwrap()
                .parse::<i64>()
                .expect("got option with non-integer value");
            let category_name = category_names
                .get(&category_id)
                .expect("got option with unknown category id");

            assert_eq!(
                option_text,
                category_name.as_ref(),
                "want option with value=\"{category_id}\" to have text \"{category_name}\", got {option_text:?}"
            );
        }
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

        // Create 30 transactions in the database
        for i in 1..=30 {
            create_transaction(TransactionBuilder::new(i as f64), &conn).unwrap();
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
            TransactionBuilder::new(1.0).finalise(13),
            TransactionBuilder::new(1.0).finalise(14),
            TransactionBuilder::new(1.0).finalise(15),
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
    fn must_get_table(html: &Html) -> ElementRef {
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
                got_id,
                want.id(),
                "Want transaction with ID {}, got {got_id}",
                want.id()
            );
        }
    }

    #[track_caller]
    fn must_get_pagination_indicator(html: &Html) -> ElementRef {
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

    use askama_axum::IntoResponse;
    use axum::{
        Form,
        body::Body,
        extract::{Path, State},
        http::{Response, StatusCode},
    };
    use axum_htmx::HX_REDIRECT;
    use time::OffsetDateTime;

    use crate::{
        db::initialize,
        state::TransactionState,
        transaction::{
            Transaction, TransactionBuilder, create_transaction as create_transaction_db,
            get_transaction,
        },
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
            category_id: 0, // 0 means no category
        };

        let response = create_transaction_endpoint(State(state.clone()), Form(form))
            .await
            .into_response();

        assert_redirects_to_transactions_view(response);

        // Verify the transaction was actually created by getting it by ID
        // We know the first transaction will have ID 1
        let connection = state.db_connection.lock().unwrap();
        let transaction = get_transaction(1, &connection).unwrap();
        assert_eq!(transaction.amount(), 12.3);
        assert_eq!(transaction.description(), "test transaction");
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
                TransactionBuilder::new(13.34).description("foobar"),
                &connection,
            )
            .unwrap()
        };

        let response = get_transaction_endpoint(State(state), Path(transaction.id()))
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

#[cfg(test)]
mod get_transaction_summary_tests {
    use time::macros::date;
    use rusqlite::Connection;

    use crate::db::initialize;
    use super::{TransactionSummary, create_transaction, get_transaction_summary, Transaction};

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn returns_summary_with_income_and_expenses() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Create test transactions
        create_transaction(Transaction::build(100.0).date(start_date).unwrap(), &conn).unwrap();
        create_transaction(Transaction::build(-50.0).date(date!(2024 - 01 - 15)).unwrap(), &conn).unwrap();
        create_transaction(Transaction::build(75.0).date(end_date).unwrap(), &conn).unwrap();
        create_transaction(Transaction::build(-25.0).date(date!(2024 - 01 - 20)).unwrap(), &conn).unwrap();

        let result = get_transaction_summary(start_date..=end_date, &conn).unwrap();

        assert_eq!(result.income, 175.0);
        assert_eq!(result.expenses, 75.0);
        assert_eq!(result.net_income, 100.0);
    }

    #[test]
    fn returns_zero_summary_for_no_transactions() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        let result = get_transaction_summary(start_date..=end_date, &conn).unwrap();

        let expected = TransactionSummary {
            income: 0.0,
            expenses: 0.0,
            net_income: 0.0,
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn excludes_transactions_outside_date_range() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Transactions within range
        create_transaction(Transaction::build(100.0).date(start_date).unwrap(), &conn).unwrap();
        create_transaction(Transaction::build(-50.0).date(end_date).unwrap(), &conn).unwrap();

        // Transactions outside range
        create_transaction(Transaction::build(200.0).date(date!(2023 - 12 - 31)).unwrap(), &conn).unwrap();
        create_transaction(Transaction::build(-100.0).date(date!(2024 - 02 - 01)).unwrap(), &conn).unwrap();

        let result = get_transaction_summary(start_date..=end_date, &conn).unwrap();

        assert_eq!(result.income, 100.0);
        assert_eq!(result.expenses, 50.0);
        assert_eq!(result.net_income, 50.0);
    }
}
