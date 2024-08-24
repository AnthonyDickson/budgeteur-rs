use std::fmt::Display;

use chrono::{NaiveDate, Utc};
use common::{Category, CategoryName, DatabaseID, Email, PasswordHash, User, UserID};
use rusqlite::{
    types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput},
    Connection, Error, Row, Transaction as SqlTransaction,
};
use serde::{Deserialize, Serialize};

/// Errors originating from operations on the app's database.
#[derive(thiserror::Error, Debug, PartialEq)]
pub enum DbError {
    /// An empty email was given. The client should try again with a non-empty email address.
    EmptyEmail,
    /// An empty password hash was given. The client should try again with a non-empty password hash.
    EmptyPassword,
    /// The user's email already exists in the database. The client should try again with a different email address.
    DuplicateEmail,
    /// The password hash clashed with an existing password hash (should be extremely rare), the caller should rehash the password and try again.
    DuplicatePassword,
    /// The specified field was empty when it is not allowed. The client should try again after replacing the field with a non-empty string.
    EmptyField(String),
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
            Self::EmptyField(description) => write!(f, "{:?}: {}", self, description),
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

/// An object schema that is stored in a database.
pub trait Model {
    type ReturnType;

    /// Create a table for the model.
    ///
    /// # Errors
    /// Returns an error if the table already exists or if there is an SQL error.
    fn create_table(connection: &Connection) -> Result<(), DbError>;

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
    /// # Examples
    /// ```
    /// use rusqlite::{Connection, Error, Row};
    ///
    /// use backend::db::{DbError, Model};
    ///
    /// struct Foo {
    ///     id: i64,
    ///     desc: String
    /// }
    ///
    /// impl Model for Foo {
    ///     type ReturnType = Self;
    ///
    ///     fn create_table(connection: &Connection) -> Result<(), DbError> {
    ///         connection.execute(
    ///             "CREATE TABLE bar (id INTEGER PRIMARY KEY, desc TEXT NOT NULL)",
    ///             (),
    ///         )?;
    ///
    ///         Ok(())
    ///     }
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
    /// impl Model for Bar {
    ///     type ReturnType = Self;
    ///
    ///     fn create_table(connection: &Connection) -> Result<(), DbError> {
    ///         connection.execute(
    ///             "CREATE TABLE bar (id INTEGER PRIMARY KEY, desc TEXT NOT NULL)",
    ///             (),
    ///         )?;
    ///
    ///         Ok(())
    ///     }
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
    ///
    /// # Errors
    /// Returns an error if a row item cannot be converted into the corresponding rust type, or if an invalid column index was used.
    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self::ReturnType, Error>;
}

pub trait Insert {
    type ParamType;
    type ResultType;

    fn insert(
        params: Self::ParamType,
        connection: &Connection,
    ) -> Result<Self::ResultType, DbError>;
}

// TODO: Implement `SelectBy` for other model types.
pub trait SelectBy<T> {
    type ResultType;

    fn select(field: T, connection: &Connection) -> Result<Self::ResultType, DbError>;
}

impl Model for User {
    type ReturnType = Self;

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

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, Error> {
        let raw_id = row.get(offset)?;
        let raw_email = row.get(offset + 1)?;
        let raw_password_hash = row.get(offset + 2)?;

        let id = UserID::new(raw_id);
        let email = unsafe { Email::new_unchecked(raw_email) };
        let password_hash = unsafe { PasswordHash::new_unchecked(raw_password_hash) };

        Ok(Self::new(id, email, password_hash))
    }
}

pub struct NewUser {
    pub email: Email,
    pub password_hash: PasswordHash,
}

impl Insert for User {
    type ParamType = NewUser;
    type ResultType = User;

    /// Create a new user in the database.
    ///
    /// It is up to the caller to ensure the password is properly hashed.
    ///
    /// # Error
    /// This function will return an error if there was a problem executing the SQL query. This could be due to:
    /// - the email is empty,
    /// - the password is empty,
    /// - a syntax error in the SQL string,
    /// - the email is already in use, or
    /// - the password hash is not unique.
    fn insert(user: Self::ParamType, connection: &Connection) -> Result<Self::ResultType, DbError> {
        connection.execute(
            "INSERT INTO user (email, password) VALUES (?1, ?2)",
            (&user.email.to_string(), user.password_hash.to_string()),
        )?;

        let id = UserID::new(connection.last_insert_rowid());

        Ok(User::new(id, user.email, user.password_hash))
    }
}

impl SelectBy<&Email> for User {
    type ResultType = User;

