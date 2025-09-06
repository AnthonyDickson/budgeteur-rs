//! This file defines the routes for displaying the log-in page and handling log-in requests.
//! The auth module handles the lower level authentication and cookie auth logic.

use std::sync::{Arc, Mutex};

use askama::Template;
use axum::{
    Form,
    extract::{FromRef, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::extract::{PrivateCookieJar, cookie::Key};
use axum_htmx::HxRedirect;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::{
    AppState, Error,
    auth_cookie::{DEFAULT_COOKIE_DURATION, invalidate_auth_cookie, set_auth_cookie},
    endpoints,
    shared_templates::{EmailInputTemplate, PasswordInputTemplate, render},
    state::create_cookie_key,
    user::{User, get_user_by_email},
};

/// Renders a log-in form with client-side and server-side validation.
#[derive(Template)]
#[template(path = "partials/log_in/form.html")]
pub struct LogInFormTemplate<'a> {
    pub email_input: EmailInputTemplate<'a>,
    pub password_input: PasswordInputTemplate<'a>,
    pub log_in_route: &'a str,
    pub forgot_password_route: &'a str,
    pub register_route: &'a str,
}

impl Default for LogInFormTemplate<'_> {
    fn default() -> Self {
        Self {
            email_input: Default::default(),
            password_input: Default::default(),
            log_in_route: endpoints::LOG_IN_API,
            forgot_password_route: endpoints::FORGOT_PASSWORD_VIEW,
            register_route: endpoints::REGISTER_VIEW,
        }
    }
}

///  Renders the full log-in page.
#[derive(Template, Default)]
#[template(path = "views/log_in.html")]
struct LogInTemplate<'a> {
    log_in_form: LogInFormTemplate<'a>,
}

/// Display the log-in page.
pub async fn get_log_in_page() -> Response {
    render(StatusCode::OK, LogInTemplate::default())
}

/// How long the auth cookie should last if the user selects "remember me" at log-in.
const REMEMBER_ME_COOKIE_DURATION: Duration = Duration::days(7);

/// The state needed to perform a login.
#[derive(Debug, Clone)]
pub struct LoginState {
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,
    /// The duration for which cookies used for authentication are valid.
    pub cookie_duration: Duration,
    pub db_connection: Arc<Mutex<Connection>>,
}

impl LoginState {
    /// Create the cookie key from a string and set the default cookie duration.
    pub fn new(cookie_secret: &str, db_connection: Arc<Mutex<Connection>>) -> Self {
        Self {
            cookie_key: create_cookie_key(cookie_secret),
            cookie_duration: DEFAULT_COOKIE_DURATION,
            db_connection: db_connection.clone(),
        }
    }
}

impl FromRef<AppState> for LoginState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            cookie_key: state.cookie_key.clone(),
            cookie_duration: state.cookie_duration,
            db_connection: state.db_connection.clone(),
        }
    }
}

// this impl tells `PrivateCookieJar` how to access the key from our state
impl FromRef<LoginState> for Key {
    fn from_ref(state: &LoginState) -> Self {
        state.cookie_key.clone()
    }
}

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
pub async fn post_log_in(
    State(state): State<LoginState>,
    jar: PrivateCookieJar,
    Form(user_data): Form<LogInData>,
) -> Response {
    let email = &user_data.email;
    let user: User = match get_user_by_email(
        email,
        &state
            .db_connection
            .lock()
            .expect("Could acquire lock to database connection"),
    ) {
        Ok(user) => user,
        Err(Error::NotFound) => {
            return render(
                StatusCode::OK,
                create_log_in_error_response(email, INVALID_CREDENTIALS_ERROR_MSG),
            );
        }
        Err(error) => {
            tracing::error!("Unhandled error while verifying credentials: {error}");
            return render(
                StatusCode::OK,
                create_log_in_error_response(
                    email,
                    "An internal error occurred. Please try again later.",
                ),
            );
        }
    };

    let is_password_valid = match user.password_hash.verify(&user_data.password) {
        Ok(is_password_valid) => is_password_valid,
        Err(error) => {
            tracing::error!("Unhandled error while verifying credentials: {error}");
            return render(
                StatusCode::OK,
                create_log_in_error_response(
                    email,
                    "An internal error occurred. Please try again later.",
                ),
            );
        }
    };

    if !is_password_valid {
        return render(
            StatusCode::OK,
            create_log_in_error_response(email, INVALID_CREDENTIALS_ERROR_MSG),
        );
    }

    let cookie_duration = if user_data.remember_me.is_some() {
        REMEMBER_ME_COOKIE_DURATION
    } else {
        state.cookie_duration
    };

    set_auth_cookie(jar.clone(), user.id, cookie_duration)
        .map(|updated_jar| {
            (
                StatusCode::SEE_OTHER,
                HxRedirect(endpoints::DASHBOARD_VIEW.to_owned()),
                updated_jar,
            )
        })
        .map_err(|err| {
            tracing::error!("Error setting auth cookie: {err}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                HxRedirect(endpoints::INTERNAL_ERROR_VIEW.to_owned()),
                invalidate_auth_cookie(jar),
            )
        })
        .into_response()
}

