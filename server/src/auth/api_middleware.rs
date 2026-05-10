//! Middleware that authenticates TUI API requests via `Authorization: Bearer <jwt>`.

use axum::{
    extract::{FromRef, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;

use crate::{AppState, auth::api_keys::TuiKeyStore};

/// The state needed for the API auth middleware.
#[derive(Clone)]
pub struct ApiAuthState {
    pub key_store: TuiKeyStore,
}

impl FromRef<AppState> for ApiAuthState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            key_store: state.tui_key_store.clone(),
        }
    }
}

/// JSON error body returned on auth failure.
#[derive(Serialize)]
struct ApiError {
    error: String,
}

/// Middleware that validates an `Authorization: Bearer <jwt>` header.
///
/// If the token is valid, the request proceeds to the handler. If no keys
/// are configured, all requests are rejected. On failure, returns `401` JSON —
/// no redirects.
pub async fn api_auth_guard(
    State(state): State<ApiAuthState>,
    request: Request,
    next: Next,
) -> Response {
    let auth_header = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiError {
                    error: "missing Authorization header".into(),
                }),
            )
                .into_response();
        }
    };

    match state.key_store.validate(token) {
        Some(_claims) => next.run(request).await,
        None => {
            tracing::debug!("JWT validation failed — invalid or expired token");
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiError {
                    error: "invalid or expired token".into(),
                }),
            )
                .into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use axum::{Router, middleware, response::Json, routing::get};
    use axum_test::TestServer;
    use ed25519_dalek::SigningKey;
    use ed25519_dalek::pkcs8::EncodePrivateKey;
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use rand::RngCore;

    use crate::auth::api_keys::{TuiClaims, TuiKeyStore};

    use super::*;

    fn generate_keypair() -> (SigningKey, ed25519_dalek::VerifyingKey) {
        let mut seed = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    fn sign_jwt(signing_key: &SigningKey, claims: &TuiClaims) -> String {
        let der = signing_key.to_pkcs8_der().unwrap();
        let encoding_key = EncodingKey::from_ed_der(der.as_bytes());
        encode(&Header::new(Algorithm::EdDSA), claims, &encoding_key).unwrap()
    }

    fn build_test_app(key_store: TuiKeyStore) -> TestServer {
        let state = ApiAuthState { key_store };

        let app = Router::new()
            .route("/", get(|| async { Json(serde_json::json!({"ok": true})) }))
            .route_layer(middleware::from_fn_with_state(state, api_auth_guard));

        TestServer::new(app)
    }

    #[tokio::test]
    async fn rejects_missing_authorization_header() {
        // Given an app with an empty key store (no keys configured)
        let server = build_test_app(TuiKeyStore::empty());

        // When a request is made without an Authorization header
        let response = server.get("/").await;

        // Then the response is 401 Unauthorized
        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rejects_empty_key_store() {
        // Given an app with an empty key store
        let (signing_key, _vk) = generate_keypair();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        let claims = TuiClaims {
            sub: "tui-client".into(),
            iat: now,
            exp: now + 3600,
        };
        let token = sign_jwt(&signing_key, &claims);
        let server = build_test_app(TuiKeyStore::empty());

        // When a request is made with a valid Bearer token but no keys are configured
        let response = server
            .get("/")
            .add_header("Authorization", format!("Bearer {token}"))
            .await;

        // Then the response is 401 Unauthorized
        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn accepts_valid_token() {
        // Given an app with a known public key
        let (signing_key, verifying_key) = generate_keypair();
        let hex_key = hex::encode(verifying_key.to_bytes());
        let config_str = format!("[[keys]]\nlabel = \"test\"\npublic_key = \"{hex_key}\"\n");
        let config: crate::auth::api_keys::TuiKeysConfig = toml::from_str(&config_str).unwrap();
        let store = TuiKeyStore::load_from_config(&config).unwrap();
        let server = build_test_app(store);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        let claims = TuiClaims {
            sub: "tui-client".into(),
            iat: now,
            exp: now + 3600,
        };
        let token = sign_jwt(&signing_key, &claims);

        // When a request is made with a valid Bearer token
        let response = server
            .get("/")
            .add_header("Authorization", format!("Bearer {token}"))
            .await;

        // Then the response is 200 OK
        assert_eq!(response.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_expired_token() {
        // Given an app with a known public key
        let (signing_key, verifying_key) = generate_keypair();
        let hex_key = hex::encode(verifying_key.to_bytes());
        let config_str = format!("[[keys]]\nlabel = \"test\"\npublic_key = \"{hex_key}\"\n");
        let config: crate::auth::api_keys::TuiKeysConfig = toml::from_str(&config_str).unwrap();
        let store = TuiKeyStore::load_from_config(&config).unwrap();
        let server = build_test_app(store);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        let claims = TuiClaims {
            sub: "tui-client".into(),
            iat: now - 7200,
            exp: now - 3600,
        };
        let token = sign_jwt(&signing_key, &claims);

        // When a request is made with an expired Bearer token
        let response = server
            .get("/")
            .add_header("Authorization", format!("Bearer {token}"))
            .await;

        // Then the response is 401 Unauthorized
        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rejects_invalid_signature() {
        // Given an app with one public key
        let (_signing_key, verifying_key) = generate_keypair();
        let hex_key = hex::encode(verifying_key.to_bytes());
        let config_str = format!("[[keys]]\nlabel = \"test\"\npublic_key = \"{hex_key}\"\n");
        let config: crate::auth::api_keys::TuiKeysConfig = toml::from_str(&config_str).unwrap();
        let store = TuiKeyStore::load_from_config(&config).unwrap();
        let server = build_test_app(store);

        // When a request is made with a token signed by a different, unknown key
        let (other_signing_key, _) = generate_keypair();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        let claims = TuiClaims {
            sub: "tui-client".into(),
            iat: now,
            exp: now + 3600,
        };
        let token = sign_jwt(&other_signing_key, &claims);

        let response = server
            .get("/")
            .add_header("Authorization", format!("Bearer {token}"))
            .await;

        // Then the response is 401 Unauthorized
        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }
}
