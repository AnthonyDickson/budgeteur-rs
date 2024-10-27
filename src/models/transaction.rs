//! This file defines the type `Transaction`, the core type of the budgeting part of the
//! application.

use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{Date, OffsetDateTime};

use crate::{
    db::{CreateTable, MapRow},
    models::{DatabaseID, UserID},
    stores::CategoryStore,
    AppState,
};

use super::CategoryError;

/// Errors that can occur during the creation or retrieval of a transaction.
#[derive(Debug, Error, PartialEq)]
pub enum TransactionError {
    /// A date in the future was used to create a transaction.
    ///
    /// Transactions record events that have already happened, therefore future dates are disallowed.
    #[error("transaction dates must not be later than the current date")]
    FutureDate,

    /// The category ID used to create a transaction did not match a valid category.
    #[error("the category ID does not refer to a valid category")]
    InvalidCategory,

    /// The user ID used to create a transaction did not match a valid user.
    #[error("the user ID does not refer to a valid user")]
    InvalidUser,

    /// There was no transaction in the database that matched the given details.
    #[error("a transaction with the given details could not be found")]
    NotFound,

    /// There was an unexpected and unhandled SQL error.
    #[error("an unexpected error occurred: {0}")]
    SqlError(rusqlite::Error),

    /// There was an unexpected and unhandled error.
    #[error("an unexpected error occurred: {0}")]
    Unspecified(String),
}

impl From<rusqlite::Error> for TransactionError {
    fn from(value: rusqlite::Error) -> Self {
        match value {
            rusqlite::Error::QueryReturnedNoRows => TransactionError::NotFound,
            value => {
                tracing::error!("an unhandled SQL error occurred: {}", value);
                TransactionError::SqlError(value)
            }
        }
    }
}

/// An expense or income, i.e. an event where money was either spent or earned.
///
/// To create a new `Transaction`, use [Transaction::build]. To retrieve an existing
/// transaction, use [Transaction::select] to get a transaction by its ID and
/// [Transaction::select_by_user] to get transactions by user.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    id: DatabaseID,
    amount: f64,
    date: Date,
    description: String,
    category_id: Option<DatabaseID>,
    user_id: UserID,
}

impl Transaction {
    /// Create a new transaction.
    ///
    /// Shortcut for [TransactionBuilder::new] for discoverability.
    ///
    /// If you are trying to retrieve an exisiting transaction, see [Transaction::select] and
    /// [Transaction::select_by_user].
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

    /// Retrieve a transaction in the database by its `id`.
    ///
    /// # Errors
    /// This function will return a:
    /// - [TransactionError::NotFound] if `id` does not refer to a valid transaction,
    /// - or [TransactionError::SqlError] there is some other SQL error.
    pub fn select(
        id: DatabaseID,
        connection: &Connection,
    ) -> Result<Transaction, TransactionError> {
        let transaction = connection
                .prepare("SELECT id, amount, date, description, category_id, user_id FROM \"transaction\" WHERE id = :id")?
                .query_row(&[(":id", &id)], Transaction::map_row)?;

        Ok(transaction)
    }

    /// Retrieve the transactions in the database that have `user_id`.
    ///
    /// An empty vector is returned if the specified user has no transactions.
    ///
    /// # Errors
    /// This function will return a [TransactionError::SqlError] if there is an SQL error.
    pub fn select_by_user(
        user_id: UserID,
        connection: &Connection,
    ) -> Result<Vec<Transaction>, TransactionError> {
        connection
                .prepare("SELECT id, amount, date, description, category_id, user_id FROM \"transaction\" WHERE user_id = :user_id")?
                .query_map(&[(":user_id", &user_id.as_i64())], Transaction::map_row)?
                .map(|maybe_category| maybe_category.map_err(TransactionError::SqlError))
                .collect()
    }
}

impl CreateTable for Transaction {
    fn create_table(connection: &Connection) -> Result<(), rusqlite::Error> {
        connection
                .execute(
                    "CREATE TABLE \"transaction\" (
                            id INTEGER PRIMARY KEY,
                            amount REAL NOT NULL,
                            date TEXT NOT NULL,
                            description TEXT NOT NULL,
                            category_id INTEGER,
                            user_id INTEGER NOT NULL,
                            FOREIGN KEY(category_id) REFERENCES category(id) ON UPDATE CASCADE ON DELETE CASCADE,
                            FOREIGN KEY(user_id) REFERENCES user(id) ON UPDATE CASCADE ON DELETE CASCADE
                            )",
                    (),
                )?;

        Ok(())
    }
}

impl MapRow for Transaction {
    type ReturnType = Self;

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            id: row.get(offset)?,
            amount: row.get(offset + 1)?,
            date: row.get(offset + 2)?,
            description: row.get(offset + 3)?,
            category_id: row.get(offset + 4)?,
            user_id: UserID::new(row.get(offset + 5)?),
        })
    }
}