fn create_log_in_error_response<'a>(
    email_input: &'a str,
    error_message: &'a str,
) -> LogInFormTemplate<'a> {
    LogInFormTemplate {
        email_input: EmailInputTemplate {
            value: email_input,
            error_message: "",
        },
        password_input: PasswordInputTemplate {
            value: "",
            min_length: 0,
            error_message,
        },
        ..Default::default()
    }
}

pub const INVALID_CREDENTIALS_ERROR_MSG: &str = "Incorrect email or password.";

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
    /// Whether to extend the initial auth cookie duration.
    ///
    /// This value comes from a checkbox, so it either has a string value or is not set
    /// (see the [MDN docs](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/input/checkbox#value_2)).
    /// The `Some` variant should be interpreted as `true` irregardless of the
    /// string value, and the `None` variant should be interpreted as `false`.
    pub remember_me: Option<String>,
}

#[cfg(test)]
mod log_in_page_tests {
    use std::{
        collections::HashMap,
        iter::zip,
        sync::{Arc, Mutex},
    };

    use axum::{
        Form,
        extract::State,
        http::{StatusCode, header::CONTENT_TYPE},
    };
    use axum_extra::extract::PrivateCookieJar;
    use rusqlite::Connection;
    use scraper::Html;

    use crate::{endpoints, user::create_user_table};

    use super::{
        INVALID_CREDENTIALS_ERROR_MSG, LogInData, LoginState, User, get_log_in_page, post_log_in,
    };

    #[tokio::test]
    async fn log_in_page_displays_form() {
        let response = get_log_in_page().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );

        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();
        let document = scraper::Html::parse_document(&text);
        assert_valid_html(&document);

        let form_selector = scraper::Selector::parse("form").unwrap();
        let forms = document.select(&form_selector).collect::<Vec<_>>();
        assert_eq!(forms.len(), 1, "want 1 form, got {}", forms.len());
        let form = forms.first().unwrap();
        let hx_post = form.value().attr("hx-post");
        assert_eq!(
            hx_post,
            Some(endpoints::LOG_IN_API),
            "want form with attribute hx-post=\"{}\", got {:?}",
            endpoints::LOG_IN_API,
            hx_post
        );

        let mut expected_form_elements: HashMap<&str, Vec<&str>> = HashMap::new();
        expected_form_elements.insert("input", vec!["email", "password"]);
        expected_form_elements.insert("button", vec!["submit"]);

        for (tag, element_types) in expected_form_elements {
            for element_type in element_types {
                let selector_string = format!("{tag}[type={element_type}]");
                let input_selector = scraper::Selector::parse(&selector_string).unwrap();
                let inputs = form.select(&input_selector).collect::<Vec<_>>();
                assert_eq!(
                    inputs.len(),
                    1,
                    "want 1 {element_type} {tag}, got {}",
                    inputs.len()
                );
            }
        }

        let register_link_selector = scraper::Selector::parse("a[href]").unwrap();
        let links = form.select(&register_link_selector).collect::<Vec<_>>();
        assert_eq!(links.len(), 2, "want 2 link, got {}", links.len());
        let want_endpoints = [endpoints::FORGOT_PASSWORD_VIEW, endpoints::REGISTER_VIEW];

        for (link, endpoint) in zip(links, want_endpoints) {
            assert_eq!(
                link.value().attr("href"),
                Some(endpoint),
                "want link to {}, got {:?}",
                endpoint,
                link.value().attr("href")
            );
        }
    }

    #[tokio::test]
    async fn log_in_page_displays_error_message() {
        let state = get_test_app_config(None);
        let jar = PrivateCookieJar::new(state.cookie_key.clone());
        let form = LogInData {
            email: "foo@bar.baz".to_string(),
            password: "wrongpassword".to_string(),
            remember_me: None,
        };
        let response = post_log_in(State(state), jar, Form(form)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );

        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();
        let document = scraper::Html::parse_fragment(&text);
        assert_valid_html(&document);

        let form_selector = scraper::Selector::parse("form").unwrap();
        let forms = document.select(&form_selector).collect::<Vec<_>>();
        assert_eq!(forms.len(), 1, "want 1 form, got {}", forms.len());
        let form = forms.first().unwrap();

        let p_selector = scraper::Selector::parse("p").unwrap();
        let p = form.select(&p_selector).collect::<Vec<_>>();
        let p = p.first();

        assert!(
            p.is_some(),
            "could not find p tag for error messsage in form"
        );

        let p = p.unwrap();

        let p_text = p.text().collect::<String>();
        assert!(
            p_text.contains(INVALID_CREDENTIALS_ERROR_MSG),
            "error message should contain string \"{INVALID_CREDENTIALS_ERROR_MSG}\" but got {p_text}"
        );
    }

    fn get_test_app_config(test_user: Option<&User>) -> LoginState {
        let connection =
            Connection::open_in_memory().expect("Could not open in-memory SQLite database");
        create_user_table(&connection).expect("Could not create user table");

        if let Some(test_user) = test_user {
            connection
                .execute(
                    "INSERT INTO user (id, email, password) VALUES (?1, ?2, ?3)",
                    (
                        test_user.id.as_i64(),
                        test_user.email.as_str(),
                        &test_user.password_hash.to_string(),
                    ),
                )
                .expect("Could not create test user");
        }

        LoginState::new("foobar", Arc::new(Mutex::new(connection)))
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors: {:?}\n{}",
            html.errors,
            html.html()
        );
    }
}

