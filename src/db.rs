/*! This module defines and implements traits for interacting with the application's database. */

use rusqlite::{Connection, Error, Row, Transaction as SqlTransaction};

use crate::{
    models::{Transaction, User},
    stores::SQLiteCategoryStore,
};

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
/// use budgeteur_rs::db::{CreateTable, MapRow};
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
/// fn example(conn: &Connection) -> Result<(Foo, Bar), Error> {
///     conn.
///         prepare("SELECT l.id, l.desc, r.id, r.desc FROM foo l INNER JOIN bar r ON l.id = r.foo_id WHERE l.id = :id")?
///         .query_row(&[(":id", &1)], |row| {
///             let foo = Foo::map_row(row)?;
///             let bar = Bar::map_row_with_offset(row, 2)?;
///
///             Ok((foo, bar))
///         })
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

/// Create the all of the database tables for the application.
///
/// # Errors
/// This function may return a [rusqlite::Error] if something went wrong creating the tables.
pub fn initialize(connection: &Connection) -> Result<(), Error> {
    let transaction =
        SqlTransaction::new_unchecked(connection, rusqlite::TransactionBehavior::Exclusive)?;

    User::create_table(&transaction)?;
    SQLiteCategoryStore::create_table(&transaction)?;
    Transaction::create_table(&transaction)?;

    transaction.commit()?;

    Ok(())
}
