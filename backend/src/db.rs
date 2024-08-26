use std::fmt::Display;

use common::{
    Category, CategoryName, DatabaseID, NewCategory, NewRecurringTransaction, NewSavingsRatio,
    NewTransaction, NewUser, PasswordHash, Ratio, RecurringTransaction, SavingsRatio, Transaction,
    User, UserID,
};
use email_address::EmailAddress;
use rusqlite::{Connection, Error, Row, Transaction as SqlTransaction};

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
    fn create_table(connection: &Connection) -> Result<(), DbError>;
}

/// A trait for mapping from a `rusqlite::Row` from a SQLite database to a concrete rust type.
///
/// # Examples
/// ```
/// use rusqlite::{Connection, Error, Row};
///
/// use backend::db::{DbError, CreateTable, MapRow};
///
/// struct Foo {
///     id: i64,
///     desc: String
/// }
///
/// impl CreateTable for Foo {
///    fn create_table(connection: &Connection) -> Result<(), DbError> {
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
///    fn create_table(connection: &Connection) -> Result<(), DbError> {
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

impl CreateTable for User {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
        connection.execute(
            "CREATE TABLE user (
                    id INTEGER PRIMARY KEY,
                    email TEXT UNIQUE NOT NULL,
                    password TEXT UNIQUE NOT NULL
                    )",
            (),
        )?;

        Ok(())
    }
}

impl MapRow for User {
    type ReturnType = Self;

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, Error> {
        let raw_id = row.get(offset)?;
        let raw_email: String = row.get(offset + 1)?;
        let raw_password_hash = row.get(offset + 2)?;

        let id = UserID::new(raw_id);
        let email = EmailAddress::new_unchecked(raw_email);
        let password_hash = PasswordHash::new_unchecked(raw_password_hash);

        Ok(Self::new(id, email, password_hash))
    }
}

impl Insert for NewUser {
    type ResultType = User;

    /// Create a new user in the database.
    ///
    /// # Error
    /// This function will return an error if there was a problem executing the SQL query. This could be due to:
    /// - a syntax error in the SQL string,
    /// - the email is already in use, or
    /// - the password hash is not unique (very unlikely).
    fn insert(self, connection: &Connection) -> Result<Self::ResultType, DbError> {
        connection.execute(
            "INSERT INTO user (email, password) VALUES (?1, ?2)",
            (&self.email.to_string(), self.password_hash.to_string()),
        )?;

        let id = UserID::new(connection.last_insert_rowid());

        Ok(User::new(id, self.email, self.password_hash))
    }
}

impl SelectBy<&EmailAddress> for User {
    type ResultType = User;

    /// Get the user from the database that has the specified `email` address or `None` if such user does not exist.
    ///
    /// # Examples
    /// ```
    /// use email_address::EmailAddress;
    /// use rusqlite::Connection;
    ///
    /// # use backend::db::{DbError, SelectBy};
    /// # use common::User;
    /// #
    /// fn get_user(email: &EmailAddress, connection: &Connection) -> Result<User, DbError> {
    ///     let user = User::select(email, connection)?;
    ///     assert_eq!(user.email(), email);
    ///
    ///     Ok(user)
    /// }
    /// ```
    /// # Panics
    ///
    /// Panics if there is no user with the specified email or there are SQL related errors.
    fn select(email: &EmailAddress, connection: &Connection) -> Result<Self::ResultType, DbError> {
        connection
            .prepare("SELECT id, email, password FROM user WHERE email = :email")?
            .query_row(&[(":email", &email.to_string())], User::map_row)
            .map_err(|e| e.into())
    }
}

