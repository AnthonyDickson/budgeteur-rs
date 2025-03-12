//! This file defines the high-level log-in route logic.
//! The auth module handles the lower level authentication and cookie auth logic.

use askama::Template;
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
    AppState, Error,
    auth::{
        cookie::{invalidate_auth_cookie, set_auth_cookie},
        log_in::{LogInData, verify_credentials},
    },
    stores::{CategoryStore, TransactionStore, UserStore},
};

use super::{
    endpoints,
    templates::{EmailInputTemplate, PasswordInputTemplate},
};

/// Renders a log-in form with client-side and server-side validation.
#[derive(Template)]
#[template(path = "partials/log_in/form.html")]
struct LogInFormTemplate<'a> {
    email_input: EmailInputTemplate<'a>,
    password_input: PasswordInputTemplate<'a>,
    log_in_route: &'a str,
    register_route: &'a str,
}

impl Default for LogInFormTemplate<'_> {
    fn default() -> Self {
        Self {
            email_input: Default::default(),
            password_input: Default::default(),
            log_in_route: endpoints::LOG_IN_API,
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
    LogInTemplate::default().into_response()
}

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
pub async fn post_log_in<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    jar: PrivateCookieJar,
    Form(user_data): Form<LogInData>,
) -> Response
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
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

const INVALID_CREDENTIALS_ERROR_MSG: &str = "Incorrect email or password.";

#[cfg(test)]
mod log_in_tests {
    use std::collections::{HashMap, HashSet};

    use axum::{
        Form, Router,
        body::Body,
        extract::State,
        http::{
            Response, StatusCode,
            header::{CONTENT_TYPE, SET_COOKIE},
        },
        routing::post,
    };
    use axum_extra::extract::{PrivateCookieJar, cookie::Cookie};
    use axum_htmx::HX_REDIRECT;
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use time::{Duration, OffsetDateTime};

    use crate::{
        AppState, Error,
        auth::{
            cookie::{COOKIE_EXPIRY, COOKIE_USER_ID},
            log_in::LogInData,
        },
        models::{
            Category, CategoryName, DatabaseID, PasswordHash, Transaction, TransactionBuilder,
            User, UserID, ValidatedPassword,
        },
        routes::{
            endpoints,
            log_in::{INVALID_CREDENTIALS_ERROR_MSG, REMEMBER_ME_COOKIE_DURATION, post_log_in},
        },
        stores::{CategoryStore, TransactionStore, UserStore, transaction::TransactionQuery},
    };

    use super::get_log_in_page;

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
    }

    #[derive(Clone)]
    struct DummyCategoryStore {}

    impl CategoryStore for DummyCategoryStore {
        fn create(&self, _name: CategoryName, _user_id: UserID) -> Result<Category, Error> {
            todo!()
        }

        fn get(&self, _category_id: DatabaseID) -> Result<Category, Error> {
            todo!()
        }

        fn get_by_user(&self, _user_id: UserID) -> Result<Vec<Category>, Error> {
            todo!()
        }
    }

    #[derive(Clone)]
    struct DummyTransactionStore {}

    impl TransactionStore for DummyTransactionStore {
        fn create(&mut self, _amount: f64, _user_id: UserID) -> Result<Transaction, Error> {
            todo!()
        }

        fn create_from_builder(
            &mut self,
            _builder: TransactionBuilder,
        ) -> Result<Transaction, Error> {
            todo!()
        }

        fn get(&self, _id: DatabaseID) -> Result<Transaction, Error> {
            todo!()
        }

        fn get_by_user_id(&self, _user_id: UserID) -> Result<Vec<Transaction>, Error> {
            todo!()
        }

        fn get_query(&self, _filter: TransactionQuery) -> Result<Vec<Transaction>, Error> {
            todo!()
        }
    }

    type TestAppState = AppState<DummyCategoryStore, DummyTransactionStore, StubUserStore>;

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
        assert_eq!(links.len(), 1, "want 1 link, got {}", links.len());
        let link = links.first().unwrap();
        assert_eq!(
            link.value().attr("href"),
            Some(endpoints::REGISTER_VIEW),
            "want link to {}, got {:?}",
            endpoints::REGISTER_VIEW,
            link.value().attr("href")
        );
    }

    #[tokio::test]
    async fn log_in_page_displays_error_message() {
        let state = get_test_app_config();
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
        let document = scraper::Html::parse_document(&text);

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
            p_text
                .to_lowercase()
                .contains("incorrect email or password"),
            "error message should contain string \"incorrect email or password\" but got {}",
            p_text
        );
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

    fn get_test_app_config() -> TestAppState {
        let mut state = AppState::new(
            "42",
            DummyCategoryStore {},
            DummyTransactionStore {},
            StubUserStore { users: vec![] },
        );

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

    fn assert_hx_redirect(response: &Response<Body>, want_location: &str) {
        let redirect_location = response.headers().get(HX_REDIRECT).unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(redirect_location, want_location);
    }

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
