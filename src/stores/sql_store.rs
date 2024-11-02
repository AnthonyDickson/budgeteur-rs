use std::sync::{Arc, Mutex};

use rusqlite::{Connection, Error};

use crate::{db::initialize, AppState};

use super::{SQLiteCategoryStore, SQLiteTransactionStore, SQLiteUserStore};

pub type SQLAppState = AppState<SQLiteCategoryStore, SQLiteTransactionStore, SQLiteUserStore>;

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
        category_store,
        transaction_store,
        user_store,
    ))
}