/// Builder for creating a new [Transaction].
///
/// The function for finalizing the builder is [TransactionBuilder::insert].
///
/// If you are trying to retrieve an exisiting transaction, see [Transaction::select] and
/// [Transaction::select_by_user].
#[derive(Debug, PartialEq)]
pub struct TransactionBuilder {
    amount: f64,
    date: Date,
    description: String,
    category_id: Option<DatabaseID>,
    user_id: UserID,
}

impl TransactionBuilder {
    /// Create a new transaction and insert it into the application database.
    ///
    /// Finalize the builder with [TransactionBuilder::insert].
    ///
    /// If you are trying to retrieve an exisiting transaction, see [Transaction::select] and
    /// [Transaction::select_by_user].
    pub fn new(amount: f64, user_id: UserID) -> Self {
        Self {
            amount,
            date: OffsetDateTime::now_utc().date(),
            description: String::new(),
            category_id: None,
            user_id,
        }
    }

    /// Set the date for the transaction.
    ///
    /// # Errors
    /// This function will return an error if `date` is a date in the future.
    pub fn date(&mut self, date: Date) -> Result<&mut Self, TransactionError> {
        if date > OffsetDateTime::now_utc().date() {
            return Err(TransactionError::FutureDate);
        }

        self.date = date;
        Ok(self)
    }

    /// Set the description for the transaction.
    pub fn description(&mut self, description: String) -> &mut Self {
        self.description = description;
        self
    }

    /// Set the category for the transaction.
    pub fn category(&mut self, category_id: Option<DatabaseID>) -> &mut Self {
        self.category_id = category_id;
        self
    }

    /// Create a new transaction in the database.
    ///
    /// Dates must be no later than today.
    ///
    /// # Errors
    /// This function will return a:
    /// - [TransactionError::InvalidCategory] if `category_id` does not refer to a valid category,
    /// - [TransactionError::InvalidUser] if `user_id` does not refer to a valid user,
    /// - [TransactionError::SqlError] if there is some other SQL error,
    /// - or [TransactionError::Unspecified] if there was an unexpected error.
    pub fn insert(&mut self, state: &AppState) -> Result<Transaction, TransactionError> {
        if let Some(category_id) = self.category_id {
            let category = state
                .category_store()
                .select(category_id)
                .map_err(|e| match e {
                    // A 'not found' error does not make sense on an insert function,
                    // so we instead indicate that the category id (a foreign key) is invalid.
                    CategoryError::NotFound => TransactionError::InvalidCategory,
                    CategoryError::SqlError(error) => TransactionError::SqlError(error),
                    e => {
                        tracing::error!("An unexpected error occurred: {e}");
                        TransactionError::Unspecified(e.to_string())
                    }
                })?;

            if self.user_id != category.user_id() {
                // The server should not give any information indicating to the client that the category exists or belongs to another user,
                // so we give the same error as if the referenced category does not exist.
                return Err(TransactionError::InvalidCategory);
            }
        }

        let connection = state.db_connection();
        let connection = connection.lock().unwrap();

        connection
                .execute(
                    "INSERT INTO \"transaction\" (amount, date, description, category_id, user_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                    (self.amount, &self.date, &self.description, self.category_id, self.user_id.as_i64()),
                ).map_err(|error| match error
                {
                    // Code 787 occurs when a FOREIGN KEY constraint failed.
                    // The client tried to add a transaction for a nonexistent user.
                    rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                        TransactionError::InvalidUser
                    }
                    error => TransactionError::SqlError(error)
                })?;

        let transaction_id = connection.last_insert_rowid();

        Ok(Transaction {
            id: transaction_id,
            amount: self.amount,
            date: self.date,
            description: self.description.to_owned(),
            category_id: self.category_id,
            user_id: self.user_id,
        })
    }
}

#[cfg(test)]
mod transaction_tests {
    use std::f64::consts::PI;

    use email_address::EmailAddress;
    use rusqlite::Connection;
    use time::{Duration, OffsetDateTime};

    use crate::db::initialize;
    use crate::models::Category;
    use crate::models::CategoryName;
    use crate::models::PasswordHash;
    use crate::models::User;
    use crate::models::UserID;
    use crate::stores::CategoryStore;
    use crate::stores::UserStore;
    use crate::AppState;

    use super::Transaction;
    use super::TransactionError;

    fn get_user_id_and_app_state() -> (User, AppState) {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        let state = AppState::new(conn, "ertsirsenrt");

        let email = "test@test.com".parse::<EmailAddress>().unwrap();
        let password_hash =
            PasswordHash::from_string("averysecretandsecurepassword".to_string()).unwrap();

        let user = state.user_store().create(email, password_hash).unwrap();

        (user, state)
    }

    #[test]
    fn new_fails_on_future_date() {
        let (user, _) = get_user_id_and_app_state();

        let tomorrow = OffsetDateTime::now_utc()
            .date()
            .checked_add(Duration::days(1))
            .unwrap();
        let mut new_transaction = Transaction::build(123.45, user.id());

        assert_eq!(
            new_transaction.date(tomorrow),
            Err(TransactionError::FutureDate)
        );
    }

