//! Implements a struct that holds the state of the REST server.

use std::sync::{Arc, Mutex};

use crate::auth::{SessionStore, TuiKeyStore};
use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use kameo::actor::ActorRef;
use kameo_actors::scheduler::Scheduler;
use rusqlite::Connection;
use sha2::{Digest, Sha512};

/// The state of the REST server.
#[derive(Debug, Clone)]
pub struct AppState {
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,

    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,

    /// The database connection
    pub db_connection: Arc<Mutex<Connection>>,

    /// An in-memory store for managing sessions
    pub session_actor: ActorRef<SessionStore>,

    /// An actor that schedules messages to actors. Primarily for clearing the
    /// session store at a fixed interval.
    pub scheduler: ActorRef<Scheduler>,

    /// Allowed Ed25519 public keys for TUI client authentication.
    pub tui_key_store: TuiKeyStore,
}

impl AppState {
    /// Create a new [AppState] with a SQLite database connection.
    ///
    /// `local_timezone` should be a valid, canonical timezone name, e.g. "Pacific/Auckland".
    pub fn new(
        db_connection: Connection,
        cookie_secret: &str,
        local_timezone: &str,
        session_actor: ActorRef<SessionStore>,
        scheduler: ActorRef<Scheduler>,
        tui_key_store: TuiKeyStore,
    ) -> Self {
        let connection = Arc::new(Mutex::new(db_connection));

        Self {
            cookie_key: create_cookie_key(cookie_secret),
            local_timezone: local_timezone.to_owned(),
            db_connection: connection,
            session_actor,
            scheduler,
            tui_key_store,
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
