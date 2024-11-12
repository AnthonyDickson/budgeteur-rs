use std::sync::{Arc, Mutex};

use rusqlite::{Connection, Row};

use crate::{
    db::{CreateTable, MapRow},
    models::{DatabaseID, Transaction, TransactionBuilder, TransactionError, UserID},
};

/// Handles the creation and retrieval of transactions.
pub trait TransactionStore {
    /// Create a new transaction in the store.
    fn create(&mut self, amount: f64, user_id: UserID) -> Result<Transaction, TransactionError>;

    /// Create a new transaction in the store.
    fn create_from_builder(
        &mut self,
        builder: TransactionBuilder,
    ) -> Result<Transaction, TransactionError>;

    /// Retrieve a transaction from the store.
    fn get(&self, id: DatabaseID) -> Result<Transaction, TransactionError>;

    /// Retrieve a user's transactions from the store.
    fn get_by_user_id(&self, user_id: UserID) -> Result<Vec<Transaction>, TransactionError>;
}

/// Stores transactions in a SQLite database.
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
    /// - [TransactionError::InvalidCategory] if `category_id` does not refer to a valid category,
    /// - [TransactionError::InvalidUser] if `user_id` does not refer to a valid user,
    /// - [TransactionError::SqlError] if there is some other SQL error,
    /// - or [TransactionError::Unspecified] if there was an unexpected error.
    fn create(&mut self, amount: f64, user_id: UserID) -> Result<Transaction, TransactionError> {
        let transaction = Transaction::build(amount, user_id);

        self.create_from_builder(transaction)
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
    fn create_from_builder(
        &mut self,
        builder: TransactionBuilder,
    ) -> Result<Transaction, TransactionError> {
        let connection = self.connection.lock().unwrap();

        let next_id: i64 = connection.query_row(
            "SELECT COALESCE(MAX(id), 0) FROM \"transaction\"",
            [],
            |row| row.get(0),
        )?;
        let next_id = next_id + 1;

        let transaction = builder.finalise(next_id);

        connection
                .execute(
                    "INSERT INTO \"transaction\" (id, amount, date, description, category_id, user_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    (transaction.id(), transaction.amount(), transaction.date(), transaction.description(), transaction.category_id(), transaction.user_id().as_i64()),
                ).map_err(|error| match error
                {
                    // Code 787 occurs when a FOREIGN KEY constraint failed.
                    // The client tried to add a transaction for a nonexistent user.
                    rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                        TransactionError::InvalidUser
                    }
                    error => TransactionError::SqlError(error)
                })?;

        Ok(transaction)
    }

    /// Retrieve a transaction in the database by its `id`.
    ///
    /// # Errors
    /// This function will return a:
    /// - [TransactionError::NotFound] if `id` does not refer to a valid transaction,
    /// - or [TransactionError::SqlError] there is some other SQL error.
    fn get(&self, id: DatabaseID) -> Result<Transaction, TransactionError> {
        let transaction = self.connection.lock().unwrap()
                .prepare("SELECT id, amount, date, description, category_id, user_id FROM \"transaction\" WHERE id = :id")?
                .query_row(&[(":id", &id)], Self::map_row)?;

        Ok(transaction)
    }

    /// Retrieve the transactions in the database that have `user_id`.
    ///
    /// An empty vector is returned if the specified user has no transactions.
    ///
    /// # Errors
    /// This function will return a [TransactionError::SqlError] if there is an SQL error.
    fn get_by_user_id(&self, user_id: UserID) -> Result<Vec<Transaction>, TransactionError> {
        self.connection.lock().unwrap()
                .prepare("SELECT id, amount, date, description, category_id, user_id FROM \"transaction\" WHERE user_id = :user_id")?
                .query_map(&[(":user_id", &user_id.as_i64())], Self::map_row)?
                .map(|maybe_category| maybe_category.map_err(TransactionError::SqlError))
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

        let transaction = Transaction::build(amount, user_id)
            // TODO: Handle error if date is invalid.
            .date(date)
            .unwrap()
            .description(description)
            .category(category_id)
            .finalise(id);

        Ok(transaction)
    }
}

#[cfg(test)]
mod sqlite_transaction_store_tests {
    use std::f64::consts::PI;

    use rusqlite::Connection;

    use crate::models::{PasswordHash, User, UserID};
    use crate::stores::sql_store::{create_app_state, SQLAppState};
    use crate::stores::UserStore;

