//! Implements a struct that holds the state of the REST server.

use std::sync::{Arc, Mutex};

use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use axum_extra::extract::cookie::Key;
use rusqlite::Connection;
use sha2::{Digest, Sha512};

use crate::{
    auth::AuthError,
    stores::{SQLiteCategoryStore, SQLiteTransactionStore, SQLiteUserStore},
};

/// The state of the REST server.
#[derive(Debug, Clone)]
pub struct AppState {
    /// The secret used to encrypt auth cookies.
    cookie_key: Key,
    category_store: SQLiteCategoryStore,
    user_store: SQLiteUserStore,
    transaction_store: SQLiteTransactionStore,
}

impl AppState {
    pub fn new(db_connection: Connection, cookie_secret: &str) -> Self {
        let hash = Sha512::digest(cookie_secret);

        let db_connection = Arc::new(Mutex::new(db_connection));

        Self {
            cookie_key: Key::from(&hash),
            category_store: SQLiteCategoryStore::new(db_connection.clone()),
            user_store: SQLiteUserStore::new(db_connection.clone()),
            transaction_store: SQLiteTransactionStore::new(db_connection.clone()),
        }
    }

    pub fn cookie_key(&self) -> &Key {
        &self.cookie_key
    }

    pub fn category_store(&self) -> &SQLiteCategoryStore {
        &self.category_store
    }

    pub fn user_store(&self) -> &SQLiteUserStore {
        &self.user_store
    }

    pub fn transaction_store(&self) -> &SQLiteTransactionStore {
        &self.transaction_store
    }
}

// this impl tells `PrivateCookieJar` how to access the key from our state
impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for AppState
where
    Self: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(_: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::from_ref(state))
    }
}