    /// Get the user from the database that has the specified `email` address or `None` if such user does not exist.
    ///
    /// # Examples
    /// ```
    /// # use rusqlite::Connection;
    /// #
    /// # use backend::db::{DbError, SelectBy};
    /// # use common::{Email, User};
    /// #
    /// fn get_user(email: &Email, connection: &Connection) -> Result<User, DbError> {
    ///     let user = User::select(email, connection)?;
    ///     assert_eq!(user.email(), email);
    ///
    ///     Ok(user)
    /// }
    /// ```
    /// # Panics
    ///
    /// Panics if there is no user with the specified email or there are SQL related errors.
    fn select(email: &Email, connection: &Connection) -> Result<Self::ResultType, DbError> {
        connection
            .prepare("SELECT id, email, password FROM user WHERE email = :email")?
            .query_row(&[(":email", &email.to_string())], User::map_row)
            .map_err(|e| e.into())
    }
}

impl Model for Category {
    type ReturnType = Self;

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

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, Error> {
        let id = row.get(offset)?;

        let raw_name: String = row.get(offset + 1)?;
        let name = unsafe { CategoryName::new_unchecked(raw_name) };

        let raw_user_id = row.get(offset + 2)?;
        let user_id = UserID::new(raw_user_id);

        Ok(Self::new(id, name, user_id))
    }
}

/// The data for creating a new category.
pub struct NewCategory {
    pub name: CategoryName,
    pub user_id: UserID,
}

impl Insert for Category {
    type ParamType = NewCategory;
    type ResultType = Category;

