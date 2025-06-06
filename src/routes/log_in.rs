//! This file defines the high-level log-in route logic.
//! The auth module handles the lower level authentication and cookie auth logic.

use axum::{
    Form,
    extract::State,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use axum_extra::extract::PrivateCookieJar;
use axum_htmx::HxRedirect;
use time::Duration;

use crate::{
    Error,
    auth::{
        cookie::{invalidate_auth_cookie, set_auth_cookie},
        log_in::{LogInData, verify_credentials},
    },
    state::LoginState,
    stores::UserStore,
};

use super::{
    endpoints,
    templates::{EmailInputTemplate, LogInFormTemplate, PasswordInputTemplate},
};

/// How long the auth cookie should last if the user selects "remember me" at log-in.
pub const REMEMBER_ME_COOKIE_DURATION: Duration = Duration::days(7);

/// Handler for log-in requests via the POST method.
///
/// On a successful log-in request, the auth cookie set and the client is redirected to the dashboard page.
/// Otherwise, the form is returned with an error message explaining the problem.
///
/// # Errors
///
/// This function will return an error in a few situations.
/// - The email does not belong to a registered user.
/// - The password is not correct.
/// - An internal error occurred when verifying the password.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn post_log_in<U>(
    State(state): State<LoginState<U>>,
    jar: PrivateCookieJar,
    Form(user_data): Form<LogInData>,
) -> Response
where
    U: UserStore + Clone + Send + Sync,
{
    verify_credentials(user_data.clone(), &state.user_store)
        .map(|user| {
            let cookie_duration = if user_data.remember_me.is_some() {
                REMEMBER_ME_COOKIE_DURATION
            } else {
                state.cookie_duration
            };

            set_auth_cookie(jar.clone(), user.id(), cookie_duration)
                .map(|updated_jar| {
                    (
                        StatusCode::SEE_OTHER,
                        HxRedirect(Uri::from_static(endpoints::DASHBOARD_VIEW)),
                        updated_jar,
                    )
                })
                .map_err(|err| {
                    tracing::error!("Error setting auth cookie: {err}");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        HxRedirect(Uri::from_static(endpoints::INTERNAL_ERROR_VIEW)),
                        invalidate_auth_cookie(jar),
                    )
                })
        })
        .map_err(|e| LogInFormTemplate {
            email_input: EmailInputTemplate {
                value: &user_data.email,
                error_message: "",
            },
            password_input: PasswordInputTemplate {
                value: "",
                min_length: 0,
                error_message: match e {
                    Error::InvalidCredentials => INVALID_CREDENTIALS_ERROR_MSG,
                    error => {
                        tracing::error!("Unhandled error while verifying credentials: {error}");
                        "An internal error occurred. Please try again later."
                    }
                },
            },
            ..Default::default()
        })
        .into_response()
}

pub const INVALID_CREDENTIALS_ERROR_MSG: &str = "Incorrect email or password.";

#[cfg(test)]
mod log_in_tests {
    use std::collections::HashSet;

    use axum::{
        Form, Router,
        body::Body,
        extract::State,
        http::{Response, StatusCode, header::SET_COOKIE},
        routing::post,
    };
    use axum_extra::extract::{PrivateCookieJar, cookie::Cookie};
    use axum_htmx::HX_REDIRECT;
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use time::{Duration, OffsetDateTime};

    use crate::{
        Error,
        auth::{
            cookie::{COOKIE_EXPIRY, COOKIE_USER_ID},
            log_in::LogInData,
        },
        models::{PasswordHash, User, UserID, ValidatedPassword},
        routes::{
            endpoints,
            log_in::{INVALID_CREDENTIALS_ERROR_MSG, REMEMBER_ME_COOKIE_DURATION, post_log_in},
        },
        state::LoginState,
        stores::UserStore,
    };

    #[derive(Clone)]
    struct StubUserStore {
        users: Vec<User>,
    }

    impl UserStore for StubUserStore {
        fn create(
            &mut self,
            email: email_address::EmailAddress,
            password_hash: PasswordHash,
        ) -> Result<User, Error> {
            let next_id = match self.users.last() {
                Some(user) => UserID::new(user.id().as_i64() + 1),
                _ => UserID::new(0),
            };

            let user = User::new(next_id, email, password_hash);
            self.users.push(user.clone());

            Ok(user)
        }

        fn get(&self, id: UserID) -> Result<User, Error> {
            self.users
                .iter()
                .find(|user| user.id() == id)
                .ok_or(Error::NotFound)
                .map(|user| user.to_owned())
        }

        fn get_by_email(&self, email: &email_address::EmailAddress) -> Result<User, Error> {
            self.users
                .iter()
                .find(|user| user.email() == email)
                .ok_or(Error::NotFound)
                .map(|user| user.to_owned())
        }

        fn count(&self) -> Result<usize, Error> {
            todo!()
        }
    }

    #[tokio::test]
    async fn log_in_succeeds_with_valid_credentials() {
        let response = new_log_in_request(LogInData {
            email: "test@test.com".to_string(),
            password: "test".to_string(),
            remember_me: None,
        })
        .await;

        assert_hx_redirect(&response, endpoints::DASHBOARD_VIEW);
        assert_set_cookie(&response);
    }