impl CreateTable for Category {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
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
    /// # use backend::db::{DbError, Insert};
    /// # use common::{Category, CategoryName, NewCategory, User};
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
    /// # use backend::db::SelectBy;
    /// # use common::{Category, DatabaseID};
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
    /// use backend::db::{Insert, SelectBy};
    /// use common::{Category, CategoryName, NewCategory, User};
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
    fn create_table(connection: &Connection) -> Result<(), DbError> {
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
    /// # use chrono::NaiveDate;
    /// # use rusqlite::Connection;
    /// #
    /// # use backend::db::Insert;
    /// # use common::{Category, NewTransaction, Transaction, User};
    /// #
    /// fn create_transaction(user: &User, category: &Category, connection: &Connection) {
    ///     let transaction = NewTransaction::new(
    ///         3.14,
    ///         NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
    ///         "Rust Pie".to_string(),
    ///         category.id(),
    ///         user.id()
    ///     )
    ///     .unwrap()
    ///     .insert(&connection)
    ///     .unwrap();
    ///
    ///     assert_eq!(transaction.amount(), 3.14);
    ///     assert_eq!(*transaction.date(), NaiveDate::from_ymd_opt(2024, 8, 7).unwrap());
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
    /// # use backend::db::{DbError, SelectBy};
    /// # use common::{DatabaseID, Transaction};
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
    /// use backend::db::SelectBy;
    /// use common::{Transaction, User};
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

impl CreateTable for SavingsRatio {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
        connection
            .execute(
                "CREATE TABLE savings_ratio (
                        transaction_id INTEGER PRIMARY KEY,
                        ratio REAL NOT NULL,
                        FOREIGN KEY(transaction_id) REFERENCES \"transaction\"(id) ON UPDATE CASCADE ON DELETE CASCADE
                        )",
                (),
            )?;

        Ok(())
    }
}

impl MapRow for SavingsRatio {
    type ReturnType = Self;

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, Error> {
        let transaction_id = row.get(offset)?;

        let raw_ratio = row.get(offset + 1)?;
        let ratio = Ratio::new_unchecked(raw_ratio);

        Ok(Self::new(transaction_id, ratio))
    }
}

impl Insert for NewSavingsRatio {
    type ResultType = SavingsRatio;

    /// Create a new savings ratio in the database.
    ///
    /// # Examples
    /// ```
    /// use rusqlite::Connection;
    ///
    /// use backend::db::Insert;
    /// use common::{Ratio, NewSavingsRatio, SavingsRatio, Transaction};
    ///
    /// fn set_savings_ratio(transaction: &Transaction, ratio: Ratio, connection: &Connection) -> SavingsRatio {
    ///     NewSavingsRatio {
    ///         transaction_id: transaction.id(),
    ///         ratio
    ///     }
    ///     .insert(connection)
    ///     .unwrap()
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if:
    /// - `transaction_id` does not refer to a valid transaction,
    /// - `ratio` is not a ratio between zero and one (inclusive),
    /// - or there is some other SQL error.
    fn insert(self, connection: &Connection) -> Result<Self::ResultType, DbError> {
        connection.execute(
            "INSERT INTO savings_ratio (transaction_id, ratio) VALUES (?1, ?2)",
            (&self.transaction_id, self.ratio.as_f64()),
        )?;

        Ok(Self::ResultType::new(self.transaction_id, self.ratio))
    }
}

impl CreateTable for RecurringTransaction {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
        connection
            .execute(
                "CREATE TABLE recurring_transaction (
                        transaction_id INTEGER PRIMARY KEY,
                        end_date TEXT NOT NULL,
                        frequency INTEGER NOT NULL,
                        FOREIGN KEY(transaction_id) REFERENCES \"transaction\"(id) ON UPDATE CASCADE ON DELETE CASCADE
                        )",
                (),
            )?;

        Ok(())
    }
}

impl MapRow for RecurringTransaction {
    type ReturnType = Self;

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, Error> {
        row.get::<usize, i64>(offset + 2)?
            .try_into()
            .map_err(|e| {
                Error::FromSqlConversionFailure(
                    offset + 2,
                    rusqlite::types::Type::Integer,
                    Box::new(e),
                )
            })
            .and_then(|frequency| {
                let transaction_id = row.get(offset)?;
                let end_date = row.get(offset + 1)?;

                Ok(Self::new_unchecked(transaction_id, end_date, frequency))
            })
    }
}