    /// Create a new category in the database.
    ///
    /// # Examples
    /// ```
    /// # use rusqlite::Connection;
    /// #
    /// # use backend::db::{DbError, NewCategory, Insert};
    /// # use common::{Category, CategoryName, User};
    /// #
    /// fn create_category(name: String, user: &User, connection: &Connection) -> Result<Category, DbError> {
    ///     let name = CategoryName::new(name).unwrap();
    ///     let category = Category::insert(NewCategory { name: name.clone(), user_id: user.id() }, &connection)?;
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
    fn insert(
        category_data: Self::ParamType,
        connection: &Connection,
    ) -> Result<Self::ResultType, DbError> {
        connection.execute(
            "INSERT INTO category (name, user_id) VALUES (?1, ?2)",
            (category_data.name.as_ref(), category_data.user_id.as_i64()),
        )?;

        let category_id = connection.last_insert_rowid();

        Ok(Self::ResultType::new(
            category_id,
            category_data.name,
            category_data.user_id,
        ))
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
    /// # use backend::db::{Model, SelectBy};
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
    /// use backend::db::{Insert, Model, NewCategory, SelectBy};
    /// use common::{Category, CategoryName, User};
    ///
    /// fn create_and_validate_categories(user: &User, connection: &Connection) -> Vec<Category> {
    ///     let inserted_categories = vec![
    ///         Category::insert(
    ///            NewCategory {
    ///                name: unsafe { CategoryName::new_unchecked("Foo".to_string()) },
    ///                user_id: user.id(),
    ///            },
    ///            &connection,
    ///         )
    ///         .unwrap(),
    ///         Category::insert(
    ///             NewCategory {
    ///                 name: unsafe { CategoryName::new_unchecked("Bar".to_string()) },
    ///                 user_id: user.id(),
    ///             },
    ///             &connection,
    ///         )
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

/// An expense or income, i.e. an event where money was either spent or earned.
///
/// New instances should be created through `Transaction::insert(...)`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    id: DatabaseID,
    amount: f64,
    date: NaiveDate,
    description: String,
    category_id: DatabaseID,
    user_id: UserID,
}

impl Model for Transaction {
    type ReturnType = Self;

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

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, Error> {
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

impl Transaction {
    pub fn id(&self) -> DatabaseID {
        self.id
    }

    pub fn amount(&self) -> f64 {
        self.amount
    }

    pub fn date(&self) -> &NaiveDate {
        &self.date
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn category_id(&self) -> DatabaseID {
        self.category_id
    }

    pub fn user_id(&self) -> UserID {
        self.user_id
    }

    /// Create a new transaction in the database.
    ///
    /// Dates must be no later than today.
    ///
    /// # Examples
    /// ```
    /// # use chrono::NaiveDate;
    /// # use rusqlite::Connection;
    /// #
    /// # use backend::db::{Insert, Model, Transaction};
    /// # use common::{Category, User};
    /// #
    /// fn create_transaction(user: &User, category: &Category, connection: &Connection) {
    ///     let transaction = Transaction::insert(
    ///         3.14,
    ///         NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
    ///         "Rust Pie".to_string(),
    ///         category.id(),
    ///         user.id(),
    ///         &connection
    ///     )
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
    /// - `date` refers to a future date,
    /// - `name` is empty,
    /// - `category_id` does not refer to a valid category,
    /// - `user_id` does not refer to a valid user,
    /// - or there is some other SQL error.
    pub fn insert(
        amount: f64,
        date: NaiveDate,
        description: String,
        category_id: DatabaseID,
        user_id: UserID,
        connection: &Connection,
    ) -> Result<Transaction, DbError> {
        if date > Utc::now().date_naive() {
            return Err(DbError::InvalidDate);
        }

        // TODO: Ensure that the category id refers to a category owned by the user.
        connection
            .execute(
                "INSERT INTO \"transaction\" (amount, date, description, category_id, user_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                (amount, &date, &description, category_id, user_id.as_i64()),
            )?;

        let transaction_id = connection.last_insert_rowid();

        Ok(Transaction {
            id: transaction_id,
            amount,
            date,
            description,
            category_id,
            user_id,
        })
    }

    /// Retrieve a transaction in the database by its `id`.
    ///
    /// # Examples
    /// ```
    /// # use rusqlite::Connection;
    /// #
    /// # use backend::db::{DbError, Transaction};
    /// # use common::DatabaseID;
    /// #
    /// fn get_transaction(id: DatabaseID, connection: &Connection) -> Result<Transaction, DbError> {
    ///     Transaction::select_by_id(id, &connection)
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if:
    /// - `id` does not refer to a valid transaction,
    /// - or there is some other SQL error.
    pub fn select_by_id(id: DatabaseID, connection: &Connection) -> Result<Transaction, DbError> {
        let transaction = connection
            .prepare("SELECT id, amount, date, description, category_id, user_id FROM \"transaction\" WHERE id = :id")?
            .query_row(&[(":id", &id)], Transaction::map_row)?;

        Ok(transaction)
    }

    /// Retrieve the transactions in the database for the user `user_id`.
    ///
    /// # Examples
    /// ```
    /// use backend::db::Transaction;
    /// use common::User;
    ///
    /// fn sum_transaction_amount_for_user(user: &User, conn: &rusqlite::Connection) -> f64 {
    ///     let transactions = Transaction::select_by_user_id(user.id(), conn).unwrap();
    ///     transactions.iter().map(|transaction| transaction.amount()).sum()
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if there is an SQL error.
    pub fn select_by_user_id(
        user_id: UserID,
        connection: &Connection,
    ) -> Result<Vec<Transaction>, DbError> {
        connection
            .prepare("SELECT id, amount, date, description, category_id, user_id FROM \"transaction\" WHERE user_id = :user_id")?
            .query_map(&[(":user_id", &user_id.as_i64())], Transaction::map_row)?
            .map(|maybe_category| maybe_category.map_err(DbError::SqlError))
            .collect()
    }
}

/// The amount of an income transaction that should counted as savings.
///
/// This object must be attached to an existing transaction and cannot exist independently.
///
/// New instances should be created through `SavingsRatio::insert(...)`.
#[derive(Debug, PartialEq)]
pub struct SavingsRatio {
    transaction_id: DatabaseID,
    ratio: f64,
}

impl Model for SavingsRatio {
    type ReturnType = Self;

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

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, Error> {
        Ok(Self {
            transaction_id: row.get(offset)?,
            ratio: row.get(offset + 1)?,
        })
    }
}

impl SavingsRatio {
    pub fn transaction_id(&self) -> DatabaseID {
        self.transaction_id
    }

    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Create a new savings ratio in the database.
    ///
    /// # Examples
    /// ```
    /// use rusqlite::Connection;
    ///
    /// use backend::db::{Transaction, SavingsRatio};
    ///
    /// fn set_savings_ratio(transaction: &Transaction, ratio: f64, connection: &Connection) -> SavingsRatio {
    ///     SavingsRatio::insert(transaction.id(), ratio, connection).unwrap()
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if:
    /// - `transaction_id` does not refer to a valid transaction,
    /// - `ratio` is not a ratio between zero and one (inclusive),
    /// - or there is some other SQL error.
    pub fn insert(
        transaction_id: DatabaseID,
        ratio: f64,
        connection: &Connection,
    ) -> Result<SavingsRatio, DbError> {
        if !(0.0..=1.0).contains(&ratio) || ratio.is_sign_negative() {
            return Err(DbError::InvalidRatio);
        }

        connection.execute(
            "INSERT INTO savings_ratio (transaction_id, ratio) VALUES (?1, ?2)",
            (transaction_id, ratio),
        )?;

        Ok(SavingsRatio {
            transaction_id,
            ratio,
        })
    }
}

/// How often a recurring transaction happens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Frequency {
    Daily,
    Weekly,
    Fortnightly,
    /// A calendar month of variable length.
    Monthly,
    /// A calendar quarter (Jan-Mar, Apr-Jun, Jul-Sep, Oct-Dec).
    Quarterly,
    Yearly,
}

impl ToSql for Frequency {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Owned(rusqlite::types::Value::Integer(
            match self {
                Frequency::Daily => 0,
                Frequency::Weekly => 1,
                Frequency::Fortnightly => 2,
                Frequency::Monthly => 3,
                Frequency::Quarterly => 4,
                Frequency::Yearly => 5,
            },
        )))
    }
}

impl FromSql for Frequency {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_i64().and_then(|number| match number {
            0 => Ok(Frequency::Daily),
            1 => Ok(Frequency::Weekly),
            2 => Ok(Frequency::Fortnightly),
            3 => Ok(Frequency::Monthly),
            4 => Ok(Frequency::Quarterly),
            5 => Ok(Frequency::Yearly),
            _ => Err(FromSqlError::OutOfRange(number)),
        })
    }
}

/// A transaction (income or expense) that repeats on a regular basis (e.g., wages, phone bill).
///
/// This object must be attached to an existing transaction and cannot exist independently.
///
/// New instances should be created through `RecurringTransaction::insert(...)`.
#[derive(Debug, PartialEq)]
pub struct RecurringTransaction {
    transaction_id: DatabaseID,
    end_date: Option<NaiveDate>,
    frequency: Frequency,
}

impl Model for RecurringTransaction {
    type ReturnType = Self;

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

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, Error> {
        Ok(Self {
            transaction_id: row.get(offset)?,
            end_date: row.get(offset + 1)?,
            frequency: row.get(offset + 2)?,
        })
    }
}

impl RecurringTransaction {
    pub fn transaction_id(&self) -> DatabaseID {
        self.transaction_id
    }