    /// Test helper macro to assert that two date times are within one second
    /// of each other. Used instead of a function so that the file and line
    /// number of the caller is included in the error message instead of the
    /// helper.
    macro_rules! assert_date_time_close {
        ($left:expr, $right:expr$(,)?) => {
            assert!(
                ($left - $right).abs() < Duration::seconds(2),
                "got date time {:?}, want {:?}",
                $left,
                $right
            );
        };
    }

    #[tokio::test]
    async fn log_in_fails_with_missing_credentials() {
        let app = Router::new()
            .route(endpoints::LOG_IN_API, post(post_log_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::LOG_IN_API)
            .content_type("application/x-www-form-urlencoded")
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn form_deserialises() {
        let app = Router::new()
            .route(endpoints::LOG_IN_API, post(post_log_in))
            .with_state(get_test_app_config());
        let server = TestServer::new(app).expect("Could not create test server.");
        let form = [
            ("email", "test@test.com"),
            ("password", "test"),
            ("remember_me", "on"),
        ];

        let response = server.post(endpoints::LOG_IN_API).form(&form).await;

        assert_ne!(response.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn remember_me_extends_auth_cookie_through_form() {
        let app = Router::new()
            .route(endpoints::LOG_IN_API, post(post_log_in))
            .with_state(get_test_app_config());
        let server = TestServer::new(app).expect("Could not create test server.");
        let form = [
            ("email", "test@test.com"),
            ("password", "test"),
            ("remember_me", "on"),
        ];

        let response = server.post(endpoints::LOG_IN_API).form(&form).await;

        assert_eq!(response.status_code(), StatusCode::SEE_OTHER);

        let auth_cookie = response.cookie(COOKIE_USER_ID);
        assert_date_time_close!(
            auth_cookie.expires_datetime().unwrap(),
            OffsetDateTime::now_utc() + REMEMBER_ME_COOKIE_DURATION
        );
    }

    #[tokio::test]
    async fn form_deserialises_without_remember_me() {
        let app = Router::new()
            .route(endpoints::LOG_IN_API, post(post_log_in))
            .with_state(get_test_app_config());
        let server = TestServer::new(app).expect("Could not create test server.");
        let form = [("email", "test@test.com"), ("password", "test")];

        let response = server.post(endpoints::LOG_IN_API).form(&form).await;

        assert_ne!(response.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn log_in_fails_with_incorrect_email() {
        let response = new_log_in_request(LogInData {
            email: "wrong@email.com".to_string(),
            password: "test".to_string(),
            remember_me: None,
        })
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_body_contains_message(response, INVALID_CREDENTIALS_ERROR_MSG).await;
    }

    #[tokio::test]
    async fn log_in_fails_with_incorrect_password() {
        let response = new_log_in_request(LogInData {
            email: "test@test.com".to_string(),
            password: "wrongpassword".to_string(),
            remember_me: None,
        })
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_body_contains_message(response, INVALID_CREDENTIALS_ERROR_MSG).await;
    }

    fn get_test_app_config() -> LoginState<StubUserStore> {
        let mut state = LoginState::new("foobar", StubUserStore { users: vec![] });

        state
            .user_store
            .create(
                EmailAddress::new_unchecked("test@test.com"),
                PasswordHash::new(ValidatedPassword::new_unchecked("test"), 4).unwrap(),
            )
            .unwrap();

        state
    }

    async fn new_log_in_request(log_in_form: LogInData) -> Response<Body> {
        let state = get_test_app_config();
        let jar = PrivateCookieJar::new(state.cookie_key.clone());

        post_log_in(State(state), jar, Form(log_in_form)).await
    }

    #[track_caller]
    fn assert_hx_redirect(response: &Response<Body>, want_location: &str) {
        let redirect_location = response.headers().get(HX_REDIRECT).unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(redirect_location, want_location);
    }

    #[track_caller]
    fn assert_set_cookie(response: &Response<Body>) {
        let mut found_cookies = HashSet::new();

        for cookie_headers in response.headers().get_all(SET_COOKIE) {
            let cookie_string = cookie_headers.to_str().unwrap();
            let cookie = Cookie::parse(cookie_string).unwrap();

            match cookie.name() {
                COOKIE_USER_ID | COOKIE_EXPIRY => {
                    assert!(cookie.expires_datetime() > Some(OffsetDateTime::now_utc()));
                    found_cookies.insert(cookie.name().to_string());
                }
                _ => panic!("Unexpected cookie found: {}", cookie.name()),
            }
        }

        assert!(
            found_cookies.contains(COOKIE_USER_ID),
            "could not find cookie '{}' in {:?}",
            COOKIE_USER_ID,
            found_cookies
        );

        assert!(
            found_cookies.contains(COOKIE_EXPIRY),
            "could not find cookie '{}' in {:?}",
            COOKIE_EXPIRY,
            found_cookies
        );
    }

    async fn assert_body_contains_message(response: Response<Body>, message: &str) {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        let text = String::from_utf8_lossy(&body).to_string();

        assert!(
            text.contains(message),
            "response body should contain the text '{}' but got {}",
            message,
            text
        );
    }
}
