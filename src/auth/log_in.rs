//! This file defines the routes for displaying the log-in page and handling log-in requests.
//! The auth module handles the lower level authentication and cookie auth logic.

use std::sync::{Arc, Mutex};

use axum::{
    Form,
    extract::{FromRef, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::extract::{PrivateCookieJar, cookie::Key};
use axum_htmx::HxRedirect;
use maud::{Markup, html};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::{
    AppState, Error,
    app_state::create_cookie_key,
    auth::{
        DEFAULT_COOKIE_DURATION, User, UserID, get_user_by_id, invalidate_auth_cookie,
        normalize_redirect_url, set_auth_cookie,
    },
    endpoints,
    html::{base, loading_spinner, log_in_register, password_input},
    timezone::get_local_offset,
};

fn log_in_form(password: &str, error_message: Option<&str>, redirect_url: Option<&str>) -> Markup {
    html! {
        form
            hx-post=(endpoints::LOG_IN_API)
            hx-indicator="#indicator"
            hx-disabled-elt="#password, #submit-button"
            class="space-y-4 md:space-y-6"
        {
            @if let Some(redirect_url) = redirect_url {
                input type="hidden" name="redirect_url" value=(redirect_url);
            }

            (password_input(password, 0, error_message))

            div class="flex items-center gap-x-3"
            {
                input
                    type="checkbox"
                    name="remember_me"
                    id="remember_me"
                    tabindex="0"
                    class="rounded-xs";

                label
                    for="remember_me"
                    class="block text-sm font-medium text-gray-900 dark:text-white"
                {
                    "Keep me logged in for one week"
                }
            }

            button
                type="submit" id="submit-button" tabindex="0"
                class="w-full px-4 py-2 bg-blue-500 dark:bg-blue-600 disabled:bg-blue-700
                    hover:enabled:bg-blue-600 hover:enabled:dark:bg-blue-700 text-white rounded"
            {
                span class="inline htmx-indicator" id="indicator"
                {
                    (loading_spinner())
                }
                "Log in"
            }

            p class="text-sm font-light text-gray-500 dark:text-gray-400"
            {
                "Forgot your password? "

                a
                    href=(endpoints::FORGOT_PASSWORD_VIEW) tabindex="0"
                    class="font-semibold leading-6 text-blue-600 hover:text-blue-500 dark:text-blue-500 dark:hover:text-blue-400"
                {
                  "Reset it here"
                }
            }

            p class="text-sm font-light text-gray-500 dark:text-gray-400" {
                "Don't have a password? "
                a
                    href=(endpoints::REGISTER_VIEW) tabindex="0"
                    class="font-semibold leading-6 text-blue-600 hover:text-blue-500 dark:text-blue-500 dark:hover:text-blue-400"
                {
                  "Register here"
                }
            }
        }
    }
}

fn parse_redirect_url(raw_url: Option<&str>, source: &str) -> Option<String> {
    match raw_url.and_then(normalize_redirect_url) {
        Some(redirect_url) => Some(redirect_url),
        None => {
            if let Some(redirect_url) = raw_url {
                tracing::warn!("Invalid redirect URL from {source}: {redirect_url}");
            }
            None
        }
    }
}

/// Display the log-in page.
pub async fn get_log_in_page(Query(query): Query<RedirectQuery>) -> Response {
    let redirect_url = parse_redirect_url(query.redirect_url.as_deref(), "log-in query");
    let log_in_form = log_in_form("", None, redirect_url.as_deref());
    let content = log_in_register("Log in to your account", &log_in_form);
    base("Log In", &[], &content).into_response()
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
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
    pub db_connection: Arc<Mutex<Connection>>,
}

impl LoginState {
    /// Create the cookie key from a string and set the default cookie duration.
    pub fn new(
        cookie_secret: &str,
        local_timezone: &str,
        db_connection: Arc<Mutex<Connection>>,
    ) -> Self {
        Self {
            cookie_key: create_cookie_key(cookie_secret),
            cookie_duration: DEFAULT_COOKIE_DURATION,
            local_timezone: local_timezone.to_owned(),
            db_connection: db_connection.clone(),
        }
    }
}

impl FromRef<AppState> for LoginState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            cookie_key: state.cookie_key.clone(),
            cookie_duration: state.cookie_duration,
            local_timezone: state.local_timezone.clone(),
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

pub const INVALID_CREDENTIALS_ERROR_MSG: &str = "Incorrect password.";

/// Handler for log-in requests via the POST method.
///
/// On a successful log-in request, the auth cookie set and the client is redirected to the dashboard page.
/// Otherwise, the form is returned with an error message explaining the problem.
///
/// # Errors
///
/// This function will return an error in a few situations.
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
    let redirect_url = parse_redirect_url(user_data.redirect_url.as_deref(), "log-in form");
    let redirect_url = redirect_url.as_deref();
    let user: User = match get_user_by_id(
        UserID::new(1),
        &state
            .db_connection
            .lock()
            .expect("Could acquire lock to database connection"),
    ) {
        Ok(user) => user,
        Err(Error::NotFound) => {
            return log_in_form(
                "",
                Some("Password not set, go to the registration page and set your password"),
                redirect_url,
            )
            .into_response();
        }
        Err(error) => {
            tracing::error!("Unhandled error while verifying credentials: {error}");
            return log_in_form(
                "",
                Some("An internal error occurred. Please try again later."),
                redirect_url,
            )
            .into_response();
        }
    };

    let is_password_valid = match user.password_hash.verify(&user_data.password) {
        Ok(is_password_valid) => is_password_valid,
        Err(error) => {
            tracing::error!("Unhandled error while verifying credentials: {error}");
            return log_in_form(
                "",
                Some("An internal error occurred. Please try again later."),
                redirect_url,
            )
            .into_response();
        }
    };

    if !is_password_valid {
        return log_in_form("", Some(INVALID_CREDENTIALS_ERROR_MSG), redirect_url).into_response();
    }

    let cookie_duration = if user_data.remember_me.is_some() {
        REMEMBER_ME_COOKIE_DURATION
    } else {
        state.cookie_duration
    };

    let local_timezone = match get_local_offset(&state.local_timezone) {
        Some(offset) => offset,
        None => return Error::InvalidTimezoneError(state.local_timezone).into_response(),
    };

    let redirect_url = redirect_url.unwrap_or(endpoints::DASHBOARD_VIEW);

    set_auth_cookie(jar.clone(), user.id, cookie_duration, local_timezone)
        .map(|updated_jar| {
            (
                StatusCode::SEE_OTHER,
                HxRedirect(redirect_url.to_owned()),
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

#[derive(Deserialize)]
pub struct RedirectQuery {
    pub redirect_url: Option<String>,
}

/// The raw data entered by the user in the log-in form.
///
/// The password is stored as a plain string. There is no need for validation here since
/// it will be compared against the password in the database, which has been verified.
#[derive(Clone, Serialize, Deserialize)]
pub struct LogInData {
    /// Password entered during log-in.
    pub password: String,

    /// Whether to extend the initial auth cookie duration.
    ///
    /// This value comes from a checkbox, so it either has a string value or is not set
    /// (see the [MDN docs](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/input/checkbox#value_2)).
    /// The `Some` variant should be interpreted as `true` irregardless of the
    /// string value, and the `None` variant should be interpreted as `false`.
    pub remember_me: Option<String>,

    /// Optional URL to redirect to after logging in.
    /// Only accepted from the log-in form submission.
    pub redirect_url: Option<String>,
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
        extract::{Query, State},
        http::{StatusCode, header::CONTENT_TYPE},
    };
    use axum_extra::extract::PrivateCookieJar;
    use rusqlite::Connection;

    use crate::{
        auth::user::create_user_table,
        endpoints,
        test_utils::{assert_valid_html, parse_html_document, parse_html_fragment},
    };

    use super::{LogInData, LoginState, RedirectQuery, User, get_log_in_page, post_log_in};

    #[tokio::test]
    async fn log_in_page_displays_form() {
        let response = get_log_in_page(Query(RedirectQuery { redirect_url: None })).await;

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

        let document = parse_html_document(response).await;
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
        expected_form_elements.insert("input", vec!["password"]);
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
            password: "wrongpassword".to_string(),
            remember_me: None,
            redirect_url: None,
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

        let document = parse_html_fragment(response).await;
        assert_valid_html(&document);

        let form_selector = scraper::Selector::parse("form").unwrap();
        let forms = document.select(&form_selector).collect::<Vec<_>>();
        assert_eq!(forms.len(), 1, "want 1 form, got {}", forms.len());
        let form = forms.first().unwrap();

        assert_password_error_present(form);
    }

    #[tokio::test]
    async fn log_in_page_preserves_redirect_url() {
        let redirect_url = "/transactions?range=month&anchor=2025-10-05".to_string();
        let response = get_log_in_page(Query(RedirectQuery {
            redirect_url: Some(redirect_url.clone()),
        }))
        .await;

        let document = parse_html_document(response).await;
        assert_valid_html(&document);

        let input_selector = scraper::Selector::parse("input[name=redirect_url]").unwrap();
        let inputs = document.select(&input_selector).collect::<Vec<_>>();
        assert_eq!(
            inputs.len(),
            1,
            "want 1 redirect_url input, got {}",
            inputs.len()
        );
        let input = inputs.first().unwrap();
        assert_eq!(
            input.value().attr("value"),
            Some(redirect_url.as_str()),
            "expected redirect_url value to be preserved"
        );
    }

    fn get_test_app_config(test_user: Option<&User>) -> LoginState {
        let connection =
            Connection::open_in_memory().expect("Could not open in-memory SQLite database");
        create_user_table(&connection).expect("Could not create user table");

        if let Some(test_user) = test_user {
            connection
                .execute(
                    "INSERT INTO user (id, password) VALUES (?1, ?2)",
                    (test_user.id.as_u32(), &test_user.password_hash.to_string()),
                )
                .expect("Could not create test user");
        }

        LoginState::new("foobar", "Etc/UTC", Arc::new(Mutex::new(connection)))
    }

    #[track_caller]
    fn assert_password_error_present(form: &scraper::ElementRef<'_>) {
        let error_selector =
            scraper::Selector::parse("input#password + p.text-red-500.text-base").unwrap();
        let error_nodes = form.select(&error_selector).collect::<Vec<_>>();
        assert_eq!(
            error_nodes.len(),
            1,
            "expected 1 password error message, got {}",
            error_nodes.len()
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
        auth::{COOKIE_TOKEN, User, UserID, create_user_table},
        endpoints,
    };

    use super::{
        INVALID_CREDENTIALS_ERROR_MSG, LogInData, LoginState, REMEMBER_ME_COOKIE_DURATION,
        post_log_in,
    };

    #[tokio::test]
    async fn log_in_succeeds_with_valid_credentials() {
        let state = get_test_app_config(Some(&User {
            id: UserID::new(1),
            password_hash: PasswordHash::new(
                ValidatedPassword::new_unchecked("test"),
                PasswordHash::DEFAULT_COST,
            )
            .expect("Could not create test user"),
        }));

        let response = new_log_in_request(
            state,
            LogInData {
                password: "test".to_string(),
                remember_me: None,
                redirect_url: None,
            },
        )
        .await;

        assert_hx_redirect(&response, endpoints::DASHBOARD_VIEW);
        assert_set_cookie(&response);
    }

    #[tokio::test]
    async fn log_in_redirects_to_requested_url() {
        let state = get_test_app_config(Some(&User {
            id: UserID::new(1),
            password_hash: PasswordHash::new(
                ValidatedPassword::new_unchecked("test"),
                PasswordHash::DEFAULT_COST,
            )
            .expect("Could not create test user"),
        }));
        let redirect_url = "/transactions?range=month&anchor=2025-10-05";

        let response = new_log_in_request(
            state,
            LogInData {
                password: "test".to_string(),
                remember_me: None,
                redirect_url: Some(redirect_url.to_string()),
            },
        )
        .await;

        assert_hx_redirect(&response, redirect_url);
    }

    #[tokio::test]
    async fn log_in_falls_back_on_invalid_redirect_url() {
        let state = get_test_app_config(Some(&User {
            id: UserID::new(1),
            password_hash: PasswordHash::new(
                ValidatedPassword::new_unchecked("test"),
                PasswordHash::DEFAULT_COST,
            )
            .expect("Could not create test user"),
        }));

        let response = new_log_in_request(
            state,
            LogInData {
                password: "test".to_string(),
                remember_me: None,
                redirect_url: Some("https://example.com".to_string()),
            },
        )
        .await;

        assert_hx_redirect(&response, endpoints::DASHBOARD_VIEW);
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
        let form = [("password", "test"), ("remember_me", "on")];

        let response = server.post(endpoints::LOG_IN_API).form(&form).await;

        assert_ne!(response.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn remember_me_extends_auth_cookie_through_form() {
        let state = get_test_app_config(Some(&User {
            id: UserID::new(1),
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
        let form = [("password", "test"), ("remember_me", "on")];

        let response = server.post(endpoints::LOG_IN_API).form(&form).await;

        assert_eq!(response.status_code(), StatusCode::SEE_OTHER);

        let token_cookie = response.cookie(COOKIE_TOKEN);
        assert_date_time_close!(
            token_cookie.expires_datetime().unwrap(),
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
        let form = [("password", "test")];

        let response = server.post(endpoints::LOG_IN_API).form(&form).await;

        assert_ne!(response.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn log_in_fails_with_incorrect_password() {
        let state = get_test_app_config(Some(&User {
            id: UserID::new(1),
            password_hash: PasswordHash::new(
                ValidatedPassword::new_unchecked("test"),
                PasswordHash::DEFAULT_COST,
            )
            .expect("Could not create test user"),
        }));

        let response = new_log_in_request(
            state,
            LogInData {
                password: "wrongpassword".to_string(),
                remember_me: None,
                redirect_url: None,
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
                    "INSERT INTO user (id,password) VALUES (?1, ?2)",
                    (test_user.id.as_u32(), &test_user.password_hash.to_string()),
                )
                .expect("Could not create test user");
        }

        LoginState::new("foobar", "Etc/UTC", Arc::new(Mutex::new(connection)))
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
                COOKIE_TOKEN => {
                    assert!(cookie.expires_datetime() > Some(OffsetDateTime::now_utc()));
                    found_cookies.insert(cookie.name().to_string());
                }
                _ => panic!("Unexpected cookie found: {}", cookie.name()),
            }
        }

        assert!(
            found_cookies.contains(COOKIE_TOKEN),
            "could not find cookie '{}' in {:?}",
            COOKIE_TOKEN,
            found_cookies
        );
    }

    async fn assert_body_contains_message(response: Response<Body>, message: &str) {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();
        let fragment = scraper::Html::parse_fragment(&text);
        let error_selector = scraper::Selector::parse("p.text-red-500.text-base").unwrap();
        let error = fragment
            .select(&error_selector)
            .next()
            .expect("expected error message paragraph");
        let error_text = error.text().collect::<String>();
        assert_eq!(
            error_text.trim(),
            message,
            "response body should include error message \"{message}\", got \"{error_text}\""
        );
    }
}