    pub fn end_date(&self) -> &Option<NaiveDate> {
        &self.end_date
    }

    pub fn frequency(&self) -> Frequency {
        self.frequency
    }

    /// Create a new recurring transaction in the database.
    ///
    /// This will attach to an existing transaction and convert that transaction from a once-off transaction to a recurring one.
    ///
    /// # Examples
    /// ```
    /// use chrono::Utc;
    /// use rusqlite::Connection;
    ///
    /// use backend::db::{Frequency, Transaction, RecurringTransaction};
    ///
    /// fn set_recurring(
    ///     transaction: &Transaction,
    ///     frequency: Frequency,
    ///     connection: &Connection
    /// ) -> RecurringTransaction {
    ///     RecurringTransaction::insert(
    ///         &transaction,
    ///         None,
    ///         frequency,
    ///         connection
    ///     )
    ///     .unwrap()
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if:
    /// - `end_date` is on or before `transaction.date()`,
    /// - or there is some other SQL error.
    pub fn insert(
        transaction: &Transaction,
        end_date: Option<NaiveDate>,
        frequency: Frequency,
        connection: &Connection,
    ) -> Result<RecurringTransaction, DbError> {
        if end_date.is_some_and(|date| date <= *transaction.date()) {
            return Err(DbError::InvalidDate);
        }

        connection.execute(
            "INSERT INTO recurring_transaction (transaction_id, end_date, frequency) VALUES (?1, ?2, ?3)",
            (transaction.id(), end_date, frequency),
        )?;

        Ok(RecurringTransaction {
            transaction_id: transaction.id(),
            end_date,
            frequency,
        })
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

// TODO: Separate types for recurring schedule (current RecurringTransaction type) and join result row of transaction and recurring schedule?
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
    use common::{Email, PasswordHash};
    use rusqlite::Connection;

    use crate::db::{initialize, DbError, Insert, NewUser, SelectBy, User};

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn create_user() {
        let conn = init_db();

        let email = Email::new("hello@world.com").unwrap();
        let password_hash = unsafe { PasswordHash::new_unchecked("hunter2".to_string()) };

        let inserted_user = User::insert(
            NewUser {
                email: email.clone(),
                password_hash: password_hash.clone(),
            },
            &conn,
        )
        .unwrap();

        assert!(inserted_user.id().as_i64() > 0);
        assert_eq!(inserted_user.email(), &email);
        assert_eq!(inserted_user.password_hash(), &password_hash);
    }

    #[test]
    fn create_user_duplicate_email() {
        let conn = init_db();

        let email = Email::new("hello@world.com").unwrap();

        assert!(User::insert(
            NewUser {
                email: email.clone(),
                password_hash: unsafe { PasswordHash::new_unchecked("hunter2".to_string()) }
            },
            &conn
        )
        .is_ok());
        assert_eq!(
            User::insert(
                NewUser {
                    email: email.clone(),
                    password_hash: unsafe { PasswordHash::new_unchecked("hunter3".to_string()) }
                },
                &conn
            ),
            Err(DbError::DuplicateEmail)
        );
    }

    #[test]
    fn create_user_duplicate_password() {
        let conn = init_db();

        let email = Email::new("hello@world.com").unwrap();
        let password = unsafe { PasswordHash::new_unchecked("hunter2".to_string()) };

        assert!(User::insert(
            NewUser {
                email,
                password_hash: password.clone()
            },
            &conn
        )
        .is_ok());
        assert_eq!(
            User::insert(
                NewUser {
                    email: Email::new("bye@world.com").unwrap(),
                    password_hash: password.clone()
                },
                &conn
            ),
            Err(DbError::DuplicatePassword)
        );
    }

    fn create_database_and_insert_test_user() -> (Connection, User) {
        let conn = init_db();

        let test_user = User::insert(
            NewUser {
                email: Email::new("foo@bar.baz").unwrap(),
                password_hash: unsafe { PasswordHash::new_unchecked("hunter2".to_string()) },
            },
            &conn,
        )
        .unwrap();

        (conn, test_user)
    }

    #[test]
    fn select_user_by_non_existent_email() {
        let (conn, _) = create_database_and_insert_test_user();

        let email = Email::new("notavalidemail@foo.bar").unwrap();

        assert_eq!(User::select(&email, &conn), Err(DbError::NotFound));
    }

    #[test]
    fn select_user_by_existing_email() {
        let conn = init_db();

        let test_user = User::insert(
            NewUser {
                email: Email::new("foo@bar.baz").unwrap(),
                password_hash: unsafe { PasswordHash::new_unchecked("hunter2".to_string()) },
            },
            &conn,
        )
        .unwrap();
        let retrieved_user = User::select(test_user.email(), &conn).unwrap();

        assert_eq!(retrieved_user, test_user);
    }
}

#[cfg(test)]
mod category_tests {
    use common::{Email, PasswordHash};
    use rusqlite::Connection;

