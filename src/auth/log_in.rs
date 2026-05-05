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
use kameo::actor::ActorRef;
use maud::{Markup, html};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcDateTime};

use crate::{
    AppState, Error,
    app_state::create_cookie_key,
    auth::{
        SessionStore, User, UserID, get_user_by_id, invalidate_auth_cookie, normalize_redirect_url,
        session::{MAX_SESSION_AGE, Session, Set},
        set_auth_cookie,
    },
    endpoints,
    html::{BUTTON_PRIMARY_STYLE, base, loading_spinner, password_input},
};

fn contianer(form_title: &str, form: &Markup) -> Markup {
    html! {
        div class="flex flex-col items-center justify-center px-6 py-8 mx-auto"
        {
            a href="#" class="flex items-center mb-6 text-2xl font-semibold text-gray-900 dark:text-white"
            {
                img class="w-8 h-8 mr-2" src="/static/favicon-128x128.png" alt="logo";
                "Budgeteur"
            }

            div class="w-full bg-white rounded shadow dark:border md:mt-0 sm:max-w-md xl:p-0 dark:bg-gray-800 dark:border-gray-700"
            {
                div class="p-6 space-y-4 md:space-y-6 sm:p-8"
                {
                    h1 class="text-xl font-bold leading-tight tracking-tight text-gray-900 md:text-2xl dark:text-white"
                    {
                        (form_title)
                    }

                    (form)
                }
            }
        }
    }
}

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

            (password_input(password, 0, error_message, true))

            button
                type="submit" id="submit-button" tabindex="0"
                class=(BUTTON_PRIMARY_STYLE)
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
    let content = contianer("Log in to your account", &log_in_form);
    base("Log In", &[], &content).into_response()
}

/// The state needed to perform a login.
#[derive(Debug, Clone)]
pub struct LoginState {
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,
    pub db_connection: Arc<Mutex<Connection>>,
    pub session_actor: ActorRef<SessionStore>,
}

impl LoginState {
    /// Create the cookie key from a string.
    pub fn new(
        cookie_secret: &str,
        db_connection: Arc<Mutex<Connection>>,
        session_actor: ActorRef<SessionStore>,
    ) -> Self {
        Self {
            cookie_key: create_cookie_key(cookie_secret),
            db_connection,
            session_actor,
        }
    }
}

impl FromRef<AppState> for LoginState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            cookie_key: state.cookie_key.clone(),
            db_connection: state.db_connection.clone(),
            session_actor: state.session_actor.clone(),
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
/// On a successful log-in request, a session is created, the auth cookie is
/// set and the client is redirected.
/// Otherwise, the form is returned with an error message explaining the
/// problem.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same
/// thread.
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
                Some("Password not set, click the link below to reset it"),
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

    let now = UtcDateTime::now();
    let session = Session::new(now);

    if let Err(err) = state
        .session_actor
        .tell(Set {
            session: session.clone(),
        })
        .await
    {
        tracing::error!("Error creating session: {err}");
        return log_in_form(
            "",
            Some("An internal error occurred. Please try again later."),
            redirect_url,
        )
        .into_response();
    }

    let redirect_url = redirect_url.unwrap_or(endpoints::DASHBOARD_VIEW);

    let cookie_expiry: OffsetDateTime = (now + MAX_SESSION_AGE).into();

    set_auth_cookie(jar.clone(), session.id, cookie_expiry)
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
#[derive(Clone, Serialize, Deserialize)]
pub struct LogInData {
    /// Password entered during log-in.
    pub password: String,

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
    use kameo::actor::Spawn;
    use rusqlite::Connection;

    use crate::{
        auth::SessionStore,
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

        let link_selector = scraper::Selector::parse("a[href]").unwrap();
        let links = form.select(&link_selector).collect::<Vec<_>>();
        assert_eq!(links.len(), 1, "want 1 link, got {}", links.len());
        let want_endpoints = [endpoints::FORGOT_PASSWORD_VIEW];

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

        LoginState::new(
            "foobar",
            Arc::new(Mutex::new(connection)),
            SessionStore::spawn(SessionStore::new()),
        )
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
    use kameo::actor::Spawn;
    use rusqlite::Connection;

    use crate::{
        PasswordHash, ValidatedPassword,
        auth::{COOKIE_TOKEN, SessionStore, User, UserID, create_user_table},
        endpoints,
    };

    use super::{INVALID_CREDENTIALS_ERROR_MSG, LogInData, LoginState, post_log_in};

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
                redirect_url: Some("https://example.com".to_string()),
            },
        )
        .await;

        assert_hx_redirect(&response, endpoints::DASHBOARD_VIEW);
    }

    #[tokio::test]
    async fn log_in_fails_with_missing_credentials() {
        let state = get_test_app_config(None);
        let app = Router::new()
            .route(endpoints::LOG_IN_API, post(post_log_in))
            .with_state(state);

        let server = TestServer::new(app);

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
        let server = TestServer::new(app);
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

        LoginState::new(
            "foobar",
            Arc::new(Mutex::new(connection)),
            SessionStore::spawn(SessionStore::new()),
        )
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
                    assert!(cookie.expires_datetime() > Some(time::OffsetDateTime::now_utc()));
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
