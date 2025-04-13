//! Defines the transaction store trait and an implementation for the SQLite backend.

use std::{
    ops::RangeInclusive,
    sync::{Arc, Mutex},
};

use rusqlite::{Connection, Row, params_from_iter, types::Value};
use time::Date;

use crate::{
    Error,
    db::{CreateTable, MapRow},
    models::{DatabaseID, Transaction, TransactionBuilder, UserID},
};

use super::SQLiteCategoryStore;

/// Handles the creation and retrieval of transactions.
pub trait TransactionStore {
    /// Create a new transaction in the store.
    fn create(&mut self, amount: f64, user_id: UserID) -> Result<Transaction, Error>;

    /// Create a new transaction in the store.
    fn create_from_builder(&mut self, builder: TransactionBuilder) -> Result<Transaction, Error>;

    /// Retrieve a transaction from the store.
    fn get(&self, id: DatabaseID) -> Result<Transaction, Error>;

    /// Retrieve a user's transactions from the store.
    fn get_by_user_id(&self, user_id: UserID) -> Result<Vec<Transaction>, Error>;

    /// Retrieve transactions from the store in the way defined by `query`.
    fn get_query(&self, query: TransactionQuery) -> Result<Vec<Transaction>, Error>;
}

/// Defines how transactions should be fetched from [TransactionStore::get_query].
#[derive(Default)]
pub struct TransactionQuery {
    /// Matches transactions belonging to the user with the ID `user_id`.
    pub user_id: Option<UserID>,
    /// Include transactions within `date_range` (inclusive).
    pub date_range: Option<RangeInclusive<Date>>,
    /// Selects up to the first N (`limit`) transactions.
    pub limit: Option<u64>,
    /// Orders transactions by date in the order `sort_date`. None returns transactions in the
    /// order they are stored.
    pub sort_date: Option<SortOrder>,
}

/// The order to sort transactions in a [TransactionQuery].
pub enum SortOrder {
    /// Sort in order of increasing value.
    Ascending,
    /// Sort in order of decreasing value.
    Descending,
}

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
    /// - [Error::InvalidUser] if `user_id` does not refer to a valid user,
    /// - [Error::SqlError] if there is some other SQL error,
    /// - or [Error::Unspecified] if there was an unexpected error.
    fn create(&mut self, amount: f64, user_id: UserID) -> Result<Transaction, Error> {
        let transaction = Transaction::build(amount, user_id);

        self.create_from_builder(transaction)
    }

    /// Create a new transaction in the database.
    ///
    /// Dates must be no later than today.
    ///
    /// # Errors
    /// This function will return a:
    /// - [Error::InvalidCategory] if `category_id` does not refer to a valid category,
    /// - [Error::InvalidUser] if `user_id` does not refer to a valid user,
    /// - [Error::SqlError] if there is some other SQL error,
    /// - or [Error::Unspecified] if there was an unexpected error.
    fn create_from_builder(&mut self, builder: TransactionBuilder) -> Result<Transaction, Error> {
        let connection = self.connection.lock().unwrap();

        let next_id: i64 = connection.query_row(
            "SELECT COALESCE(MAX(id), 0) FROM \"transaction\"",
            [],
            |row| row.get(0),
        )?;
        let next_id = next_id + 1;

        let transaction = builder.finalise(next_id);

        if let Some(category_id) = transaction.category_id() {
            let category = connection
                .query_row(
                    "SELECT id, name, user_id FROM category WHERE id = ?1",
                    (category_id,),
                    SQLiteCategoryStore::map_row,
                )
                .map_err(|error| match error {
                    // We enforce the foreign key constraint (the ID refers to a valid, existing
                    // record) here so that we know later that if a foreign key constraint is
                    // violated, it is for the user ID. Otherwise, it would difficult to know
                    // which foreign key constraint was violated since the SQL error does not
                    // provide any useful information.
                    rusqlite::Error::QueryReturnedNoRows => Error::InvalidCategory,
                    error => Error::SqlError(error),
                })?;

            if category.user_id != transaction.user_id() {
                // Use same error as if the category doesn't exist so that unauthorized users can't
                // poke around to find out what data exists.
                return Err(Error::InvalidCategory);
            }
        }

        connection
                .execute(
                    "INSERT INTO \"transaction\" (id, amount, date, description, category_id, user_id, import_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    (transaction.id(), transaction.amount(), transaction.date(), transaction.description(), transaction.category_id(), transaction.user_id().as_i64(), transaction.import_id()),
                ).map_err(|error| match error
                {
                    // Code 787 occurs when a FOREIGN KEY constraint failed.
                    // The client tried to add a transaction for a nonexistent user.
                    rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                        Error::InvalidUser
                    }
                    error => Error::SqlError(error)
                })?;

        Ok(transaction)
    }

    /// Retrieve a transaction in the database by its `id`.
    ///
    /// # Errors
    /// This function will return a:
    /// - [Error::NotFound] if `id` does not refer to a valid transaction,
    /// - or [Error::SqlError] there is some other SQL error.
    fn get(&self, id: DatabaseID) -> Result<Transaction, Error> {
        let transaction = self.connection.lock().unwrap()
                .prepare("SELECT id, amount, date, description, category_id, user_id, import_id FROM \"transaction\" WHERE id = :id")?
                .query_row(&[(":id", &id)], Self::map_row)?;

        Ok(transaction)
    }

    /// Retrieve the transactions in the database that have `user_id`.
    ///
    /// An empty vector is returned if the specified user has no transactions.
    ///
    /// # Errors
    /// This function will return a [Error::SqlError] if there is an SQL error.
    fn get_by_user_id(&self, user_id: UserID) -> Result<Vec<Transaction>, Error> {
        self.connection.lock().unwrap()
                .prepare("SELECT id, amount, date, description, category_id, user_id, import_id FROM \"transaction\" WHERE user_id = :user_id")?
                .query_map(&[(":user_id", &user_id.as_i64())], Self::map_row)?
                .map(|maybe_category| maybe_category.map_err(Error::SqlError))
                .collect()
    }

    fn get_query(&self, filter: TransactionQuery) -> Result<Vec<Transaction>, Error> {
        let mut query_string_parts = vec![
            "SELECT id, amount, date, description, category_id, user_id, import_id FROM \"transaction\""
                .to_string(),
        ];
        let mut where_clause_parts = vec![];
        let mut query_parameters = vec![];

        if let Some(user_id) = filter.user_id {
            where_clause_parts.push(format!("user_id = ?{}", query_parameters.len() + 1));
            query_parameters.push(Value::Integer(user_id.as_i64()));
        }

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
            query_string_parts.push(format!("LIMIT {}", limit));
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
}