    use crate::db::{
        initialize, Category, CategoryName, DbError, NewCategory, SelectBy, User, UserID,
    };

    use super::{Insert, NewUser};

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    fn create_database_and_insert_test_user() -> (Connection, User) {
        let conn = init_db();

        let test_user = User::insert(
            NewUser {
                email: Email::new("foo@bar.baz").unwrap(),
                password_hash: unsafe { PasswordHash::new_unchecked("hunter2".to_string()) },
            },
            &conn,
        )
        .unwrap();

        (conn, test_user)
    }

    #[test]
    fn create_category() {
        let (conn, test_user) = create_database_and_insert_test_user();

        let name = CategoryName::new("Categorically a category".to_string()).unwrap();
        let category = Category::insert(
            NewCategory {
                name: name.clone(),
                user_id: test_user.id(),
            },
            &conn,
        )
        .unwrap();

        assert!(category.id() > 0);
        assert_eq!(category.name(), &name);
        assert_eq!(category.user_id(), test_user.id());
    }

    #[test]
    fn create_category_with_invalid_user_id_returns_error() {
        let conn = init_db();

        let name = unsafe { CategoryName::new_unchecked("Foo".to_string()) };
        let maybe_category = Category::insert(
            NewCategory {
                name,
                user_id: UserID::new(42),
            },
            &conn,
        );

        assert_eq!(maybe_category, Err(DbError::InvalidForeignKey));
    }

