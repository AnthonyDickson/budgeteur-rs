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

use crate::auth::AuthError;

/// The state of the REST server.
#[derive(Clone)]
pub struct AppState {
    /// The connection to the application's database.
    db_connection: Arc<Mutex<Connection>>,
    /// The secret used to encrypt auth cookies.
    cookie_key: Key,
}

impl AppState {
    pub fn new(db_connection: Connection, cookie_secret: String) -> Self {
        let hash = Sha512::digest(cookie_secret);

        Self {
            db_connection: Arc::new(Mutex::new(db_connection)),
            cookie_key: Key::from(&hash),
        }
    }

    pub fn db_connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.db_connection)
    }

    pub fn cookie_key(&self) -> &Key {
        &self.cookie_key
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
