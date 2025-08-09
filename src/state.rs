//! Implements a struct that holds the state of the REST server.

use std::{
    marker::{Send, Sync},
    sync::{Arc, Mutex},
};

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use rusqlite::Connection;
use sha2::{Digest, Sha512};
use time::Duration;

use crate::{
    auth::cookie::DEFAULT_COOKIE_DURATION, pagination::PaginationConfig, stores::TransactionStore,
};

/// The state of the REST server.
#[derive(Debug, Clone)]
pub struct AppState<T>
where
    T: TransactionStore + Send + Sync,
{
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,
    /// The duration for which cookies used for authentication are valid.
    pub cookie_duration: Duration,
    /// The config that controls how to display pages of data.
    pub pagination_config: PaginationConfig,
    /// The database connection
    pub db_connection: Arc<Mutex<Connection>>,
    /// The store for managing user [transactions](crate::models::Transaction).
    pub transaction_store: T,
}

impl<T> AppState<T>
where
    T: TransactionStore + Send + Sync,
{
    /// Create a new [AppState].
    pub fn new(
        cookie_secret: &str,
        pagination_config: PaginationConfig,
        db_connection: Arc<Mutex<Connection>>,
        transaction_store: T,
    ) -> Self {
        Self {
            cookie_key: create_cookie_key(cookie_secret),
            cookie_duration: DEFAULT_COOKIE_DURATION,
            pagination_config,
            db_connection,
            transaction_store,
        }
    }
}

// this impl tells `PrivateCookieJar` how to access the key from our state
impl<T> FromRef<AppState<T>> for Key
where
    T: TransactionStore + Send + Sync,
{
    fn from_ref(state: &AppState<T>) -> Self {
        state.cookie_key.clone()
    }
}

/// Create a signing key for cookies from a `secret`s string.
pub fn create_cookie_key(secret: &str) -> Key {
    let hash = Sha512::digest(secret);

    Key::from(&hash)
}

/// The state needed for the auth middleware
#[derive(Clone)]
pub struct AuthState {
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,
    /// The duration for which cookies used for authentication are valid.
    pub cookie_duration: Duration,
}

impl<T> FromRef<AppState<T>> for AuthState
where
    T: TransactionStore + Send + Sync,
{
    fn from_ref(state: &AppState<T>) -> Self {
        Self {
            cookie_key: state.cookie_key.clone(),
            cookie_duration: state.cookie_duration,
        }
    }
}

// this impl tells `PrivateCookieJar` how to access the key from our state
impl FromRef<AuthState> for Key {
    fn from_ref(state: &AuthState) -> Self {
        state.cookie_key.clone()
    }
}

/// The state needed to get or create a transaction.
#[derive(Debug, Clone)]
pub struct TransactionState<T>
where
    T: TransactionStore + Send + Sync,
{
    /// The store for managing user [transactions](crate::models::Transaction).
    pub transaction_store: T,
}

impl<T> FromRef<AppState<T>> for TransactionState<T>
where
    T: TransactionStore + Clone + Send + Sync,
{
    fn from_ref(state: &AppState<T>) -> Self {
        Self {
            transaction_store: state.transaction_store.clone(),
        }
    }
}

/// The state needed for displaying the dashboard page.
pub type DashboardState<T> = TransactionState<T>;

/// The state needed for the new transactions page.
#[derive(Debug, Clone)]
pub struct NewTransactionState {
    pub db_connection: Arc<Mutex<Connection>>,
}

impl<T> FromRef<AppState<T>> for NewTransactionState
where
    T: TransactionStore + Send + Sync,
{
    fn from_ref(state: &AppState<T>) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}
