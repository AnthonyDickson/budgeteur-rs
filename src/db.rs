/*! This module defines and implements traits for interacting with the application's database. */

use std::fmt::Display;

use rusqlite::{Connection, Error, Row, Transaction as SqlTransaction};

use crate::models::{
    Category, CategoryName, DatabaseID, NewCategory, NewTransaction, Transaction, User, UserID,
};

/// Errors originating from operations on the app's database.
#[derive(thiserror::Error, Debug, PartialEq)]
pub enum DbError {
    /// The user's email already exists in the database. The client should try again with a different email address.
    DuplicateEmail,
    /// The password hash clashed with an existing password hash (should be extremely rare), the caller should rehash the password and try again.
    DuplicatePassword,
    /// A query was given an invalid foreign key. The client should check that the ids are valid.
    InvalidForeignKey,
    /// An invalid date was provided (e.g., a future date on a transaction or an end date before or on a start date for recurring transactions).
    /// The client should try again with a date no later than today.
    InvalidDate,
    /// An invalid ratio was given. The client should try again with a number between 0.0 and 1.0 (inclusive).
    InvalidRatio,
    /// The row could not be found with the provided info (e.g., id). The client should try again with different parameters.
    NotFound,
    /// Wrapper for Sqlite errors not handled by the other enum entries.
    SqlError(Error),
}

impl Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SqlError(inner_error) => write!(f, "{:?}: {}", self, inner_error),
            other => write!(f, "{:?}", other),
        }
    }
}

impl From<Error> for DbError {
    fn from(error: Error) -> Self {
        match error {
            // Code 787 occurs when a FOREIGN KEY constraint failed.
            Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                DbError::InvalidForeignKey
            }
            // Code 2067 occurs when a UNIQUE constraint failed.
            Error::SqliteFailure(sql_error, Some(ref desc))
                if sql_error.extended_code == 2067 && desc.contains("email") =>
            {
                DbError::DuplicateEmail
            }
            Error::SqliteFailure(sql_error, Some(ref desc))
                if sql_error.extended_code == 2067 && desc.contains("password") =>
            {
                DbError::DuplicatePassword
            }
            Error::QueryReturnedNoRows => DbError::NotFound,
            e => DbError::SqlError(e),
        }
    }
}

/// A trait for adding an object schema to a database.
pub trait CreateTable {
    /// Create a table for the model.
    ///
    /// # Errors
    /// Returns an error if the table already exists or if there is an SQL error.
    fn create_table(connection: &Connection) -> Result<(), Error>;
}

/// A trait for mapping from a `rusqlite::Row` from a SQLite database to a concrete rust type.
///
/// # Examples
/// ```
/// use rusqlite::{Connection, Error, Row};
///
/// use budgeteur_rs::db::{DbError, CreateTable, MapRow};
///
/// struct Foo {
///     id: i64,
///     desc: String
/// }
///
/// impl CreateTable for Foo {
///    fn create_table(connection: &Connection) -> Result<(), Error> {
///        connection.execute(
///            "CREATE TABLE foo (id INTEGER PRIMARY KEY, desc TEXT NOT NULL)",
///            (),
///        )?;
///
///        Ok(())
///    }
/// }
///
/// impl MapRow for Foo {
///     type ReturnType = Self;
///
///     fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, Error> {
///         Ok(Self {
///             id: row.get(offset)?,
///             desc: row.get(offset + 1)?,
///         })
///     }
/// }
///
/// struct Bar {
///     id: i64,
///     desc: String
/// }
///
/// impl CreateTable for Bar {
///    fn create_table(connection: &Connection) -> Result<(), Error> {
///        connection.execute(
///            "CREATE TABLE bar (id INTEGER PRIMARY KEY, desc TEXT NOT NULL)",
///            (),
///        )?;
///
///        Ok(())
///    }
/// }
///
/// impl MapRow for Bar {
///     type ReturnType = Self;
///
///     fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, Error> {
///         Ok(Self {
///             id: row.get(offset)?,
///             desc: row.get(offset + 1)?,
///         })
///     }
/// }
///
/// fn example(conn: &Connection) -> Result<(Foo, Bar), DbError> {
///     conn.
///         prepare("SELECT l.id, l.desc, r.id, r.desc FROM foo l INNER JOIN bar r ON l.id = r.foo_id WHERE l.id = :id")?
///         .query_row(&[(":id", &1)], |row| {
///             let foo = Foo::map_row(row)?;
///             let bar = Bar::map_row_with_offset(row, 2)?;
///
///             Ok((foo, bar))
///         })
///         .map_err(|e| e.into())
/// }
/// ```
pub trait MapRow {
    type ReturnType;