    #[test]
    fn select_category() {
        let (conn, test_user) = create_database_and_insert_test_user();
        let name = unsafe { CategoryName::new_unchecked("Foo".to_string()) };
        let inserted_category = Category::insert(
            NewCategory {
                name,
                user_id: test_user.id(),
            },
            &conn,
        )
        .unwrap();

        let selected_category = Category::select(inserted_category.id(), &conn).unwrap();

        assert_eq!(inserted_category, selected_category);
    }

    #[test]
    fn select_category_with_invalid_id() {
        let conn = init_db();

        let selected_category = Category::select(1337, &conn);

        assert_eq!(selected_category, Err(DbError::NotFound));
    }

    #[test]
    fn select_category_with_user_id() {
        let (conn, test_user) = create_database_and_insert_test_user();
        let inserted_categories = vec![
            Category::insert(
                NewCategory {
                    name: unsafe { CategoryName::new_unchecked("Foo".to_string()) },
                    user_id: test_user.id(),
                },
                &conn,
            )
            .unwrap(),
            Category::insert(
                NewCategory {
                    name: unsafe { CategoryName::new_unchecked("Bar".to_string()) },
                    user_id: test_user.id(),
                },
                &conn,
            )
            .unwrap(),
        ];

        let selected_categories = Category::select(test_user.id(), &conn).unwrap();

        assert_eq!(inserted_categories, selected_categories);
    }

    #[test]
    fn select_category_with_invalid_user_id() {
        let (conn, test_user) = create_database_and_insert_test_user();
        Category::insert(
            NewCategory {
                name: unsafe { CategoryName::new_unchecked("Foo".to_string()) },
                user_id: test_user.id(),
            },
            &conn,
        )
        .unwrap();
        Category::insert(
            NewCategory {
                name: unsafe { CategoryName::new_unchecked("Bar".to_string()) },
                user_id: test_user.id(),
            },
            &conn,
        )
        .unwrap();

        let selected_categories =
            Category::select(UserID::new(test_user.id().as_i64() + 1), &conn).unwrap();

        assert_eq!(selected_categories, []);
    }
}

#[cfg(test)]
mod transaction_tests {
    use std::f64::consts::PI;

    use chrono::{Days, NaiveDate, Utc};
    use common::{CategoryName, Email, PasswordHash, UserID};
    use rusqlite::Connection;

    use crate::db::{initialize, Category, DbError, Transaction, User};

    use super::{Insert, NewCategory, NewUser};

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    fn create_database_and_insert_test_user_and_category() -> (Connection, User, Category) {
        let conn = init_db();

        let test_user = User::insert(
            NewUser {
                email: Email::new("foo@bar.baz").unwrap(),
                password_hash: unsafe { PasswordHash::new_unchecked("hunter2".to_string()) },
            },
            &conn,
        )
        .unwrap();

        let category = Category::insert(
            NewCategory {
                name: CategoryName::new("Food".to_string()).unwrap(),
                user_id: test_user.id(),
            },
            &conn,
        )
        .unwrap();

        (conn, test_user, category)
    }

    #[test]
    fn create_transaction() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let amount = PI;
        let date = Utc::now().date_naive();
        let description = "Rust Pie".to_string();

