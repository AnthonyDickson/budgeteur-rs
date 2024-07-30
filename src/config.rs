use axum::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;

use crate::auth::AuthError;

#[derive(Clone)]
pub struct AppConfig {
    // TODO: Construct and store JWT encode and decode keys in AppConfig.
    pub jwt_secret: String,
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
