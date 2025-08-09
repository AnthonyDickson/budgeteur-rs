//! Contains convenience type alias and function for [AppState] that uses
//! the SQLite backend.

pub mod transaction;

pub use transaction::SQLiteTransactionStore;

use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::{AppState, Error, db::initialize, pagination::PaginationConfig};

/// An alias for an [AppState] that uses SQLite for the backend.
pub type SQLAppState = AppState<SQLiteTransactionStore>;

/// Creates an [AppState] instance that uses SQLite for the backend.
///
/// This function will modify the database by adding the tables for the domain
/// models to the database.
pub fn create_app_state(
    db_connection: Connection,
    cookie_secret: &str,
    pagination_config: PaginationConfig,
) -> Result<SQLAppState, Error> {
    initialize(&db_connection)?;

    let connection = Arc::new(Mutex::new(db_connection));
    let transaction_store = SQLiteTransactionStore::new(connection.clone());

    Ok(AppState::new(
        cookie_secret,
        pagination_config,
        connection,
        transaction_store,
    ))
}
