/*! This module defines and implements traits for interacting with the application's database. */

use std::fmt::Display;

use rusqlite::{Connection, Error, Row, Transaction as SqlTransaction};

use crate::models::{Category, Transaction, User};

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
    /// An unexpected error occurred.
    Unspecified(String),
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

pub fn initialize(connection: &Connection) -> Result<(), DbError> {
    let transaction =
        SqlTransaction::new_unchecked(connection, rusqlite::TransactionBehavior::Exclusive)?;

    User::create_table(&transaction)?;
    Category::create_table(&transaction)?;
    Transaction::create_table(&transaction)?;

    transaction.commit()?;

    Ok(())
}