    #[test]
    fn new_succeeds_on_today() {
        let (user, state) = get_user_id_and_app_state();

        let new_transaction = Transaction::build(123.45, user.id()).insert(&state);

        assert!(new_transaction.is_ok())
    }

    #[test]
    fn new_succeeds_on_past_date() {
        let (user, connection) = get_user_id_and_app_state();

        let date = OffsetDateTime::now_utc()
            .date()
            .checked_sub(Duration::days(1))
            .unwrap();

        let new_transaction = Transaction::build(123.45, user.id())
            .date(date)
            .unwrap()
            .insert(&connection);

        assert!(new_transaction.is_ok());
        assert_eq!(new_transaction.unwrap().date(), &date);
    }

    fn get_user_id_category_and_app_state() -> (User, Category, AppState) {
        let (user, state) = get_user_id_and_app_state();

        let category = state
            .category_store()
            .create(CategoryName::new_unchecked("Food"), user.id())
            .unwrap();

        (user, category, state)
    }

    #[test]
    fn insert_transaction_succeeds() {
        let (user, category, conn) = get_user_id_category_and_app_state();

        let amount = PI;
        let date = OffsetDateTime::now_utc().date();
        let description = "Rust Pie".to_string();

        let transaction = Transaction::build(amount, user.id())
            .category(Some(category.id()))
            .description(description.clone())
            .date(date)
            .unwrap()
            .insert(&conn)
            .unwrap();

        assert_eq!(transaction.amount(), amount);
        assert_eq!(*transaction.date(), date);
        assert_eq!(transaction.description(), description);
        assert_eq!(transaction.category_id(), Some(category.id()));
        assert_eq!(transaction.user_id(), user.id());
    }

    #[test]
    fn insert_transaction_fails_on_invalid_user_id() {
        let (user, _, conn) = get_user_id_category_and_app_state();

        let transaction =
            Transaction::build(PI, UserID::new(user.id().as_i64() + 42)).insert(&conn);

        assert_eq!(transaction, Err(TransactionError::InvalidUser));
    }

    #[test]
    fn insert_transaction_fails_on_invalid_category_id() {
        let (user, category, conn) = get_user_id_category_and_app_state();

        let transaction = Transaction::build(PI, user.id())
            .category(Some(category.id() + 198371))
            .insert(&conn);

        assert_eq!(transaction, Err(TransactionError::InvalidCategory));
    }

    #[test]
    fn insert_transaction_fails_on_user_id_mismatch() {
        // `_user` is the owner of `someone_elses_category`.
        let (_user, someone_elses_category, state) = get_user_id_category_and_app_state();

        let unauthorized_user = {
            state
                .user_store()
                .create(
                    "bar@baz.qux".parse().unwrap(),
                    PasswordHash::new_unchecked("hunter3".to_string()),
                )
                .unwrap()
        };

        let maybe_transaction = Transaction::build(PI, unauthorized_user.id())
            .category(Some(someone_elses_category.id()))
            .insert(&state);

        // The server should not give any information indicating to the client that the category exists or belongs to another user,
        // so we give the same error as if the referenced category does not exist.
        assert_eq!(maybe_transaction, Err(TransactionError::InvalidCategory));
    }

    #[test]
    fn select_transaction_by_id_succeeds() {
        let (user, state) = get_user_id_and_app_state();

        let transaction = Transaction::build(PI, user.id()).insert(&state).unwrap();

        let selected_transaction =
            Transaction::select(transaction.id(), &state.db_connection().lock().unwrap()).unwrap();

        assert_eq!(transaction, selected_transaction);
    }

    #[test]
    fn select_transaction_fails_on_invalid_id() {
        let (user, state) = get_user_id_and_app_state();

        let transaction = Transaction::build(PI, user.id()).insert(&state).unwrap();

        let maybe_transaction =
            Transaction::select(transaction.id() + 1, &state.db_connection().lock().unwrap());

        assert_eq!(maybe_transaction, Err(TransactionError::NotFound));
    }

    #[test]
    fn select_transactions_by_user_id_succeeds_with_no_transactions() {
        let (user, state) = get_user_id_and_app_state();

        let expected_transactions = vec![];

        let transactions =
            Transaction::select_by_user(user.id(), &state.db_connection().lock().unwrap()).unwrap();

        assert_eq!(transactions, expected_transactions);
    }

    #[test]
    fn select_transactions_by_user_id_succeeds() {
        let (user, state) = get_user_id_and_app_state();

        let expected_transactions = vec![
            Transaction::build(PI, user.id()).insert(&state).unwrap(),
            Transaction::build(PI + 1.0, user.id())
                .insert(&state)
                .unwrap(),
        ];

        let transactions =
            Transaction::select_by_user(user.id(), &state.db_connection().lock().unwrap()).unwrap();

        assert_eq!(transactions, expected_transactions);
    }
}
