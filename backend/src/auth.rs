use std::fmt::Debug;

use axum::{
    async_trait,
    body::Body,
    extract::{FromRef, FromRequestParts, Json, State},
    http::request::Parts,
    http::{Response, StatusCode},
    response::IntoResponse,
    RequestPartsExt,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use chrono::{Duration, Utc};
use common::{Email, RawPassword, User};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    config::AppConfig,
    db::{DbError, SelectBy},
};

// Code in this module is adapted from https://github.com/ezesundayeze/axum--auth and https://github.com/tokio-rs/axum/blob/main/examples/jwt/src/main.rs

/// The contents of a JSON Web Token.
#[derive(Serialize, Deserialize)]
pub struct Claims {
    /// The expiry time of the token.
    pub exp: usize,
    /// The time the token was issued.
    pub iat: usize,
    /// Email associated with the token.
    pub email: Email,
}

#[async_trait]
impl<S> FromRequestParts<S> for Claims
where
    AppConfig: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AuthError::InvalidToken)?;

        let app_config = parts
            .extract_with_state::<AppConfig, _>(state)
            .await
            .map_err(|_| AuthError::InvalidToken)?;

        let token_data = decode_jwt(bearer.token(), app_config.decoding_key())?;

        Ok(token_data.claims)
    }
}

#[derive(Deserialize)]
pub struct Credentials {
    /// Email entered during sign-in.
    pub email: Email,
    /// Password entered during sign-in.
    pub password: RawPassword,
}

