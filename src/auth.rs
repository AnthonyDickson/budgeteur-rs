/*! This module defines and implements the data structures, response handlers and functions for authenticating a user and generating JWTs (JSON Web Token). */

use std::fmt::Debug;

use axum::{
    body::Body,
    extract::{Json, State},
    http::{Response, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    PrivateCookieJar,
};
use email_address::EmailAddress;
use serde::Deserialize;
use serde_json::json;
use time::{Duration, OffsetDateTime};

use crate::{
    config::AppState,
    db::{DbError, SelectBy},
    model::{RawPassword, User, UserID},
};

#[derive(Deserialize)]
pub struct Credentials {
    /// Email entered during sign-in.
    pub email: EmailAddress,
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
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Json(user_data): Json<Credentials>,
) -> Result<PrivateCookieJar, AuthError> {
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
                let updated_jar = set_auth_cookie(jar, user.id());

                Ok(updated_jar)
            } else {
                Err(AuthError::WrongCredentials)
            }
        })?
}

pub(crate) const COOKIE_USER_ID: &str = "user_id";
const COOKIE_DURATION: i64 = 15;

fn set_auth_cookie(jar: PrivateCookieJar, user_id: UserID) -> PrivateCookieJar {
    jar.add(
        Cookie::build((COOKIE_USER_ID, user_id.as_i64().to_string()))
            .expires(OffsetDateTime::now_utc() + Duration::minutes(COOKIE_DURATION))
            .http_only(true)
            .same_site(SameSite::Lax)
            .secure(true),
    )
}

pub fn get_user_id_from_auth_cookie(jar: PrivateCookieJar) -> Result<UserID, AuthError> {
    match jar.get(COOKIE_USER_ID) {
        Some(user_id_cookie) => user_id_cookie
            .value_trimmed()
            .parse()
            .map(UserID::new)
            .map_err(|_| AuthError::InvalidToken),
        None => Err(AuthError::InvalidToken),
    }
}

#[cfg(test)]
mod cookie_tests {
    use axum_extra::extract::{cookie::Key, PrivateCookieJar};
    use sha2::{Digest, Sha512};

    use crate::{
        auth::{get_user_id_from_auth_cookie, COOKIE_USER_ID},
        model::UserID,
    };

    use super::set_auth_cookie;

    fn get_key() -> Key {
        let hash = Sha512::digest(b"foobar");

        Key::from(&hash)
    }

    #[test]
    fn test_set_cookie_succeeds() {
        let key = get_key();
        let jar = PrivateCookieJar::new(key);
        let user_id = UserID::new(1);

        let updated_jar = set_auth_cookie(jar, user_id);
        let user_id_cookie = updated_jar.get(COOKIE_USER_ID).unwrap();

        let retrieved_user_id = UserID::new(user_id_cookie.value_trimmed().parse().unwrap());

        assert_eq!(retrieved_user_id, user_id);
    }

    #[test]
    fn test_get_user_id_from_cookie_succeeds() {
        let key = get_key();
        let user_id = UserID::new(1);
        let jar = set_auth_cookie(PrivateCookieJar::new(key), user_id);

        let retrieved_user_id = get_user_id_from_auth_cookie(jar).unwrap();

        assert_eq!(retrieved_user_id, user_id);
    }
}

#[cfg(test)]
mod auth_tests {
    use std::str::FromStr;

    use axum::{
        http::StatusCode,
        response::Html,
        routing::{get, post},
        Router,
    };
    use axum_extra::extract::PrivateCookieJar;
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use rusqlite::Connection;
    use serde_json::json;

    use crate::db::initialize;
    use crate::{
        auth,
        db::Insert,
        model::{NewUser, PasswordHash, RawPassword},
    };
    use crate::{auth::COOKIE_USER_ID, config::AppState};

    use super::{get_user_id_from_auth_cookie, AuthError};

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "foobar".to_string())
    }

    #[tokio::test]
    async fn sign_in_succeeds_with_valid_credentials() {
        let app_config = get_test_app_config();

        let raw_password = RawPassword::new("averysafeandsecurepassword".to_string()).unwrap();
        let test_user = NewUser {
            email: EmailAddress::from_str("foo@bar.baz").unwrap(),
            password_hash: PasswordHash::new(raw_password.clone()).unwrap(),
        }
        .insert(&app_config.db_connection().lock().unwrap())
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
            .route("/sign_in", post(auth::sign_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post("/sign_in")
            .content_type("application/json")
            .await
            .assert_status(StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn sign_in_fails_with_invalid_credentials() {
        let app = Router::new()
            .route("/sign_in", post(auth::sign_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post("/sign_in")
            .content_type("application/json")
            .json(&json!({
                "email": "wrongemail@gmail.com",
                "password": "definitelyNotTheCorrectPassword",
            }))
            .await
            .assert_status(StatusCode::UNAUTHORIZED);
    }

    async fn handler_with_auth(jar: PrivateCookieJar) -> Result<Html<&'static str>, AuthError> {
        get_user_id_from_auth_cookie(jar)?;

        Ok(Html("<h1>Hello, World!</h1>"))
    }

    #[tokio::test]
    async fn get_protected_route_succeeds_with_valid_cookie() {
        let app_config = get_test_app_config();

        let raw_password = RawPassword::new("averysafeandsecurepassword".to_owned()).unwrap();
        let test_user = NewUser {
            email: EmailAddress::from_str("foo@bar.baz").unwrap(),
            password_hash: PasswordHash::new(raw_password.clone()).unwrap(),
        }
        .insert(&app_config.db_connection().lock().unwrap())
        .unwrap();

        let app = Router::new()
            .route("/sign_in", post(auth::sign_in))
            .route("/protected", get(handler_with_auth))
            .with_state(app_config);

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server
            .post("/sign_in")
            .content_type("application/json")
            .json(&json!({
                "email": &test_user.email(),
                "password": raw_password,
            }))
            .await;

        response.assert_status_ok();

        let auth_cookie = response.maybe_cookie(COOKIE_USER_ID).unwrap();

        server
            .get("/protected")
            .add_cookie(auth_cookie)
            .await
            .assert_status_ok();
    }

    #[tokio::test]
    async fn get_protected_route_fails_with_no_auth_cookie() {
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
}
