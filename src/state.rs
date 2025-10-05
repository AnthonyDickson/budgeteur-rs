//! Implements a struct that holds the state of the REST server.

use std::sync::{Arc, Mutex};

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use rusqlite::Connection;
use sha2::{Digest, Sha512};
use time::{Duration, UtcOffset};

use crate::{
    Error, auth_cookie::DEFAULT_COOKIE_DURATION, db::initialize, pagination::PaginationConfig,
};

/// The state of the REST server.
#[derive(Debug, Clone)]
pub struct AppState {
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,

    /// The duration for which cookies used for authentication are valid.
    pub cookie_duration: Duration,

    /// The local timezone as a UTC offset.
    pub local_timezone: UtcOffset,

    /// The config that controls how to display pages of data.
    pub pagination_config: PaginationConfig,

    /// The database connection
    pub db_connection: Arc<Mutex<Connection>>,
}

impl AppState {
    /// Create a new [AppState] with a SQLite database connection.
    ///
    /// This function will initialize the database by adding the tables for the domain models.
    ///
    /// # Errors
    /// Returns an error if the database cannot be initialized.
    pub fn new(
        db_connection: Connection,
        cookie_secret: &str,
        local_timezone: UtcOffset,
        pagination_config: PaginationConfig,
    ) -> Result<Self, Error> {
        initialize(&db_connection)?;

        let connection = Arc::new(Mutex::new(db_connection));

        Ok(Self {
            cookie_key: create_cookie_key(cookie_secret),
            cookie_duration: DEFAULT_COOKIE_DURATION,
            local_timezone,
            pagination_config,
            db_connection: connection,
        })
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

/// The state needed to get or create a transaction.
#[derive(Debug, Clone)]
pub struct TransactionState {
    /// The database connection for managing transactions.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for TransactionState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}
