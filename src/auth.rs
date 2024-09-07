/*! This module defines and implements the data structures, response handlers and functions for authenticating a user and handling cookie auth. */

use std::fmt::Debug;

use axum::{
    body::Body,
    extract::{FromRequestParts, Json, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::{
    cookie::{Cookie, Key, SameSite},
    PrivateCookieJar,
};
use email_address::EmailAddress;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::{Duration, OffsetDateTime};

use crate::{
    config::AppState,
    db::{DbError, SelectBy},
    models::{User, UserID},
    routes::endpoints,
};

/// The raw data entered by the user in the log-in form.
///
/// The email and password are stored as plain strings. There is no need for validation here since
/// they will be compared against the email and password in the database, which have been verified.
#[derive(Clone, Serialize, Deserialize)]
pub struct LogInData {
    /// Email entered during log-in.
    pub email: String,
    /// Password entered during log-in.
    pub password: String,
}

#[derive(Debug)]
pub enum AuthError {
    InvalidCredentials,
    InternalError,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response<Body> {
        let (status, error_message) = match self {
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid credentials"),
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

/// Verify the user `credentials` against the data in the database `connection`.
///
/// # Errors
///
/// This function will return an error in a few situations.
/// - The email does not belong to a registered user.
/// - The password is not correct.
/// - An internal error occurred when verifying the password.
pub fn verify_credentials(
    credentials: LogInData,
    connection: &Connection,
) -> Result<User, AuthError> {
    let email: EmailAddress = credentials
        .email
        .parse()
        .map_err(|_| AuthError::InvalidCredentials)?;

    let user = User::select(&email, connection).map_err(|e| match e {
        DbError::NotFound => AuthError::InvalidCredentials,
        _ => {
            tracing::error!("Error matching user: {e}");
            AuthError::InternalError
        }
    })?;

    let is_password_correct = user
        .password_hash()
        .verify(&credentials.password)
        .map_err(|e| {
            tracing::error!("Error verifying password: {e}");
            AuthError::InternalError
        })?;

    match is_password_correct {
        true => Ok(user),
        false => Err(AuthError::InvalidCredentials),
    }
}

pub(crate) const COOKIE_USER_ID: &str = "user_id";
const COOKIE_DURATION_MINUTES: i64 = 15;

pub(crate) fn set_auth_cookie(jar: PrivateCookieJar, user_id: UserID) -> PrivateCookieJar {
    jar.add(
        Cookie::build((COOKIE_USER_ID, user_id.as_i64().to_string()))
            .expires(OffsetDateTime::now_utc() + Duration::minutes(COOKIE_DURATION_MINUTES))
            .http_only(true)
            .same_site(SameSite::Lax)
            .secure(true),
    )
}

pub(crate) fn get_user_id_from_auth_cookie(jar: PrivateCookieJar) -> Result<UserID, AuthError> {
    match jar.get(COOKIE_USER_ID) {
        None => Err(AuthError::InvalidCredentials),
        Some(user_id_cookie) => user_id_cookie
            .value_trimmed()
            .parse()
            .map(UserID::new)
            .map_err(|_| AuthError::InvalidCredentials),
    }
}

/// Middleware function that checks for a valid authorization cookie.
/// The user ID is placed into request and then the request executed normally if the cookie is valid, otherwise a redirect to the log-in page is returned.
///
/// **Note**: Route handlers can use the function argument `Extension(user_id): Extension<UserID>` to receive the user ID.
///
/// **Note**: The app state must contain an `axum_extra::extract::cookie::Key` for decrypting and verifying the cookie contents.
pub async fn auth_guard(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let (mut parts, body) = request.into_parts();
    let jar: PrivateCookieJar<Key> = PrivateCookieJar::from_request_parts(&mut parts, &state)
        .await
        .expect("could not get cookie jar from request parts");

    match get_user_id_from_auth_cookie(jar) {
        Ok(user_id) => {
            parts.extensions.insert(user_id);
            let request = Request::from_parts(parts, body);

            next.run(request).await
        }
        Err(_) => Redirect::to(endpoints::LOG_IN).into_response(),
    }
}

#[cfg(test)]
mod cookie_tests {
    use axum_extra::extract::{cookie::Key, PrivateCookieJar};
    use sha2::{Digest, Sha512};

    use crate::{
        auth::{get_user_id_from_auth_cookie, COOKIE_USER_ID},
        models::UserID,
    };

    use super::set_auth_cookie;

    fn get_jar() -> PrivateCookieJar {
        let hash = Sha512::digest(b"foobar");
        let key = Key::from(&hash);

        PrivateCookieJar::new(key)
    }

    #[test]
    fn test_set_cookie_succeeds() {
        let jar = get_jar();
        let user_id = UserID::new(1);

        let updated_jar = set_auth_cookie(jar, user_id);
        let user_id_cookie = updated_jar.get(COOKIE_USER_ID).unwrap();

        let retrieved_user_id = UserID::new(user_id_cookie.value_trimmed().parse().unwrap());

        assert_eq!(retrieved_user_id, user_id);
    }

    #[test]
    fn test_get_user_id_from_cookie_succeeds() {
        let user_id = UserID::new(1);
        let jar = set_auth_cookie(get_jar(), user_id);

        let retrieved_user_id = get_user_id_from_auth_cookie(jar).unwrap();

        assert_eq!(retrieved_user_id, user_id);
    }
}

#[cfg(test)]
mod auth_tests {
    use std::str::FromStr;

    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::auth::{verify_credentials, AuthError, LogInData};
    use crate::config::AppState;
    use crate::db::initialize;
    use crate::{
        db::Insert,
        models::{NewUser, PasswordHash, ValidatedPassword},
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "foobar".to_string())
    }

    #[tokio::test]
    async fn log_in_succeeds_with_valid_credentials() {
        let app_state = get_test_app_config();

        let validated_password =
            ValidatedPassword::new("averysafeandsecurepassword".to_string()).unwrap();
        let test_user = NewUser {
            email: EmailAddress::from_str("foo@bar.baz").unwrap(),
            password_hash: PasswordHash::new(validated_password.clone()).unwrap(),
        }
        .insert(&app_state.db_connection().lock().unwrap())
        .unwrap();

        let user_data = LogInData {
            email: test_user.email().to_string(),
            password: validated_password.to_string(),
        };

        assert!(verify_credentials(user_data, &app_state.db_connection().lock().unwrap()).is_ok());
    }

    #[tokio::test]
    async fn log_in_fails_with_invalid_credentials() {
        let app_state = get_test_app_config();
        let user_data = LogInData {
            email: "wrongemail@gmail.com".to_string(),
            password: "definitelyNotTheCorrectPassword".to_string(),
        };

        let result = verify_credentials(user_data, &app_state.db_connection().lock().unwrap());

        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
    }
}

#[cfg(test)]
mod auth_guard_tests {
    use std::str::FromStr;

    use axum::extract::State;
    use axum::routing::post;
    use axum::Form;
    use axum::{middleware, routing::get, Router};
    use axum_extra::extract::cookie::Cookie;
    use axum_extra::{extract::PrivateCookieJar, response::Html};
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::auth::{set_auth_cookie, LogInData};
    use crate::{
        auth::{auth_guard, verify_credentials, COOKIE_USER_ID},
        db::{initialize, Insert},
        models::{NewUser, PasswordHash, ValidatedPassword},
        routes::endpoints,
        AppState,
    };

    use super::AuthError;

    fn get_test_app_state() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "foobar".to_string())
    }

    async fn test_handler() -> Html<&'static str> {
        Html("<h1>Hello, World!</h1>")
    }

    async fn test_log_in_route(
        State(state): State<AppState>,
        jar: PrivateCookieJar,
        Form(user_data): Form<LogInData>,
    ) -> Result<PrivateCookieJar, AuthError> {
        verify_credentials(user_data, &state.db_connection().lock().unwrap())
            .map(|user| Ok(set_auth_cookie(jar, user.id())))?
    }

    #[tokio::test]
    async fn get_protected_route_succeeds_with_valid_cookie() {
        let state = get_test_app_state();

        let validated_password =
            ValidatedPassword::new("averysafeandsecurepassword".to_owned()).unwrap();
        let test_user = NewUser {
            email: EmailAddress::from_str("foo@bar.baz").unwrap(),
            password_hash: PasswordHash::new(validated_password.clone()).unwrap(),
        }
        .insert(&state.db_connection().lock().unwrap())
        .unwrap();

        let app = Router::new()
            .route("/protected", get(test_handler))
            .route_layer(middleware::from_fn_with_state(state.clone(), auth_guard))
            .route(endpoints::LOG_IN, post(test_log_in_route))
            .with_state(state.clone());

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: test_user.email().to_string(),
                password: validated_password.to_string(),
            })
            .await;

        response.assert_status_ok();
        let auth_cookie = response.cookie(COOKIE_USER_ID);

        server
            .get("/protected")
            .add_cookie(auth_cookie)
            .await
            .assert_status_ok();
    }

    #[tokio::test]
    async fn get_protected_route_with_no_auth_cookie_redirects_to_log_in() {
        let state = get_test_app_state();
        let app = Router::new()
            .route("/protected", get(test_handler))
            .route_layer(middleware::from_fn_with_state(state.clone(), auth_guard))
            .with_state(state);

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server.get("/protected").await;

        response.assert_status_see_other();
        assert_eq!(response.header("location"), endpoints::LOG_IN);
    }

    #[tokio::test]
    async fn get_protected_route_with_invalid_auth_cookie_redirects_to_log_in() {
        let state = get_test_app_state();
        let app = Router::new()
            .route("/protected", get(test_handler))
            .route_layer(middleware::from_fn_with_state(state.clone(), auth_guard))
            .with_state(state);

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server
            .get("/protected")
            .add_cookie(Cookie::build((COOKIE_USER_ID, "1")).build())
            .await;

        response.assert_status_see_other();
        assert_eq!(response.header("location"), endpoints::LOG_IN);
    }
}
