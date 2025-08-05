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
    auth::cookie::DEFAULT_COOKIE_DURATION,
    pagination::PaginationConfig,
    stores::{CategoryStore, TransactionStore},
};

/// The state of the REST server.
#[derive(Debug, Clone)]
pub struct AppState<C, T>
where
    C: CategoryStore + Send + Sync,
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
    /// The store for managing user [categories](crate::models::Category).
    pub category_store: C,
    /// The store for managing user [transactions](crate::models::Transaction).
    pub transaction_store: T,
}

impl<C, T> AppState<C, T>
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
{
    /// Create a new [AppState].
    pub fn new(
        cookie_secret: &str,
        pagination_config: PaginationConfig,
        db_connection: Arc<Mutex<Connection>>,
        category_store: C,
        transaction_store: T,
    ) -> Self {
        Self {
            cookie_key: create_cookie_key(cookie_secret),
            cookie_duration: DEFAULT_COOKIE_DURATION,
            pagination_config,
            db_connection,
            category_store,
            transaction_store,
        }
    }
}

// this impl tells `PrivateCookieJar` how to access the key from our state
impl<C, T> FromRef<AppState<C, T>> for Key
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
{
    fn from_ref(state: &AppState<C, T>) -> Self {
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

impl<C, T> FromRef<AppState<C, T>> for AuthState
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
{
    fn from_ref(state: &AppState<C, T>) -> Self {
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

impl<C, T> FromRef<AppState<C, T>> for TransactionState<T>
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Clone + Send + Sync,
{
    fn from_ref(state: &AppState<C, T>) -> Self {
        Self {
            transaction_store: state.transaction_store.clone(),
        }
    }
}

/// The state needed for creating a category.
#[derive(Debug, Clone)]
pub struct CategoryState<C>
where
    C: CategoryStore + Send + Sync,
{
    /// The store for managing user [categories](crate::models::Category).
    pub category_store: C,
}

impl<C, T> FromRef<AppState<C, T>> for CategoryState<C>
where
    C: CategoryStore + Clone + Send + Sync,
    T: TransactionStore + Send + Sync,
{
    fn from_ref(state: &AppState<C, T>) -> Self {
        Self {
            category_store: state.category_store.clone(),
        }
    }
}

/// The state needed for displaying the dashboard page.
pub type DashboardState<T> = TransactionState<T>;

/// The state needed for the new transactions page.
pub type NewTransactionState<C> = CategoryState<C>;