    use super::TransactionError;
    use super::TransactionStore;

    fn get_app_state_and_test_user() -> (SQLAppState, User) {
        let conn = Connection::open_in_memory().unwrap();
        let mut state = create_app_state(conn, "stneaoetse").unwrap();

        let test_user = state
            .user_store()
            .create(
                "test@test.com".parse().unwrap(),
                PasswordHash::new_unchecked("hunter2".to_string()),
            )
            .unwrap();

        (state, test_user)
    }

    #[test]
    fn create_succeeds() {
        let (mut state, user) = get_app_state_and_test_user();
        let amount = 12.3;

        let result = state.transaction_store().create(amount, user.id());

        assert!(result.is_ok());

        let transaction = result.unwrap();

        assert_eq!(transaction.amount(), amount);
        assert_eq!(transaction.user_id(), user.id());
    }

    #[test]
    fn create_fails_on_invalid_user_id() {
        let (mut state, user) = get_app_state_and_test_user();

        let transaction = state
            .transaction_store()
            .create(PI, UserID::new(user.id().as_i64() + 42));

        assert_eq!(transaction, Err(TransactionError::InvalidUser));
    }

    // TODO: Move the below tests to a new type that coordinates stores and upholds invariants such
    // as foreign keys.
    //
    // #[test]
    // fn create_fails_on_invalid_category_id() {
    //     let (state, user) = get_app_state_and_test_user();
    //     let category = state
    //         .category_store()
    //         .create(CategoryName::new_unchecked("state"), user.id())
    //         .unwrap();
    //
    //     let transaction = state.transaction_store().create_from_builder(
    //         Transaction::build(PI, user.id()).category(Some(category.id() + 198371)),
    //     );
    //
    //     assert_eq!(transaction, Err(TransactionError::InvalidCategory));
    // }
    //
    // #[test]
    // fn create_fails_on_user_id_mismatch() {
    //     // `user` is the owner of `someone_elses_category`.
    //     let (state, user) = get_app_state_and_test_user();
    //     let someone_elses_category = state
    //         .category_store()
    //         .create(CategoryName::new_unchecked("state"), user.id())
    //         .unwrap();
    //
    //     let unauthorized_user = state
    //         .user_store()
    //         .create(
    //             "bar@baz.qux".parse().unwrap(),
    //             PasswordHash::new_unchecked("hunter3".to_string()),
    //         )
    //         .unwrap();
    //
    //     let maybe_transaction = state.transaction_store().create_from_builder(
    //         Transaction::build(PI, unauthorized_user.id())
    //             .category(Some(someone_elses_category.id())),
    //     );
    //
    //     // The server should not give any information indicating to the client that the category exists or belongs to another user,
    //     // so we give the same error as if the referenced category does not exist.
    //     assert_eq!(maybe_transaction, Err(TransactionError::InvalidCategory));
    // }

    #[test]
    fn get_transaction_by_id_succeeds() {
        let (mut state, user) = get_app_state_and_test_user();
        let store = state.transaction_store();
        let transaction = store.create(PI, user.id()).unwrap();

        let selected_transaction = store.get(transaction.id());

        assert_eq!(Ok(transaction), selected_transaction);
    }

    #[test]
    fn get_transaction_fails_on_invalid_id() {
        let (mut state, user) = get_app_state_and_test_user();
        let store = state.transaction_store();
        let transaction = store.create(123.0, user.id()).unwrap();

        let maybe_transaction = store.get(transaction.id() + 654);

        assert_eq!(maybe_transaction, Err(TransactionError::NotFound));
    }

    #[test]
    fn get_transactions_by_user_id_succeeds_with_no_transactions() {
        let (mut state, user) = get_app_state_and_test_user();
        let store = state.transaction_store();
        let expected_transactions = vec![];

        let transactions = store.get_by_user_id(user.id());

        assert_eq!(transactions, Ok(expected_transactions));
    }

    #[test]
    fn get_transactions_by_user_id_succeeds() {
        let (mut state, user) = get_app_state_and_test_user();
        let store = state.transaction_store();

        let expected_transactions = vec![
            store.create(PI, user.id()).unwrap(),
            store.create(PI + 1.0, user.id()).unwrap(),
        ];

        let transactions = store.get_by_user_id(user.id());

        assert_eq!(transactions, Ok(expected_transactions));
    }
}