    /// Convert a row into a concrete type.
    ///
    /// **Note:** This function expects that the row object contains all the table columns in the order they were defined.
    ///
    /// # Errors
    /// Returns an error if a row item cannot be converted into the corresponding rust type, or if an invalid column index was used.
    fn map_row(row: &Row) -> Result<Self::ReturnType, Error> {
        Self::map_row_with_offset(row, 0)
    }

    /// Convert a row into a concrete type.
    ///
    /// The `offset` indicates which column the row should be read from.
    /// This is useful in cases where tables have been joined and you want to construct two different types from the one query.
    ///
    /// **Note:** This function expects that the row object contains all the table columns in the order they were defined.
    ///
    /// # Errors
    /// Returns an error if a row item cannot be converted into the corresponding rust type, or if an invalid column index was used.
    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self::ReturnType, Error>;
}

/// A trait for inserting a record into the application database.
pub trait Insert {
    type ResultType;

    /// Insert the object into the application database.
    ///
    /// # Errors
    ///
    /// This function will return an error if the insertion failed.
    fn insert(self, connection: &Connection) -> Result<Self::ResultType, DbError>;
}

/// A trait for retrieving records from the application database by a field of type `T`.
pub trait SelectBy<T> {
    type ResultType;

    /// Select records from the application database that match `field`.
    fn select(field: T, connection: &Connection) -> Result<Self::ResultType, DbError>;
}

impl CreateTable for Category {
    fn create_table(connection: &Connection) -> Result<(), Error> {
        connection.execute(
            "CREATE TABLE category (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                user_id INTEGER NOT NULL,
                FOREIGN KEY(user_id) REFERENCES user(id) ON UPDATE CASCADE ON DELETE CASCADE
                )",
            (),
        )?;

        Ok(())
    }
}

impl MapRow for Category {
    type ReturnType = Self;

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, Error> {
        let id = row.get(offset)?;

        let raw_name: String = row.get(offset + 1)?;
        let name = CategoryName::new_unchecked(raw_name);

        let raw_user_id = row.get(offset + 2)?;
        let user_id = UserID::new(raw_user_id);

        Ok(Self::new(id, name, user_id))
    }
}

impl Insert for NewCategory {
    type ResultType = Category;

    /// Create a new category in the database.
    ///
    /// # Examples
    /// ```
    /// # use rusqlite::Connection;
    /// #
    /// # use budgeteur_rs::{db::{DbError, Insert}, models::{Category, CategoryName, NewCategory, User}};
    /// #
    /// fn create_category(name: String, user: &User, connection: &Connection) -> Result<Category, DbError> {
    ///     let name = CategoryName::new(name).unwrap();
    ///     let category = NewCategory { name: name.clone(), user_id: user.id() }.insert(&connection)?;
    ///
    ///     assert_eq!(category.name(), &name);
    ///     assert_eq!(category.user_id(), user.id());
    ///
    ///     Ok(category)
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if:
    /// - `user_id` does not refer to a valid user,
    /// - or there is some other SQL error.
    fn insert(self, connection: &Connection) -> Result<Self::ResultType, DbError> {
        connection.execute(
            "INSERT INTO category (name, user_id) VALUES (?1, ?2)",
            (self.name.as_ref(), self.user_id.as_i64()),
        )?;

        let category_id = connection.last_insert_rowid();

        Ok(Self::ResultType::new(category_id, self.name, self.user_id))
    }
}

impl SelectBy<DatabaseID> for Category {
    type ResultType = Self;