        let transaction = Transaction::insert(
            amount,
            date,
            description.clone(),
            category.id(),
            test_user.id(),
            &conn,
        )
        .unwrap();

        assert_eq!(transaction.amount(), amount);
        assert_eq!(*transaction.date(), date);
        assert_eq!(transaction.description(), description);
        assert_eq!(transaction.category_id(), category.id());
        assert_eq!(transaction.user_id(), test_user.id());
    }

    #[test]
    fn create_transaction_fails_on_invalid_user_id() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let amount = PI;
        let date = Utc::now().date_naive();
        let description = "Rust Pie".to_string();

        let maybe_transaction = Transaction::insert(
            amount,
            date,
            description.clone(),
            category.id(),
            UserID::new(test_user.id().as_i64() + 1),
            &conn,
        );

        assert_eq!(maybe_transaction, Err(DbError::InvalidForeignKey));
    }

    #[test]
    fn create_transaction_fails_on_invalid_category_id() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let amount = PI;
        let date = Utc::now().date_naive();
        let description = "Rust Pie".to_string();

        let maybe_transaction = Transaction::insert(
            amount,
            date,
            description.clone(),
            category.id() + 1,
            test_user.id(),
            &conn,
        );

        assert_eq!(maybe_transaction, Err(DbError::InvalidForeignKey));
    }

    #[test]
    fn create_transaction_fails_on_future_date() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let amount = PI;
        let date = Utc::now()
            .date_naive()
            .checked_add_days(Days::new(1))
            .unwrap();
        let description = "Rust Pie".to_string();

        let maybe_transaction = Transaction::insert(
            amount,
            date,
            description.clone(),
            category.id(),
            test_user.id(),
            &conn,
        );

        assert_eq!(maybe_transaction, Err(DbError::InvalidDate));
    }

    #[test]
    fn select_transaction_by_id() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let inserted_transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
            &conn,
        )
        .unwrap();

        let selected_transaction =
            Transaction::select_by_id(inserted_transaction.id(), &conn).unwrap();

        assert_eq!(inserted_transaction, selected_transaction);
    }

    #[test]
    fn select_transaction_by_invalid_id_fails() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let inserted_transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
            &conn,
        )
        .unwrap();

        let maybe_transaction = Transaction::select_by_id(inserted_transaction.id() + 1, &conn);

        assert_eq!(maybe_transaction, Err(DbError::NotFound));
    }

    #[test]
    fn select_transactions_by_user_id() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();

        let expected_transactions = vec![
            Transaction::insert(
                PI,
                NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
                "Rust Pie".to_string(),
                category.id(),
                test_user.id(),
                &conn,
            )
            .unwrap(),
            Transaction::insert(
                PI + 1.0,
                NaiveDate::from_ymd_opt(2024, 8, 8).unwrap(),
                "Rust Pif".to_string(),
                category.id(),
                test_user.id(),
                &conn,
            )
            .unwrap(),
        ];

        let transactions = Transaction::select_by_user_id(test_user.id(), &conn).unwrap();

        assert_eq!(transactions, expected_transactions);
    }
}

#[cfg(test)]
mod savings_ratio_tests {
    use std::f64::consts::PI;

    use chrono::NaiveDate;
    use common::{CategoryName, Email, PasswordHash};
    use rusqlite::Connection;

    use crate::db::{initialize, Category, DbError, SavingsRatio, Transaction, User};

    use super::{Insert, NewCategory, NewUser};

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    fn create_database_and_insert_test_user_and_category() -> (Connection, User, Category) {
        let conn = init_db();

        let test_user = User::insert(
            NewUser {
                email: Email::new("foo@bar.baz").unwrap(),
                password_hash: unsafe { PasswordHash::new_unchecked("hunter2".to_string()) },
            },
            &conn,
        )
        .unwrap();

        let category = Category::insert(
            NewCategory {
                name: CategoryName::new("Food".to_string()).unwrap(),
                user_id: test_user.id(),
            },
            &conn,
        )
        .unwrap();

        (conn, test_user, category)
    }

