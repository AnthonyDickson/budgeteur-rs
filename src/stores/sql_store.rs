//! Contains convenience type alias and function for [AppState] that uses
//! the SQLite backend.

use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::{
    AppState, Error,
    db::initialize,
    models::{Balance, DatabaseID, UserID},
};

use super::{BalanceStore, SQLiteCategoryStore, SQLiteTransactionStore, SQLiteUserStore};

// TODO: Implement SQLiteBalanceStore
/// Placeholder
#[derive(Debug, Clone)]
pub struct StubBalanceStore;

impl BalanceStore for StubBalanceStore {
    fn create(&mut self, _account: &str, _balance: f64) -> Result<Balance, Error> {
        todo!()
    }

    fn get(&self, _id: DatabaseID) -> Result<Balance, Error> {
        todo!()
    }

    fn get_by_user_id(&self, _user_id: UserID) -> Result<Vec<Balance>, Error> {
        Ok(vec![])
    }
}

/// An alias for an [AppState] that uses SQLite for the backend.
pub type SQLAppState =
    AppState<StubBalanceStore, SQLiteCategoryStore, SQLiteTransactionStore, SQLiteUserStore>;

/// Creates an [AppState] instance that uses SQLite for the backend.
///
/// This function will modify the database by adding the tables for the domain
/// models to the database.
pub fn create_app_state(
    db_connection: Connection,
    cookie_secret: &str,
) -> Result<SQLAppState, Error> {
    initialize(&db_connection)?;

    let connection = Arc::new(Mutex::new(db_connection));
    let category_store = SQLiteCategoryStore::new(connection.clone());
    let transaction_store = SQLiteTransactionStore::new(connection.clone());
    let user_store = SQLiteUserStore::new(connection.clone());

    Ok(AppState::new(
        cookie_secret,
        StubBalanceStore {},
        category_store,
        transaction_store,
        user_store,
    ))
}