impl CreateTable for SQLiteTransactionStore {
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
                            import_id INTEGER,
                            FOREIGN KEY(category_id) REFERENCES category(id) ON UPDATE CASCADE ON DELETE CASCADE,
                            FOREIGN KEY(user_id) REFERENCES user(id) ON UPDATE CASCADE ON DELETE CASCADE
                            )",
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
        let user_id = UserID::new(row.get(offset + 5)?);
        let import_id = row.get(offset + 6)?;

        let transaction = Transaction::new_unchecked(
            id,
            amount,
            date,
            description,
            category_id,
            user_id,
            import_id,
        );

        Ok(transaction)
    }
}

#[cfg(test)]
mod sqlite_transaction_store_tests {
    use std::f64::consts::PI;

    use rusqlite::Connection;
    use time::{Duration, OffsetDateTime};

    use crate::{
        models::{CategoryName, PasswordHash, Transaction, TransactionBuilder, User, UserID},
        stores::{
            CategoryStore, UserStore,
            sql_store::{SQLAppState, create_app_state},
            transaction::{SortOrder, TransactionQuery},
        },
    };

    use super::{Error, TransactionStore};

    fn get_app_state_and_test_user() -> (SQLAppState, User) {
        let conn = Connection::open_in_memory().unwrap();
        let mut state = create_app_state(conn, "stneaoetse").unwrap();

        let test_user = state
            .user_store
            .create(
                "test@test.com".parse().unwrap(),
                PasswordHash::new_unchecked("hunter2"),
            )
            .unwrap();

        (state, test_user)
    }

    #[test]
    fn create_succeeds() {
        let (mut state, user) = get_app_state_and_test_user();
        let amount = 12.3;

        let result = state.transaction_store.create(amount, user.id());

        assert!(result.is_ok());

        let transaction = result.unwrap();

        assert_eq!(transaction.amount(), amount);
        assert_eq!(transaction.user_id(), user.id());
    }

    #[test]
    fn create_fails_on_invalid_user_id() {
        let (mut state, user) = get_app_state_and_test_user();

        let transaction = state
            .transaction_store
            .create(PI, UserID::new(user.id().as_i64() + 42));

        assert_eq!(transaction, Err(Error::InvalidUser));
    }

    #[test]
    fn create_fails_on_invalid_category_id() {
        let (mut state, user) = get_app_state_and_test_user();

        let transaction = state
            .transaction_store
            .create_from_builder(Transaction::build(PI, user.id()).category(Some(999)));

        assert_eq!(transaction, Err(Error::InvalidCategory));
    }