#[derive(Debug)]
pub enum AuthError {
    WrongCredentials,
    MissingCredentials,
    TokenCreation,
    InvalidToken,
    InternalError,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response<Body> {
        let (status, error_message) = match self {
            AuthError::WrongCredentials => (StatusCode::UNAUTHORIZED, "Wrong credentials"),
            AuthError::MissingCredentials => (StatusCode::BAD_REQUEST, "Missing credentials"),
            AuthError::TokenCreation => (StatusCode::INTERNAL_SERVER_ERROR, "Token creation error"),
            AuthError::InvalidToken => (StatusCode::BAD_REQUEST, "Invalid token"),
            AuthError::InternalError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

/// Handler for sign-in requests.
///
/// # Errors
///
/// This function will return an error in a few situtations.
/// - The email is empty.
/// - The email does not belong to a registered user.
/// - The password is empty.
/// - The password is not correct.
/// - An internal error occurred when verifying the password.
pub async fn sign_in(
    State(state): State<AppConfig>,
    Json(user_data): Json<Credentials>,
) -> Result<Json<String>, AuthError> {
    let user = User::select(&user_data.email, &state.db_connection().lock().unwrap()).map_err(
        |e| match e {
            DbError::NotFound => AuthError::WrongCredentials,
            _ => {
                tracing::error!("Error matching user: {e:?}");
                AuthError::InternalError
            }
        },
    )?;

    user.password_hash()
        .verify(&user_data.password)
        .map_err(|e| {
            tracing::error!("Error verifying password: {}", e);
            AuthError::InternalError
        })
        .map(|password_is_correct| {
            if password_is_correct {
                let token = encode_jwt(user.email(), state.encoding_key());

                Ok(Json(token))
            } else {
                Err(AuthError::WrongCredentials)
            }
        })?
}

fn encode_jwt(email: &Email, encoding_key: &EncodingKey) -> String {
    let now = Utc::now();
    let exp = (now + Duration::minutes(15)).timestamp() as usize;
    let iat = now.timestamp() as usize;
    let claim = Claims {
        exp,
        iat,
        email: email.to_owned(),
    };

    encode(&Header::default(), &claim, encoding_key).unwrap()
}

fn decode_jwt(jwt_token: &str, decoding_key: &DecodingKey) -> Result<TokenData<Claims>, AuthError> {
    decode(jwt_token, decoding_key, &Validation::default()).map_err(|_| AuthError::InvalidToken)
}

#[cfg(test)]
mod tests {
    use axum::{
        http::StatusCode,
        response::Html,
        routing::{get, post},
        Router,
    };
    use axum_test::TestServer;
    use common::{Email, PasswordHash, RawPassword, User};
    use rusqlite::Connection;
    use serde_json::json;

    use crate::db::initialize;
    use crate::{auth, db::Insert};
    use crate::{config::AppConfig, db::UserData};

    fn get_test_app_config() -> AppConfig {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppConfig::new(db_connection, "foobar".to_string())
    }

    #[test]
    fn jwt_encode_does_not_panic() {
        let email = Email::new("averyemail@email.com").unwrap();
        auth::encode_jwt(&email, get_test_app_config().encoding_key());
    }

    #[test]
    fn decode_jwt_gives_correct_email_address() {
        let config = get_test_app_config();
        let email = Email::new("averyemail@email.com").unwrap();
        let jwt = auth::encode_jwt(&email, config.encoding_key());
        let claims = auth::decode_jwt(&jwt, config.decoding_key())
            .unwrap()
            .claims;

        assert_eq!(email, claims.email);
    }

    #[tokio::test]
    async fn sign_in_succeeds_with_valid_credentials() {
        let app_config = get_test_app_config();

        let raw_password = RawPassword::new("averysafeandsecurepassword".to_string()).unwrap();
        let test_user = User::insert(
            UserData {
                email: Email::new("foo@bar.baz").unwrap(),
                password_hash: PasswordHash::new(raw_password.clone()).unwrap(),
            },
            &app_config.db_connection().lock().unwrap(),
        )
        .unwrap();

        let app = Router::new()
            .route("/sign_in", post(auth::sign_in))
            .with_state(app_config);

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post("/sign_in")
            .content_type("application/json")
            .json(&json!({
                "email": &test_user.email(),
                "password": raw_password,
            }))
            .await
            .assert_status_ok();
    }

    #[tokio::test]
    async fn sign_in_fails_with_missing_credentials() {
        let app = Router::new()
            .route("/signin", post(auth::sign_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post("/signin")
            .content_type("application/json")
            .await
            .assert_status(StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn sign_in_fails_with_invalid_credentials() {
        let app = Router::new()
            .route("/signin", post(auth::sign_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post("/signin")
            .content_type("application/json")
            .json(&json!({
                "email": "wrongemail@gmail.com",
                "password": "definitelyNotTheCorrectPassword",
            }))
            .await
            .assert_status(StatusCode::UNAUTHORIZED);
    }

    async fn handler_with_auth(_: auth::Claims) -> Html<&'static str> {
        Html("<h1>Hello, World!</h1>")
    }

    #[tokio::test]
    async fn get_protected_route_with_valid_jwt() {
        let app_config = get_test_app_config();

        let raw_password = RawPassword::new("averysafeandsecurepassword".to_owned()).unwrap();
        let test_user = User::insert(
            UserData {
                email: Email::new("foo@bar.baz").unwrap(),
                password_hash: PasswordHash::new(raw_password.clone()).unwrap(),
            },
            &app_config.db_connection().lock().unwrap(),
        )
        .unwrap();

        let app = Router::new()
            .route("/signin", post(auth::sign_in))
            .route("/protected", get(handler_with_auth))
            .with_state(app_config);

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server
            .post("/signin")
            .content_type("application/json")
            .json(&json!({
                "email": &test_user.email(),
                "password": raw_password,
            }))
            .await;

        response.assert_status_ok();

        let token = response.json::<String>();

        server
            .get("/protected")
            .authorization_bearer(token)
            .await
            .assert_status_ok();
    }

    #[tokio::test]
    async fn get_protected_route_with_missing_header() {
        let app_config = get_test_app_config();

        let app = Router::new()
            .route("/protected", get(handler_with_auth))
            .with_state(app_config.clone());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .get("/protected")
            .await
            .assert_status(StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn get_protected_route_with_empty_token() {
        let app_config = get_test_app_config();

        let app = Router::new()
            .route("/protected", get(handler_with_auth))
            .with_state(app_config.clone());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .get("/protected")
            .authorization_bearer("")
            .await
            .assert_status(StatusCode::BAD_REQUEST);
    }
}