    #[test]
    fn create_savings_ratio() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();
        let transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
            &conn,
        )
        .unwrap();

        let ratio = 0.5;
        let savings_ratio = SavingsRatio::insert(transaction.id(), ratio, &conn).unwrap();

        assert_eq!(savings_ratio.transaction_id(), transaction.id());
        assert_eq!(savings_ratio.ratio(), ratio);
    }

    #[test]
    fn create_savings_ratio_fails_with_ratio_below_zero() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();
        let transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
            &conn,
        )
        .unwrap();

        let ratio = -0.01;
        let savings_ratio = SavingsRatio::insert(transaction.id(), ratio, &conn);

        assert_eq!(savings_ratio, Err(DbError::InvalidRatio));
    }

    #[test]
    fn create_savings_ratio_fails_with_negative_zero_ratio() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();
        let transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
            &conn,
        )
        .unwrap();

        let ratio = -0.0;
        let savings_ratio = SavingsRatio::insert(transaction.id(), ratio, &conn);

        assert_eq!(savings_ratio, Err(DbError::InvalidRatio));
    }

    #[test]
    fn create_savings_ratio_fails_with_ratio_above_one() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();
        let transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
            &conn,
        )
        .unwrap();

        let ratio = 1.01;
        let savings_ratio = SavingsRatio::insert(transaction.id(), ratio, &conn);

        assert_eq!(savings_ratio, Err(DbError::InvalidRatio));
    }
}

#[cfg(test)]
mod recurring_transaction_tests {
    use std::f64::consts::PI;

    use chrono::{Days, Months, NaiveDate};
    use common::{CategoryName, Email, PasswordHash, User};
    use rusqlite::Connection;

    use crate::db::{
        select_recurring_transactions_by_user, DbError, Frequency, RecurringTransaction,
        Transaction,
    };

    use super::{initialize, Category, Insert, NewCategory, NewUser};
    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    fn create_database_and_insert_test_user() -> (Connection, User) {
        let conn = init_db();

        let test_user = User::insert(
            NewUser {
                email: Email::new("foo@bar.baz").unwrap(),
                password_hash: unsafe { PasswordHash::new_unchecked("hunter2".to_string()) },
            },
            &conn,
        )
        .unwrap();

        (conn, test_user)
    }

    fn create_database_and_insert_test_user_and_category() -> (Connection, User, Category) {
        let (conn, test_user) = create_database_and_insert_test_user();

        let category = Category::insert(
            NewCategory {
                name: CategoryName::new("Food".to_string()).unwrap(),
                user_id: test_user.id(),
            },
            &conn,
        )
        .unwrap();

        (conn, test_user, category)
    }

    #[test]
    fn create_recurring_transaction() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();
        let transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
            &conn,
        )
        .unwrap();

        let end_date = transaction.date().checked_add_months(Months::new(3));

        let recurring =
            RecurringTransaction::insert(&transaction, end_date, Frequency::Weekly, &conn).unwrap();

        assert_eq!(recurring.transaction_id(), transaction.id);
        assert_eq!(*recurring.end_date(), end_date);
        assert_eq!(recurring.frequency(), Frequency::Weekly);
    }

    #[test]
    fn create_recurring_transaction_fails_on_past_end_date() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();
        let transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
            &conn,
        )
        .unwrap();

        assert_eq!(
            RecurringTransaction::insert(
                &transaction,
                Some(*transaction.date()),
                Frequency::Weekly,
                &conn
            ),
            Err(DbError::InvalidDate)
        );

        assert_eq!(
            RecurringTransaction::insert(
                &transaction,
                Some(transaction.date().checked_sub_days(Days::new(1)).unwrap()),
                Frequency::Weekly,
                &conn
            ),
            Err(DbError::InvalidDate)
        );
    }

    #[test]
    fn select_recurring_transactions_succeeds() {
        let (conn, test_user, category) = create_database_and_insert_test_user_and_category();
        let inserted_transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            test_user.id(),
            &conn,
        )
        .unwrap();

        let end_date = inserted_transaction
            .date()
            .checked_add_months(Months::new(3));

        let inserted_recurring_transction =
            RecurringTransaction::insert(&inserted_transaction, end_date, Frequency::Weekly, &conn)
                .unwrap();

        let expected = vec![(inserted_transaction, inserted_recurring_transction)];

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