    /// Retrieve a category in the database by its `id`.
    ///
    /// # Examples
    /// ```
    /// # use rusqlite::Connection;
    /// #
    /// # use budgeteur_rs::{db::SelectBy, models::{Category, DatabaseID}};
    /// #
    /// fn get_category(id: DatabaseID, connection: &Connection) -> Option<Category> {
    ///     Category::select(id, &connection).ok()
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if:
    /// - `id` does not refer to a valid category,
    /// - or there is some other SQL error.
    fn select(id: DatabaseID, connection: &Connection) -> Result<Self::ResultType, DbError> {
        let category = connection
            .prepare("SELECT id, name, user_id FROM category WHERE id = :id")?
            .query_row(&[(":id", &id)], Category::map_row)?;

        Ok(category)
    }
}

impl SelectBy<UserID> for Category {
    type ResultType = Vec<Self>;

    /// Retrieve categories in the database for the user `user_id`.
    ///
    /// # Examples
    /// ```
    /// use rusqlite::Connection;
    ///
    /// use budgeteur_rs::{db::{Insert, SelectBy}, models::{Category, CategoryName, NewCategory, User}};
    ///
    /// fn create_and_validate_categories(user: &User, connection: &Connection) -> Vec<Category> {
    ///     let inserted_categories = vec![
    ///         NewCategory {
    ///             name: CategoryName::new_unchecked("Foo".to_string()),
    ///             user_id: user.id(),
    ///         }.insert(&connection)
    ///         .unwrap(),
    ///         NewCategory {
    ///             name: CategoryName::new_unchecked("Bar".to_string()),
    ///             user_id: user.id(),
    ///         }.insert(&connection)
    ///         .unwrap(),
    ///     ];
    ///
    ///     let selected_categories = Category::select(user.id(), &connection).unwrap();
    ///
    ///     assert_eq!(inserted_categories, selected_categories);
    ///
    ///     selected_categories
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if there is an SQL error.
    fn select(user_id: UserID, connection: &Connection) -> Result<Self::ResultType, DbError> {
        connection
            .prepare("SELECT id, name, user_id FROM category WHERE user_id = :user_id")?
            .query_map(&[(":user_id", &user_id.as_i64())], Category::map_row)?
            .map(|maybe_category| maybe_category.map_err(DbError::SqlError))
            .collect()
    }
}

impl CreateTable for Transaction {
    fn create_table(connection: &Connection) -> Result<(), Error> {
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

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, Error> {
        Ok(Self::new_unchecked(
            row.get(offset)?,
            row.get(offset + 1)?,
            row.get(offset + 2)?,
            row.get(offset + 3)?,
            row.get(offset + 4)?,
            UserID::new(row.get(offset + 5)?),
        ))
    }
}

impl Insert for NewTransaction {
    type ResultType = Transaction;

    /// Create a new transaction in the database.
    ///
    /// Dates must be no later than today.
    ///
    /// # Examples
    /// ```
    /// # use rusqlite::Connection;
    /// # use time::{Date, Month, OffsetDateTime, Time};
    /// #
    /// # use budgeteur_rs::{db::Insert, models::{Category, NewTransaction, Transaction, User}};
    /// #
    /// fn create_transaction(user: &User, category: &Category, connection: &Connection) {
    ///     let transaction = NewTransaction::new(
    ///         3.14,
    ///         OffsetDateTime::new_utc(
    ///             Date::from_calendar_date(2024, Month::August, 7).unwrap(),
    ///             Time::from_hms(12, 0, 0).unwrap(),
    ///         ),
    ///         "Rust Pie".to_string(),
    ///         category.id(),
    ///         user.id()
    ///     )
    ///     .unwrap()
    ///     .insert(&connection)
    ///     .unwrap();
    ///
    ///     assert_eq!(transaction.amount(), 3.14);
    ///     assert_eq!(
    ///         *transaction.date(),
    ///         OffsetDateTime::new_utc(
    ///             Date::from_calendar_date(2024, Month::August, 7).unwrap(),
    ///             Time::from_hms(12, 0, 0).unwrap(),
    ///         )
    ///     );
    ///     assert_eq!(transaction.description(), "Rust Pie");
    ///     assert_eq!(transaction.category_id(), category.id());
    ///     assert_eq!(transaction.user_id(), user.id());
    /// }
    ///
    /// ```
    ///
    /// # Errors
    /// This function will return an error if:
    /// - `category_id` does not refer to a valid category,
    /// - `user_id` does not refer to a valid user,
    /// - or there is some other SQL error.
    fn insert(self, connection: &Connection) -> Result<Self::ResultType, DbError> {
        let category = Category::select(self.category_id(), connection).map_err(|e| match e {
            // A 'not found' error does not make sense on an insert function,
            // so we instead indicate that the category id (a foreign key) is invalid.
            DbError::NotFound => DbError::InvalidForeignKey,
            e => e,
        })?;

        if self.user_id() != category.user_id() {
            return Err(DbError::InvalidForeignKey);
        }

        connection
                .execute(
                    "INSERT INTO \"transaction\" (amount, date, description, category_id, user_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                    (self.amount(), &self.date(), &self.description(), self.category_id(), self.user_id().as_i64()),
                )?;

        let transaction_id = connection.last_insert_rowid();

        // The type `NewTransaction` will validate the date, so we can skip validation here by calling the `new_unchecked` function.
        Ok(Self::ResultType::new_unchecked(
            transaction_id,
            self.amount(),
            self.date(),
            self.description().to_owned(),
            self.category_id(),
            self.user_id(),
        ))
    }
}

impl SelectBy<DatabaseID> for Transaction {
    type ResultType = Self;