impl Insert for NewRecurringTransaction {
    type ResultType = RecurringTransaction;

    /// Create a new recurring transaction in the database.
    ///
    /// This will attach to an existing transaction and convert that transaction from a once-off transaction to a recurring one.
    ///
    /// # Examples
    /// ```
    /// use chrono::Utc;
    /// use rusqlite::Connection;
    ///
    /// use backend::db::Insert;
    /// use common::{Frequency, NewRecurringTransaction, RecurringTransaction, Transaction};
    ///
    /// fn set_recurring(
    ///     transaction: &Transaction,
    ///     frequency: Frequency,
    ///     connection: &Connection
    /// ) -> RecurringTransaction {
    ///     NewRecurringTransaction::new(transaction.clone(), None, frequency)
    ///         .unwrap()
    ///         .insert(connection)
    ///         .unwrap()
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if there is an SQL error.
    fn insert(self, connection: &Connection) -> Result<Self::ResultType, DbError> {
        connection.execute(
            "INSERT INTO recurring_transaction (transaction_id, end_date, frequency) VALUES (?1, ?2, ?3)",
            (self.transaction_id(), self.end_date(), self.frequency() as i64),
        )?;

        // The type `NewRecurringTransaction` will validate the end date, so we can skip validation here by calling the `new_unchecked` function.
        Ok(Self::ResultType::new_unchecked(
            self.transaction_id(),
            self.end_date().map(|date| date.to_owned()),
            self.frequency(),
        ))
    }
}

pub fn initialize(connection: &Connection) -> Result<(), DbError> {
    let transaction =
        SqlTransaction::new_unchecked(connection, rusqlite::TransactionBehavior::Exclusive)?;

    User::create_table(&transaction)?;
    Category::create_table(&transaction)?;
    Transaction::create_table(&transaction)?;
    SavingsRatio::create_table(&transaction)?;
    RecurringTransaction::create_table(&transaction)?;

    transaction.commit()?;

    Ok(())
}

// TODO: Separate types for recurring schedule (current RecurringTransaction type) and join result row of transaction and recurring schedule to create a RecurringTransaction?
pub fn select_recurring_transactions_by_user(
    user: &User,
    connection: &Connection,
) -> Result<Vec<(Transaction, RecurringTransaction)>, DbError> {
    connection
        .prepare(
            "SELECT l.id, l.amount, l.date, l.description, l.category_id, l.user_id, r.transaction_id, r.end_date, r.frequency
            FROM \"transaction\" l
            INNER JOIN recurring_transaction r ON r.transaction_id = l.id
            WHERE l.user_id = :user_id;",
        )?
        .query_and_then(&[(":user_id", &user.id().as_i64())], |row| {
            let transaction = Transaction::map_row(row)?;
            let recurring_transaction = RecurringTransaction::map_row_with_offset(row, 6)?;

            Ok((transaction, recurring_transaction))
        })?
        .map(|item| item.map_err(DbError::SqlError))
        .collect()
}

#[cfg(test)]
mod user_tests {
    use std::str::FromStr;

    use common::{NewUser, PasswordHash};
    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::db::{initialize, DbError, Insert, SelectBy, User};

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn insert_user_succeeds() {
        let conn = init_db();

        let email = EmailAddress::from_str("hello@world.com").unwrap();
        let password_hash = PasswordHash::new_unchecked("hunter2".to_string());

        let inserted_user = NewUser {
            email: email.clone(),
            password_hash: password_hash.clone(),
        }
        .insert(&conn)
        .unwrap();

        assert!(inserted_user.id().as_i64() > 0);
        assert_eq!(inserted_user.email(), &email);
        assert_eq!(inserted_user.password_hash(), &password_hash);
    }

