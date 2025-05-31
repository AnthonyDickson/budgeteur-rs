//! Implements a struct that holds the state of the REST server.

use std::marker::{Send, Sync};

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use sha2::{Digest, Sha512};
use time::Duration;

use crate::{
    auth::cookie::DEFAULT_COOKIE_DURATION,
    stores::{BalanceStore, CategoryStore, TransactionStore, UserStore},
};

/// The state of the REST server.
#[derive(Debug, Clone)]
pub struct AppState<B, C, T, U>
where
    B: BalanceStore + Send + Sync,
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,
    /// The duration for which cookies used for authentication are valid.
    pub cookie_duration: Duration,
    /// The store for managing user [balances](crate::models::Balance).
    pub balance_store: B,
    /// The store for managing user [categories](crate::models::Category).
    pub category_store: C,
    /// The store for managing user [transactions](crate::models::Transaction).
    pub transaction_store: T,
    /// The store for managing [users](crate::models::User).
    pub user_store: U,
}

impl<B, C, T, U> AppState<B, C, T, U>
where
    B: BalanceStore + Send + Sync,
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    /// Create a new [AppState].
    pub fn new(
        cookie_secret: &str,
        balance_store: B,
        category_store: C,
        transaction_store: T,
        user_store: U,
    ) -> Self {
        Self {
            cookie_key: create_cookie_key(cookie_secret),
            cookie_duration: DEFAULT_COOKIE_DURATION,
            balance_store,
            category_store,
            transaction_store,
            user_store,
        }
    }
}

// this impl tells `PrivateCookieJar` how to access the key from our state
impl<B, C, T, U> FromRef<AppState<B, C, T, U>> for Key
where
    B: BalanceStore + Send + Sync,
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    fn from_ref(state: &AppState<B, C, T, U>) -> Self {
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

impl<B, C, T, U> FromRef<AppState<B, C, T, U>> for AuthState
where
    B: BalanceStore + Send + Sync,
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    fn from_ref(state: &AppState<B, C, T, U>) -> Self {
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

/// The state needed to perform a login.
#[derive(Debug, Clone)]
pub struct LoginState<U>
where
    U: UserStore + Send + Sync,
{
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,
    /// The duration for which cookies used for authentication are valid.
    pub cookie_duration: Duration,
    /// The store for managing [users](crate::models::User).
    pub user_store: U,
}

impl<U> LoginState<U>
where
    U: UserStore + Clone + Send + Sync,
{
    /// Create the cookie key from a string and set the default cookie duration.
    pub fn new(cookie_secret: &str, user_store: U) -> Self {
        Self {
            cookie_key: create_cookie_key(cookie_secret),
            cookie_duration: DEFAULT_COOKIE_DURATION,
            user_store,
        }
    }
}

impl<B, C, T, U> FromRef<AppState<B, C, T, U>> for LoginState<U>
where
    B: BalanceStore + Send + Sync,
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Clone + Send + Sync,
{
    fn from_ref(state: &AppState<B, C, T, U>) -> Self {
        Self {
            cookie_key: state.cookie_key.clone(),
            cookie_duration: state.cookie_duration,
            user_store: state.user_store.clone(),
        }
    }
}

// this impl tells `PrivateCookieJar` how to access the key from our state
impl<U> FromRef<LoginState<U>> for Key
where
    U: UserStore + Clone + Send + Sync,
{
    fn from_ref(state: &LoginState<U>) -> Self {
        state.cookie_key.clone()
    }
}

/// The state needed for creating a new user.
pub type RegistrationState<U> = LoginState<U>;

/// The state needed to get or create a transaction.
#[derive(Debug, Clone)]
pub struct TransactionState<T>
where
    T: TransactionStore + Send + Sync,
{
    /// The store for managing user [transactions](crate::models::Transaction).
    pub transaction_store: T,
}

impl<B, C, T, U> FromRef<AppState<B, C, T, U>> for TransactionState<T>
where
    B: BalanceStore + Send + Sync,
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Clone + Send + Sync,
    U: UserStore + Send + Sync,
{
    fn from_ref(state: &AppState<B, C, T, U>) -> Self {
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

impl<B, C, T, U> FromRef<AppState<B, C, T, U>> for CategoryState<C>
where
    B: BalanceStore + Send + Sync,
    C: CategoryStore + Clone + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    fn from_ref(state: &AppState<B, C, T, U>) -> Self {
        Self {
            category_store: state.category_store.clone(),
        }
    }
}

/// The state needed for creating account balances.
#[derive(Debug, Clone)]
pub struct BalanceState<B>
where
    B: BalanceStore + Send + Sync,
{
    /// The store for managing user [balances](crate::models::Balance).
    pub balance_store: B,
}

impl<B, C, T, U> FromRef<AppState<B, C, T, U>> for BalanceState<B>
where
    B: BalanceStore + Clone + Send + Sync,
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    fn from_ref(state: &AppState<B, C, T, U>) -> Self {
        Self {
            balance_store: state.balance_store.clone(),
        }
    }
}

/// The state needed for displaying the dashboard page.
pub type DashboardState<T> = TransactionState<T>;

/// The state needed for importing transactions.
#[derive(Debug, Clone)]
pub struct ImportState<B, T>
where
    B: BalanceStore + Send + Sync,
    T: TransactionStore + Send + Sync,
{
    /// The store for managing user [balances](crate::models::Balance).
    pub balance_store: B,
    /// The store for managing user [transactions](crate::models::Transaction).
    pub transaction_store: T,
}

impl<B, C, T, U> FromRef<AppState<B, C, T, U>> for ImportState<B, T>
where
    B: BalanceStore + Clone + Send + Sync,
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Clone + Send + Sync,
    U: UserStore + Send + Sync,
{
    fn from_ref(state: &AppState<B, C, T, U>) -> Self {
        Self {
            balance_store: state.balance_store.clone(),
            transaction_store: state.transaction_store.clone(),
        }
    }
}

/// The state needed for the new transactions page.
pub type NewTransactionState<C> = CategoryState<C>;

/// The state needed for the transactions page.
pub type TransactionsViewState<T> = TransactionState<T>;
