use std::sync::{Arc, Mutex};

use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use jsonwebtoken::{DecodingKey, EncodingKey};
use rusqlite::Connection;

use crate::auth::AuthError;

#[derive(Clone)]
struct JwtKeys {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

#[derive(Clone)]
pub struct AppConfig {
    db_connection: Arc<Mutex<Connection>>,
    jwt_keys: JwtKeys,
}

impl AppConfig {
    pub fn new(db_connection: Connection, jwt_secret: String) -> AppConfig {
        AppConfig {
            db_connection: Arc::new(Mutex::new(db_connection)),
            jwt_keys: JwtKeys {
                encoding_key: EncodingKey::from_secret(jwt_secret.as_ref()),
                decoding_key: DecodingKey::from_secret(jwt_secret.as_ref()),
            },
        }
    }

    pub fn db_connection(&self) -> &Mutex<Connection> {
        &self.db_connection
    }

    /// The encoding key for JWTs.
    pub fn encoding_key(&self) -> &EncodingKey {
        &self.jwt_keys.encoding_key
    }

    /// The decoding key for JWTs.
    pub fn decoding_key(&self) -> &DecodingKey {
        &self.jwt_keys.decoding_key
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for AppConfig
where
    Self: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(_: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::from_ref(state))
    }
}
