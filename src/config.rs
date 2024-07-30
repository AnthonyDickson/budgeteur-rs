use axum::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use jsonwebtoken::{DecodingKey, EncodingKey};

use crate::auth::AuthError;

#[derive(Clone)]
struct JwtKeys {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

#[derive(Clone)]
pub struct AppConfig {
    jwt_keys: JwtKeys,
}

impl AppConfig {
    pub fn new(jwt_secret: String) -> AppConfig {
        AppConfig {
            jwt_keys: JwtKeys {
                encoding_key: EncodingKey::from_secret(jwt_secret.as_ref()),
                decoding_key: DecodingKey::from_secret(jwt_secret.as_ref()),
            },
        }
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