    /// Retrieve a transaction in the database by its `id`.
    ///
    /// # Examples
    /// ```
    /// # use rusqlite::Connection;
    /// #
    /// # use budgeteur_rs::{db::{DbError, SelectBy}, models::{DatabaseID, Transaction}};
    /// #
    /// fn get_transaction(id: DatabaseID, connection: &Connection) -> Result<Transaction, DbError> {
    ///     Transaction::select(id, &connection)
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if:
    /// - `id` does not refer to a valid transaction,
    /// - or there is some other SQL error.
    fn select(id: DatabaseID, connection: &Connection) -> Result<Self::ResultType, DbError> {
        let transaction = connection
                .prepare("SELECT id, amount, date, description, category_id, user_id FROM \"transaction\" WHERE id = :id")?
                .query_row(&[(":id", &id)], Transaction::map_row)?;

        Ok(transaction)
    }
}

impl SelectBy<UserID> for Transaction {
    type ResultType = Vec<Self>;

    /// Retrieve the transactions in the database that have `user_id`.
    ///
    /// # Examples
    /// ```
    /// use budgeteur_rs::{db::SelectBy, models::{Transaction, User}};
    ///
    /// fn sum_transaction_amount_for_user(user: &User, conn: &rusqlite::Connection) -> f64 {
    ///     let transactions = Transaction::select(user.id(), conn).unwrap();
    ///     transactions.iter().map(|transaction| transaction.amount()).sum()
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if there is an SQL error.
    fn select(user_id: UserID, connection: &Connection) -> Result<Self::ResultType, DbError> {
        connection
                .prepare("SELECT id, amount, date, description, category_id, user_id FROM \"transaction\" WHERE user_id = :user_id")?
                .query_map(&[(":user_id", &user_id.as_i64())], Transaction::map_row)?
                .map(|maybe_category| maybe_category.map_err(DbError::SqlError))
                .collect()
    }
}

pub fn initialize(connection: &Connection) -> Result<(), DbError> {
    let transaction =
        SqlTransaction::new_unchecked(connection, rusqlite::TransactionBehavior::Exclusive)?;

    User::create_table(&transaction)?;
    Category::create_table(&transaction)?;
    Transaction::create_table(&transaction)?;

    transaction.commit()?;

    Ok(())
}

#[cfg(test)]
mod category_tests {
    use std::str::FromStr;

    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::{
        db::{initialize, Category, CategoryName, DbError, SelectBy, UserID},
        models::{NewCategory, PasswordHash, User},
    };

    use super::Insert;

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    fn create_database_and_insert_test_user() -> (Connection, User) {
        let conn = init_db();

        let test_user = User::build(
            EmailAddress::from_str("foo@bar.baz").unwrap(),
            PasswordHash::new_unchecked("hunter2".to_string()),
        )
        .insert(&conn)
        .unwrap();

        (conn, test_user)
    }

    #[test]
    fn insert_category_succeeds() {
        let (conn, test_user) = create_database_and_insert_test_user();

        let name = CategoryName::new("Categorically a category".to_string()).unwrap();

        let category = NewCategory {
            name: name.clone(),
            user_id: test_user.id(),
        }
        .insert(&conn)
        .unwrap();

        assert!(category.id() > 0);
        assert_eq!(category.name(), &name);
        assert_eq!(category.user_id(), test_user.id());
    }