    #[test]
    fn insert_user_fails_on_duplicate_email() {
        let conn = init_db();

        let email = EmailAddress::from_str("hello@world.com").unwrap();

        assert!(NewUser {
            email: email.clone(),
            password_hash: PasswordHash::new_unchecked("hunter2".to_string())
        }
        .insert(&conn)
        .is_ok());

        assert_eq!(
            NewUser {
                email: email.clone(),
                password_hash: PasswordHash::new_unchecked("hunter3".to_string())
            }
            .insert(&conn),
            Err(DbError::DuplicateEmail)
        );
    }

    #[test]
    fn insert_user_fails_on_duplicate_password() {
        let conn = init_db();

        let email = EmailAddress::from_str("hello@world.com").unwrap();
        let password = PasswordHash::new_unchecked("hunter2".to_string());

        assert!(NewUser {
            email,
            password_hash: password.clone()
        }
        .insert(&conn)
        .is_ok());

        assert_eq!(
            NewUser {
                email: EmailAddress::from_str("bye@world.com").unwrap(),
                password_hash: password.clone()
            }
            .insert(&conn),
            Err(DbError::DuplicatePassword)
        );
    }

    #[test]
    fn select_user_fails_with_non_existent_email() {
        let conn = init_db();

        // This email is not in the database.
        let email = EmailAddress::from_str("notavalidemail@foo.bar").unwrap();

        assert_eq!(User::select(&email, &conn), Err(DbError::NotFound));
    }

    #[test]
    fn select_user_succeeds_with_existing_email() {
        let conn = init_db();

        let test_user = NewUser {
            email: EmailAddress::from_str("foo@bar.baz").unwrap(),
            password_hash: PasswordHash::new_unchecked("hunter2".to_string()),
        }
        .insert(&conn)
        .unwrap();

        let retrieved_user = User::select(test_user.email(), &conn).unwrap();

        assert_eq!(retrieved_user, test_user);
    }
}

#[cfg(test)]
mod category_tests {
    use std::str::FromStr;

    use common::{NewCategory, NewUser, PasswordHash};
    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::db::{initialize, Category, CategoryName, DbError, SelectBy, User, UserID};

