//! Implements a struct that holds the state of the REST server.

use std::marker::{Send, Sync};

use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use axum_extra::extract::cookie::Key;
use sha2::{Digest, Sha512};
use time::Duration;

use crate::{
    auth::{cookie::COOKIE_DURATION, AuthError},
    stores::{CategoryStore, TransactionStore, UserStore},
};

/// The state of the REST server.
#[derive(Debug, Clone)]
pub struct AppState<C, T, U>
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,
    /// The duration for which cookies used for authentication are valid.
    pub cookie_duration: Duration,
    /// The store for managing user [categories](crate::models::Category).
    pub category_store: C,
    /// The store for managing user [transactions](crate::models::Transaction).
    pub transaction_store: T,
    /// The store for managing [users](crate::models::User).
    pub user_store: U,
}

impl<C, T, U> AppState<C, T, U>
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    /// Create a new [AppState].
    pub fn new(
        cookie_secret: &str,
        category_store: C,
        transaction_store: T,
        user_store: U,
    ) -> Self {
        let hash = Sha512::digest(cookie_secret);

        Self {
            cookie_key: Key::from(&hash),
            cookie_duration: COOKIE_DURATION,
            category_store,
            transaction_store,
            user_store,
        }
    }
}

// this impl tells `PrivateCookieJar` how to access the key from our state
impl<C, T, U> FromRef<AppState<C, T, U>> for Key
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    fn from_ref(state: &AppState<C, T, U>) -> Self {
        state.cookie_key.clone()
    }
}

#[async_trait]
impl<S, C, T, U> FromRequestParts<S> for AppState<C, T, U>
where
    Self: FromRef<S>,
    S: Send + Sync,
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(_: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::from_ref(state))
    }
}