    #[test]
    fn create_fails_on_user_id_mismatch() {
        // `user` is the owner of `someone_elses_category`.
        let (mut state, user) = get_app_state_and_test_user();
        let someone_elses_category = state
            .category_store
            .create(CategoryName::new_unchecked("hands off"), user.id())
            .unwrap();

        let unauthorized_user = state
            .user_store
            .create(
                "bar@baz.qux".parse().unwrap(),
                PasswordHash::new_unchecked("hunter3"),
            )
            .unwrap();

        let maybe_transaction = state.transaction_store.create_from_builder(
            Transaction::build(PI, unauthorized_user.id())
                .category(Some(someone_elses_category.id)),
        );

        // The server should not give any information indicating to the client that the category exists or belongs to another user,
        // so we give the same error as if the referenced category does not exist.
        assert_eq!(maybe_transaction, Err(Error::InvalidCategory));
    }

    #[test]
    fn get_transaction_by_id_succeeds() {
        let (state, user) = get_app_state_and_test_user();
        let mut store = state.transaction_store;
        let transaction = store.create(PI, user.id()).unwrap();

        let selected_transaction = store.get(transaction.id());

        assert_eq!(Ok(transaction), selected_transaction);
    }

    #[test]
    fn get_transaction_fails_on_invalid_id() {
        let (state, user) = get_app_state_and_test_user();
        let mut store = state.transaction_store;
        let transaction = store.create(123.0, user.id()).unwrap();

        let maybe_transaction = store.get(transaction.id() + 654);

        assert_eq!(maybe_transaction, Err(Error::NotFound));
    }

    #[test]
    fn get_transactions_by_user_id_succeeds_with_no_transactions() {
        let (state, user) = get_app_state_and_test_user();
        let store = state.transaction_store;
        let expected_transactions = vec![];

        let transactions = store.get_by_user_id(user.id());

        assert_eq!(transactions, Ok(expected_transactions));
    }

    #[test]
    fn get_transactions_by_user_id_succeeds() {
        let (state, user) = get_app_state_and_test_user();
        let mut store = state.transaction_store;

        let expected_transactions = vec![
            store.create(PI, user.id()).unwrap(),
            store.create(PI + 1.0, user.id()).unwrap(),
        ];

        let transactions = store.get_by_user_id(user.id());

        assert_eq!(transactions, Ok(expected_transactions));
    }

    #[test]
    fn get_transactions_by_date_range() {
        let (mut state, user) = get_app_state_and_test_user();

        let other_user = state
            .user_store
            .create(
                "other@example.com".parse().unwrap(),
                PasswordHash::from_raw_password("averysecretpassword", 4).unwrap(),
            )
            .unwrap();

        let mut store = state.transaction_store;

        let end_date = OffsetDateTime::now_utc()
            .date()
            .checked_sub(Duration::weeks(1))
            .unwrap();
        let start_date = end_date.checked_sub(Duration::weeks(1)).unwrap();

        let want = [
            store
                .create_from_builder(
                    TransactionBuilder::new(12.3, user.id())
                        .date(start_date)
                        .unwrap(),
                )
                .unwrap(),
            store
                .create_from_builder(
                    TransactionBuilder::new(23.4, user.id())
                        .date(start_date.checked_add(Duration::days(3)).unwrap())
                        .unwrap(),
                )
                .unwrap(),
            store
                .create_from_builder(
                    TransactionBuilder::new(34.5, user.id())
                        .date(end_date)
                        .unwrap(),
                )
                .unwrap(),
        ];

        // The below transactions should NOT be returned by the query.
        let cases = [
            (
                user.id(),
                start_date.checked_sub(Duration::days(1)).unwrap(),
            ),
            (user.id(), end_date.checked_add(Duration::days(1)).unwrap()),
            (
                other_user.id(),
                start_date.checked_sub(Duration::days(1)).unwrap(),
            ),
            (other_user.id(), start_date),
            (
                other_user.id(),
                start_date.checked_add(Duration::days(3)).unwrap(),
            ),
            (other_user.id(), end_date),
            (
                other_user.id(),
                end_date.checked_add(Duration::days(1)).unwrap(),
            ),
        ];

        for (user_id, date) in cases {
            store
                .create_from_builder(TransactionBuilder::new(999.99, user_id).date(date).unwrap())
                .unwrap();
        }

        let got = store
            .get_query(TransactionQuery {
                user_id: Some(user.id()),
                date_range: Some(start_date..=end_date),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(got, want, "got transactions {:?}, want {:?}", got, want);
    }

    #[test]
    fn get_transactions_with_limit() {
        let (mut state, user) = get_app_state_and_test_user();

        let today = OffsetDateTime::now_utc().date();

        for i in 1..=10 {
            let transaction_builder = TransactionBuilder::new(i as f64, user.id())
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
    fn get_transactions_descending_date() {
        let (mut state, user) = get_app_state_and_test_user();

        let mut want = vec![];
        let start_date = OffsetDateTime::now_utc()
            .date()
            .checked_sub(Duration::weeks(2))
            .unwrap();

        for i in 1..=3 {
            let transaction_builder = TransactionBuilder::new(i as f64, user.id())
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
}