#[cfg(test)]
mod log_in_tests {
    use std::{
        collections::HashSet,
        sync::{Arc, Mutex},
    };

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

    use rusqlite::Connection;
    use time::{Duration, OffsetDateTime};

    use crate::{
        PasswordHash, ValidatedPassword,
        auth_cookie::{COOKIE_EXPIRY, COOKIE_USER_ID},
        endpoints,
        user::{User, UserID, create_user_table},
    };

    use super::{
        INVALID_CREDENTIALS_ERROR_MSG, LogInData, LoginState, REMEMBER_ME_COOKIE_DURATION,
        post_log_in,
    };

    #[tokio::test]
    async fn log_in_succeeds_with_valid_credentials() {
        let state = get_test_app_config(Some(&User {
            id: UserID::new(1),
            email: "test@test.com".parse().expect("Could not parse test email"),
            password_hash: PasswordHash::new(
                ValidatedPassword::new_unchecked("test"),
                PasswordHash::DEFAULT_COST,
            )
            .expect("Could not create test user"),
        }));

        let response = new_log_in_request(
            state,
            LogInData {
                email: "test@test.com".to_string(),
                password: "test".to_string(),
                remember_me: None,
            },
        )
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
        let state = get_test_app_config(None);
        let app = Router::new()
            .route(endpoints::LOG_IN_API, post(post_log_in))
            .with_state(state);

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::LOG_IN_API)
            .content_type("application/x-www-form-urlencoded")
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn form_deserialises() {
        let state = get_test_app_config(None);
        let app = Router::new()
            .route(endpoints::LOG_IN_API, post(post_log_in))
            .with_state(state);
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
        let state = get_test_app_config(Some(&User {
            id: UserID::new(1),
            email: "test@test.com".parse().expect("Could not parse test email"),
            password_hash: PasswordHash::new(
                ValidatedPassword::new_unchecked("test"),
                PasswordHash::DEFAULT_COST,
            )
            .expect("Could not create test user"),
        }));
        let app = Router::new()
            .route(endpoints::LOG_IN_API, post(post_log_in))
            .with_state(state);
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
        let state = get_test_app_config(None);
        let app = Router::new()
            .route(endpoints::LOG_IN_API, post(post_log_in))
            .with_state(state);
        let server = TestServer::new(app).expect("Could not create test server.");
        let form = [("email", "test@test.com"), ("password", "test")];

        let response = server.post(endpoints::LOG_IN_API).form(&form).await;

        assert_ne!(response.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn log_in_fails_with_incorrect_email() {
        let state = get_test_app_config(None);

        let response = new_log_in_request(
            state,
            LogInData {
                email: "wrong@email.com".to_string(),
                password: "test".to_string(),
                remember_me: None,
            },
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_body_contains_message(response, INVALID_CREDENTIALS_ERROR_MSG).await;
    }

    #[tokio::test]
    async fn log_in_fails_with_incorrect_password() {
        let state = get_test_app_config(Some(&User {
            id: UserID::new(1),
            email: "test@test.com".parse().expect("Could not parse test email"),
            password_hash: PasswordHash::new(
                ValidatedPassword::new_unchecked("test"),
                PasswordHash::DEFAULT_COST,
            )
            .expect("Could not create test user"),
        }));

        let response = new_log_in_request(
            state,
            LogInData {
                email: "test@test.com".to_string(),
                password: "wrongpassword".to_string(),
                remember_me: None,
            },
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_body_contains_message(response, INVALID_CREDENTIALS_ERROR_MSG).await;
    }

    fn get_test_app_config(test_user: Option<&User>) -> LoginState {
        let connection =
            Connection::open_in_memory().expect("Could not open in-memory SQLite database");

        create_user_table(&connection).expect("Could not create user table");

        if let Some(test_user) = test_user {
            connection
                .execute(
                    "INSERT INTO user (id, email, password) VALUES (?1, ?2, ?3)",
                    (
                        test_user.id.as_i64(),
                        test_user.email.as_str(),
                        &test_user.password_hash.to_string(),
                    ),
                )
                .expect("Could not create test user");
        }

        LoginState::new("foobar", Arc::new(Mutex::new(connection)))
    }

    async fn new_log_in_request(state: LoginState, log_in_form: LogInData) -> Response<Body> {
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