    #[test]
    fn insert_category_fails_with_invalid_user_id() {
        let conn = init_db();
        let name = CategoryName::new_unchecked("Foo".to_string());

        let maybe_category = NewCategory {
            name,
            user_id: UserID::new(42),
        }
        .insert(&conn);

        assert_eq!(maybe_category, Err(DbError::InvalidForeignKey));
    }

    #[test]
    fn select_category_succeeds() {
        let (conn, test_user) = create_database_and_insert_test_user();
        let name = CategoryName::new_unchecked("Foo".to_string());
        let inserted_category = NewCategory {
            name,
            user_id: test_user.id(),
        }
        .insert(&conn)
        .unwrap();

        let selected_category = Category::select(inserted_category.id(), &conn).unwrap();

        assert_eq!(inserted_category, selected_category);
    }

    #[test]
    fn select_category_fails_with_invalid_id() {
        let conn = init_db();

        let selected_category = Category::select(1337, &conn);

        assert_eq!(selected_category, Err(DbError::NotFound));
    }

    #[test]
    fn select_category_with_user_id() {
        let (conn, test_user) = create_database_and_insert_test_user();
        let inserted_categories = vec![
            NewCategory {
                name: CategoryName::new_unchecked("Foo".to_string()),
                user_id: test_user.id(),
            }
            .insert(&conn)
            .unwrap(),
            NewCategory {
                name: CategoryName::new_unchecked("Bar".to_string()),
                user_id: test_user.id(),
            }
            .insert(&conn)
            .unwrap(),
        ];

        let selected_categories = Category::select(test_user.id(), &conn).unwrap();

        assert_eq!(inserted_categories, selected_categories);
    }

    #[test]
    fn select_category_with_invalid_user_id() {
        let (conn, test_user) = create_database_and_insert_test_user();

        NewCategory {
            name: CategoryName::new_unchecked("Foo".to_string()),
            user_id: test_user.id(),
        }
        .insert(&conn)
        .unwrap();

        NewCategory {
            name: CategoryName::new_unchecked("Bar".to_string()),
            user_id: test_user.id(),
        }
        .insert(&conn)
        .unwrap();

        let selected_categories =
            Category::select(UserID::new(test_user.id().as_i64() + 1), &conn).unwrap();

        assert_eq!(selected_categories, []);
    }
}

#[cfg(test)]
mod transaction_tests {
    use std::{f64::consts::PI, str::FromStr};

    use email_address::EmailAddress;
    use rusqlite::Connection;
    use time::{Date, Month, OffsetDateTime, Time};

    use crate::{
        db::{initialize, Category, DbError, SelectBy, Transaction},
        models::{CategoryName, NewCategory, NewTransaction, PasswordHash, User, UserID},
    };

    use super::Insert;

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    fn create_database_and_insert_test_user_and_category() -> (Connection, User, Category) {
        let conn = init_db();

        let test_user = User::build(
            EmailAddress::from_str("foo@bar.baz").unwrap(),
            PasswordHash::new_unchecked("hunter2".to_string()),
        )
        .insert(&conn)
        .unwrap();

        let category = NewCategory {
            name: CategoryName::new("Food".to_string()).unwrap(),
            user_id: test_user.id(),
        }
        .insert(&conn)
        .unwrap();

        (conn, test_user, category)
    }

    #[test]
    fn insert_transaction_succeeds() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let amount = PI;
        let date = OffsetDateTime::now_utc();
        let description = "Rust Pie".to_string();

        let transaction = NewTransaction::new(
            amount,
            date,
            description.clone(),
            category.id(),
            test_user.id(),
        )
        .unwrap()
        .insert(&conn)
        .unwrap();

        assert_eq!(transaction.amount(), amount);
        assert_eq!(*transaction.date(), date);
        assert_eq!(transaction.description(), description);
        assert_eq!(transaction.category_id(), category.id());
        assert_eq!(transaction.user_id(), test_user.id());
    }

    #[test]
    fn insert_transaction_fails_on_invalid_user_id() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let amount = PI;
        let date = OffsetDateTime::now_utc();
        let description = "Rust Pie".to_string();

