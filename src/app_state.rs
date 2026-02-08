//! Implements a struct that holds the state of the REST server.

use std::sync::{Arc, Mutex};

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use rusqlite::Connection;
use sha2::{Digest, Sha512};
use time::Duration;

use crate::auth::DEFAULT_COOKIE_DURATION;

/// The state of the REST server.
#[derive(Debug, Clone)]
pub struct AppState {
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,

    /// The duration for which cookies used for authentication are valid.
    pub cookie_duration: Duration,

    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,

    /// The database connection
    pub db_connection: Arc<Mutex<Connection>>,
}

impl AppState {
    /// Create a new [AppState] with a SQLite database connection.
    ///
    /// `local_timezone` should be a valid, canonical timezone name, e.g. "Pacific/Auckland".
    pub fn new(db_connection: Connection, cookie_secret: &str, local_timezone: &str) -> Self {
        let connection = Arc::new(Mutex::new(db_connection));

        Self {
            cookie_key: create_cookie_key(cookie_secret),
            cookie_duration: DEFAULT_COOKIE_DURATION,
            local_timezone: local_timezone.to_owned(),
            db_connection: connection,
        }
    }
}

// this impl tells `PrivateCookieJar` how to access the key from our state
impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}

/// Create a signing key for cookies from a `secret`s string.
pub fn create_cookie_key(secret: &str) -> Key {
    let hash = Sha512::digest(secret);

    Key::from(&hash)
}
