/*! This module defines and implements the data structures, response handlers and functions for authenticating a user and handling cookie auth. */

use std::{cmp::max, fmt::Debug, num::ParseIntError};

use axum::{
    body::Body,
    extract::{FromRequestParts, Json, Request, State},
    http::{StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::{
    cookie::{Cookie, Key, SameSite},
    PrivateCookieJar,
};
use axum_htmx::HxRedirect;
use email_address::EmailAddress;
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::{
    format_description::BorrowedFormatItem, macros::format_description, Duration, OffsetDateTime,
};

use crate::{
    models::{User, UserID},
    routes::endpoints,
    state::AppState,
    stores::{CategoryStore, TransactionStore, UserError, UserStore},
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

/// Errors that can occur when authenticating a user.
#[derive(Debug, PartialEq)]
pub enum AuthError {
    /// The user provided an invalid combination of email and password.
    InvalidCredentials,
    /// An unexpected error occurred when hashing a password or parsing a password hash.
    InternalError,
    // TODO: Add doc string
    CookieMissing,
    // TODO: Add doc string
    DateError,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response<Body> {
        let (status, error_message) = match self {
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid credentials"),
            // TODO: Handle cookie missing and date errors separately.
            AuthError::DateError | AuthError::CookieMissing | AuthError::InternalError => {
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
    store: &impl UserStore,
) -> Result<User, AuthError> {
    let email: EmailAddress = credentials
        .email
        .parse()
        .map_err(|_| AuthError::InvalidCredentials)?;

    let user = store.get_by_email(&email).map_err(|e| match e {
        UserError::NotFound => AuthError::InvalidCredentials,
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
pub(crate) const COOKIE_EXPIRY: &str = "expiry";
const COOKIE_DURATION_MINUTES: i64 = 5;

/// Add an auth cookie to the cookie jar, indicating that a user is logged in and authenticated.
///
/// Sets the initial expiry of the cookie to [COOKIE_DURATION_MINUTES] from the current time.
///
/// Returns the cookie jar with the cookie added.
///
/// # Errors
///
/// Returns an error if the expiry time cannot be formatted.
pub(crate) fn set_auth_cookie(
    jar: PrivateCookieJar,
    user_id: UserID,
) -> Result<PrivateCookieJar, time::error::Format> {
    let expiry = OffsetDateTime::now_utc() + Duration::minutes(COOKIE_DURATION_MINUTES);
    // Use format instead of to_string to avoid errors at midnight when the hour is printed as
    // a single digit when [DATE_TIME_FORMAT] expects two digits.
    let expiry_string = expiry.format(DATE_TIME_FORMAT)?;

    Ok(jar
        .add(
            Cookie::build((COOKIE_USER_ID, user_id.as_i64().to_string()))
                .expires(expiry)
                .http_only(true)
                .same_site(SameSite::Strict)
                .secure(true),
        )
        .add(
            Cookie::build((COOKIE_EXPIRY, expiry_string))
                .expires(expiry)
                .http_only(true)
                .same_site(SameSite::Strict)
                .secure(true),
        ))
}

/// Set the auth cookie to an invalid value and set its max age to zero, which should delete the cookie on the client side.
pub(crate) fn invalidate_auth_cookie(jar: PrivateCookieJar) -> PrivateCookieJar {
    jar.add(
        Cookie::build((COOKIE_USER_ID, "deleted"))
            .expires(OffsetDateTime::UNIX_EPOCH)
            .max_age(Duration::ZERO)
            .http_only(true)
            .same_site(SameSite::Strict)
            .secure(true),
    )
}

/// Set the expiry of the auth cookie in `jar` to the latest of UTC now
/// plus `duration` and the cookie's expiry.
///
/// # Errors
///
/// Returns an error if:
/// - The cookie is not in the cookie jar.
/// - Extending the cookie by `duration` would overflow the duration.
pub(crate) fn extend_auth_cookie_duration_if_needed(
    jar: PrivateCookieJar,
    duration: Duration,
) -> Result<PrivateCookieJar, AuthError> {
    let mut auth_cookie = match jar.get(COOKIE_USER_ID) {
        Some(cookie) => cookie,
        None => return Err(AuthError::CookieMissing),
    };

    let mut expiry_cookie = match jar.get(COOKIE_EXPIRY) {
        Some(cookie) => cookie,
        None => return Err(AuthError::CookieMissing),
    };

    println!("{auth_cookie:?}");
    println!("{expiry_cookie:?}");

    let current_expiry = extract_date_time(&expiry_cookie).map_err(|_| AuthError::DateError)?;

    let new_expiry = OffsetDateTime::now_utc()
        .checked_add(duration)
        .ok_or(AuthError::DateError)?;

    let expiry = max(current_expiry, new_expiry);

    let jar = jar
        .remove(auth_cookie.clone())
        .remove(expiry_cookie.clone());

    auth_cookie.set_expires(expiry);
    expiry_cookie.set_expires(expiry);

    Ok(jar.add(auth_cookie).add(expiry_cookie))
}

pub(crate) fn get_user_id_from_auth_cookie(jar: &PrivateCookieJar) -> Result<UserID, AuthError> {
    match jar.get(COOKIE_USER_ID) {
        Some(user_id_cookie) => {
            extract_user_id(&user_id_cookie).map_err(|_| AuthError::InvalidCredentials)
        }
        _ => Err(AuthError::InvalidCredentials),
    }
}

const DATE_TIME_FORMAT: &[BorrowedFormatItem] = format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond] [offset_hour \
         sign:mandatory]:[offset_minute]:[offset_second]"
);

pub(crate) fn extract_date_time(cookie: &Cookie) -> Result<OffsetDateTime, time::error::Parse> {
    OffsetDateTime::parse(cookie.value_trimmed(), DATE_TIME_FORMAT)
}

pub(crate) fn extract_user_id(cookie: &Cookie) -> Result<UserID, ParseIntError> {
    let id: i64 = cookie.value_trimmed().parse()?;

    Ok(UserID::new(id))
}

// TODO: There should be a 'remember me' button on the log in page that sets the initial cookie
// duration to something like a week.

/// Middleware function that checks for a valid authorization cookie.
/// The user ID is placed into request and then the request executed normally if the cookie is valid, otherwise a redirect to the log-in page is returned using `get_redirect`.
///
/// **Note**: Route handlers can use the function argument `Extension(user_id): Extension<UserID>` to receive the user ID.
///
/// **Note**: The app state must contain an `axum_extra::extract::cookie::Key` for decrypting and verifying the cookie contents.
#[inline]
async fn auth_guard_internal<C, T, U>(
    state: AppState<C, T, U>,
    request: Request,
    next: Next,
    get_redirect: fn() -> Response,
) -> Response
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    let (mut parts, body) = request.into_parts();
    let jar: PrivateCookieJar<Key> = PrivateCookieJar::from_request_parts(&mut parts, &state)
        .await
        .expect("could not get cookie jar from request parts");

    match get_user_id_from_auth_cookie(&jar) {
        Ok(user_id) => {
            parts.extensions.insert(user_id);
            let request = Request::from_parts(parts, body);

            let response = next.run(request).await;
            let (mut parts, body) = response.into_parts();

            // TODO: Handle error.
            let jar = extend_auth_cookie_duration_if_needed(jar, Duration::minutes(5)).unwrap();
            let (x, _) = jar.into_response().into_parts();
            for (key, val) in x.headers.iter() {
                parts.headers.insert(key, val.to_owned());
            }

            Response::from_parts(parts, body)
        }
        Err(_) => get_redirect(),
    }
}

/// Middleware function that checks for a valid authorization cookie.
/// The user ID is placed into request and then the request executed normally if the cookie is valid, otherwise a redirect to the log-in page is returned.
///
/// **Note**: Route handlers can use the function argument `Extension(user_id): Extension<UserID>` to receive the user ID.
///
/// **Note**: The app state must contain an `axum_extra::extract::cookie::Key` for decrypting and verifying the cookie contents.
pub async fn auth_guard<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    request: Request,
    next: Next,
) -> Response
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    auth_guard_internal(state, request, next, || {
        Redirect::to(endpoints::LOG_IN).into_response()
    })
    .await
}

/// Middleware function that checks for a valid authorization cookie.
/// The user ID is placed into request and then the request executed normally if the cookie is valid, otherwise a HTMX redirect to the log-in page is returned.
///
/// **Note**: Route handlers can use the function argument `Extension(user_id): Extension<UserID>` to receive the user ID.
///
/// **Note**: The app state must contain an `axum_extra::extract::cookie::Key` for decrypting and verifying the cookie contents.
pub async fn auth_guard_hx<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    request: Request,
    next: Next,
) -> Response
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    auth_guard_internal(state, request, next, || {
        (
            HxRedirect(Uri::from_static(endpoints::LOG_IN)),
            StatusCode::OK,
        )
            .into_response()
    })
    .await
}

#[cfg(test)]
mod cookie_tests {

    use axum_extra::extract::{
        cookie::{Cookie, Key},
        PrivateCookieJar,
    };
    use sha2::{Digest, Sha512};
    use time::{macros::datetime, Duration, OffsetDateTime, UtcOffset};

    use crate::{
        auth::{
            extend_auth_cookie_duration_if_needed, extract_date_time, extract_user_id,
            get_user_id_from_auth_cookie, AuthError, COOKIE_EXPIRY, COOKIE_USER_ID,
            DATE_TIME_FORMAT,
        },
        models::UserID,
    };

    use super::{invalidate_auth_cookie, set_auth_cookie};

    #[test]
    fn can_extract_date_time() {
        let want = OffsetDateTime::now_utc() + Duration::minutes(5);
        let date_time_string = want.format(DATE_TIME_FORMAT).unwrap();
        let cookie = Cookie::build((COOKIE_EXPIRY, date_time_string)).build();

        let got = extract_date_time(&cookie).unwrap();

        assert_eq!(got, want, "got date time {:?}, want {:?}", got, want);
    }

    #[test]
    fn can_extract_date_time_at_midnight() {
        let want = datetime!(2021-01-01 00:00:00).assume_offset(UtcOffset::UTC);
        // Use format instead of to_string to avoid errors at midnight when the hour is printed as
        // a single digit when [DATE_TIME_FORMAT] expects two digits.
        let date_time_string = want.format(DATE_TIME_FORMAT).unwrap();
        let cookie = Cookie::build((COOKIE_EXPIRY, date_time_string)).build();

        let got = extract_date_time(&cookie).unwrap();

        assert_eq!(got, want, "got date time {:?}, want {:?}", got, want);
    }

    #[test]
    fn can_extract_user_id() {
        let user_id = UserID::new(1);
        let cookie = Cookie::build((COOKIE_USER_ID, user_id.as_i64().to_string())).build();

        let got = extract_user_id(&cookie).unwrap();

        assert_eq!(got, user_id);
    }

    fn get_jar() -> PrivateCookieJar {
        let hash = Sha512::digest(b"foobar");
        let key = Key::from(&hash);

        PrivateCookieJar::new(key)
    }

    #[test]
    fn can_set_cookie() {
        let jar = get_jar();
        let user_id = UserID::new(1);

        let jar = set_auth_cookie(jar, user_id).unwrap();
        let user_id_cookie = jar.get(COOKIE_USER_ID).unwrap();
        let expiry_cookie = jar.get(COOKIE_EXPIRY).unwrap();

        let retrieved_user_id = extract_user_id(&user_id_cookie).unwrap();
        let got_expiry = extract_date_time(&expiry_cookie).unwrap();

        assert_eq!(retrieved_user_id, user_id);
        assert_date_time_close(got_expiry, OffsetDateTime::now_utc() + Duration::minutes(5));
    }

    #[test]
    fn get_user_id_from_cookie_succeeds() {
        let user_id = UserID::new(1);
        let jar = set_auth_cookie(get_jar(), user_id).unwrap();

        let retrieved_user_id = get_user_id_from_auth_cookie(&jar).unwrap();

        assert_eq!(retrieved_user_id, user_id);
    }

    #[test]
    fn can_extend_cookie_duration() {
        let jar = get_jar();
        let jar = set_auth_cookie(jar, UserID::new(1)).unwrap();

        let initial_cookie = jar.get(COOKIE_EXPIRY).unwrap();
        let want = extract_date_time(&initial_cookie)
            .unwrap()
            .checked_add(Duration::minutes(5))
            .unwrap();

        let jar = extend_auth_cookie_duration_if_needed(jar, Duration::minutes(10)).unwrap();
        let got_cookie = jar.get(COOKIE_EXPIRY).unwrap();
        let got = extract_date_time(&got_cookie).unwrap();

        assert_date_time_close(got, want);
    }

    #[test]
    fn cookie_duration_does_not_change() {
        let user_id = UserID::new(1);
        let jar = set_auth_cookie(get_jar(), user_id).unwrap();
        let stale_cookie = jar.get(COOKIE_USER_ID).unwrap();
        let want = Some(stale_cookie.expires_datetime().unwrap());

        // The initial cookie is set to expire in 5 minutes, so extending it by 5 seconds should not change the expiry.
        let jar = extend_auth_cookie_duration_if_needed(jar, Duration::seconds(5)).unwrap();

        let cookie = jar.get(COOKIE_USER_ID).unwrap();
        assert_eq!(cookie.expires_datetime(), want);
    }

    fn assert_date_time_close(got: OffsetDateTime, want: OffsetDateTime) {
        assert!(
            got - want < Duration::seconds(1),
            "got date time {:?}, want {:?}",
            got,
            want
        );
    }

    #[test]
    fn invalidate_auth_cookie_succeeds() {
        let user_id = UserID::new(1);
        let jar = set_auth_cookie(get_jar(), user_id).unwrap();

        let jar = invalidate_auth_cookie(jar);
        let cookie = jar.get(COOKIE_USER_ID).unwrap();

        assert_eq!(cookie.value(), "deleted");
        assert_eq!(cookie.expires_datetime(), Some(OffsetDateTime::UNIX_EPOCH));
        assert_eq!(cookie.max_age(), Some(Duration::ZERO));

        assert_eq!(
            get_user_id_from_auth_cookie(&jar),
            Err(AuthError::InvalidCredentials),
        );
    }
}

#[cfg(test)]
mod auth_tests {
    use std::str::FromStr;

    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::auth::{verify_credentials, AuthError, LogInData};
    use crate::models::PasswordHash;
    use crate::stores::sql_store::{create_app_state, SQLAppState};
    use crate::stores::UserStore;

    fn get_test_app_config() -> SQLAppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");

        create_app_state(db_connection, "eaunsnafouts").unwrap()
    }

    #[tokio::test]
    async fn log_in_succeeds_with_valid_credentials() {
        let mut app_state = get_test_app_config();

        let password = "averysafeandsecurepassword".to_string();
        let test_user = app_state
            .user_store()
            .create(
                EmailAddress::from_str("foo@bar.baz").unwrap(),
                PasswordHash::from_raw_password(&password, 4).unwrap(),
            )
            .unwrap();

        let user_data = LogInData {
            email: test_user.email().to_string(),
            password,
        };

        assert!(verify_credentials(user_data, app_state.user_store()).is_ok());
    }

    #[tokio::test]
    async fn log_in_fails_with_invalid_credentials() {
        let mut app_state = get_test_app_config();
        let user_data = LogInData {
            email: "wrongemail@gmail.com".to_string(),
            password: "definitelyNotTheCorrectPassword".to_string(),
        };

        let result = verify_credentials(user_data, app_state.user_store());

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
    use time::{Duration, OffsetDateTime};

    use crate::auth::{set_auth_cookie, LogInData, COOKIE_EXPIRY};
    use crate::stores::sql_store::{create_app_state, SQLAppState};
    use crate::stores::UserStore;
    use crate::{
        auth::{auth_guard, verify_credentials, COOKIE_USER_ID},
        models::PasswordHash,
        routes::endpoints,
    };

    use super::AuthError;

    fn get_test_app_state() -> SQLAppState {
        let conn = Connection::open_in_memory().unwrap();
        create_app_state(conn, "nafstenoas").unwrap()
    }

    async fn test_handler() -> Html<&'static str> {
        Html("<h1>Hello, World!</h1>")
    }

    async fn test_log_in_route(
        State(mut state): State<SQLAppState>,
        jar: PrivateCookieJar,
        Form(user_data): Form<LogInData>,
    ) -> Result<PrivateCookieJar, AuthError> {
        let user = verify_credentials(user_data, state.user_store())?;

        set_auth_cookie(jar, user.id()).map_err(|_| AuthError::DateError)
    }

    #[tokio::test]
    async fn get_protected_route_with_valid_cookie() {
        let mut state = get_test_app_state();

        let password = "averysafeandsecurepassword".to_string();
        let test_user = state
            .user_store()
            .create(
                EmailAddress::from_str("foo@bar.baz").unwrap(),
                PasswordHash::from_raw_password(&password, 4).unwrap(),
            )
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
                password,
            })
            .await;

        response.assert_status_ok();
        let auth_cookie = response.cookie(COOKIE_USER_ID);
        let expiry_cookie = response.cookie(COOKIE_EXPIRY);

        server
            .get("/protected")
            .add_cookie(auth_cookie)
            .add_cookie(expiry_cookie)
            .await
            .assert_status_ok();
    }

    #[tokio::test]
    async fn auth_guard_extends_valid_cookie_duration() {
        let mut state = get_test_app_state();

        let password = "averysafeandsecurepassword".to_string();
        let test_user = state
            .user_store()
            .create(
                EmailAddress::from_str("foo@bar.baz").unwrap(),
                PasswordHash::from_raw_password(&password, 4).unwrap(),
            )
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
                password,
            })
            .await;

        response.assert_status_ok();
        let response_time = OffsetDateTime::now_utc();
        let auth_cookie = response.cookie(COOKIE_USER_ID);
        let expiry_cookie = response.cookie(COOKIE_EXPIRY);

        let response = server
            .get("/protected")
            .add_cookie(auth_cookie)
            .add_cookie(expiry_cookie)
            .await;
        let auth_cookie = response.cookie(COOKIE_USER_ID);

        assert!(auth_cookie.expires_datetime().unwrap() - response_time < Duration::seconds(1));
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

    #[tokio::test]
    async fn get_protected_route_with_expired_auth_cookie_redirects_to_log_in() {
        let mut state = get_test_app_state();

        let password = "averysafeandsecurepassword".to_string();
        let test_user = state
            .user_store()
            .create(
                EmailAddress::from_str("foo@bar.baz").unwrap(),
                PasswordHash::from_raw_password(&password, 4).unwrap(),
            )
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
                password,
            })
            .await;

        response.assert_status_ok();
        let mut auth_cookie = response.cookie(COOKIE_USER_ID);
        auth_cookie.set_expires(OffsetDateTime::UNIX_EPOCH);

        server
            .get("/protected")
            .add_cookie(auth_cookie)
            .await
            .assert_status_see_other();
    }
}