        let maybe_transaction = NewTransaction::new(
            amount,
            date,
            description.clone(),
            category.id(),
            UserID::new(test_user.id().as_i64() + 1),
        )
        .unwrap()
        .insert(&conn);

        assert_eq!(maybe_transaction, Err(DbError::InvalidForeignKey));
    }

    #[test]
    fn insert_transaction_fails_on_invalid_category_id() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let amount = PI;
        let date = OffsetDateTime::now_utc();
        let description = "Rust Pie".to_string();

        let maybe_transaction = NewTransaction::new(
            amount,
            date,
            description.clone(),
            category.id() + 1,
            test_user.id(),
        )
        .unwrap()
        .insert(&conn);

        assert_eq!(maybe_transaction, Err(DbError::InvalidForeignKey));
    }

    #[test]
    fn insert_transaction_fails_on_user_id_mismatch() {
        // `_test_user` is the owner of `someone_elses_category`.
        let (conn, _test_user, someone_elses_category) =
            create_database_and_insert_test_user_and_category();

        let unauthorized_user = User::build(
            EmailAddress::from_str("bar@baz.qux").unwrap(),
            PasswordHash::new_unchecked("hunter3".to_string()),
        )
        .insert(&conn)
        .unwrap();

        let amount = PI;
        let date = OffsetDateTime::now_utc();
        let description = "Rust Pie".to_string();

        let maybe_transaction = NewTransaction::new(
            amount,
            date,
            description.clone(),
            someone_elses_category.id(),
            // The user below should not be allowed to use the above category because it belongs to someone else!
            unauthorized_user.id(),
        )
        .unwrap()
        .insert(&conn);

        // The server should not give any information indicating to the client that the category exists or belongs to another user,
        // so we give the same error as if the referenced category does not exist.
        assert_eq!(maybe_transaction, Err(DbError::InvalidForeignKey));
    }

    #[test]
    fn select_transaction_by_id_succeeds() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let inserted_transaction = NewTransaction::new(
            PI,
            OffsetDateTime::new_utc(
                Date::from_calendar_date(2024, Month::August, 7).unwrap(),
                Time::from_hms(12, 0, 0).unwrap(),
            ),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
        )
        .unwrap()
        .insert(&conn)
        .unwrap();

        let selected_transaction = Transaction::select(inserted_transaction.id(), &conn).unwrap();

        assert_eq!(inserted_transaction, selected_transaction);
    }

    #[test]
    fn select_transaction_fails_on_invalid_id() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let inserted_transaction = NewTransaction::new(
            PI,
            OffsetDateTime::new_utc(
                Date::from_calendar_date(2024, Month::August, 7).unwrap(),
                Time::from_hms(12, 0, 0).unwrap(),
            ),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
        )
        .unwrap()
        .insert(&conn)
        .unwrap();

        let maybe_transaction = Transaction::select(inserted_transaction.id() + 1, &conn);

        assert_eq!(maybe_transaction, Err(DbError::NotFound));
    }

    #[test]
    fn select_transactions_by_user_id_succeeds_with_no_transactions() {
        let (conn, test_user, _category) = create_database_and_insert_test_user_and_category();

        let expected_transactions = vec![];

        let transactions = Transaction::select(test_user.id(), &conn).unwrap();

        assert_eq!(transactions, expected_transactions);
    }

    #[test]
    fn select_transactions_by_user_id_succeeds() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let expected_transactions = vec![
            NewTransaction::new(
                PI,
                OffsetDateTime::new_utc(
                    Date::from_calendar_date(2024, Month::August, 7).unwrap(),
                    Time::from_hms(12, 0, 0).unwrap(),
                ),
                "Rust Pie".to_string(),
                category.id(),
                test_user.id(),
            )
            .unwrap()
            .insert(&conn)
            .unwrap(),
            NewTransaction::new(
                PI + 1.0,
                OffsetDateTime::new_utc(
                    Date::from_calendar_date(2024, Month::August, 7).unwrap(),
                    Time::from_hms(12, 0, 0).unwrap(),
                ),
                "Rust Pif".to_string(),
                category.id(),
                test_user.id(),
            )
            .unwrap()
            .insert(&conn)
            .unwrap(),
        ];

        let transactions = Transaction::select(test_user.id(), &conn).unwrap();

        assert_eq!(transactions, expected_transactions);
    }
}