    use super::Insert;

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    fn create_database_and_insert_test_user() -> (Connection, User) {
        let conn = init_db();

        let test_user = NewUser {
            email: EmailAddress::from_str("foo@bar.baz").unwrap(),
            password_hash: PasswordHash::new_unchecked("hunter2".to_string()),
        }
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

    use chrono::{NaiveDate, Utc};
    use common::{CategoryName, NewCategory, NewTransaction, NewUser, PasswordHash, UserID};
    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::db::{initialize, Category, DbError, SelectBy, Transaction, User};

    use super::Insert;

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    fn create_database_and_insert_test_user_and_category() -> (Connection, User, Category) {
        let conn = init_db();

        let test_user = NewUser {
            email: EmailAddress::from_str("foo@bar.baz").unwrap(),
            password_hash: PasswordHash::new_unchecked("hunter2".to_string()),
        }
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
        let date = Utc::now().date_naive();
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
        let date = Utc::now().date_naive();
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
        let date = Utc::now().date_naive();
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

        let unauthorized_user = NewUser {
            email: EmailAddress::from_str("bar@baz.qux").unwrap(),
            password_hash: PasswordHash::new_unchecked("hunter3".to_string()),
        }
        .insert(&conn)
        .unwrap();

        let amount = PI;
        let date = Utc::now().date_naive();
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
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
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
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
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
    fn select_transactions_by_user_id_suceeds_with_no_transactions() {
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
                NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
                "Rust Pie".to_string(),
                category.id(),
                test_user.id(),
            )
            .unwrap()
            .insert(&conn)
            .unwrap(),
            NewTransaction::new(
                PI + 1.0,
                NaiveDate::from_ymd_opt(2024, 8, 8).unwrap(),
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

#[cfg(test)]
mod savings_ratio_tests {
    use std::{f64::consts::PI, str::FromStr};

    use chrono::NaiveDate;
    use common::{
        CategoryName, NewCategory, NewSavingsRatio, NewTransaction, NewUser, PasswordHash, Ratio,
    };
    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::db::{initialize, Transaction};

    use super::Insert;

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    fn create_database_and_insert_test_transaction() -> (Connection, Transaction) {
        let conn = init_db();

        let test_user = NewUser {
            email: EmailAddress::from_str("foo@bar.baz").unwrap(),
            password_hash: PasswordHash::new_unchecked("hunter2".to_string()),
        }
        .insert(&conn)
        .unwrap();

        let category = NewCategory {
            name: CategoryName::new("Food".to_string()).unwrap(),
            user_id: test_user.id(),
        }
        .insert(&conn)
        .unwrap();

        let transaction = NewTransaction::new(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
        )
        .unwrap()
        .insert(&conn)
        .unwrap();

        (conn, transaction)
    }

    #[test]
    fn insert_savings_ratio() {
        let (conn, transaction) = create_database_and_insert_test_transaction();

        let ratio = Ratio::new(0.5).unwrap();
        let savings_ratio = NewSavingsRatio {
            transaction_id: transaction.id(),
            ratio: ratio.clone(),
        }
        .insert(&conn)
        .unwrap();

        assert_eq!(savings_ratio.transaction_id(), transaction.id());
        assert_eq!(savings_ratio.ratio(), &ratio);
    }
}

#[cfg(test)]
mod recurring_transaction_tests {
    use std::{f64::consts::PI, str::FromStr};

    use chrono::{Months, NaiveDate};
    use common::{
        CategoryName, Frequency, NewCategory, NewRecurringTransaction, NewTransaction, NewUser,
        PasswordHash, Transaction, User,
    };
    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::db::select_recurring_transactions_by_user;

    use super::{initialize, Category, Insert};

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    fn create_database_and_insert_test_user() -> (Connection, User) {
        let conn = init_db();

        let test_user = NewUser {
            email: EmailAddress::from_str("foo@bar.baz").unwrap(),
            password_hash: PasswordHash::new_unchecked("hunter2".to_string()),
        }
        .insert(&conn)
        .unwrap();

        (conn, test_user)
    }

    fn create_database_and_insert_test_user_category_and_transaction(
    ) -> (Connection, User, Category, Transaction) {
        let (conn, test_user) = create_database_and_insert_test_user();

        let category = NewCategory {
            name: CategoryName::new("Food".to_string()).unwrap(),
            user_id: test_user.id(),
        }
        .insert(&conn)
        .unwrap();

        let transaction = NewTransaction::new(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
        )
        .unwrap()
        .insert(&conn)
        .unwrap();

        (conn, test_user, category, transaction)
    }

    #[test]
    fn insert_recurring_transaction() {
        let (conn, _, _, transaction) =
            create_database_and_insert_test_user_category_and_transaction();

        let end_date = transaction.date().checked_add_months(Months::new(3));

        let recurring = NewRecurringTransaction::new(&transaction, end_date, Frequency::Weekly)
            .unwrap()
            .insert(&conn)
            .unwrap();

        assert_eq!(recurring.transaction_id(), transaction.id());
        assert_eq!(recurring.end_date(), end_date.as_ref());
        assert_eq!(recurring.frequency(), Frequency::Weekly);
    }

    #[test]
    fn select_recurring_transactions_succeeds() {
        let (conn, test_user, _, transaction) =
            create_database_and_insert_test_user_category_and_transaction();

        let end_date = transaction.date().checked_add_months(Months::new(3));

        let inserted_recurring_transction =
            NewRecurringTransaction::new(&transaction, end_date, Frequency::Weekly)
                .unwrap()
                .insert(&conn)
                .unwrap();

        let expected = vec![(transaction, inserted_recurring_transction)];

        let results = select_recurring_transactions_by_user(&test_user, &conn).unwrap();

        assert_eq!(results, expected);
    }

    #[test]
    fn select_recurring_transactions_returns_empty_list() {
        let (conn, test_user) = create_database_and_insert_test_user();

        let expected = vec![];
        let results = select_recurring_transactions_by_user(&test_user, &conn).unwrap();

        assert_eq!(results, expected);
    }
}
