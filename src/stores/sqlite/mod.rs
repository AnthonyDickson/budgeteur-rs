//! Contains convenience type alias and function for [AppState] that uses
//! the SQLite backend.

pub mod balance;
pub mod category;
pub mod transaction;
pub mod user;

pub use balance::SQLiteBalanceStore;
pub use category::SQLiteCategoryStore;
pub use transaction::SQLiteTransactionStore;
pub use user::SQLiteUserStore;

use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::{AppState, Error, db::initialize, pagination::PaginationConfig};

/// An alias for an [AppState] that uses SQLite for the backend.
pub type SQLAppState =
    AppState<SQLiteBalanceStore, SQLiteCategoryStore, SQLiteTransactionStore, SQLiteUserStore>;

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
    let balance_store = SQLiteBalanceStore::new(connection.clone());
    let category_store = SQLiteCategoryStore::new(connection.clone());
    let transaction_store = SQLiteTransactionStore::new(connection.clone());
    let user_store = SQLiteUserStore::new(connection.clone());

    Ok(AppState::new(
        cookie_secret,
        pagination_config,
        balance_store,
        category_store,
        transaction_store,
        user_store,
    ))
}
